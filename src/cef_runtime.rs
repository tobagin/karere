use anyhow::{Result, anyhow};
use cef::{
    self, App, BrowserProcessHandler, CommandLine, ImplApp, ImplBrowserProcessHandler,
    ImplCommandLine, RenderProcessHandler, Settings, WrapApp, WrapBrowserProcessHandler,
    args::Args, rc::Rc, wrap_app, wrap_browser_process_handler,
};

use crate::handlers::render_process::ShellRenderProcessHandlerBuilder;
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Process-global message-pump coalescing flag (browser process only): true
/// while a single do_message_loop_work() is already scheduled but not yet run.
static PUMP_SCHEDULED: AtomicBool = AtomicBool::new(false);

/// True when launched with `--debug` / `--debuglevel=…`. Gates developer-only
/// surfaces that widen the attack surface (the F12 DevTools CDP port). Release
/// builds run without them; notifications use the in-process CDP API instead.
pub fn debug_enabled() -> bool {
    use std::sync::OnceLock;
    static D: OnceLock<bool> = OnceLock::new();
    *D.get_or_init(|| {
        std::env::args().any(|a| a == "--debug" || a.starts_with("--debuglevel="))
    })
}

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
            // DocumentPictureInPictureAPI: WhatsApp's "open call in another
            // window" can't be hosted by the OSR shell (dropped call + blank
            // view); disabling keeps the call in the main webview. Always on.
            // (Same switch name can't be appended twice — CEF's CommandLine map
            // overwrites — so the debug PNA/LNA disables are merged into this one
            // value below.)
            // Memory: WhatsApp is a single trusted site, so Chromium's per-frame
            // site isolation just forks the cross-origin WA/CDN iframes into
            // extra renderer processes (~hundreds of MB) for no security gain we
            // need. Fold them back into one renderer and drop the warm spare.
            // (Account isolation is preserved at the RequestContext level, not by
            // process splitting.) Pairs with --disable-site-isolation-trials below.
            let mut disabled = String::from(
                "DocumentPictureInPictureAPI\
                 ,IsolateOrigins\
                 ,site-per-process\
                 ,SpareRendererForSitePerProcess\
                 ,PersistentHistograms",
            );
            // PersistentHistograms backs metrics in a memory-mapped .pma file in
            // the user-data-dir; a truncated/invalidated mapping SIGBUSes in
            // PersistentSampleMap::Accumulate (seen crashing the browser process
            // via a UKM "dropped entry" histogram write). Off → histograms live on
            // the heap, removing the mmap fault path — this alone fixes that SIGBUS.
            //
            // Do NOT also disable `Ukm`: the segmentation platform still runs
            // UkmDatabaseClient::PreProfileInit at startup, which constructs a
            // UkmObserver and calls UkmRecorderImpl::AddUkmRecorderObserver on the
            // recorder. With Ukm disabled that recorder is null → SIGSEGV in
            // PreProfileInit before the window ever shows (symbolized 2026-06-09).
            // UKM has no metrics consent in CEF, so the recorder stays inert (no
            // upload) anyway — leaving the feature on is privacy-neutral here.

            // Embedded F12 DevTools (CDP frontend) is DEBUG-ONLY. It needs a
            // loopback debugging PORT, a wildcard inspector origin, and the
            // Local/Private Network Access gate disabled so the remote frontend
            // can reach loopback. Each widens the attack surface (a website can
            // DNS-rebind to the fixed port and drive the authenticated session),
            // so they ship ONLY with --debug. Release notifications use the
            // in-process DevTools message API (crate::cdp) — no port, no network.
            if debug_enabled() {
                cmd.append_switch_with_value(
                    Some(&"remote-debugging-port".into()),
                    Some(&crate::devtools::DEVTOOLS_PORT.to_string().as_str().into()),
                );
                cmd.append_switch_with_value(
                    Some(&"remote-allow-origins".into()),
                    Some(&"*".into()),
                );
                disabled.push_str(
                    ",BlockInsecurePrivateNetworkRequests\
                     ,LocalNetworkAccessChecks\
                     ,PrivateNetworkAccessForNavigations\
                     ,PrivateNetworkAccessForWorkers",
                );
            }
            cmd.append_switch_with_value(
                Some(&"disable-features".into()),
                Some(&disabled.as_str().into()),
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
            // Memory: turn off the site-isolation field trials too (the
            // disable-features list above only covers the static features), so
            // WhatsApp's cross-origin frames don't each fork their own renderer.
            cmd.append_switch(Some(&"disable-site-isolation-trials".into()));
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

        // On-demand pump driver (external_message_pump): CEF calls this from any
        // thread when it next needs do_message_loop_work() in delay_ms. Schedule
        // one pump on the GLOBAL default GLib main context (glib::timeout_add_once
        // targets it regardless of calling thread), so the closure runs on the
        // main thread. Idle => CEF schedules far out => near-zero idle CPU,
        // replacing the old fixed 125 Hz busy-poll.
        fn on_schedule_message_pump_work(&self, delay_ms: i64) {
            // Floor at 8ms: under continuous work CEF reschedules with delay 0,
            // and a 0ms timeout would fire every main-loop iteration → 100% spin.
            // 8ms caps the busy rate at ~125 Hz while still letting CEF's larger
            // idle delays through for low idle CPU.
            let delay = u64::try_from(delay_ms).unwrap_or(0).max(8);
            // Coalesce to a SINGLE pending pump, PROCESS-GLOBAL: CEF can hand out
            // fresh handler instances, so a per-handler flag may not dedupe.
            // Without coalescing CEF's high-frequency schedule calls each queue
            // another glib timeout, the backlog fires together, re-triggers work,
            // and pins a core (#151). Clear before do_message_loop_work() so a
            // pump scheduled *during* that work registers the next single timeout.
            if PUMP_SCHEDULED.swap(true, Ordering::AcqRel) {
                return;
            }
            glib::timeout_add_once(Duration::from_millis(delay), move || {
                PUMP_SCHEDULED.store(false, Ordering::Release);
                cef::do_message_loop_work();
            });
        }
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
    // 0700: this tree holds the WhatsApp auth (cookies/IndexedDB). World-readable
    // dirs would expose the session to other local users -> account takeover.
    {
        use std::os::unix::fs::DirBuilderExt;
        let _ = std::fs::DirBuilder::new()
            .mode(0o700)
            .recursive(true)
            .create(&root_cache);
    }
    // Match Chromium's own UI (context menus, error pages) to the UI-language
    // override by mapping it onto the nearest shipped `.pak` locale. Empty =
    // Chromium default (system).
    let override_lang = crate::i18n::override_locale();
    let cef_locale = override_lang
        .as_deref()
        .and_then(crate::i18n::cef_locale_for)
        .unwrap_or_default();
    // Drives Accept-Language + navigator.languages so the web content
    // (WhatsApp Web) localizes to the chosen UI language, not just Chromium.
    let accept_lang = override_lang
        .as_deref()
        .map(crate::i18n::accept_language_for)
        .unwrap_or_default();

    let settings = Settings {
        windowless_rendering_enabled: 1,
        external_message_pump: 1,
        no_sandbox: 1,
        root_cache_path: cef::CefString::from(root_cache.to_string_lossy().as_ref()),
        locale: cef::CefString::from(cef_locale.as_str()),
        accept_language_list: cef::CefString::from(accept_lang.as_str()),
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

    // Backstop only: on_schedule_message_pump_work drives normal pumping. This
    // slow tick guards against a missed schedule (e.g. early init) without the
    // old 8ms (125 Hz) busy-poll that pinned idle CPU.
    glib::timeout_add_local(Duration::from_millis(100), || {
        cef::do_message_loop_work();
        glib::ControlFlow::Continue
    });

    Ok(())
}
