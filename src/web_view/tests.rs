use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::sync::{Mutex, Once};

use super::{CpuUpload, KarereWebView, cpu_upload};

static GTK_TEST_LOCK: Mutex<()> = Mutex::new(());
static TEST_SCHEMA: Once = Once::new();

fn prepare_widget_test_runtime() {
    TEST_SCHEMA.call_once(|| {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let dir = root.join("target/web-view-test-schemas");
        std::fs::create_dir_all(&dir).unwrap();
        let schema =
            std::fs::read_to_string(root.join("data/io.github.tobagin.karere.gschema.xml.in"))
                .unwrap()
                .replace("@APP_ID@", crate::application::APP_ID)
                .replace("@APP_PATH@", "/io/github/tobagin/karere/");
        std::fs::write(dir.join("io.github.tobagin.karere.gschema.xml"), schema).unwrap();
        assert!(
            std::process::Command::new("glib-compile-schemas")
                .arg(&dir)
                .status()
                .unwrap()
                .success()
        );
        // SAFETY: GTK/GSettings widget tests are serialized by GTK_TEST_LOCK,
        // and this Once runs before either test initializes GTK or GSettings.
        unsafe {
            std::env::set_var("GSETTINGS_SCHEMA_DIR", dir);
            std::env::set_var("GSETTINGS_BACKEND", "memory");
        }
    });
}

/// Both production constructors configure the inherited GLArea before it is
/// realized. This exercises the real widget construction path rather than a
/// detached API-selection helper.
#[test]
fn gles_contract_is_shared_by_main_and_devtools_views() {
    let _guard = GTK_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    prepare_widget_test_runtime();
    if gtk::init().is_err() {
        eprintln!("skipping GTK widget contract test: no display available");
        return;
    }

    for view in [KarereWebView::new(), KarereWebView::new_devtools()] {
        assert!(!view.is_realized());
        assert_eq!(view.allowed_apis(), gtk::gdk::GLAPI::GLES);
        assert!(
            view.required_version() >= (3, 0),
            "GLSL ES 3.00 shaders require GLES 3.0 or newer"
        );
    }

    // Force GTK's production create-context signal to fail. GDK cannot
    // produce a context below GLES 3.0, which is all a PinePhone's Mali-400
    // offers, so realize must fall back to software presentation rather
    // than leave the user with GTK's error label (#177): it clears the
    // error, flips `software_present`, and still bootstraps the browser,
    // because CEF's CPU on_paint frames are what `snapshot` then draws.
    let failed = KarereWebView::new();
    use std::sync::atomic::Ordering as AtomicOrdering;
    failed
        .imp()
        .suppress_browser_creation
        .store(true, AtomicOrdering::Release);
    failed.connect_create_context(|area| {
        let error =
            gtk::glib::Error::new(gtk::gio::IOErrorEnum::Failed, "injected GL context failure");
        area.set_error(Some(&error));
        None
    });
    let window = gtk::Window::builder()
        .default_width(64)
        .default_height(64)
        .child(&failed)
        .build();
    window.present();
    while gtk::glib::MainContext::default().iteration(false) {}

    assert!(
        failed.error().is_none(),
        "the GLArea error label must not be left covering a usable software path"
    );
    assert!(failed.imp().software_present.load(AtomicOrdering::Relaxed));
    assert!(
        failed
            .imp()
            .fallback_test_events
            .borrow()
            .iter()
            .any(|(kind, _)| *kind == "create"),
        "software presentation still needs a browser to produce frames"
    );
    // Shared textures are unimportable without a context, and asking for
    // them stops CEF ever sending the CPU frames the fallback draws.
    // Assert the flag, not accel_osr_enabled(): that caches in a
    // process-wide OnceLock whichever test reaches it first.
    assert!(super::gl::NO_GL.load(AtomicOrdering::Relaxed));
    window.destroy();

    assert_rejected_accelerated_frame_restarts_pool_then_renders_cpu_callback();
}

fn assert_rejected_accelerated_frame_restarts_pool_then_renders_cpu_callback() {
    use std::os::fd::OwnedFd;
    use std::sync::atomic::Ordering;

    crate::load_gl();

    // Use the production DevTools construction/realize/draw path, but stop at
    // the CEF create boundary so this unit test does not need a second live CEF
    // runtime. The observations below are emitted by close_browser/spawn_browser.
    let view = KarereWebView::new_devtools();
    let imp = view.imp();
    imp.suppress_browser_creation.store(true, Ordering::Release);
    let window = gtk::Window::builder()
        .default_width(64)
        .default_height(64)
        .child(&view)
        .build();
    window.present();
    while gtk::glib::MainContext::default().iteration(false) {}
    assert!(
        view.is_realized(),
        "test requires the production GLArea draw path"
    );
    imp.fallback_test_events.borrow_mut().clear();

    // An invalid DMA-BUF reaches the real import call from draw() and must be
    // rejected deterministically by EGL. /dev/null supplies an owned, valid fd
    // while fourcc=0 is intentionally not a DRM pixel format.
    let fd: OwnedFd = std::fs::File::open("/dev/null").unwrap().into();
    let shared = imp.shared.lock().as_ref().unwrap().clone();
    shared.lock().accel = Some(crate::gl_dmabuf::AccelFrame {
        width: 1,
        height: 1,
        visible: (0, 0, 1, 1),
        fourcc: 0,
        modifier: 0,
        planes: vec![crate::gl_dmabuf::Plane {
            fd,
            offset: 0,
            stride: 4,
        }],
        dirty: true,
    });
    unsafe { imp.draw() };
    assert!(
        shared.lock().accel.is_none(),
        "draw must discard rejected DMA-BUF"
    );
    assert!(imp.software_osr_forced.load(Ordering::Acquire));

    // draw() schedules, rather than recursively performs, browser teardown.
    assert!(imp.fallback_test_events.borrow().is_empty());
    while gtk::glib::MainContext::default().iteration(false) {}
    assert_eq!(
        imp.fallback_test_events.borrow().as_slice(),
        [("close", None), ("create", Some(false))],
        "idle restart must close the affected pool before recreating it without shared textures"
    );

    // Invoke the exact CEF RenderHandler::on_paint implementation used by the
    // recreated software browser, then draw the resulting BGRA frame through
    // the production upload path. A clean frame proves the callback is visible.
    let pixels = [1_u8, 2, 3, 255];
    crate::handlers::render::dispatch_cpu_paint_for_test(&shared, &pixels, 1, 1);
    assert_eq!(
        cpu_upload(&shared.lock().frame, (0, 0)),
        CpuUpload::Allocate
    );
    unsafe { imp.draw() };
    let state = shared.lock();
    assert!(!state.frame.dirty, "CPU callback must be consumed by draw");
    assert_eq!(state.frame.pixels, pixels);
    drop(state);
    assert_eq!(cpu_upload(&shared.lock().frame, (1, 1)), CpuUpload::Reuse);

    // A `<select>` popup is composited from its own texture: drawing it
    // consumes the popup frame and leaves the view frame untouched. (I3)
    let popup = [9_u8, 9, 9, 255];
    crate::handlers::render::dispatch_popup_for_test(&shared, (0, 0, 1, 1), &popup, 1, 1);
    assert!(!shared.lock().frame.dirty, "a popup paint is not a view paint");
    unsafe { imp.draw() };
    let state = shared.lock();
    assert!(!state.popup.frame.dirty, "popup frame must be uploaded by draw");
    assert_eq!(state.frame.pixels, pixels, "view frame untouched by the popup");
    drop(state);
    assert_eq!(
        (
            imp.popup_tex_w.load(Ordering::Relaxed),
            imp.popup_tex_h.load(Ordering::Relaxed)
        ),
        (1, 1)
    );

    window.destroy();
}
