//! Remember that accelerated OSR delivered no frame with this exact GPU,
//! driver, CEF engine and Karere build, so later launches start on CPU paint
//! instead of waiting for the first-frame watchdog and recreating every
//! browser. Any change to that identity (driver or app update, another GPU)
//! retries. This is derived state in the cache directory, never a preference:
//! the `gpu-rendering` setting is untouched and `KARERE_GPU_OSR=1` forces a
//! fresh attempt.
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

static IDENTITY: OnceLock<String> = OnceLock::new();
static CLEARED: AtomicBool = AtomicBool::new(false);

fn path() -> PathBuf {
    gtk::glib::user_cache_dir()
        .join("karere")
        .join("accelerated-osr-unavailable")
}

/// Describe the rendering stack. Call with the GL context that will import
/// frames current, so the strings name the GPU and driver actually in use.
pub fn capture_identity() -> &'static str {
    IDENTITY.get_or_init(|| {
        let gl_string = |name| unsafe {
            let value = gl::GetString(name);
            if value.is_null() {
                String::from("unknown")
            } else {
                std::ffi::CStr::from_ptr(value.cast())
                    .to_string_lossy()
                    .into_owned()
            }
        };
        let identity = identity(
            env!("CARGO_PKG_VERSION"),
            &String::from_utf8_lossy(cef::sys::CEF_VERSION.strip_suffix(b"\0").unwrap_or(b"")),
            loaded_libcef_bytes(),
            &gl_string(gl::VENDOR),
            &gl_string(gl::RENDERER),
            &gl_string(gl::VERSION),
        );
        log::debug!(
            "accel_osr: stack identity {}",
            identity.trim_end().replace('\n', "; ")
        );
        identity
    })
}

fn identity(
    karere: &str,
    cef: &str,
    libcef_bytes: Option<u64>,
    vendor: &str,
    renderer: &str,
    version: &str,
) -> String {
    let bytes = libcef_bytes.map_or_else(|| String::from("unknown"), |b| b.to_string());
    format!(
        "karere={karere}\ncef={cef}\nlibcef_bytes={bytes}\ngl_vendor={vendor}\n\
         gl_renderer={renderer}\ngl_version={version}\n"
    )
}

/// The engine build actually loaded (a patch release keeps the crate's
/// version string but changes the library).
fn loaded_libcef_bytes() -> Option<u64> {
    let maps = std::fs::read_to_string("/proc/self/maps").ok()?;
    let library = maps
        .lines()
        .filter_map(|line| line.split_whitespace().nth(5))
        .find(|path| path.ends_with("/libcef.so"))?;
    std::fs::metadata(library).ok().map(|meta| meta.len())
}

/// Whether accelerated OSR already failed with this identity.
pub fn previously_failed(identity: &str) -> bool {
    !crate::gpu_recovery::user_render_override() && failed_at(&path(), identity)
}

fn failed_at(path: &Path, identity: &str) -> bool {
    std::fs::read_to_string(path).is_ok_and(|stored| stored == identity)
}

/// Record that the first-frame watchdog had to fall back to CPU paint.
pub fn record_failure() {
    let Some(identity) = IDENTITY.get() else {
        return;
    };
    if crate::gpu_recovery::user_render_override() {
        log::info!("accel_osr: rendering override in effect; not remembering this fallback");
        return;
    }
    match record_at(&path(), identity) {
        Ok(()) => log::info!(
            "accel_osr: remembered that GPU rendering delivered no frame on this GPU, driver and \
             engine; later launches start on CPU paint until one of them changes \
             (KARERE_GPU_OSR=1 retries)"
        ),
        Err(error) => log::warn!("accel_osr: could not remember the GPU fallback: {error}"),
    }
}

fn record_at(path: &Path, identity: &str) -> std::io::Result<()> {
    let directory = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(directory)?;
    let temporary = directory.join(".accelerated-osr-unavailable.tmp");
    std::fs::write(&temporary, identity)?;
    std::fs::rename(&temporary, path)
}

/// An accelerated frame arrived: forget any earlier failure (once per run).
pub fn clear_after_success() {
    if !CLEARED.swap(true, Ordering::Relaxed) {
        clear_at(&path());
    }
}

fn clear_at(path: &Path) {
    if let Err(error) = std::fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        log::warn!("accel_osr: could not clear the remembered GPU fallback: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(driver: &str) -> String {
        identity(
            "4.4.1",
            "156.0.26+g8ea0c73+chromium-156.0.8078.88",
            Some(123),
            "NVIDIA Corporation",
            "Quadro P1000/PCIe/SSE2",
            driver,
        )
    }

    #[test]
    fn failure_is_remembered_only_for_the_same_stack() {
        let directory =
            std::env::temp_dir().join(format!("karere-accel-memo-{}", std::process::id()));
        let path = directory.join("karere").join("accelerated-osr-unavailable");
        let failed = sample("OpenGL ES 3.2 NVIDIA 580.178.04");
        assert!(!failed_at(&path, &failed), "nothing recorded yet");

        record_at(&path, &failed).unwrap();
        assert!(failed_at(&path, &failed));
        // A driver update, another engine or another GPU retries.
        assert!(!failed_at(
            &path,
            &sample("OpenGL ES 3.2 NVIDIA 580.182.01")
        ));
        assert!(!failed_at(
            &path,
            &identity(
                "4.4.1",
                "156.0.30",
                Some(123),
                "NVIDIA Corporation",
                "Quadro P1000/PCIe/SSE2",
                "x"
            )
        ));

        clear_at(&path);
        assert!(!failed_at(&path, &failed));
        clear_at(&path); // already absent: no error
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn unknown_library_size_is_part_of_the_identity() {
        let known = sample("v");
        let unknown = identity(
            "4.4.1",
            "156.0.26+g8ea0c73+chromium-156.0.8078.88",
            None,
            "NVIDIA Corporation",
            "Quadro P1000/PCIe/SSE2",
            "v",
        );
        assert_ne!(known, unknown);
        assert!(unknown.contains("libcef_bytes=unknown"));
    }
}
