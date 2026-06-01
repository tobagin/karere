use anyhow::Result;
use cef::{CefString, ImplCommandLine, args::Args};
use gettextrs::{LocaleCategory, bind_textdomain_codeset, bindtextdomain, gettext, setlocale, textdomain};
use gtk::glib;
use gtk::prelude::*;

mod accounts;
mod actions;
mod application;
mod cdp;
mod cef_runtime;
mod devtools;
mod handlers;
mod ipc;
mod notifications;
mod paste;
mod permissions_store;
mod preferences;
mod sound;
mod spellcheck;
mod spellcheck_ui;
mod tray;
mod web_view;
mod window;

use application::KarereApplication;

fn main() -> Result<()> {
    // `--debuglevel=LEVEL` (debug/info/warn/error/trace/off) sets the app's log
    // level; `--debug` is the debug-level alias; default INFO. `--debuglevel`
    // wins if both are passed. RUST_LOG still overrides.
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(log_filter_from_args()),
    )
    .init();

    init_gettext();
    let banner = gettext("Karere starting");
    log::info!("{}", banner);

    let _ = cef::api_hash(cef::sys::CEF_API_VERSION_LAST, 0);
    load_gl();

    let args = Args::new();
    let cmd = args
        .as_cmd_line()
        .ok_or_else(|| anyhow::anyhow!("failed to parse CEF cmdline"))?;

    let type_switch = CefString::from("type");
    let is_browser_process = cmd.has_switch(Some(&type_switch)) != 1;

    let mut cef_app = cef_runtime::build_app();
    let ret = cef::execute_process(
        Some(args.as_main_args()),
        Some(&mut cef_app),
        std::ptr::null_mut(),
    );

    if !is_browser_process {
        assert!(ret >= 0, "subprocess execute_process failed: {ret}");
        return Ok(());
    }
    assert_eq!(ret, -1, "browser process must return -1 from execute_process");

    let url = parse_url_flag().unwrap_or_else(|| "https://web.whatsapp.com".into());

    gio::resources_register_include!("karere.gresource")
        .expect("failed to register gresource");

    let app = KarereApplication::new();

    app.connect_command_line(move |app, _cmd| {
        if let Some(app) = app.downcast_ref::<KarereApplication>() {
            app.activate_with_url(&url);
        }
        glib::ExitCode::SUCCESS
    });

    // Arbitrate single-instance BEFORE initializing the CEF browser context.
    // A second launch becomes the remote instance, forwards its command line to
    // the primary (which re-presents the window), and exits without touching CEF
    // — otherwise CEF's user-data-dir singleton lock makes the 2nd init fail.
    app.register(gio::Cancellable::NONE)?;
    let is_primary = !app.is_remote();
    if is_primary {
        cef_runtime::initialize_browser_process(&args, &mut cef_app)?;
        // Bridge service-worker notifications over CDP (the only realm-reaching
        // mechanism; CEF exposes no notification API). Uses the same debug port.
        cdp::start(devtools::DEVTOOLS_PORT);
    } else {
        log::info!("secondary instance — forwarding to primary, skipping CEF init");
    }

    let code = i32::from(app.run().get());

    if is_primary {
        cef::shutdown();
    }
    std::process::exit(code);
}

fn init_gettext() {
    if setlocale(LocaleCategory::LcAll, "").is_none() {
        log::warn!("setlocale(LC_ALL, \"\") failed — falling back to C.UTF-8");
        let _ = setlocale(LocaleCategory::LcAll, "C.UTF-8");
    }

    let locale_dir = if std::env::var("FLATPAK_ID").is_ok() {
        std::path::PathBuf::from("/app/share/locale")
    } else {
        std::env::current_dir()
            .map(|d| d.join("locale"))
            .unwrap_or_else(|_| std::path::PathBuf::from("/app/share/locale"))
    };

    if let Err(e) = bindtextdomain("karere", locale_dir) {
        log::warn!("bindtextdomain failed: {e}");
    }
    if let Err(e) = bind_textdomain_codeset("karere", "UTF-8") {
        log::warn!("bind_textdomain_codeset failed: {e}");
    }
    if let Err(e) = textdomain("karere") {
        log::warn!("textdomain failed: {e}");
    }
}

/// Build the env_logger filter from CLI flags: `--debuglevel=LEVEL` (wins) or
/// the `--debug` alias, defaulting to INFO. Deps stay at `warn` to keep the
/// `karere` lines readable; use RUST_LOG for dependency logs.
fn log_filter_from_args() -> String {
    let mut level: Option<String> = None;
    let mut debug_flag = false;
    for arg in std::env::args() {
        if arg == "--debug" {
            debug_flag = true;
        } else if let Some(v) = arg.strip_prefix("--debuglevel=") {
            level = Some(normalize_log_level(v));
        }
    }
    let level = level
        .or_else(|| debug_flag.then(|| "debug".to_owned()))
        .unwrap_or_else(|| "info".to_owned());
    format!("warn,karere={level}")
}

fn normalize_log_level(value: &str) -> String {
    let v = value.to_ascii_lowercase();
    match v.as_str() {
        "warning" => "warn".to_owned(),
        "err" => "error".to_owned(),
        "trace" | "debug" | "info" | "warn" | "error" | "off" => v,
        other => {
            eprintln!("karere: unknown --debuglevel={other:?}, using debug");
            "debug".to_owned()
        }
    }
}

fn parse_url_flag() -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if let Some(v) = arg.strip_prefix("--url=") {
            return Some(v.to_owned());
        }
        if arg == "--url" {
            return args.next();
        }
    }
    None
}

fn load_gl() {
    use libloading::os::unix::Library;
    let library = unsafe {
        Library::new("libepoxy.so.0")
            .or_else(|_| Library::new("libepoxy.so"))
            .expect("failed to load libepoxy")
    };
    let library: &'static Library = Box::leak(Box::new(library));
    epoxy::load_with(|name| unsafe {
        library
            .get::<unsafe extern "C" fn()>(name.as_bytes())
            .map(|sym| std::mem::transmute::<_, *const std::ffi::c_void>(sym))
            .unwrap_or(std::ptr::null())
    });
    gl::load_with(|name| epoxy::get_proc_addr(name) as *const _);
}
