use std::time::Duration;

use cef::{
    self, Browser, CefString, Errorcode, Frame, ImplBrowser, ImplFrame, ImplLoadHandler,
    LoadHandler, TransitionType, WrapLoadHandler, rc::Rc, wrap_load_handler,
};

use super::SharedRef;

const MAX_BACKOFF_MS: u64 = 60_000;

#[derive(Clone)]
pub struct ShellLoadHandler {
    shared: SharedRef,
}

impl ShellLoadHandler {
    pub fn new(shared: SharedRef) -> Self {
        Self { shared }
    }
}

wrap_load_handler! {
    pub struct ShellLoadHandlerBuilder {
        handler: ShellLoadHandler,
    }

    impl LoadHandler {
        fn on_load_start(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _transition_type: TransitionType,
        ) {
            // Main-frame navigation invalidates every cached notification tag:
            // the page (and its observer stub map) is being rebuilt, so any
            // pending withdrawal would target a dead frame (M14 3.5).
            let is_main = frame.is_none_or(|f| f.is_main() == 1);
            if is_main {
                crate::notifications::tracker().on_load_start();
                // New page → spellcheck must be re-applied once the renderer is
                // live again (signalled by the first editable-field focus).
                self.handler.shared.lock().spellcheck_last = None;
            }
        }

        fn on_loading_state_change(
            &self,
            _browser: Option<&mut Browser>,
            is_loading: ::std::os::raw::c_int,
            _can_go_back: ::std::os::raw::c_int,
            _can_go_forward: ::std::os::raw::c_int,
        ) {
            self.handler.shared.lock().is_loading = is_loading != 0;
            log::debug!("loading_state_change is_loading={}", is_loading != 0);
        }

        fn on_load_end(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            http_status_code: ::std::os::raw::c_int,
        ) {
            let (is_main, is_error_page, url) = frame
                .map(|f| {
                    let url = CefString::from(&f.url()).to_string();
                    (f.is_main() == 1, url.starts_with("chrome-error"), url)
                })
                .unwrap_or((true, false, String::new()));
            log::debug!(
                "on_load_end main={is_main} error_page={is_error_page} status={http_status_code} url={url}"
            );

            // A failed load still fires on_load_end on the main frame for the
            // Chromium error page. That is not a success: ignore it so the
            // scheduled retry and the backoff counter survive, and the offline
            // overlay stays up.
            if is_error_page || !is_main {
                return;
            }

            {
                let mut s = self.handler.shared.lock();
                s.load_error_count = 0;
                if let Some(id) = s.pending_reload.take() {
                    id.remove();
                }
                s.offline = false;
            }

            // Spellcheck is applied by the first editable-field focus (see
            // `render.rs::on_virtual_keyboard_requested`), which proves the
            // renderer is live — `on_load_end` is too early for it to stick.
            // The auto-correct flag IS pushed here since the page's `window`
            // reset on navigation.
            if let Some(browser) = browser {
                apply_autocorrect_from_settings(browser);
                // M18 4.1: restore this account's persisted zoom (floor-lifted)
                // on first paint and after each navigation.
                crate::web_view::apply_zoom_from_account(browser);
                // M21: inject the verbatim mobile-responsive script when the host
                // decides the layout is mobile for the current window width. The
                // logical width is the shared physical size divided by the scale
                // factor. Mirrors karere v3's inject-on-load.
                let width_logical = {
                    let s = self.handler.shared.lock();
                    let scale = if s.scale_factor > 0.0 { s.scale_factor } else { 1.0 };
                    (s.size.0 as f32 / scale).round() as i32
                };
                crate::web_view::apply_mobile_layout(browser, width_logical);
                // M14x: push the notification-sound mute flag so the bundle hook
                // silences WhatsApp's ding when sounds (or the master toggle) are
                // off — survives navigation since the page `window` reset.
                crate::web_view::apply_notif_sound_from_settings(browser);
            }
        }

        fn on_load_error(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            error_code: Errorcode,
            _error_text: Option<&CefString>,
            failed_url: Option<&CefString>,
        ) {
            let is_main = frame.is_none_or(|f| f.is_main() == 1);
            let url = failed_url.map(CefString::to_string).unwrap_or_default();
            log::debug!("on_load_error code={error_code:?} main={is_main} url={url}");

            if error_code == Errorcode::ABORTED {
                return;
            }
            // Only the main-frame load drives retries + the offline overlay;
            // a failing subframe (ad, analytics) must not reload the page.
            if !is_main {
                return;
            }
            // Backoff uses the 0-based retry index so the first failure waits
            // 500 ms, then 1000 ms, 2000 ms, … capped at 60 s.
            let delay_ms = {
                let mut s = self.handler.shared.lock();
                s.offline = true;
                if let Some(id) = s.pending_reload.take() {
                    id.remove();
                }
                let retry = s.load_error_count;
                s.load_error_count = retry.saturating_add(1);
                (500u64 << retry.min(20)).min(MAX_BACKOFF_MS)
            };
            log::warn!("load error {error_code:?} for {url}; reloading in {delay_ms} ms");

            let Some(browser) = browser else { return };
            let browser = browser.clone();
            let shared = self.handler.shared.clone();
            let id = glib::timeout_add_local_once(Duration::from_millis(delay_ms), move || {
                shared.lock().pending_reload = None;
                browser.reload_ignore_cache();
            });
            self.handler.shared.lock().pending_reload = Some(id);
        }
    }
}

impl ShellLoadHandlerBuilder {
    pub fn build(handler: ShellLoadHandler) -> LoadHandler {
        Self::new(handler)
    }
}

/// Resolve the effective spellcheck `(enabled, languages)` from GSettings —
/// explicit list, else auto-detected locale; empty when spellcheck is disabled.
pub(crate) fn resolve_spellcheck_settings() -> (bool, Vec<String>) {
    use gtk::gio;
    use gtk::prelude::{SettingsExt, SettingsExtManual};

    let settings = gio::Settings::new(crate::application::APP_ID);
    let enabled = settings.boolean("enable-spell-checking");
    let langs = if enabled {
        let explicit: Vec<String> = settings
            .strv("spell-checking-languages")
            .iter()
            .map(|s| s.to_string())
            .collect();
        crate::spellcheck::resolve_languages(&explicit, settings.boolean("auto-detect-language"))
    } else {
        Vec::new()
    };
    (enabled, langs)
}

/// Seed the injected auto-correct bundle's runtime flag (`enable-auto-correct`)
/// in the page's main frame. Called on every successful main-frame load and
/// whenever the preference changes, so the JS knows whether to replace words.
pub(crate) fn apply_autocorrect_from_settings(browser: &Browser) {
    use gtk::gio;
    use gtk::prelude::SettingsExt;

    let on = gio::Settings::new(crate::application::APP_ID).boolean("enable-auto-correct");
    let Some(frame) = browser.main_frame() else {
        return;
    };
    let js = format!("window.__karereAutoCorrect = {on};");
    frame.execute_java_script(
        Some(&CefString::from(js.as_str())),
        Some(&CefString::from("karere://autocorrect")),
        0,
    );
}
