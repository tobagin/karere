use anyhow::{Result, anyhow};
use cef::{
    self, App, BrowserProcessHandler, CommandLine, ImplApp, ImplBrowserProcessHandler,
    ImplCommandLine, RenderProcessHandler, Settings, WrapApp, WrapBrowserProcessHandler,
    args::Args, rc::Rc, wrap_app, wrap_browser_process_handler,
};

use crate::handlers::render_process::ShellRenderProcessHandlerBuilder;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Default)]
pub struct ShellApp;

wrap_app! {
    pub struct ShellAppBuilder {
        app: ShellApp,
    }

    impl App {
        fn on_before_command_line_processing(
            &self,
            process_type: Option<&cef::CefStringUtf16>,
            command_line: Option<&mut CommandLine>,
        ) {
            let Some(cmd) = command_line else { return };
            // Chromium spellcheck switches belong on the browser process command
            // line only. `process_type` is empty/None for the browser process and
            // a value ("renderer", "gpu", "utility", …) for children; skip the
            // GSettings read in those subprocesses.
            let is_browser_process = process_type
                .map(|p| p.to_string().is_empty())
                .unwrap_or(true);
            if is_browser_process {
                append_spellcheck_switches(cmd);
            }
            // Embedded DevTools (CDP): expose a loopback debugging endpoint and
            // allow the bundled frontend page (served from the same port) to
            // open the inspector websocket. See `crate::devtools`.
            cmd.append_switch_with_value(
                Some(&"remote-debugging-port".into()),
                Some(&crate::devtools::DEVTOOLS_PORT.to_string().as_str().into()),
            );
            // The DevTools frontend may be served from a remote origin
            // (chrome-devtools-frontend.appspot.com); allow any origin to open
            // the inspector websocket. The endpoint is loopback-only.
            cmd.append_switch_with_value(
                Some(&"remote-allow-origins".into()),
                Some(&"*".into()),
            );
            // The DevTools frontend is served from a public origin
            // (chrome-devtools-frontend.appspot.com) but must open a websocket
            // to the loopback CDP endpoint. Private/Local Network Access blocks
            // that public->private connection, leaving DevTools blank. Disable
            // the relevant features (the gate was renamed PNA -> LNA in M14x).
            cmd.append_switch_with_value(
                Some(&"disable-features".into()),
                Some(
                    &"BlockInsecurePrivateNetworkRequests,\
                      LocalNetworkAccessChecks,\
                      PrivateNetworkAccessForNavigations,\
                      PrivateNetworkAccessForWorkers"
                        .into(),
                ),
            );
            // NOTE: do NOT disable SystemNotifications/NativeNotifications —
            // that makes Chromium fall back to its own in-window message-center
            // popup instead of suppressing. Suppression is done in the SW shim
            // (it never calls the real showNotification), so Chromium renders
            // nothing and Karere emits its own gio::Notification.
            // M14 5.5 — Chromium notification-sound suppression: NOT needed.
            // The M14 observer replaces `window.Notification` with a Proxy whose
            // construct trap never builds the real Notification, so Chromium
            // renders no native banner and therefore plays no notification
            // sound. Karere's own `paplay` path is the only audio. If a future
            // code path lets a native banner through, append the verified
            // `--disable-notification-sound` switch here.
            cmd.append_switch(Some(&"enable-features=UseOzonePlatform".into()));
            cmd.append_switch_with_value(
                Some(&"ozone-platform-hint".into()),
                Some(&"auto".into()),
            );
            cmd.append_switch(Some(&"enable-webrtc-vea-vda".into()));
            // M17 paste bridge: large clipboard/drop payloads round-trip through
            // a tempfile the renderer fetches over file://. Chromium blocks
            // file:// fetches from non-file origins by default; this re-enables
            // them. The reach is scoped to `$XDG_RUNTIME_DIR/karere/` by the
            // resource request handler (`handlers::request`).
            cmd.append_switch(Some(&"allow-file-access-from-files".into()));
            cmd.append_switch(Some(&"no-startup-window".into()));
            cmd.append_switch(Some(&"noerrdialogs".into()));
            cmd.append_switch(Some(&"hide-crash-restore-bubble".into()));
            // Single-webview app → at most one renderer, so the zygote's
            // fork-sharing wins are negligible. Disabling it keeps a flat,
            // debuggable process tree where each renderer is a real
            // `--type=renderer` process (the zygote otherwise leaves forked
            // renderers mislabeled `--type=zygote`).
            cmd.append_switch(Some(&"no-zygote".into()));
            if std::env::var_os("FLATPAK_ID").is_some() {
                // Flatpak namespace sandbox conflicts with Chromium suid sandbox.
                cmd.append_switch(Some(&"no-sandbox".into()));
            }
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(ShellBrowserProcessHandlerBuilder::build(
                ShellBrowserProcessHandler::new(),
            ))
        }

        // Invoked only in the renderer subprocess. Returns the handler that
        // injects the JS bundle and bridges page <-> host IPC. The same
        // `ShellApp` is constructed in both processes (see `build_app`); CEF
        // calls this getter only where it matters.
        fn render_process_handler(&self) -> Option<RenderProcessHandler> {
            Some(ShellRenderProcessHandlerBuilder::build())
        }
    }
}

pub fn build_app() -> App {
    ShellAppBuilder::new(ShellApp)
}

/// Resolve the active spellcheck language list from GSettings and append the
/// matching Chromium switches to the browser process command line.
///
/// Sourced from the existing app gschema keys (M16):
/// - `enable-spell-checking` (b): when false, append `--disable-spell-checking`
///   and add no language switch.
/// - `spell-checking-languages` (as): explicit BCP-47 list; joined into
///   `--spell-check-languages=<csv>` when non-empty.
/// - `auto-detect-language` (b): when true and the explicit list is empty,
///   derive a single BCP-47 code from `glib::language_names()[0]`.
///
/// Chromium reads these at process startup and auto-downloads the matching
/// `.bdic` dictionaries into the cache on first need; no Hunspell module is
/// bundled. A live language change goes through
/// `KarereWebView::recreate_active_browser()` (see web_view.rs).
fn append_spellcheck_switches(cmd: &mut CommandLine) {
    use gtk::gio;
    use gtk::prelude::{SettingsExt, SettingsExtManual};

    let settings = gio::Settings::new(crate::application::APP_ID);

    if !settings.boolean("enable-spell-checking") {
        cmd.append_switch(Some(&"disable-spell-checking".into()));
        log::info!("spellcheck: disabled via GSettings");
        return;
    }

    let explicit: Vec<String> = settings
        .strv("spell-checking-languages")
        .iter()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let languages = if !explicit.is_empty() {
        explicit
    } else if settings.boolean("auto-detect-language") {
        // glib returns the user's preferred locales most-specific first, ending
        // with the "C" fallback; the first entry is the best auto-detect guess.
        gtk::glib::language_names()
            .into_iter()
            .map(|s| s.to_string())
            .filter(|s| s != "C" && !s.is_empty())
            .find_map(|lang| crate::spellcheck::best_supported_code(&lang))
            .into_iter()
            .collect()
    } else {
        Vec::new()
    };

    if languages.is_empty() {
        log::info!("spellcheck: enabled, no language resolved (Chromium default)");
        return;
    }

    let joined = languages.join(",");
    cmd.append_switch_with_value(
        Some(&"spell-check-languages".into()),
        Some(&joined.as_str().into()),
    );
    log::info!("spellcheck: --spell-check-languages={joined}");
}

/// Browser process handler — drives the external CEF message pump from the
/// glib main loop. CEF schedules work via `on_schedule_message_pump_work`;
/// we translate that into a one-shot glib timeout that calls
/// `cef::do_message_loop_work` on the main thread.
#[derive(Clone)]
pub struct ShellBrowserProcessHandler {
    state: Arc<Mutex<PumpState>>,
}

struct PumpState {
    ready: bool,
}

impl ShellBrowserProcessHandler {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(PumpState { ready: false })),
        }
    }
}

wrap_browser_process_handler! {
    pub struct ShellBrowserProcessHandlerBuilder {
        handler: ShellBrowserProcessHandler,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            self.handler.state.lock().ready = true;
            log::info!("CEF context initialized");
        }

        // on_schedule_message_pump_work fires from any CEF thread, so we
        // cannot do main-loop scheduling from here directly. We drive the
        // pump unconditionally from a glib timer instead (see
        // initialize_browser_process), so this callback is a no-op.
    }
}

impl ShellBrowserProcessHandlerBuilder {
    pub fn build(handler: ShellBrowserProcessHandler) -> BrowserProcessHandler {
        Self::new(handler)
    }
}


pub fn initialize_browser_process(args: &Args, app: &mut App) -> Result<()> {
    // Reclaim any paste tempfiles leaked by a prior crash before CEF starts.
    crate::paste::sweep_old();

    // M20 multi-account: each account's RequestContext gets its own cache_path
    // under accounts/sessions/<id>/data. CEF requires a request-context
    // cache_path to be a SUBDIRECTORY of the global root_cache_path, so root it
    // at accounts/sessions here. (v4 is a hard-fork; the old single
    // cef_user_data profile is intentionally orphaned and accounts re-linked —
    // see CHANGELOG.) The shared `.bdic` spellcheck dictionaries live under this
    // root too and still persist across launches.
    let root_cache = crate::accounts::accounts_root().join("sessions");
    let _ = std::fs::create_dir_all(&root_cache);
    let settings = Settings {
        windowless_rendering_enabled: 1,
        external_message_pump: 1,
        no_sandbox: 1,
        root_cache_path: cef::CefString::from(root_cache.to_string_lossy().as_ref()),
        log_severity: cef::LogSeverity::WARNING,
        ..Default::default()
    };

    let ok = cef::initialize(
        Some(args.as_main_args()),
        Some(&settings),
        Some(app),
        std::ptr::null_mut(),
    );
    if ok != 1 {
        return Err(anyhow!("cef::initialize failed (returned {ok})"));
    }
    log::info!("CEF initialized");

    // Safety net: external_message_pump schedules work via the browser
    // process handler, but during early init or rapid bursts we sometimes
    // miss a tick. Pump at 8ms steady so the browser never stalls.
    glib::timeout_add_local(Duration::from_millis(8), || {
        cef::do_message_loop_work();
        glib::ControlFlow::Continue
    });

    Ok(())
}
