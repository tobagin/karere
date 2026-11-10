use anyhow::Result;
use cef::{CefString, ImplCommandLine, args::Args};
use gettextrs::{
    LocaleCategory, bind_textdomain_codeset, bindtextdomain, gettext, setlocale, textdomain,
};
use gtk::glib;
use gtk::prelude::*;
use std::io::Write as _;

mod accounts;
mod actions;
mod application;
mod cdp;
mod cef_pump;
mod cef_runtime;
mod deeplink;
mod devtools;
mod gl_dmabuf;
mod handlers;
mod i18n;
mod ipc;
mod notifications;
mod paste;
mod permissions_store;
mod preferences;
mod spellcheck;
mod spellcheck_ui;
mod tray;
mod web_view;
mod window;

use application::KarereApplication;

/// Set by Preferences' "Restart Now": after the normal quit teardown and
/// `cef::shutdown()`, `main` re-execs the same binary instead of exiting.
pub static RESTART_REQUESTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn main() -> Result<()> {
    // `--debuglevel=LEVEL` sets the log level (`--debug` = debug alias; default
    // INFO; --debuglevel wins). RUST_LOG still overrides.
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(log_filter_from_args()),
    )
    .target(env_logger::Target::Pipe(Box::new(TeeWriter)))
    .init();

    install_panic_hook();

    init_gettext();
    let banner = gettext("Karere starting");
    log::info!("{}", banner);

    let _ = cef::api_hash(cef::sys::CEF_API_VERSION_LAST, 0);
    load_gl();

    let args = cef_args();
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
    assert_eq!(
        ret, -1,
        "browser process must return -1 from execute_process"
    );

    let url = parse_url_flag().unwrap_or_else(|| "https://web.whatsapp.com".into());

    gio::resources_register_include!("karere.gresource").expect("failed to register gresource");

    let app = KarereApplication::new();

    app.connect_command_line(move |app, cmd| {
        if let Some(app) = app.downcast_ref::<KarereApplication>() {
            // `cmd` carries this launch's argv, whether it is the first
            // instance or a forwarded second one.
            let argv: Vec<String> = cmd
                .arguments()
                .iter()
                .map(|a| a.to_string_lossy().into_owned())
                .collect();
            let link = deeplink::from_args(argv.iter().map(String::as_str));
            let background = has_background_flag(argv.iter().map(String::as_str));
            app.activate_with_url(&url, link.as_deref(), background);
        }
        glib::ExitCode::SUCCESS
    });

    // Arbitrate single-instance BEFORE CEF init: a second launch forwards its
    // command line to the primary and exits without touching CEF, else CEF's
    // user-data-dir singleton lock makes the 2nd init fail.
    app.register(gio::Cancellable::NONE)?;
    let is_primary = !app.is_remote();
    if is_primary {
        // Only the primary browser process writes the log file; CEF children
        // returned above and secondary instances never get here.
        start_file_logging();
        // Delete leftover v3 (WebKitGTK) data once, before CEF starts.
        accounts::purge_legacy_v3_data();
        cef_runtime::initialize_browser_process(&args, &mut cef_app)?;
        // Notifications are bridged per-browser via the in-process DevTools
        // message API (crate::cdp::attach, called at browser creation) — no
        // network debugging port. The F12 inspector still uses a debug-only
        // port; see cef_runtime::debug_enabled / crate::devtools.
    } else {
        log::info!("secondary instance — forwarding to primary, skipping CEF init");
    }

    let code = i32::from(app.run().get());

    if is_primary {
        cef_pump::stop();
        cef::shutdown();
        if RESTART_REQUESTED.load(std::sync::atomic::Ordering::Acquire) {
            use std::os::unix::process::CommandExt;
            // The D-Bus name and CEF profile lock went with the old instance,
            // so the new image comes up as the primary.
            match std::env::current_exe() {
                Ok(exe) => {
                    let err = std::process::Command::new(exe)
                        .args(deeplink::chromium_args(std::env::args()).into_iter().skip(1))
                        .exec();
                    log::error!("restart: exec failed: {err}");
                }
                Err(e) => log::error!("restart: cannot resolve own executable: {e}"),
            }
        }
    }
    std::process::exit(code);
}

/// Directory holding `karere.log`; the crash dialog's "Open logs" opens it.
pub(crate) fn log_dir() -> std::path::PathBuf {
    glib::user_data_dir()
        .join(application::APP_ID)
        .join("logs")
}

const LOG_FILE_CAP: u64 = 2 * 1024 * 1024;

struct LogFile {
    file: std::fs::File,
    written: u64,
}

/// Set only by `start_file_logging` in the primary browser process.
static LOG_FILE: std::sync::Mutex<Option<LogFile>> = std::sync::Mutex::new(None);

/// env_logger target: every record goes to stderr and, once a file is
/// attached, to the log file until it reaches `LOG_FILE_CAP`.
struct TeeWriter;

impl std::io::Write for TeeWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = std::io::stderr().write_all(buf);
        if let Ok(mut guard) = LOG_FILE.lock()
            && let Some(log) = guard.as_mut()
            && log.written < LOG_FILE_CAP
            && log.file.write_all(buf).is_ok()
        {
            log.written += buf.len() as u64;
            if log.written >= LOG_FILE_CAP {
                let _ = log
                    .file
                    .write_all(b"--- log size limit reached; file output stopped ---\n");
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        std::io::stderr().flush()
    }
}

/// Keep the previous run as `karere.log.1`, create a fresh private
/// `karere.log` and start teeing log output into it.
fn start_file_logging() {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let dir = log_dir();
    if let Err(e) = std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)
    {
        log::warn!("log file disabled: cannot create {}: {e}", dir.display());
        return;
    }
    let path = dir.join("karere.log");
    let _ = std::fs::rename(&path, dir.join("karere.log.1"));
    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)
    {
        Ok(f) => f,
        Err(e) => {
            log::warn!("log file disabled: cannot open {}: {e}", path.display());
            return;
        }
    };
    let header = format!(
        "Karere {} log. When run with --debug or --debuglevel=debug this file may contain \
         personal details (contact names, phone numbers, message text).\n",
        env!("CARGO_PKG_VERSION")
    );
    if file.write_all(header.as_bytes()).is_ok()
        && let Ok(mut guard) = LOG_FILE.lock()
    {
        *guard = Some(LogFile {
            file,
            written: header.len() as u64,
        });
    }
}

/// Log every Rust panic with file:line, message, and (when `RUST_BACKTRACE` is
/// set) a backtrace, before the process unwinds/aborts. A browser-process panic
/// otherwise dies with no diagnosable trace — the Ukm SIGSEGV had to be
/// recovered from a coredump; a Rust-side panic now lands in the same log
/// stream with its exact location. Runs in every process (set before the
/// browser/subprocess split), so subprocess panics are captured too. This does
/// not recover: it logs, then the default panic behavior proceeds.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown location>".to_string());
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("<non-string panic payload>");
        let bt = std::backtrace::Backtrace::capture();
        log::error!("PANIC at {loc}: {msg}\n{bt}");
        // Preserve default reporting (stderr print / abort) for good measure.
        default_hook(info);
    }));
}

fn init_gettext() {
    // SAFETY: gettext-rs 0.8 marks `setlocale` unsafe because concurrent calls
    // can corrupt the process locale. `init_gettext` runs at the top of `main`,
    // before CEF or GTK spawn any thread, so this is the only thread alive.
    unsafe {
        if setlocale(LocaleCategory::LcAll, "").is_none() {
            log::warn!("setlocale(LC_ALL, \"\") failed — falling back to C.UTF-8");
            let _ = setlocale(LocaleCategory::LcAll, "C.UTF-8");
        }
    }

    // UI-language override: export LANGUAGE before textdomain so gettext picks
    // the pinned catalog over the system LC_MESSAGES. Must run here in main,
    // single-threaded, before any CEF/GTK thread starts.
    if let Some(lang) = i18n::override_locale() {
        log::info!("UI language overridden to {lang}");
        unsafe {
            std::env::set_var("LANGUAGE", &lang);
        }
    }

    // Ship our catalogs under share/karere/locale, NOT the standard
    // share/locale: flatpak auto-splits share/locale into a per-language
    // `.Locale` extension that only deploys the host's languages, which would
    // strip every locale the in-app language picker offers. A non-standard dir
    // stays wholly inside the app so all catalogs are always present.
    let locale_dir = if std::env::var("FLATPAK_ID").is_ok() {
        std::path::PathBuf::from("/app/share/karere/locale")
    } else {
        std::env::current_dir()
            .map(|d| d.join("locale"))
            .unwrap_or_else(|_| std::path::PathBuf::from("/app/share/karere/locale"))
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

/// Build the env_logger filter from CLI flags (default INFO). Deps stay at
/// `warn` to keep `karere` lines readable; use RUST_LOG for dep logs.
fn log_filter_from_args() -> String {
    log_filter_from(std::env::args())
}

fn log_filter_from<I: IntoIterator<Item = String>>(args: I) -> String {
    let mut level: Option<String> = None;
    let mut debug_flag = false;
    for arg in args {
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
    parse_url_from(std::env::args())
}

/// `--url=X` / `--url X` from a full argv (first element is the program).
fn parse_url_from<I: IntoIterator<Item = String>>(args: I) -> Option<String> {
    let mut args = args.into_iter().skip(1);
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

/// True when `--background` is on the command line (autostart launches only).
fn has_background_flag<'a, I: IntoIterator<Item = &'a str>>(args: I) -> bool {
    args.into_iter().skip(1).any(|a| a == "--background")
}

/// CEF's view of argv: same as ours minus positional arguments (deep links),
/// which Chromium could otherwise try to open as a URL.
fn cef_args() -> Args {
    use std::ffi::{CString, c_char};
    let filtered = deeplink::chromium_args(std::env::args());
    let strings: Vec<CString> = filtered
        .into_iter()
        .filter_map(|a| CString::new(a).ok())
        .collect();
    let mut ptrs: Vec<*mut c_char> = strings.iter().map(|s| s.as_ptr() as *mut c_char).collect();
    let argc = ptrs.len() as i32;
    // CEF keeps reading argv for the life of the process; leak both buffers.
    let argv = ptrs.as_mut_ptr();
    std::mem::forget(ptrs);
    std::mem::forget(strings);
    Args::from(cef::MainArgs { argc, argv })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn url_flag_forms() {
        assert_eq!(parse_url_from(v(&["karere"])), None);
        assert_eq!(
            parse_url_from(v(&["karere", "--url=https://a"])).as_deref(),
            Some("https://a")
        );
        assert_eq!(
            parse_url_from(v(&["karere", "--debug", "--url", "https://b"])).as_deref(),
            Some("https://b")
        );
        assert_eq!(parse_url_from(v(&["karere", "--url"])), None);
        // argv[0] is never parsed as a flag.
        assert_eq!(parse_url_from(v(&["--url=x"])), None);
    }

    #[test]
    fn log_filter_levels() {
        assert_eq!(log_filter_from(v(&["karere"])), "warn,karere=info");
        assert_eq!(log_filter_from(v(&["karere", "--debug"])), "warn,karere=debug");
        assert_eq!(
            log_filter_from(v(&["karere", "--debuglevel=trace"])),
            "warn,karere=trace"
        );
        assert_eq!(
            log_filter_from(v(&["karere", "--debuglevel=WARNING", "--debug"])),
            "warn,karere=warn"
        );
        assert_eq!(
            log_filter_from(v(&["karere", "--debug", "--debuglevel=err"])),
            "warn,karere=error"
        );
    }

    #[test]
    fn background_flag() {
        assert!(has_background_flag(["karere", "--background"]));
        assert!(has_background_flag(["karere", "--debug", "--background"]));
        assert!(!has_background_flag(["karere"]));
        assert!(!has_background_flag(["karere", "--url", "--background-x"]));
        assert!(!has_background_flag(["--background"]));
    }

    #[test]
    fn normalize_levels() {
        assert_eq!(normalize_log_level("Warning"), "warn");
        assert_eq!(normalize_log_level("INFO"), "info");
        assert_eq!(normalize_log_level("bogus"), "debug");
    }
}
