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
            // Spellcheck/screen-reader switches belong on the browser process
            // only: process_type is empty/None for it, set for children.
            let is_browser_process = process_type
                .map(|p| p.to_string().is_empty())
                .unwrap_or(true);
            if is_browser_process {
                append_spellcheck_switches(cmd);
                append_screen_reader_switches(cmd);
            }
            // Embedded DevTools (CDP): loopback debugging endpoint. See
            // `crate::devtools`.
            cmd.append_switch_with_value(
                Some(&"remote-debugging-port".into()),
                Some(&crate::devtools::DEVTOOLS_PORT.to_string().as_str().into()),
            );
            // Allow any origin to open the inspector websocket (DevTools frontend
            // is served from a remote origin); endpoint is loopback-only.
            cmd.append_switch_with_value(
                Some(&"remote-allow-origins".into()),
                Some(&"*".into()),
            );
            // Private/Local Network Access blocks the public DevTools frontend
            // origin from reaching the loopback CDP endpoint, blanking DevTools;
            // disable the gate (renamed PNA -> LNA in M14x).
            cmd.append_switch_with_value(
                Some(&"disable-features".into()),
                Some(
                    &"BlockInsecurePrivateNetworkRequests,\
                      LocalNetworkAccessChecks,\
                      PrivateNetworkAccessForNavigations,\
                      PrivateNetworkAccessForWorkers,\
                      DocumentPictureInPictureAPI"
                        .into(),
                ),
            );
            // ^ DocumentPictureInPictureAPI: WhatsApp's "open call in another
            // window" can't be hosted by the OSR shell (dropped call + blank
            // view); disabling keeps the call in the main webview.
            // Do NOT disable SystemNotifications/NativeNotifications: that makes
            // Chromium show its own in-window popup instead of suppressing.
            // Notifications are suppressed in the SW/Notification shim (it never
            // calls the real showNotification), so no native banner and no
            // notification sound; Karere emits its own gio::Notification + paplay.
            // If a native banner ever leaks through, add --disable-notification-sound.
            cmd.append_switch(Some(&"enable-features=UseOzonePlatform".into()));
            cmd.append_switch_with_value(
                Some(&"ozone-platform-hint".into()),
                Some(&"auto".into()),
            );
            cmd.append_switch(Some(&"enable-webrtc-vea-vda".into()));
            // M17 paste bridge: lets the renderer fetch tempfile payloads over
            // file:// (blocked from non-file origins by default). Reach is scoped
            // to $XDG_RUNTIME_DIR/karere/ by the resource request handler.
            cmd.append_switch(Some(&"allow-file-access-from-files".into()));
            cmd.append_switch(Some(&"no-startup-window".into()));
            cmd.append_switch(Some(&"noerrdialogs".into()));
            cmd.append_switch(Some(&"hide-crash-restore-bubble".into()));
            // Single-webview app: zygote fork-sharing wins are negligible.
            // Disabling keeps a flat, debuggable process tree (forked renderers
            // are otherwise mislabeled --type=zygote).
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

        // Renderer-subprocess handler: injects the JS bundle and bridges
        // page <-> host IPC. CEF calls this getter only in the renderer.
        fn render_process_handler(&self) -> Option<RenderProcessHandler> {
            Some(ShellRenderProcessHandlerBuilder::build())
        }
    }
}

pub fn build_app() -> App {
    ShellAppBuilder::new(ShellApp)
}

/// Append `--enable-caret-browsing` when the `screen-reader-opts` GSetting is on
/// (M19). RESTART-REQUIRED: Chromium reads switches once at subprocess launch
/// (surfaced with a restart-required subtitle in prefs).
fn append_screen_reader_switches(cmd: &mut CommandLine) {
    use gtk::gio;
    use gtk::prelude::SettingsExt;

    let settings = gio::Settings::new(crate::application::APP_ID);
    if settings.boolean("screen-reader-opts") {
        cmd.append_switch(Some(&"enable-caret-browsing".into()));
        log::info!("screen-reader: --enable-caret-browsing (restart-required)");
    }
}

/// Resolve the spellcheck language list from GSettings (M16) and append the
/// Chromium switches. Keys: `enable-spell-checking` (off → --disable-spell-checking),
/// `spell-checking-languages` (explicit BCP-47 csv), `auto-detect-language`
/// (derive one code from the locale when the list is empty). Chromium
/// auto-downloads the `.bdic` dicts on first need; live changes go through
/// `KarereWebView::recreate_active_browser()`.
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
        // glib lists preferred locales most-specific first (ending in "C");
        // first non-C match is the best auto-detect guess.
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
/// glib main loop (see `initialize_browser_process`).
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

        // on_schedule_message_pump_work fires from any CEF thread, so we can't
        // schedule the main loop here; a glib timer drives the pump instead.
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

    // M20: per-account RequestContext cache_path lives under accounts/sessions/
    // <id>/data, and CEF requires it to be a SUBDIRECTORY of root_cache_path —
    // so root that at accounts/sessions. Shared `.bdic` dicts persist here too.
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

    // Safety net: pump at 8ms steady since external_message_pump scheduling can
    // miss a tick during early init or rapid bursts.
    glib::timeout_add_local(Duration::from_millis(8), || {
        cef::do_message_loop_work();
        glib::ControlFlow::Continue
    });

    Ok(())
}
