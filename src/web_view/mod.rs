use gtk::glib;
use gtk::subclass::prelude::*;

mod clipboard_dnd;
mod context_menu;
mod gl;
mod imp;
mod input;
mod page_settings;
mod pool;
mod x11_origin;
mod zoom;

pub(crate) use clipboard_dnd::recent_user_input;
use clipboard_dnd::note_user_input;
pub(crate) use context_menu::dispatch_context_menu;
use context_menu::{register_context_menu_widget, unregister_context_menu_widget};
use gl::{CpuUpload, cpu_upload, gl_viewport_for, page_background, texture_uv};
pub(crate) use page_settings::{
    apply_autocorrect_language, apply_mobile_layout, apply_notif_sound_from_settings,
    apply_spellcheck_to_browser, apply_user_css, user_css_path,
};
use page_settings::{
    mobile_layout_mismatch, record_mobile_layout, should_use_mobile_layout, with_app_settings,
};
pub(crate) use x11_origin::current_window_origin;
pub(crate) use zoom::{ZOOM_MAX, ZOOM_MIN, apply_zoom_from_account, zoom_floor};
use zoom::{cef_to_linear, host_zoom_level};

glib::wrapper! {
    pub struct KarereWebView(ObjectSubclass<imp::KarereWebView>)
        @extends gtk::GLArea, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for KarereWebView {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl KarereWebView {
    pub fn new() -> Self {
        Self::default()
    }

    /// Embedded DevTools frontend: permissive client keeps every navigation in-view.
    pub fn new_devtools() -> Self {
        let obj: Self = glib::Object::new();
        obj.imp()
            .devtools
            .store(true, std::sync::atomic::Ordering::Relaxed);
        obj
    }

    pub fn load_url(&self, url: &str) {
        self.imp().load_url(url);
    }

    pub fn close_browser(&self) {
        self.imp().close_browser();
    }

    pub fn reload(&self) {
        self.imp().reload();
    }

    /// Reload bypassing the cache (hard refresh).
    pub fn reload_hard(&self) {
        self.imp().reload_hard();
    }

    /// Reload every account's browser (live setting change, e.g. mobile-layout).
    pub fn reload_all(&self) {
        self.imp().reload_all();
    }

    /// Record chrome-window visibility (drives notification-sound muting).
    pub fn set_window_visible(&self, visible: bool) {
        self.imp().set_window_visible(visible);
    }

    /// Re-apply notification-sound muting from the current settings (call after a
    /// `notify-sound-enabled` / `notifications-enabled` change).
    pub fn apply_audio_mute(&self) {
        self.imp().apply_audio_mute();
    }

    pub fn find(&self, text: &str, forward: bool, find_next: bool) {
        self.imp().find(text, forward, find_next);
    }

    pub fn stop_finding(&self) {
        self.imp().stop_finding();
    }

    /// Set the active browser's zoom to `linear` (1.0 = 100 %), clamped to
    /// `[0.5, 3.0]` and converted to CEF's log level. No-op if no live browser. (M18)
    pub fn set_zoom_linear(&self, linear: f64) {
        self.imp().set_zoom_linear(linear);
    }

    /// The active browser's zoom as a linear factor (1.0 = 100 %), or 1.0 if none. (M18)
    pub fn get_zoom_linear(&self) -> f64 {
        self.imp().get_zoom_linear()
    }

    /// Forward Ctrl+Shift+C into this (DevTools) view to toggle its element
    /// picker. Only meaningful on the embedded DevTools view.
    pub fn dispatch_inspect_shortcut(&self) {
        input::keyboard::send_inspect_shortcut(self);
    }

    /// Re-emulate `prefers-color-scheme` on every live browser to match the
    /// current Karere theme (call after the theme changes). (#160)
    pub fn reapply_color_scheme(&self) {
        let imp = self.imp();
        let mut hosts = Vec::new();
        {
            use cef::ImplBrowser;
            for b in imp.browsers.lock().values() {
                if let Some(h) = b.host() {
                    hosts.push(h);
                }
            }
            if let Some(b) = imp.browser.lock().as_ref()
                && let Some(h) = b.host()
            {
                hosts.push(h);
            }
        }
        for h in &hosts {
            crate::cdp::apply_color_scheme(h);
        }
    }

    pub fn shared(&self) -> crate::handlers::SharedRef {
        self.imp()
            .shared
            .lock()
            .as_ref()
            .expect("KarereWebView::shared() called before construction initialized it")
            .clone()
    }

    /// Run `script` in the page's main frame if a browser is live. Drives the
    /// notification tracker's `__karereCloseNotif` / `__karereActivateNotif`.
    pub fn run_js(&self, script: &str) {
        self.imp().run_js(script);
    }

    /// Run `script` in the main frame of `account_id`'s browser (foreground or
    /// background); no-op when that account has no live browser.
    pub fn run_js_in_account(&self, account_id: &str, script: &str) {
        use cef::{CefString, ImplBrowser, ImplFrame};
        let browser = self.imp().browsers.lock().get(account_id).cloned();
        if let Some(frame) = browser.and_then(|b| b.main_frame()) {
            frame.execute_java_script(
                Some(&CefString::from(script)),
                Some(&CefString::from("karere://notify")),
                0,
            );
        }
    }

    /// Switch the live browser's spellcheck languages without recreating it, via the
    /// `spellcheck.dictionaries` / `browser.enable_spellchecking` request-context prefs;
    /// Chromium downloads missing `.bdic` dictionaries on demand. `langs` are BCP-47
    /// codes (e.g. `["pt-BR"]`); empty + `enabled` keeps Chromium's auto behaviour.
    pub fn set_spellcheck_languages(&self, langs: &[String], enabled: bool) {
        self.imp().set_spellcheck_languages(langs, enabled);
    }

    /// Spawn (or, if already present, surface) the browser for `account_id`.
    /// `foreground` makes it the visible account immediately.
    pub fn spawn_account(&self, account_id: &str, foreground: bool) {
        self.imp()
            .spawn_browser(Some(account_id.to_owned()), foreground);
    }

    /// Pre-warm every account's browser without showing any foreground, so
    /// background (tray) launch loads WhatsApp and notifications before the window
    /// shows. Idempotent — safe to call again on realize.
    pub fn prewarm(&self) {
        self.imp().spawn_all_accounts(false);
    }

    /// Switch the visible account to `account_id` (must already be spawned).
    pub fn switch_to_account(&self, account_id: &str) {
        self.imp().switch_to(account_id);
    }

    /// Close and drop the browser for `account_id` (the account-removal path).
    pub fn close_account(&self, account_id: &str) {
        self.imp().close_account_browser(account_id);
    }

    /// Quit path: close every browser for good. Unlike [`close_browser`](Self::close_browser)
    /// (also used to recreate the pool), contexts still initialising will not
    /// create a browser afterwards.
    pub fn shutdown_browsers(&self) {
        self.imp()
            .shutting_down
            .store(true, std::sync::atomic::Ordering::Release);
        self.imp().close_browser();
    }

    /// True once every browser this view asked to close (pooled accounts, the
    /// legacy/DevTools browser, removed accounts) has reported `on_before_close`,
    /// and the current foreground handler, if any, is closed too.
    pub fn is_browser_closed(&self) -> bool {
        let imp = self.imp();
        let mut closing = imp.closing_life_spans.lock();
        closing.retain(|l| !l.state.lock().closed);
        let current_open = imp
            .life_span
            .lock()
            .as_ref()
            .is_some_and(|l| !l.state.lock().closed);
        closing.is_empty() && !current_open
    }
}

/// Apply the cursor / IM focus / announcement requests pending in `area`'s
/// shared state. Called from the idle callback `handlers::wake_view` queues.
/// Main-thread only.
pub(crate) fn flush_view_requests(area: &gtk::GLArea) {
    use gtk::prelude::Cast;
    if let Some(view) = area.downcast_ref::<KarereWebView>() {
        view.imp().flush_requests(false);
    }
}

#[cfg(test)]
mod tests;
