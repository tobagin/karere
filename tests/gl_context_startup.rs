//! Symptom-level regression for issue #177.
//!
//! Run under the documented X11 software fixture:
//! `dbus-run-session -- xvfb-run -a cargo test --test gl_context_startup -- --nocapture`

use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

const APP_ID: &str = "io.github.tobagin.karere";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

struct RunningApp {
    child: Child,
    lines: Receiver<String>,
    output: String,
}

impl RunningApp {
    fn spawn(fixture: &Path, force_legacy_desktop_gl: bool) -> Self {
        assert!(
            std::env::var_os("DISPLAY").is_some(),
            "run this test under xvfb-run"
        );
        assert!(
            std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some(),
            "run this test under dbus-run-session"
        );

        let binary = env!("CARGO_BIN_EXE_karere");
        let mut command = Command::new(binary);
        command
            .args(["--url", "about:blank", "--debuglevel=debug"])
            .current_dir(
                Path::new(binary)
                    .parent()
                    .expect("Karere binary must have a parent directory"),
            )
            .env("GDK_BACKEND", "x11")
            .env("GDK_DEBUG", "gl-prefer-gl")
            .env("GSK_RENDERER", "gl")
            // Pin Mesa's software driver and expose GLES 3.2 while capping the
            // legacy desktop API below the renderer's GL 3.0 contract.
            .env("LIBGL_ALWAYS_SOFTWARE", "1")
            .env("GALLIUM_DRIVER", "llvmpipe")
            .env("MESA_LOADER_DRIVER_OVERRIDE", "llvmpipe")
            .env("MESA_GL_VERSION_OVERRIDE", "2.1")
            .env("MESA_GLES_VERSION_OVERRIDE", "3.2")
            .env("__GLX_VENDOR_LIBRARY_NAME", "mesa")
            .env("KARERE_GPU_OSR", "0")
            .env("GSETTINGS_BACKEND", "keyfile")
            .env("GSETTINGS_SCHEMA_DIR", fixture.join("schemas"))
            .env("XDG_CONFIG_HOME", fixture.join("config"))
            .env("XDG_CACHE_HOME", fixture.join("cache"))
            .env("XDG_DATA_HOME", fixture.join("data"))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(mesa_vendor) = mesa_egl_vendor_file() {
            command.env("__EGL_VENDOR_LIBRARY_FILENAMES", mesa_vendor);
        }
        if force_legacy_desktop_gl {
            command.env("KARERE_TEST_FORCE_DESKTOP_GL", "1");
        }
        let mut child = command
            .spawn()
            .expect("launch the production karere binary");

        let (sender, lines) = mpsc::channel();
        forward_lines(child.stdout.take().unwrap(), sender.clone());
        forward_lines(child.stderr.take().unwrap(), sender);

        Self {
            child,
            lines,
            output: String::new(),
        }
    }

    fn wait_for(&mut self, expected: &[&str]) {
        self.wait_for_output(expected, false);
    }

    fn wait_for_context_failure(&mut self) {
        self.wait_for_output(&["GLArea realize error"], true);
        assert!(
            !self.output.contains("browser spawned"),
            "legacy desktop-GL context failure crossed the browser fence:\n{}",
            self.output
        );
    }

    fn wait_for_output(&mut self, expected: &[&str], allow_context_error: bool) {
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        loop {
            if let Some(status) = self.child.try_wait().expect("poll Karere") {
                panic!(
                    "Karere exited before startup markers {expected:?}: {status}\n{}",
                    self.output
                );
            }
            if expected.iter().all(|marker| self.output.contains(marker)) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {expected:?}\n{}",
                self.output
            );
            if let Ok(line) = self.lines.recv_timeout(Duration::from_millis(100)) {
                println!("{line}");
                self.output.push_str(&line);
                self.output.push('\n');
                if !allow_context_error {
                    assert_no_context_error(&self.output);
                }
            }
        }
    }

    fn quit(self, fixture: &Path) {
        self.quit_inner(fixture, false);
    }

    fn quit_after_expected_context_failure(self, fixture: &Path) {
        self.quit_inner(fixture, true);
    }

    fn quit_inner(mut self, fixture: &Path, allow_context_error: bool) {
        let status = Command::new("gapplication")
            .args(["action", APP_ID, "quit"])
            .env("GSETTINGS_BACKEND", "keyfile")
            .env("GSETTINGS_SCHEMA_DIR", fixture.join("schemas"))
            .env("XDG_CONFIG_HOME", fixture.join("config"))
            .status()
            .expect("invoke Karere quit action");
        assert!(
            status.success(),
            "gapplication quit action failed: {status}"
        );

        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(status) = self.child.try_wait().expect("wait for Karere shutdown") {
                assert!(
                    status.success(),
                    "Karere did not terminate cleanly: {status}"
                );
                if !allow_context_error {
                    assert_no_context_error(&self.output);
                }
                return;
            }
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                panic!("Karere did not exit after its quit action\n{}", self.output);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

fn forward_lines(stream: impl Read + Send + 'static, sender: mpsc::Sender<String>) {
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            let _ = sender.send(line);
        }
    });
}

impl Drop for RunningApp {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn assert_no_context_error(output: &str) {
    let lower = output.to_ascii_lowercase();
    assert!(!output.contains("GLArea realize error"), "{output}");
    assert!(
        !(lower.contains("gl context")
            && (lower.contains("failed to create") || lower.contains("unable to create"))),
        "GL context creation failed:\n{output}"
    );
}

fn mesa_egl_vendor_file() -> Option<&'static Path> {
    [
        "/usr/share/glvnd/egl_vendor.d/50_mesa.json",
        "/usr/share/egl/egl_external_platform.d/50_mesa.json",
    ]
    .into_iter()
    .map(Path::new)
    .find(|path| path.exists())
}

fn fixture() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = root
        .join("target")
        .join(format!("gl-context-startup-{}", std::process::id()));
    let _ = fs::remove_dir_all(&fixture);
    for child in ["schemas", "config", "cache", "data"] {
        fs::create_dir_all(fixture.join(child)).unwrap();
    }

    let schema = fs::read_to_string(root.join("data/io.github.tobagin.karere.gschema.xml.in"))
        .expect("read application settings schema")
        .replace("@APP_ID@", APP_ID)
        .replace("@APP_PATH@", "/io/github/tobagin/karere/");
    fs::write(
        fixture
            .join("schemas")
            .join("io.github.tobagin.karere.gschema.xml"),
        schema,
    )
    .unwrap();
    let status = Command::new("glib-compile-schemas")
        .arg(fixture.join("schemas"))
        .status()
        .expect("compile test GSettings schema");
    assert!(status.success());
    fixture
}

fn set_background(fixture: &Path, enabled: bool) {
    let status = Command::new("gsettings")
        .args([
            "set",
            APP_ID,
            "start-in-background",
            if enabled { "true" } else { "false" },
        ])
        .env("GSETTINGS_BACKEND", "keyfile")
        .env("GSETTINGS_SCHEMA_DIR", fixture.join("schemas"))
        .env("XDG_CONFIG_HOME", fixture.join("config"))
        .status()
        .expect("set startup ordering fixture");
    assert!(status.success());
}

fn present_background_app(fixture: &Path) {
    let status = Command::new("gapplication")
        .args(["action", APP_ID, "present-window"])
        .env("GSETTINGS_BACKEND", "keyfile")
        .env("GSETTINGS_SCHEMA_DIR", fixture.join("schemas"))
        .env("XDG_CONFIG_HOME", fixture.join("config"))
        .status()
        .expect("present background Karere window");
    assert!(status.success());
}

#[test]
fn real_binary_starts_with_software_gles_for_visible_and_prewarmed_windows() {
    let fixture = fixture();

    // Prove the fixture models the pre-fix boundary with the actual production
    // binary/widget: its debug-only legacy desktop-GL contract fails before CEF.
    set_background(&fixture, false);
    let mut legacy = RunningApp::spawn(&fixture, true);
    legacy.wait_for_context_failure();
    legacy.quit_after_expected_context_failure(&fixture);

    set_background(&fixture, false);
    let mut visible = RunningApp::spawn(&fixture, false);
    visible.wait_for(&[
        "GLArea context ready: api=GLAPI(GLES) version=3",
        "browser spawned",
    ]);
    visible.quit(&fixture);

    set_background(&fixture, true);
    let mut background = RunningApp::spawn(&fixture, false);
    background.wait_for(&["start-in-background=true", "browser spawned"]);
    present_background_app(&fixture);
    background.wait_for(&["GLArea context ready: api=GLAPI(GLES) version=3"]);
    background.quit(&fixture);

    fs::remove_dir_all(fixture).unwrap();
}

#[test]
fn stable_and_devel_flatpak_graphics_policy_stays_synchronized() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let stable = fs::read_to_string(root.join("packaging/io.github.tobagin.karere.yml")).unwrap();
    let devel =
        fs::read_to_string(root.join("packaging/io.github.tobagin.karere.Devel.yml")).unwrap();

    for policy in [
        "--socket=wayland",
        "--socket=fallback-x11",
        "--env=GSK_RENDERER=gl",
    ] {
        assert_eq!(stable.matches(policy).count(), 1, "stable policy: {policy}");
        assert_eq!(devel.matches(policy).count(), 1, "Devel policy: {policy}");
    }
}
