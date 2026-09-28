//! A GPU copy must finish before returning CEF's borrowed frame. If completion
//! is unknown, terminate the rendering process without unwinding its resources.
//! A small parent retries once with CPU transfer; preferences are never changed.
use std::{
    collections::BTreeMap,
    os::unix::process::{CommandExt, ExitStatusExt},
    process::Command,
    sync::{Arc, Condvar, Mutex, OnceLock},
    time::{Duration, Instant},
};

const GPU_FAILURE: i32 = 86;
const CEF_GL: i32 = 87;
const CEF_VULKAN: i32 = 88;
const GTK_GL: i32 = 89;
const GTK_VULKAN: i32 = 90;
const WORKER: &str = "_KARERE_RENDER_WORKER";
pub const FENCE_TIMEOUT_NS: u64 = 1_000_000_000;

/// Runs before GTK, CEF or any threads. CEF children stay in their worker tree.
pub fn supervise() -> std::io::Result<Option<i32>> {
    let cef_child = std::env::args_os().any(|arg| arg.to_string_lossy().starts_with("--type="));
    if std::env::var_os(WORKER).is_some() || cef_child {
        // Do not leave a CEF child accessing a dead worker's cache or resources.
        let parent = unsafe { libc::getppid() };
        unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) };
        if unsafe { libc::getppid() } != parent {
            std::process::exit(1);
        }
        return Ok(None);
    }
    // Adopt and reap grandchildren after a failed worker, before reopening CEF.
    unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1) };
    let mut command = Command::new(std::env::current_exe()?);
    command.args(std::env::args_os().skip(1)).env(WORKER, "1");
    Ok(Some(run_supervised(&mut command)?))
}

fn run_supervised(command: &mut Command) -> std::io::Result<i32> {
    let mut recovered = false;
    let initial = std::env::var("KARERE_CEF_GRAPHICS").unwrap_or_else(|_| "gl".into());
    let mut backends = vec![initial];
    let mut gtk_backends = Vec::new();
    loop {
        let mut child = command.process_group(0).spawn()?;
        let group = child.id() as i32;
        let status = child.wait()?;
        let code = status.code().unwrap_or(128 + status.signal().unwrap_or(1));
        if !matches!(
            code,
            GPU_FAILURE | CEF_GL | CEF_VULKAN | GTK_GL | GTK_VULKAN
        ) {
            return Ok(code);
        }
        // Kill only this worker's process group, never another application.
        unsafe { libc::kill(-group, libc::SIGKILL) };
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let result = unsafe { libc::waitpid(-group, std::ptr::null_mut(), libc::WNOHANG) };
            if result < 0 {
                break;
            }
            if Instant::now() >= deadline {
                eprintln!("karere: GPU worker cleanup exceeded deadline; no restart");
                return Ok(GPU_FAILURE);
            }
            if result == 0 {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        if code == GTK_GL || code == GTK_VULKAN {
            let failed = if code == GTK_GL { "gl" } else { "vulkan" };
            if gtk_backends.contains(&failed) {
                // An override which GTK cannot honor must not restart forever.
                return Ok(code);
            }
            gtk_backends.push(failed);
            let alternative = if failed == "gl" { "vulkan" } else { "gl" };
            let next = if gtk_backends.contains(&alternative) {
                "cairo"
            } else {
                alternative
            };
            eprintln!("karere: GTK {failed} frame stalled; retrying GTK {next}");
            command.env("GSK_RENDERER", next);
            // CEF's working graphics backend and transfer preference survive.
            continue;
        }
        if code == CEF_GL || code == CEF_VULKAN {
            let requested = if code == CEF_GL { "gl" } else { "vulkan" };
            let next = if backends.iter().any(|tried| tried == requested) {
                "software"
            } else {
                requested
            };
            if backends.iter().any(|tried| tried == next) {
                return Ok(code);
            }
            eprintln!("karere: no initial CEF frame; retrying {next}");
            command.env("KARERE_CEF_GRAPHICS", next);
            backends.push(next.into());
            continue;
        }
        if recovered || std::env::var("KARERE_GPU_OSR").as_deref() == Ok("0") {
            return Ok(code);
        }
        eprintln!("karere: GPU transfer completion unknown; restarting once with CPU transfer");
        command.env("KARERE_GPU_OSR", "0");
        recovered = true;
    }
}

pub fn restart_cef(backend: &str) -> ! {
    std::process::exit(if backend == "vulkan" {
        CEF_VULKAN
    } else {
        CEF_GL
    });
}

/// No Drop handlers or callback return: neither is safe with an outstanding read
/// of CEF's allocation. Kernel process cleanup retains in-flight GPU references.
pub fn completion_unknown(reason: &str) -> ! {
    eprintln!("karere: GPU transfer recovery: {reason}");
    unsafe { libc::_exit(GPU_FAILURE) }
}

#[derive(Default)]
struct Deadlines {
    next: u64,
    active: BTreeMap<u64, (Instant, i32)>,
}
type Watch = Arc<(Mutex<Deadlines>, Condvar)>;
static WATCH: OnceLock<Watch> = OnceLock::new();

/// Also bounds driver calls which fail to honor their advertised fence timeout.
/// The watchdog sleeps without polling while no GPU operation is in progress.
pub struct Deadline {
    watch: Watch,
    id: u64,
}
impl Deadline {
    pub fn arm() -> Self {
        Self::with_failure(GPU_FAILURE)
    }

    pub fn gtk(gl: bool) -> Self {
        Self::with_failure(if gl { GTK_GL } else { GTK_VULKAN })
    }

    fn with_failure(failure: i32) -> Self {
        let watch = WATCH
            .get_or_init(|| {
                let watch = Arc::new((Mutex::new(Deadlines::default()), Condvar::new()));
                let thread_watch = watch.clone();
                std::thread::Builder::new()
                    .name("gpu-deadline".into())
                    .spawn(move || {
                        let (lock, changed) = &*thread_watch;
                        let mut state = lock.lock().unwrap();
                        loop {
                            if let Some((deadline, failure)) =
                                state.active.values().min_by_key(|(at, _)| at).copied()
                            {
                                let left = deadline.saturating_duration_since(Instant::now());
                                if left.is_zero() {
                                    // Even stderr can block (for example a full
                                    // logging pipe). This last-resort deadline
                                    // must not acquire a lock or perform I/O.
                                    // The parent records the recovery reason.
                                    unsafe { libc::_exit(failure) }
                                }
                                state = changed.wait_timeout(state, left).unwrap().0;
                            } else {
                                state = changed.wait(state).unwrap();
                            }
                        }
                    })
                    .expect("GPU deadline thread");
                watch
            })
            .clone();
        let id = {
            let mut state = watch.0.lock().unwrap();
            state.next += 1;
            let id = state.next;
            state
                .active
                .insert(id, (Instant::now() + Duration::from_secs(5), failure));
            watch.1.notify_one();
            id
        };
        Self { watch, id }
    }
}
impl Drop for Deadline {
    fn drop(&mut self) {
        self.watch.0.lock().unwrap().active.remove(&self.id);
        self.watch.1.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supervisor_preserves_normal_exit() {
        assert_eq!(
            run_supervised(Command::new("sh").args(["-c", "exit 7"])).unwrap(),
            7
        );
    }

    #[test]
    fn supervisor_retries_failed_transfer_with_cpu() {
        let code = run_supervised(
            Command::new("sh")
                .args([
                    "-c",
                    "if [ \"$KARERE_GPU_OSR\" = 0 ]; then exit 0; else exit 86; fi",
                ])
                .env_remove("KARERE_GPU_OSR"),
        )
        .unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn persistent_failure_does_not_restart_forever() {
        let code = run_supervised(Command::new("sh").args(["-c", "exit 86"])).unwrap();
        assert_eq!(code, GPU_FAILURE);
    }

    #[test]
    fn failed_backends_are_not_revisited() {
        let code = run_supervised(Command::new("sh").args([
            "-c", "case \"$KARERE_CEF_GRAPHICS\" in vulkan) exit 87;; software) exit 0;; *) exit 88;; esac",
        ]).env_remove("KARERE_CEF_GRAPHICS")).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn gtk_fallback_preserves_cef_acceleration_and_terminates() {
        let code = run_supervised(Command::new("sh").args([
            "-c",
            "test \"$KARERE_GPU_OSR\" = 1 && test \"$KARERE_CEF_GRAPHICS\" = gl || exit 9; case \"$GSK_RENDERER\" in gl) exit 89;; vulkan) exit 90;; cairo) exit 0;; *) exit 9;; esac",
        ]).env("GSK_RENDERER", "vulkan").env("KARERE_GPU_OSR", "1").env("KARERE_CEF_GRAPHICS", "gl")).unwrap();
        assert_eq!(code, 0);
        let code = run_supervised(Command::new("sh").args(["-c", "exit 89"])).unwrap();
        assert_eq!(code, GTK_GL, "a renderer ignoring the override cannot loop");
    }

    #[test]
    fn gtk_deadline_retries_only_the_failed_layer() {
        let start = Instant::now();
        let code = run_supervised(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "gpu_recovery::tests::stalled_gtk_worker",
                ])
                .env("GSK_RENDERER", "vulkan")
                .env("KARERE_GPU_OSR", "1")
                .env("KARERE_CEF_GRAPHICS", "gl"),
        )
        .unwrap();
        assert_eq!(code, 0);
        assert!(start.elapsed() < Duration::from_secs(9));
    }

    #[test]
    #[ignore = "subprocess fixture for the GTK frame watchdog"]
    fn stalled_gtk_worker() {
        assert_eq!(std::env::var("KARERE_GPU_OSR").as_deref(), Ok("1"));
        assert_eq!(std::env::var("KARERE_CEF_GRAPHICS").as_deref(), Ok("gl"));
        if std::env::var("GSK_RENDERER").as_deref() == Ok("gl") {
            return;
        }
        let _deadline = Deadline::gtk(false);
        std::thread::sleep(Duration::from_secs(10));
        panic!("GTK watchdog did not terminate the stalled worker");
    }

    #[test]
    fn watchdog_restarts_without_unwinding_borrowed_resources() {
        let marker =
            std::env::temp_dir().join(format!("karere-unsafe-gpu-drop-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let start = Instant::now();
        let code = run_supervised(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "gpu_recovery::tests::stalled_driver_worker",
                ])
                .env("KARERE_RECOVERY_TEST_MARKER", &marker)
                .env_remove("KARERE_GPU_OSR"),
        )
        .unwrap();
        assert_eq!(code, 0);
        assert!(start.elapsed() < Duration::from_secs(9));
        assert!(
            !marker.exists(),
            "must not run a resource destructor with unknown GPU completion"
        );
    }

    #[test]
    #[ignore = "subprocess fixture for the GPU watchdog"]
    fn stalled_driver_worker() {
        if std::env::var("KARERE_GPU_OSR").as_deref() == Ok("0") {
            return;
        }
        struct BorrowedResource(std::path::PathBuf);
        impl Drop for BorrowedResource {
            fn drop(&mut self) {
                std::fs::write(&self.0, "unsafe drop").unwrap();
            }
        }
        let _borrowed = BorrowedResource(
            std::env::var_os("KARERE_RECOVERY_TEST_MARKER")
                .unwrap()
                .into(),
        );
        let _deadline = Deadline::arm();
        std::thread::sleep(Duration::from_secs(10));
        panic!("GPU watchdog did not terminate the stalled worker");
    }
}
