use gtk::glib;
use gtk::subclass::prelude::*;

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
        imp::send_inspect_shortcut(self);
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

    pub fn is_browser_closed(&self) -> bool {
        match self.imp().life_span.lock().as_ref() {
            Some(life) => life.state.lock().closed,
            None => true, // no life-span handler yet → no browser to wait on
        }
    }
}

/// Min/max linear zoom factor the CEF boundary will apply (clamp guards against
/// pathological levels). (M18)
pub(crate) const ZOOM_MIN: f64 = 0.5;
pub(crate) const ZOOM_MAX: f64 = 3.0;

/// Linear zoom factor (1.0 = 100 %) → CEF's logarithmic `BrowserHost` level
/// (each unit = factor 1.2, 0 = 100 %). Clamped to `[ZOOM_MIN, ZOOM_MAX]`. (M18)
pub(crate) fn linear_to_cef(linear: f64) -> f64 {
    linear.clamp(ZOOM_MIN, ZOOM_MAX).ln() / 1.2_f64.ln()
}

/// Inverse of [`linear_to_cef`]: CEF logarithmic level → linear factor. (M18)
pub(crate) fn cef_to_linear(cef: f64) -> f64 {
    (cef * 1.2_f64.ln()).exp()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CpuUpload {
    Empty,
    Allocate,
    Update,
    Reuse,
}

/// Decide how the production draw path should handle the latest CPU frame.
/// Keeping this decision independent from raw GL calls makes empty/resize/reuse
/// behavior explicit while the actual upload remains in `KarereWebView::draw`.
fn cpu_upload(frame: &crate::handlers::FrameBuffer, texture_size: (i32, i32)) -> CpuUpload {
    if frame.width <= 0 || frame.height <= 0 || frame.pixels.is_empty() {
        CpuUpload::Empty
    } else if texture_size != (frame.width, frame.height) {
        CpuUpload::Allocate
    } else if frame.dirty {
        CpuUpload::Update
    } else {
        CpuUpload::Reuse
    }
}

fn discard_failed_accel<T>(pending: &mut Option<T>) {
    pending.take();
}

/// CEF `BrowserHost` zoom level to apply for a user zoom factor at a given
/// integer display scale. Because this CEF build ignores device_scale_factor for
/// OSR, the paint buffer is sized to the physical (logical × scale) view rect for
/// crispness; an equal page zoom then restores content to its logical size. The
/// display-scale term is added in CEF's log domain (each unit = ×1.2) so it is
/// NOT subject to the user-zoom `[ZOOM_MIN, ZOOM_MAX]` clamp. (#158)
pub(crate) fn host_zoom_level(user_linear: f64, display_scale: f64) -> f64 {
    let scale_term = display_scale.max(1.0).ln() / 1.2_f64.ln();
    linear_to_cef(user_linear) + scale_term
}

/// Effective accessibility zoom floor (linear): `zoom-level` setting when the
/// `webview-zoom` toggle is on, else the hard CEF minimum. Shared by the window's
/// zoom actions and the load handler's first-paint apply. (M18 5.x)
pub(crate) fn zoom_floor() -> f64 {
    use gtk::gio;
    use gtk::prelude::SettingsExt;
    let s = gio::Settings::new(crate::application::APP_ID);
    if s.boolean("webview-zoom") {
        s.double("zoom-level").clamp(ZOOM_MIN, ZOOM_MAX)
    } else {
        ZOOM_MIN
    }
}

/// The width (logical px) below which `auto` mode switches WhatsApp Web to the
/// single-pane mobile layout. Matches karere v3's `MOBILE_WIDTH_THRESHOLD`. (M21)
pub(crate) const MOBILE_WIDTH_THRESHOLD: i32 = 768;

/// Verbatim karere v3 `mobile_responsive.js` (git `890148c`), embedded for
/// on-demand injection (NOT in the always-run M13 bundle). Applies the single-pane
/// layout unconditionally when run, so the host gates *when* it runs (see
/// [`should_use_mobile_layout`] / [`apply_mobile_layout`]). (M21)
const EMBED_MOBILE: &str = include_str!("../data/js-deferred/mobile_responsive.js");

/// Whether WhatsApp Web should use the single-pane mobile layout (mirrors v3):
/// `mobile-layout` GSetting forces `enabled`/`disabled`; `auto` is true on a mobile
/// desktop (phosh/plasma-mobile/lomiri) or width in `(0, MOBILE_WIDTH_THRESHOLD)`. (M21)
pub(crate) fn should_use_mobile_layout(width_logical_px: i32) -> bool {
    use gtk::gio;
    use gtk::prelude::SettingsExt;

    match gio::Settings::new(crate::application::APP_ID)
        .string("mobile-layout")
        .as_str()
    {
        "enabled" => true,
        "disabled" => false,
        _ => {
            if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
                let d = desktop.to_lowercase();
                if ["phosh", "plasma-mobile", "lomiri"]
                    .iter()
                    .any(|m| d.contains(m))
                {
                    return true;
                }
            }
            width_logical_px > 0 && width_logical_px < MOBILE_WIDTH_THRESHOLD
        }
    }
}

/// Inject the mobile-responsive script into `browser`'s main frame when the layout
/// is mobile for `width_logical_px`. Idempotent per page via `window.__karereMobileApplied`.
/// Called from `on_load_end`; the script only un-applies via a full reload (driven
/// by the resize gate in `size_allocate`). (M21)
pub(crate) fn apply_mobile_layout(browser: &cef::Browser, width_logical_px: i32) {
    use cef::{CefString, ImplBrowser, ImplFrame};
    if !should_use_mobile_layout(width_logical_px) {
        return;
    }
    let Some(frame) = browser.main_frame() else {
        return;
    };
    // Guard against double-run within a page; flag resets on navigation.
    let guarded = format!(
        "(function(){{if(window.__karereMobileApplied)return;\
window.__karereMobileApplied=true;{EMBED_MOBILE}\n}})();"
    );
    frame.execute_java_script(
        Some(&CefString::from(guarded.as_str())),
        Some(&CefString::from("karere://mobile-responsive")),
        0,
    );
}

/// Effective notification-sound mute for `browser`: muted when the global
/// master/sound toggle is off OR this browser's account is individually muted
/// (per-account mute must silence the ding, not just the banner).
pub(crate) fn notif_sound_muted_for(browser: &cef::Browser) -> bool {
    use cef::ImplBrowser;
    use gtk::prelude::SettingsExt;
    let s = gtk::gio::Settings::new(crate::application::APP_ID);
    if !s.boolean("notifications-enabled") || !s.boolean("notify-sound-enabled") {
        return true;
    }
    crate::accounts::account_for_browser(browser.identifier())
        .map(|id| crate::accounts::manager().is_muted(&id))
        .unwrap_or(false)
}

/// Push `window.__karereMuteNotifSound` so the bundle hook (70-notification-sound.js)
/// blocks WhatsApp's notification/UI tones when the master/notification-sound
/// toggle is off OR the account is muted. Called from `on_load_end` (survives
/// navigation), on live settings change, and on a per-account mute toggle. (M14x)
pub(crate) fn apply_notif_sound_from_settings(browser: &cef::Browser) {
    use cef::{CefString, ImplBrowser, ImplFrame};
    let muted = notif_sound_muted_for(browser);
    let Some(frame) = browser.main_frame() else {
        return;
    };
    let js = format!("window.__karereMuteNotifSound = {muted};");
    frame.execute_java_script(
        Some(&CefString::from(js.as_str())),
        Some(&CefString::from("karere://notif-sound")),
        0,
    );
}

/// Apply the per-account persisted zoom (lifted to the accessibility floor) to
/// `browser`, rewriting the persisted value if the floor lifted it. Called from
/// `on_load_end` so each account restores its zoom on first paint and after
/// navigation. (M18 4.1 / 5.2)
pub(crate) fn apply_zoom_from_account(browser: &cef::Browser, display_scale: f64) {
    use cef::{ImplBrowser, ImplBrowserHost};
    let Some(id) = crate::accounts::account_for_browser(browser.identifier()) else {
        log::info!(
            "coord: J7 apply_zoom account=none browser_id={} display_scale={:.3} (uncompensated)",
            browser.identifier(),
            display_scale
        );
        return;
    };
    let persisted = crate::accounts::manager()
        .get(&id)
        .map(|a| a.zoom_level)
        .unwrap_or(1.0);
    let effective = persisted.max(zoom_floor());
    let cef_level = host_zoom_level(effective, display_scale);
    log::info!(
        "coord: J7 apply_zoom account={} user_linear={:.3} display_scale={:.3} cef_level={:.3}",
        id, effective, display_scale, cef_level
    );
    if let Some(host) = browser.host() {
        host.set_zoom_level(cef_level);
    }
    if (effective - persisted).abs() >= f64::EPSILON {
        crate::accounts::manager().set_zoom(&id, effective);
    }
}

/// Force Chromium to re-spellcheck the focused editable. A `spellcheck.dictionaries`
/// change only affects text typed AFTER it, so toggling the element's `spellcheck`
/// off→on makes Chromium recompute markers with the new dictionary. CEF UI thread only.
fn force_spellcheck_recheck(browser: &cef::Browser) {
    use cef::{CefString, ImplBrowser, ImplFrame};
    let Some(frame) = browser.main_frame() else {
        return;
    };
    let js = "(function(){var e=document.activeElement;if(!e)return;\
try{var t=e.closest&&e.closest('[contenteditable=\"true\"],input,textarea');var n=t||e;\
n.spellcheck=false;void n.offsetHeight;n.spellcheck=true;}catch(_){}})();";
    frame.execute_java_script(
        Some(&CefString::from(js)),
        Some(&CefString::from("karere://spellrecheck")),
        0,
    );
}

/// Write the `spellcheck.dictionaries` list pref on the browser's request context.
/// Returns false on failure. CEF UI thread only.
fn write_spellcheck_dictionaries(browser: &cef::Browser, langs: &[String]) -> bool {
    use cef::{
        CefString, ImplBrowser, ImplBrowserHost, ImplListValue, ImplPreferenceManager, ImplValue,
        list_value_create, value_create,
    };

    let Some(host) = browser.host() else {
        return false;
    };
    let Some(ctx) = host.request_context() else {
        return false;
    };
    let Some(mut list) = list_value_create() else {
        return false;
    };
    list.set_size(langs.len());
    for (i, lang) in langs.iter().enumerate() {
        let s = CefString::from(lang.as_str());
        list.set_string(i, Some(&s));
    }
    let Some(mut v) = value_create() else {
        return false;
    };
    v.set_list(Some(&mut list));
    let name = CefString::from("spellcheck.dictionaries");
    let mut err = CefString::from("");
    ctx.set_preference(Some(&name), Some(&mut v), Some(&mut err)) != 0
}

/// Apply the spellcheck enable flag + dictionary list to a live browser.
///
/// `force_clear` selects how the list is written:
/// - `true` (first apply of a page load): Chromium ignores a `Set` equal to the
///   persisted pref, so force a real `[]`→`[lang]` transition across a loop tick
///   so the renderer sees two distinct updates.
/// - `false` (live switch): value differs, so set `[lang]` DIRECTLY. The `[]` clear
///   must NOT be used — clearing after spellcheck is running tears the OSR service
///   down and the re-set doesn't revive it (dead until restart).
pub(crate) fn apply_spellcheck_to_browser(
    browser: &cef::Browser,
    langs: &[String],
    enabled: bool,
    force_clear: bool,
) {
    use cef::ImplBrowser;
    use cef::{CefString, ImplBrowserHost, ImplPreferenceManager, ImplValue, value_create};

    let Some(host) = browser.host() else { return };
    let Some(ctx) = host.request_context() else {
        log::warn!("apply_spellcheck: no request context");
        return;
    };

    if let Some(mut v) = value_create() {
        v.set_bool(enabled as i32);
        let name = CefString::from("browser.enable_spellchecking");
        let mut err = CefString::from("");
        if ctx.set_preference(Some(&name), Some(&mut v), Some(&mut err)) == 0 {
            log::warn!("set browser.enable_spellchecking failed: {}", err);
        }
    }

    if force_clear && !langs.is_empty() {
        // First apply: force the [] → [lang] transition across a loop tick.
        write_spellcheck_dictionaries(browser, &[]);
        log::info!("spellcheck.dictionaries = [] (force re-check)");
        let browser = browser.clone();
        let langs = langs.to_vec();
        glib::idle_add_local_once(move || {
            if write_spellcheck_dictionaries(&browser, &langs) {
                log::info!("spellcheck.dictionaries = {langs:?}");
            } else {
                log::warn!("set spellcheck.dictionaries failed");
            }
        });
        return;
    }

    // Live switch (or disable): write directly, no [] teardown.
    if write_spellcheck_dictionaries(browser, langs) {
        log::info!("spellcheck.dictionaries = {langs:?} (direct)");
    } else {
        log::warn!("set spellcheck.dictionaries failed");
    }
    if !langs.is_empty() {
        // Re-scan existing text in the new language (pref change only affects
        // newly-typed); deferred so the dictionary reaches the renderer first.
        let browser = browser.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
            force_spellcheck_recheck(&browser);
        });
    }
}

// ---- context-menu widget registry (main-thread only) ----------------------
//
// `run_context_menu` runs on the CEF UI thread = the glib main thread (external
// pump), so this registry and its non-`Send` `RunContextMenuCallback` never cross
// threads (deliberately NOT routed through the `Arc<Mutex>` `SharedRef`). Maps a
// CEF browser id to the widget rendering it.
thread_local! {
    static CTX_MENU_WIDGETS: std::cell::RefCell<
        std::collections::HashMap<i32, glib::WeakRef<KarereWebView>>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

pub(crate) fn register_context_menu_widget(id: i32, view: &KarereWebView) {
    use glib::prelude::ObjectExt;
    CTX_MENU_WIDGETS.with(|m| {
        m.borrow_mut().insert(id, view.downgrade());
    });
}

pub(crate) fn unregister_context_menu_widget(id: i32) {
    CTX_MENU_WIDGETS.with(|m| {
        m.borrow_mut().remove(&id);
    });
}

/// Route a snapshotted context menu to the widget owning `browser_id`. If the
/// widget is gone, cancel the callback so CEF doesn't leak pending-menu state.
/// Main-thread only.
pub fn dispatch_context_menu(
    browser_id: i32,
    items: Vec<crate::handlers::context_menu::MenuEntry>,
    x: i32,
    y: i32,
    callback: cef::RunContextMenuCallback,
) {
    let view = CTX_MENU_WIDGETS.with(|m| m.borrow().get(&browser_id).and_then(|w| w.upgrade()));
    match view {
        Some(v) => v.imp().show_context_menu(items, x, y, callback),
        None => {
            use cef::ImplRunContextMenuCallback;
            callback.cancel();
        }
    }
}

mod imp {
    use cef::{
        self, Browser, BrowserSettings, CefString, EventFlags, ImplBrowser, ImplBrowserHost,
        ImplFrame, ImplRequestContextHandler, ImplRunContextMenuCallback, KeyEvent, KeyEventType,
        MouseButtonType, MouseEvent, PointerType, ProcessId, RequestContext, RequestContextHandler,
        RequestContextSettings, RunContextMenuCallback, TouchEvent, TouchEventType, WindowInfo,
        WrapRequestContextHandler, browser_host_create_browser_sync, rc::Rc,
        request_context_create_context, sys, wrap_request_context_handler,
    };
    use gl::types::{GLenum, GLint, GLuint};
    use glib::subclass::Signal;
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use once_cell::sync::Lazy;
    use parking_lot::Mutex;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::ffi::{CString, c_void};

    use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

    use crate::handlers::{ClientBuilder, SharedRef, life_span::ShellLifeSpanHandler, new_shared};

    #[derive(Default)]
    pub struct KarereWebView {
        pub shared: Mutex<Option<SharedRef>>,
        /// The foreground browser (clone of the `browsers` entry for `foreground`).
        /// Input/render/spellcheck paths resolve through this → the visible account.
        pub browser: Mutex<Option<Browser>>,
        /// M20 browser pool: every account's CEF browser, keyed by account id.
        /// Background browsers stay alive (paused via `was_hidden(true)`) so a
        /// switch never logs the account out.
        pub browsers: Mutex<HashMap<String, Browser>>,
        /// Per-account life-span handlers, kept alive alongside their browsers.
        pub life_spans: Mutex<HashMap<String, ShellLifeSpanHandler>>,
        /// Per-account in-process CDP notification-bridge registrations. Held
        /// alive for the browser's lifetime; dropping one detaches the observer.
        pub cdp_registrations: RefCell<HashMap<String, cef::Registration>>,
        /// RequestContexts kept alive until the browser is created against them
        /// (in their init callback). Keyed by account id.
        pub pending_contexts: Mutex<HashMap<String, RequestContext>>,
        /// The account id whose browser is currently foreground.
        pub foreground: Mutex<Option<String>>,
        pub life_span: Mutex<Option<ShellLifeSpanHandler>>,
        pub pending_url: Mutex<Option<String>>,
        /// Set for the embedded DevTools view; selects the permissive client.
        pub devtools: AtomicBool,
        /// One-way runtime kill switch for this widget's accelerated OSR browsers.
        /// A rejected DMA-BUF flips it before the browser pool is recreated, so CEF
        /// resumes delivering CPU `on_paint` frames instead of repeatedly exporting
        /// accelerated frames that this GL context cannot import.
        pub software_osr_forced: AtomicBool,
        /// Test-only observations at the real close/create boundaries. This lets the
        /// DMA-BUF regression execute the production deferred restart without
        /// requiring a live CEF subprocess inside the unit-test process.
        #[cfg(test)]
        pub fallback_test_events: RefCell<Vec<(&'static str, Option<bool>)>>,
        #[cfg(test)]
        pub suppress_browser_creation: AtomicBool,
        /// Last chrome-window visibility (recorded; sound gating no longer uses it).
        #[allow(dead_code)]
        pub window_visible: AtomicBool,
        program: AtomicU32,
        vao: AtomicU32,
        vbo: AtomicU32,
        texture: AtomicU32,
        tex_w: AtomicI32,
        tex_h: AtomicI32,
        /// GPU-OSR: dedicated texture the imported DMA-BUF EGLImage binds to, and
        /// the live EGLImage kept alive while that texture is sampled. (gpu-osr)
        accel_tex: AtomicU32,
        imported: RefCell<Option<crate::gl_dmabuf::ImportedImage>>,
        /// Last pointer position (logical px) so wheel events hit the element
        /// under the cursor, not the top-left corner.
        last_mouse_x: AtomicI32,
        last_mouse_y: AtomicI32,
        /// Sub-pixel scroll remainder per axis, carried between scroll events so
        /// fine touchpad deltas aren't truncated away. Main-thread only. (#161)
        scroll_accum_x: std::cell::Cell<f64>,
        scroll_accum_y: std::cell::Cell<f64>,
        /// CEF browser id for context-menu registry (de)registration; `0` until spawn.
        browser_id: AtomicI32,
        /// Pending OSR context-menu callback (non-`Send`, main-thread only). Resolved
        /// exactly once in the popover `closed` handler: `cont(command)` or `cancel()`.
        /// Dispatching from `closed` (after popdown) re-focuses the webview first —
        /// required for spellcheck-replace and edit commands to apply.
        pub pending_menu_callback: RefCell<Option<RunContextMenuCallback>>,
        /// Command id selected by item activation, consumed by `closed`. `None` →
        /// dismissed without a selection.
        pub pending_menu_command: std::cell::Cell<Option<i32>>,
        /// The currently-displayed context popover, kept alive while open.
        pub context_popover: RefCell<Option<gtk::Popover>>,
        /// M21 mobile-layout resize gate. `mobile_active` = last mobile/desktop
        /// decision; `mobile_init` guards the first allocation so it seeds state
        /// without a spurious reload. A width-threshold crossing reloads so
        /// `on_load_end` re-evaluates the gate.
        pub mobile_active: std::cell::Cell<bool>,
        pub mobile_init: std::cell::Cell<bool>,
        /// The GLArea's IM context, stored so the widget timer can focus it in/out
        /// in response to CEF's editable-focus signal (keyboard_request). Set once
        /// in `install_input_controllers`.
        pub im_context: RefCell<Option<gtk::IMMulticontext>>,
        /// Mirror of the IM context's focus state. The page can leave the IM
        /// focused-out (a send re-renders the input → virtual-keyboard NONE with
        /// no follow-up), which kills dead-key composition; the key handler uses
        /// this to re-focus the IM on the next physical keypress. (#154)
        pub im_focused: std::cell::Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for KarereWebView {
        const NAME: &'static str = "KarereWebView";
        type Type = super::KarereWebView;
        type ParentType = gtk::GLArea;
    }

    impl ObjectImpl for KarereWebView {
        fn constructed(&self) {
            self.parent_constructed();
            let widget = self.obj();
            // The production shaders are GLSL ES 3.00. Negotiate that exact API
            // before realization instead of letting GtkGLArea prefer desktop GL;
            // GLES-only Mesa stacks (including PinePhone) otherwise fail context
            // creation before either the CPU or DMA-BUF OSR path can present.
            #[cfg(debug_assertions)]
            let allowed_api = if std::env::var_os("KARERE_TEST_FORCE_DESKTOP_GL").is_some() {
                // Integration-test-only reproduction of the pre-fix contract. The
                // Mesa fixture rejects desktop GL while retaining GLES 3.x.
                gtk::gdk::GLAPI::GL
            } else {
                gtk::gdk::GLAPI::GLES
            };
            #[cfg(not(debug_assertions))]
            let allowed_api = gtk::gdk::GLAPI::GLES;
            widget.set_allowed_apis(allowed_api);
            widget.set_required_version(3, 0);
            widget.set_has_depth_buffer(false);
            widget.set_has_stencil_buffer(false);
            widget.set_auto_render(false);

            let scale = widget.scale_factor() as f32;
            // Default viewport so prewarm browsers (created before sizing) lay out
            // usably; the real allocation replaces it on first show. Stored
            // PHYSICAL (logical 1280×800 × scale) like size_allocate writes it —
            // a logical seed with the pre-realize scale guess of 2 (multi-monitor
            // mixed scale) read back as 640 logical in on_load_end and wrongly
            // injected the mobile layout (#176).
            let shared = new_shared(((1280.0 * scale) as i32, (800.0 * scale) as i32), scale);
            *self.shared.lock() = Some(shared.clone());

            // CEF on_paint runs on the glib main thread (external_message_pump);
            // poll the dirty flag and queue a render when a frame arrives. Uses a
            // 60 Hz WALL-CLOCK timer, NOT add_tick_callback: a tick callback is
            // paced by the widget frame clock, which free-runs (pinning a core)
            // when vsync is unreliable — e.g. software/non-conformant Vulkan
            // (issue #151). A timeout is bounded, and idle ticks are a cheap
            // lock + bool check (~0 CPU) since nothing is dirty.
            let w_weak = widget.downgrade();
            glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
                let Some(w) = w_weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                let imp = w.imp();
                let Some(shared) = imp.shared.lock().clone() else {
                    return glib::ControlFlow::Continue;
                };
                let (cursor_name, keyboard_request) = {
                    let mut s = shared.lock();
                    if s.frame.dirty || s.accel.as_ref().is_some_and(|a| a.dirty) {
                        w.queue_render();
                    }
                    let cursor = if s.cursor_dirty {
                        s.cursor_dirty = false;
                        Some(s.cursor_name)
                    } else {
                        None
                    };
                    (cursor, s.keyboard_request.take())
                };
                if let Some(name) = cursor_name {
                    w.set_cursor_from_name(Some(name));
                }
                // Editable focus changed in the page → focus the IM context so the
                // on-screen keyboard shows only for real text fields.
                if let Some(focused) = keyboard_request
                    && let Some(im) = imp.im_context.borrow().as_ref()
                {
                    if focused {
                        im.focus_in();
                    } else {
                        im.focus_out();
                    }
                    imp.im_focused.set(focused);
                }
                glib::ControlFlow::Continue
            });

            widget.set_focusable(true);
            widget.set_can_focus(true);
            install_input_controllers(&widget);
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![
                    Signal::builder("title-changed")
                        .param_types([String::static_type()])
                        .build(),
                ]
            });
            SIGNALS.as_ref()
        }
    }

    impl WidgetImpl for KarereWebView {
        fn realize(&self) {
            self.parent_realize();
            let widget = self.obj();
            widget.make_current();
            if let Some(err) = widget.error() {
                // Fence all GL setup and browser creation behind a valid current
                // context. Keep GTK's original error intact for diagnostics.
                log::error!("GLArea realize error: {err}");
                return;
            }
            if let Some(context) = widget.context() {
                let (major, minor) = context.version();
                log::info!(
                    "GLArea context ready: api={:?} version={major}.{minor}",
                    widget.api()
                );
            } else {
                // A successful make_current() should always expose its context.
                // Do not continue into raw GL or CEF bootstrap if GTK does not.
                log::error!("GLArea realize error: no context after make_current");
                return;
            }
            unsafe {
                self.init_gl();
            }
            self.bootstrap_pool();

            // Follow scale changes (e.g. dragging between monitors of different
            // scale) so device_scale_factor / paint buffer track. The surface
            // `scale` notify fires across the integer boundaries the OSR buffer
            // cares about; a pure fractional change leaves the integer paint
            // scale unchanged, so refresh_screen_scale is then a cheap no-op. (#155, #158)
            if let Some(surface) = widget.native().and_then(|n| n.surface()) {
                surface.connect_scale_notify(glib::clone!(
                    #[weak]
                    widget,
                    move |_s| widget.imp().refresh_screen_scale()
                ));
            }
        }

        fn unrealize(&self) {
            self.close_browser();
            unsafe {
                self.teardown_gl();
            }
            self.parent_unrealize();
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
            // This CEF build ignores device_scale_factor for OSR: on_paint always
            // matches GetViewRect exactly. So the view rect must be PHYSICAL
            // (logical × integer GLArea scale) to fill the framebuffer 1:1 and stay
            // crisp; the resulting too-many-CSS-px (content too small) is corrected
            // by an equal page zoom applied to the browser (see host_zoom_level).
            // Mouse/wheel/menu coords are therefore physical too. (#158)
            let scale = paint_scale(&self.obj());
            let phys_w = (width as f64 * scale).round() as i32;
            let phys_h = (height as f64 * scale).round() as i32;

            let (old_size, old_scale) = self
                .shared
                .lock()
                .as_ref()
                .map(|sh| {
                    let s = sh.lock();
                    (s.size, s.scale_factor as f64)
                })
                .unwrap_or(((0, 0), 1.0));
            if let Some(shared) = self.shared.lock().as_ref() {
                let mut s = shared.lock();
                s.size = (phys_w, phys_h);
                s.scale_factor = scale as f32;
                s.window_origin = window_origin_for(&self.obj());
            }
            log::info!(
                "coord: J1 size_allocate logical={}x{} scale={:.3} physical={}x{} old_scale={:.3} new_scale={:.3} old_physical={}x{}",
                width, height, scale, phys_w, phys_h, old_scale, scale, old_size.0, old_size.1
            );

            if let Some(browser) = resolved_browser(self)
                && let Some(host) = browser.host()
            {
                host.notify_screen_info_changed();
                host.was_resized();
                // If the integer scale changed since the last allocation (e.g.
                // stale prewarm seed #176 or monitor move), re-sync the zoom so
                // the physical buffer and input transform stay coupled (#158).
                if (scale - old_scale).abs() > f64::EPSILON {
                    super::apply_zoom_from_account(&browser, scale);
                }
            }

            // M21: gate the mobile single-pane layout on logical width. The v3
            // script can't un-apply its DOM/CSS, so a threshold crossing reloads
            // and `on_load_end` re-injects (or not).
            if width > 0 {
                let is_mobile = super::should_use_mobile_layout(width);
                if !self.mobile_init.get() {
                    // First real allocation: seed state; first on_load_end injects if mobile.
                    self.mobile_init.set(true);
                    self.mobile_active.set(is_mobile);
                } else if is_mobile != self.mobile_active.get() {
                    self.mobile_active.set(is_mobile);
                    if let Some(browser) = resolved_browser(self) {
                        browser.reload();
                    }
                }
            }
        }
    }

    impl GLAreaImpl for KarereWebView {
        fn render(&self, _ctx: &gtk::gdk::GLContext) -> glib::Propagation {
            unsafe {
                self.draw();
            }
            glib::Propagation::Stop
        }
    }

    // ---- public-from-parent helpers ------------------------------------

    impl KarereWebView {
        pub fn load_url(&self, url: &str) {
            if let Some(browser) = self.browser.lock().as_ref()
                && let Some(frame) = browser.main_frame()
            {
                let s = CefString::from(url);
                frame.load_url(Some(&s));
                return;
            }
            *self.pending_url.lock() = Some(url.to_owned());
        }

        fn cancel_context_menu(&self) {
            self.pending_menu_command.set(None);
            if let Some(cb) = self.pending_menu_callback.borrow_mut().take() {
                cb.cancel();
            }
            if let Some(pop) = self.context_popover.borrow_mut().take() {
                pop.unparent();
            }
        }

        pub fn close_browser(&self) {
            #[cfg(test)]
            if self.suppress_browser_creation.load(Ordering::Acquire) {
                self.fallback_test_events.borrow_mut().push(("close", None));
            }

            // Cancel any in-flight OSR menu so CEF doesn't leak pending-menu state
            // on mid-menu teardown, and drop the popover.
            self.cancel_context_menu();
            let id = self.browser_id.swap(0, Ordering::Relaxed);
            if id != 0 {
                super::unregister_context_menu_widget(id);
            }

            // Close every pooled account browser, then the legacy/DevTools single
            // browser if it's outside the pool.
            let pooled: Vec<Browser> = self.browsers.lock().drain().map(|(_, b)| b).collect();
            self.life_spans.lock().clear();
            self.cdp_registrations.borrow_mut().clear();
            self.pending_contexts.lock().clear();
            *self.foreground.lock() = None;
            if let Some(shared) = self.shared.lock().as_ref() {
                shared.lock().foreground_browser_id = 0;
            }
            for browser in &pooled {
                crate::accounts::unregister_browser(browser.identifier());
                if let Some(host) = browser.host() {
                    host.close_browser(0);
                }
            }
            if pooled.is_empty()
                && let Some(browser) = self.browser.lock().as_ref()
                && let Some(host) = browser.host()
            {
                host.close_browser(0);
            }
            *self.browser.lock() = None;
            *self.life_span.lock() = None;
        }

        /// Recreate this widget's browser pool with CEF shared textures disabled.
        /// `shared_texture_enabled` is immutable after browser creation, so dropping
        /// only the failed accelerated frame cannot restore CPU `on_paint` delivery.
        fn restart_with_software_osr(&self) {
            log::warn!("accelerated OSR disabled for this view; recreating browsers for CPU paint");
            let is_devtools = self.devtools.load(Ordering::Relaxed);
            // Keep account contexts that were already initializing. Their callbacks
            // consult the new software-only flag, and retaining them avoids racing a
            // duplicate context/browser creation during a first-frame fallback.
            let pending = std::mem::take(&mut *self.pending_contexts.lock());
            self.close_browser();
            *self.pending_contexts.lock() = pending;
            if is_devtools {
                self.spawn_browser(None, true);
            } else {
                self.spawn_all_accounts(true);
            }
        }

        /// Make the accelerated-to-software transition exactly once and defer the
        /// CEF pool restart until after the current GLArea render callback returns.
        pub(super) fn force_software_osr(&self) -> bool {
            !self.software_osr_forced.swap(true, Ordering::AcqRel)
        }

        fn schedule_software_osr_fallback(&self) {
            if !self.force_software_osr() {
                return;
            }
            let weak = self.obj().downgrade();
            glib::idle_add_local_once(move || {
                if let Some(widget) = weak.upgrade() {
                    widget.imp().restart_with_software_osr();
                }
            });
        }

        pub(super) fn shared_texture_enabled_for_browser(&self) -> bool {
            !self.software_osr_forced.load(Ordering::Acquire) && accel_osr_enabled()
        }

        /// Recompute the physical paint size + scale for a live scale change
        /// (e.g. dragging between monitors) and tell CEF, then re-apply zoom so
        /// the display-scale term tracks the new scale. (#155, #158)
        fn refresh_screen_scale(&self) {
            let widget = self.obj();
            let scale = paint_scale(&widget);
            let (lw, lh) = (widget.width(), widget.height());
            let (old_size, old_scale) = self
                .shared
                .lock()
                .as_ref()
                .map(|sh| {
                    let s = sh.lock();
                    (s.size, s.scale_factor as f64)
                })
                .unwrap_or(((0, 0), 1.0));
            let new_phys = if lw > 0 && lh > 0 {
                ((lw as f64 * scale).round() as i32, (lh as f64 * scale).round() as i32)
            } else {
                old_size
            };
            if let Some(shared) = self.shared.lock().as_ref() {
                let mut s = shared.lock();
                s.scale_factor = scale as f32;
                if lw > 0 && lh > 0 {
                    s.size = new_phys;
                }
                s.window_origin = window_origin_for(&widget);
            }
            log::info!(
                "coord: J6 refresh_scale logical={}x{} scale={:.3} physical={}x{} old_scale={:.3} new_scale={:.3} old_physical={}x{}",
                lw, lh, scale, new_phys.0, new_phys.1, old_scale, scale, old_size.0, old_size.1
            );
            if let Some(browser) = resolved_browser(self)
                && let Some(host) = browser.host()
            {
                host.notify_screen_info_changed();
                host.was_resized();
                super::apply_zoom_from_account(&browser, scale);
            }
            widget.queue_render();
        }

        /// Present the snapshotted CEF context menu as a GTK `Popover` of buttons at
        /// the cursor over the `GLArea`. Main-thread only. Button click records the
        /// command + pops down; `closed` dispatches `cont`/`cancel` (exactly one).
        /// Manual button popover, not `PopoverMenu`+`gio::Menu`: the model menu's
        /// actions didn't activate when parented to the OSR `GLArea`.
        pub fn show_context_menu(
            &self,
            items: Vec<crate::handlers::context_menu::MenuEntry>,
            x_dev: i32,
            y_dev: i32,
            callback: RunContextMenuCallback,
        ) {
            // Replace any stale menu (shouldn't normally overlap).
            if let Some(old) = self.context_popover.borrow_mut().take() {
                old.unparent();
            }
            if let Some(cb) = self.pending_menu_callback.borrow_mut().take() {
                cb.cancel();
            }
            *self.pending_menu_callback.borrow_mut() = Some(callback);
            self.pending_menu_command.set(None);

            let obj = self.obj();
            let popover = gtk::Popover::new();
            popover.set_parent(&*obj);
            popover.set_has_arrow(false);
            popover.set_autohide(true);
            popover.set_position(gtk::PositionType::Bottom);
            popover.add_css_class("menu");

            let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
            build_menu_box(&items, &container, &obj, &popover);
            popover.set_child(Some(&container));

            // CEF reports the cursor in view coords = physical pixels (view_rect
            // is physical, #158); GTK widget coords are logical, so divide by the
            // integer scale to anchor the popover at the cursor.
            let s = obj.scale_factor().max(1) as f64;
            let rect = gtk::gdk::Rectangle::new(
                (x_dev as f64 / s).round() as i32,
                (y_dev as f64 / s).round() as i32,
                1,
                1,
            );
            popover.set_pointing_to(Some(&rect));

            // Resolve the callback AFTER popdown (webview re-focused): dispatch the
            // activated command, or cancel if dismissed without a selection.
            popover.connect_closed(glib::clone!(
                #[weak]
                obj,
                move |pop| {
                    let imp = obj.imp();
                    let cmd = imp.pending_menu_command.take();
                    if let Some(cb) = imp.pending_menu_callback.borrow_mut().take() {
                        // Re-assert CEF focus before the command (replace/cut/paste
                        // act on the focused frame's selection).
                        set_focus(&obj, true);
                        match cmd {
                            Some(id) => {
                                log::debug!("context menu: cont(id={id})");
                                dispatch_explicit_copy(
                                    ExplicitCopyTrigger::ContextMenu(id),
                                    || request_live_selection(&obj),
                                );
                                cb.cont(id, EventFlags::default());
                            }
                            None => cb.cancel(),
                        }
                    }
                    pop.unparent();
                    *imp.context_popover.borrow_mut() = None;
                }
            ));

            *self.context_popover.borrow_mut() = Some(popover.clone());
            popover.popup();
        }

        pub fn run_js(&self, script: &str) {
            if let Some(browser) = self.browser.lock().as_ref()
                && let Some(frame) = browser.main_frame()
            {
                let code = CefString::from(script);
                let url = CefString::from("karere://notify");
                frame.execute_java_script(Some(&code), Some(&url), 0);
            }
        }

        /// Current integer display scale from the shared state (≥ 1). Folded into
        /// the applied zoom to emulate the HiDPI paint buffer (#158).
        fn display_scale(&self) -> f64 {
            self.shared
                .lock()
                .as_ref()
                .map(|s| s.lock().scale_factor as f64)
                .unwrap_or(1.0)
                .max(1.0)
        }

        /// Set the foreground browser's zoom from a linear factor. The display
        /// scale is added on top (page-zoom HiDPI emulation, #158). CEF UI thread
        /// only. (M18)
        pub fn set_zoom_linear(&self, linear: f64) {
            let display_scale = self.display_scale();
            let cef_level = super::host_zoom_level(linear, display_scale);
            if let Some(browser) = self.browser.lock().as_ref().cloned() {
                let acct = crate::accounts::account_for_browser(browser.identifier())
                    .unwrap_or_else(|| "none".to_string());
                log::info!(
                    "coord: J7 set_zoom_linear account={} user_linear={:.3} display_scale={:.3} cef_level={:.3}",
                    acct, linear, display_scale, cef_level
                );
                if let Some(host) = browser.host() {
                    host.set_zoom_level(cef_level);
                }
            } else {
                log::info!(
                    "coord: J7 set_zoom_linear account=none user_linear={:.3} display_scale={:.3} cef_level={:.3} (no browser)",
                    linear, display_scale, cef_level
                );
            }
        }

        /// Foreground browser's USER zoom as a linear factor (the display-scale
        /// term removed), or 1.0 if none. (M18)
        pub fn get_zoom_linear(&self) -> f64 {
            if let Some(browser) = self.browser.lock().as_ref().cloned()
                && let Some(host) = browser.host()
            {
                let scale_term = self.display_scale().ln() / 1.2_f64.ln();
                super::cef_to_linear(host.zoom_level() - scale_term)
            } else {
                1.0
            }
        }

        /// Live spellcheck-language switch via request-context prefs. CEF UI thread
        /// only — the driving GTK callbacks are already on the glib main thread.
        pub fn set_spellcheck_languages(&self, langs: &[String], enabled: bool) {
            let Some(browser) = self.browser.lock().as_ref().cloned() else {
                log::warn!("set_spellcheck_languages: no live browser");
                return;
            };
            // Live switch: never use the [] force-clear (tears down OSR spellcheck).
            super::apply_spellcheck_to_browser(&browser, langs, enabled, false);
        }

        pub fn reload(&self) {
            if let Some(browser) = self.browser.lock().as_ref() {
                browser.reload();
            }
        }

        /// Reload every account's browser (foreground + background) so each
        /// re-evaluates a changed load-time setting (e.g. `mobile-layout`) via `on_load_end`.
        pub fn reload_all(&self) {
            let browsers = self.browsers.lock();
            if browsers.is_empty() {
                if let Some(browser) = self.browser.lock().as_ref() {
                    browser.reload();
                }
                return;
            }
            for browser in browsers.values() {
                browser.reload();
            }
        }

        /// Record window visibility (kept for callers; sound gating uses the JS hook now).
        pub fn set_window_visible(&self, visible: bool) {
            self.window_visible.store(visible, Ordering::Relaxed);
            // Gate the foreground browser's compositing on real window
            // visibility. Without this CEF keeps compositing WhatsApp's
            // animations while the window is hidden (tray/minimized/
            // start-in-background), pinning a core in software-GL OSR (#151).
            if let Some(b) = self.browser.lock().as_ref()
                && let Some(host) = b.host()
            {
                if visible {
                    host.was_hidden(0);
                    host.was_resized();
                    // Window re-shown after prewarm/tray: stale seed may have
                    // sized the buffer wrong (#176); ensure zoom tracks scale.
                    super::apply_zoom_from_account(b, self.display_scale());
                    host.invalidate(cef::PaintElementType::VIEW);
                } else {
                    host.was_hidden(1);
                }
            }
            if visible {
                self.obj().queue_render();
            }
        }

        /// Push the mute flag (`window.__karereMuteNotifSound`) to every account.
        /// The bundle hook (70-notification-sound.js) blocks WhatsApp's notification/UI
        /// tones (not WebRTC call audio or voice notes). Muted when master OR
        /// notification-sound toggle is off.
        pub fn apply_audio_mute(&self) {
            // Per-browser: each re-evaluates global toggle OR its own account's
            // mute (see super::notif_sound_muted_for).
            let browsers = self.browsers.lock();
            if browsers.is_empty() {
                if let Some(b) = self.browser.lock().as_ref() {
                    super::apply_notif_sound_from_settings(b);
                }
            } else {
                for b in browsers.values() {
                    super::apply_notif_sound_from_settings(b);
                }
            }
        }

        pub fn reload_hard(&self) {
            if let Some(browser) = self.browser.lock().as_ref() {
                browser.reload_ignore_cache();
            }
        }

        /// WhatsApp Web entry point; every account browser starts here.
        const WHATSAPP_URL: &'static str = "https://web.whatsapp.com/";

        /// Bring up the account pool on first realize: spawn every account and adopt
        /// the active one as the visible foreground.
        fn bootstrap_pool(&self) {
            self.spawn_all_accounts(true);
        }

        /// Spawn a CEF browser for EVERY account so each loads WhatsApp and delivers
        /// notifications, not just the visible one. `adopt_active` makes the active
        /// account the foreground; otherwise all stay paused (`was_hidden`) — the
        /// background-start prewarm path (no window yet). Idempotent: [`spawn_browser`]
        /// skips already-spawned accounts, so realize-after-prewarm only adopts the foreground.
        pub fn spawn_all_accounts(&self, adopt_active: bool) {
            if self.devtools.load(Ordering::Relaxed) {
                // DevTools view is not an account: single legacy browser, default context.
                self.spawn_browser(None, true);
                return;
            }

            let mgr = crate::accounts::manager();
            if mgr.get_accounts_sorted().is_empty() {
                mgr.add();
            }
            // Default account is created label-less; backfill it (and migrate
            // older installs) to "Account 1" so it matches added accounts.
            mgr.backfill_labels();
            let accounts = mgr.get_accounts_sorted();
            // Boot into the last-used account, not list-first (#166).
            let Some(first) = mgr.mru_first() else {
                log::error!("spawn_all_accounts: no account after add()");
                return;
            };
            mgr.activate(&first.id);
            for acc in &accounts {
                let foreground = adopt_active && acc.id == first.id;
                self.spawn_browser(Some(acc.id.clone()), foreground);
            }
        }

        /// Spawn a CEF browser for `account_id` (or the legacy default context when
        /// `None`, DevTools only). `make_foreground` shows it; else it starts paused.
        ///
        /// A per-account isolated `RequestContext` must finish init before
        /// `CreateBrowserSync` succeeds (CEF Chrome runtime), so creation is deferred
        /// to its init callback ([`on_account_context_ready`]). The legacy/DevTools
        /// path uses the global context and is created synchronously.
        pub fn spawn_browser(&self, account_id: Option<String>, make_foreground: bool) {
            #[cfg(test)]
            if self.suppress_browser_creation.load(Ordering::Acquire) {
                // Before fallback the exact capability probe is irrelevant to this
                // test seam (and may require an installed GSettings schema). After
                // fallback, call the production selector and observe the immutable
                // WindowInfo choice that the recreated browser would receive.
                let shared_textures = if self.software_osr_forced.load(Ordering::Acquire) {
                    self.shared_texture_enabled_for_browser()
                } else {
                    true
                };
                self.fallback_test_events
                    .borrow_mut()
                    .push(("create", Some(shared_textures)));
                return;
            }

            if let Some(id) = account_id.as_ref() {
                if self.browsers.lock().contains_key(id) {
                    if make_foreground {
                        self.switch_to(id);
                    }
                    return;
                }
                if self.pending_contexts.lock().contains_key(id) {
                    return;
                }
            }

            let Some(id) = account_id else {
                // Legacy / DevTools view: global context, synchronous create.
                self.create_browser_now(None, None, make_foreground);
                return;
            };

            // Per-account: build the isolated context now (held in `pending_contexts`),
            // create the browser once it signals initialized.
            let cache = crate::accounts::session_cache_path(&id);
            // 0700: per-account profile holds auth cookies/session — keep it
            // unreadable by other local users (see cef_runtime root_cache).
            {
                use std::os::unix::fs::DirBuilderExt;
                let _ = std::fs::DirBuilder::new()
                    .mode(0o700)
                    .recursive(true)
                    .create(&cache);
            }
            // Inherit the UI-language override so WhatsApp Web localizes too;
            // empty (no override) leaves the global CefSettings default.
            let accept_lang = crate::i18n::override_locale()
                .as_deref()
                .map(crate::i18n::accept_language_for)
                .unwrap_or_default();
            let rc_settings = RequestContextSettings {
                cache_path: CefString::from(cache.to_string_lossy().as_ref()),
                persist_session_cookies: 1,
                accept_language_list: CefString::from(accept_lang.as_str()),
                ..Default::default()
            };
            let mut handler =
                ContextReadyHandler::new(self.obj().downgrade(), id.clone(), make_foreground);
            match request_context_create_context(Some(&rc_settings), Some(&mut handler)) {
                Some(ctx) => {
                    self.pending_contexts.lock().insert(id, ctx);
                }
                None => log::error!("request_context_create_context returned None for {id}"),
            }
        }

        /// Continuation from the per-account `RequestContext` init callback: context
        /// ready, so create the browser against it.
        pub fn on_account_context_ready(
            &self,
            request_context: Option<&mut RequestContext>,
            account_id: &str,
            make_foreground: bool,
        ) {
            self.create_browser_now(
                request_context,
                Some(account_id.to_owned()),
                make_foreground,
            );
            // Browser now holds its own ref; release the pending hold.
            self.pending_contexts.lock().remove(account_id);
        }

        /// Create the browser (sync) against `request_context` (or global if `None`)
        /// and wire it into the pool.
        fn create_browser_now(
            &self,
            request_context: Option<&mut RequestContext>,
            account_id: Option<String>,
            make_foreground: bool,
        ) {
            let shared = self
                .shared
                .lock()
                .as_ref()
                .expect("create_browser_now called before shared was initialized")
                .clone();
            let (client, life) = if self.devtools.load(Ordering::Relaxed) {
                ClientBuilder::build_devtools_for(shared.clone())
            } else {
                ClientBuilder::build_for(shared.clone())
            };

            let window_info = WindowInfo {
                windowless_rendering_enabled: 1,
                // GPU-accelerated OSR: hand us a DMA-BUF via on_accelerated_paint
                // instead of a CPU buffer, when supported + opted in. (gpu-osr)
                shared_texture_enabled: self.shared_texture_enabled_for_browser() as i32,
                ..Default::default()
            };
            let settings = BrowserSettings {
                // 60fps OSR: the old 30 cap read as scroll/typing lag next to a
                // real browser (#173) — measured 29 vs 58fps under wheel load.
                // Idle cost is nil (an idle chat paints ~0 frames; hidden windows
                // are paused via was_hidden, #151), so this only spends CPU while
                // content actually animates. Runtime set_windowless_frame_rate is
                // a no-op in this CEF build, so the rate is fixed at creation.
                windowless_frame_rate: 60,
                ..Default::default()
            };

            let url_string = self
                .pending_url
                .lock()
                .take()
                .unwrap_or_else(|| Self::WHATSAPP_URL.to_owned());
            let url = CefString::from(url_string.as_str());

            let mut client = client;
            let browser = browser_host_create_browser_sync(
                Some(&window_info),
                Some(&mut client),
                Some(&url),
                Some(&settings),
                None,
                request_context,
            );
            let Some(b) = browser else {
                log::error!("browser_host_create_browser_sync returned None");
                return;
            };
            log::info!("browser spawned (account={account_id:?}, foreground={make_foreground})");

            if let Some(id) = account_id.clone() {
                crate::accounts::register_browser(b.identifier(), &id);
                self.browsers.lock().insert(id.clone(), b.clone());
                self.life_spans.lock().insert(id.clone(), life.clone());
                // In-process notification bridge (no CDP port). Attributes
                // notifications to this account via the trusted browser id.
                if let Some(reg) = crate::cdp::attach(&b, &id) {
                    self.cdp_registrations.borrow_mut().insert(id, reg);
                }
            }

            #[cfg(debug_assertions)]
            {
                let browser = b.clone();
                glib::timeout_add_local_once(std::time::Duration::from_secs(2), move || {
                    if let Some(frame) = browser.main_frame()
                        && let Some(mut msg) = crate::ipc::BrowserMessage::Ping.to_cef_message()
                    {
                        frame.send_process_message(cef::ProcessId::RENDERER, Some(&mut msg));
                        log::info!("IPC verify: Ping sent to renderer");
                    }
                });
            }

            if make_foreground {
                self.adopt_foreground(account_id, b, life);
            } else if let Some(host) = b.host() {
                host.was_hidden(1); // background: pause until switched in
            }
            self.apply_audio_mute();
        }

        /// Make `browser` the foreground: cache it for input/render, publish its CEF
        /// id for paint gating, register it in the context-menu registry, and show it.
        fn adopt_foreground(
            &self,
            account_id: Option<String>,
            browser: Browser,
            life: ShellLifeSpanHandler,
        ) {
            // A menu belongs to the browser that opened it. Cancel it before an
            // account switch so a delayed Copy activation cannot target the newly
            // foregrounded account's selection.
            if self
                .browser
                .lock()
                .as_ref()
                .is_some_and(|previous| previous.identifier() != browser.identifier())
            {
                self.cancel_context_menu();
            }

            // Pause the OUTGOING foreground. Required so a later switch back wakes
            // it with a real visibility change: `was_hidden(0)` is a no-op on an
            // already-visible browser and emits no paint, which would leave a
            // stale frame on screen (the add-account path reaches here without
            // having paused the previous account — e.g. deleting the foreground
            // account then left its QR frame stuck).
            if let Some(prev) = self.browser.lock().as_ref()
                && prev.identifier() != browser.identifier()
                && let Some(host) = prev.host()
            {
                host.was_hidden(1);
            }

            // Unregister the previous foreground from the context-menu registry.
            let prev_id = self.browser_id.swap(0, Ordering::Relaxed);
            if prev_id != 0 {
                super::unregister_context_menu_widget(prev_id);
            }

            let id = browser.identifier();
            self.browser_id.store(id, Ordering::Relaxed);
            super::register_context_menu_widget(id, &self.obj());
            if let Some(shared) = self.shared.lock().as_ref() {
                shared.lock().foreground_browser_id = id;
            }
            *self.foreground.lock() = account_id;
            *self.life_span.lock() = Some(life);
            *self.browser.lock() = Some(browser.clone());

            if let Some(host) = browser.host() {
                // Respect window visibility: when the window is hidden (e.g.
                // start-in-background) the new foreground must stay paused, or
                // CEF composites an invisible page at full rate (#151).
                if self.window_visible.load(Ordering::Relaxed) {
                    host.was_hidden(0);
                    host.notify_screen_info_changed();
                    host.was_resized();
                    // Re-sync the HiDPI zoom for the newly-visible browser so a
                    // scale change or stale prewarm seed does not leave its
                    // layout/paint offset from input (#158/#176).
                    super::apply_zoom_from_account(&browser, self.display_scale());
                    // Force a fresh paint even if CEF thinks visibility is
                    // unchanged, so the new foreground immediately overwrites any
                    // stale texture.
                    host.invalidate(cef::PaintElementType::VIEW);
                } else {
                    host.was_hidden(1);
                }
            }
            self.obj().queue_render();
            self.apply_audio_mute();
        }

        /// Switch the foreground to the account `new_id`, pausing the previous
        /// foreground and resuming the target. No-op if already foreground or the
        /// target has no spawned browser.
        pub fn switch_to(&self, new_id: &str) {
            if self.foreground.lock().as_deref() == Some(new_id) {
                return;
            }
            if let Some(prev) = self.browser.lock().as_ref()
                && let Some(host) = prev.host()
            {
                host.was_hidden(1);
            }
            let Some(browser) = self.browsers.lock().get(new_id).cloned() else {
                log::warn!("switch_to: no spawned browser for account {new_id}");
                return;
            };
            let life = self.life_spans.lock().get(new_id).cloned();
            let Some(life) = life else {
                log::warn!("switch_to: no life-span handler for account {new_id}");
                return;
            };
            self.adopt_foreground(Some(new_id.to_owned()), browser, life);
        }

        /// Close and drop the browser for `account_id` (the `remove(id)` path).
        pub fn close_account_browser(&self, account_id: &str) {
            let was_foreground = self.foreground.lock().as_deref() == Some(account_id);
            self.life_spans.lock().remove(account_id);
            // Detach the notification bridge (drop = remove observer).
            self.cdp_registrations.borrow_mut().remove(account_id);
            if let Some(browser) = self.browsers.lock().remove(account_id) {
                crate::accounts::unregister_browser(browser.identifier());
                if let Some(host) = browser.host() {
                    host.close_browser(1);
                }
            }
            if was_foreground {
                let id = self.browser_id.swap(0, Ordering::Relaxed);
                if id != 0 {
                    super::unregister_context_menu_widget(id);
                }
                *self.browser.lock() = None;
                *self.life_span.lock() = None;
                *self.foreground.lock() = None;
                if let Some(shared) = self.shared.lock().as_ref() {
                    shared.lock().foreground_browser_id = 0;
                }
            }
        }

        // ---- find-in-page -------------------------------------------------

        pub fn find(&self, text: &str, forward: bool, find_next: bool) {
            let search = CefString::from(text);
            with_host(&self.obj(), |host| {
                host.find(Some(&search), forward as i32, 0, find_next as i32);
            });
        }

        pub fn stop_finding(&self) {
            with_host(&self.obj(), |host| host.stop_finding(1));
        }

        // ---- GL ------------------------------------------------------------

        unsafe fn init_gl(&self) {
            let vsrc = CString::new(VS).unwrap();
            let fsrc = CString::new(FS).unwrap();
            let program = unsafe { compile_program(&vsrc, &fsrc) };
            self.program.store(program, Ordering::Relaxed);

            // fullscreen quad (pos.xy, uv.xy), y-flipped for CEF's BGRA top-left origin.
            let verts: [f32; 16] = [
                -1.0, -1.0, 0.0, 1.0, //
                1.0, -1.0, 1.0, 1.0, //
                -1.0, 1.0, 0.0, 0.0, //
                1.0, 1.0, 1.0, 0.0, //
            ];

            let mut vao = 0;
            let mut vbo = 0;
            unsafe {
                gl::GenVertexArrays(1, &mut vao);
                gl::GenBuffers(1, &mut vbo);
                gl::BindVertexArray(vao);
                gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
                gl::BufferData(
                    gl::ARRAY_BUFFER,
                    (verts.len() * std::mem::size_of::<f32>()) as isize,
                    verts.as_ptr() as *const c_void,
                    gl::STATIC_DRAW,
                );
                let stride = (4 * std::mem::size_of::<f32>()) as i32;
                gl::EnableVertexAttribArray(0);
                gl::VertexAttribPointer(0, 2, gl::FLOAT, gl::FALSE, stride, std::ptr::null());
                gl::EnableVertexAttribArray(1);
                gl::VertexAttribPointer(
                    1,
                    2,
                    gl::FLOAT,
                    gl::FALSE,
                    stride,
                    (2 * std::mem::size_of::<f32>()) as *const c_void,
                );
                gl::BindBuffer(gl::ARRAY_BUFFER, 0);
                gl::BindVertexArray(0);
            }
            self.vao.store(vao, Ordering::Relaxed);
            self.vbo.store(vbo, Ordering::Relaxed);

            let mut tex = 0;
            unsafe {
                gl::GenTextures(1, &mut tex);
                gl::BindTexture(gl::TEXTURE_2D, tex);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
                gl::TexParameteri(
                    gl::TEXTURE_2D,
                    gl::TEXTURE_WRAP_S,
                    gl::CLAMP_TO_EDGE as GLint,
                );
                gl::TexParameteri(
                    gl::TEXTURE_2D,
                    gl::TEXTURE_WRAP_T,
                    gl::CLAMP_TO_EDGE as GLint,
                );
                gl::BindTexture(gl::TEXTURE_2D, 0);
            }
            self.texture.store(tex, Ordering::Relaxed);
            self.tex_w.store(0, Ordering::Relaxed);
            self.tex_h.store(0, Ordering::Relaxed);

            // GPU-OSR: a second texture the imported DMA-BUF EGLImage targets.
            let mut atex = 0;
            unsafe {
                gl::GenTextures(1, &mut atex);
                gl::BindTexture(gl::TEXTURE_2D, atex);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
                gl::TexParameteri(
                    gl::TEXTURE_2D,
                    gl::TEXTURE_WRAP_S,
                    gl::CLAMP_TO_EDGE as GLint,
                );
                gl::TexParameteri(
                    gl::TEXTURE_2D,
                    gl::TEXTURE_WRAP_T,
                    gl::CLAMP_TO_EDGE as GLint,
                );
                gl::BindTexture(gl::TEXTURE_2D, 0);
            }
            self.accel_tex.store(atex, Ordering::Relaxed);
        }

        unsafe fn teardown_gl(&self) {
            // Release the live EGLImage while the GL context is still current.
            *self.imported.borrow_mut() = None;
            unsafe {
                let tex = self.texture.load(Ordering::Relaxed);
                if tex != 0 {
                    gl::DeleteTextures(1, &tex);
                    self.texture.store(0, Ordering::Relaxed);
                }
                let atex = self.accel_tex.load(Ordering::Relaxed);
                if atex != 0 {
                    gl::DeleteTextures(1, &atex);
                    self.accel_tex.store(0, Ordering::Relaxed);
                }
                let vbo = self.vbo.load(Ordering::Relaxed);
                if vbo != 0 {
                    gl::DeleteBuffers(1, &vbo);
                    self.vbo.store(0, Ordering::Relaxed);
                }
                let vao = self.vao.load(Ordering::Relaxed);
                if vao != 0 {
                    gl::DeleteVertexArrays(1, &vao);
                    self.vao.store(0, Ordering::Relaxed);
                }
                let p = self.program.load(Ordering::Relaxed);
                if p != 0 {
                    gl::DeleteProgram(p);
                    self.program.store(0, Ordering::Relaxed);
                }
            }
        }

        pub(super) unsafe fn draw(&self) {
            let shared = match self.shared.lock().as_ref() {
                Some(s) => s.clone(),
                None => return,
            };
            let mut s = shared.lock();

            unsafe {
                gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);

                // GPU path: import the pending DMA-BUF to the accel texture. (gpu-osr)
                let atex = self.accel_tex.load(Ordering::Relaxed);
                let mut accel_failed = false;
                if let Some(af) = s.accel.as_mut()
                    && af.dirty
                {
                    if let Some(img) = crate::gl_dmabuf::import_to_texture(
                        atex,
                        af.width,
                        af.height,
                        af.fourcc,
                        af.modifier,
                        &af.planes,
                    ) {
                        // Keep the EGLImage alive while the texture is sampled;
                        // dropping the previous one releases it.
                        *self.imported.borrow_mut() = Some(img);
                        af.dirty = false;
                    } else {
                        accel_failed = true;
                    }
                }
                if accel_failed {
                    // A rejected DMA-BUF must not pin draw() to a permanently
                    // blank/stale accelerated branch. Drop it so an existing or
                    // subsequent CEF CPU on_paint frame can become visible.
                    log::warn!("accelerated OSR import failed; falling back to CPU paint");
                    super::discard_failed_accel(&mut s.accel);
                    *self.imported.borrow_mut() = None;
                    // CEF's shared-texture choice is fixed at browser creation.
                    // Recreate the pool after this render callback so subsequent
                    // callbacks are CPU on_paint rather than more unusable DMA-BUFs.
                    self.schedule_software_osr_fallback();
                }

                let use_accel = s.accel.is_some() && self.imported.borrow().is_some();
                // J4 instrumentation: capture widget-physical size for draw logging.
                let dbg_widget = self.obj();
                let dbg_scale = paint_scale(&dbg_widget);
                let dbg_phys_w = (dbg_widget.width() as f64 * dbg_scale).round() as i32;
                let dbg_phys_h = (dbg_widget.height() as f64 * dbg_scale).round() as i32;
                if use_accel {
                    let (aw, ah) = s
                        .accel
                        .as_ref()
                        .map(|a| (a.width, a.height))
                        .unwrap_or((0, 0));
                    log::debug!(
                        "coord: J4 draw frame={}x{} tex={}x{} widget_physical={}x{} accel=true scale={:.3}",
                        aw, ah, aw, ah, dbg_phys_w, dbg_phys_h, dbg_scale
                    );
                }
                let (tex, bgra) = if use_accel {
                    (atex, 0_i32)
                } else {
                    // Software path: upload CEF's CPU BGRA buffer through the
                    // valid GLES context (software OSR does not eliminate GLArea).
                    let tex = self.texture.load(Ordering::Relaxed);
                    let tw = self.tex_w.load(Ordering::Relaxed);
                    let th = self.tex_h.load(Ordering::Relaxed);
                    match super::cpu_upload(&s.frame, (tw, th)) {
                        super::CpuUpload::Empty => {
                            log::debug!(
                                "coord: J4 draw empty frame={}x{} tex={}x{} widget_physical={}x{} accel=false scale={:.3}",
                                s.frame.width, s.frame.height, tw, th, dbg_phys_w, dbg_phys_h, dbg_scale
                            );
                            return;
                        },
                        super::CpuUpload::Allocate => {
                            gl::BindTexture(gl::TEXTURE_2D, tex);
                            gl::TexImage2D(
                                gl::TEXTURE_2D,
                                0,
                                gl::RGBA8 as GLint,
                                s.frame.width,
                                s.frame.height,
                                0,
                                gl::RGBA,
                                gl::UNSIGNED_BYTE,
                                s.frame.pixels.as_ptr() as *const c_void,
                            );
                            self.tex_w.store(s.frame.width, Ordering::Relaxed);
                            self.tex_h.store(s.frame.height, Ordering::Relaxed);
                            s.frame.dirty = false;
                        }
                        super::CpuUpload::Update => {
                            gl::BindTexture(gl::TEXTURE_2D, tex);
                            gl::TexSubImage2D(
                                gl::TEXTURE_2D,
                                0,
                                0,
                                0,
                                s.frame.width,
                                s.frame.height,
                                gl::RGBA,
                                gl::UNSIGNED_BYTE,
                                s.frame.pixels.as_ptr() as *const c_void,
                            );
                            s.frame.dirty = false;
                        }
                        super::CpuUpload::Reuse => {}
                    }
                    (tex, 1_i32)
                };
                if !use_accel {
                    let tw2 = self.tex_w.load(Ordering::Relaxed);
                    let th2 = self.tex_h.load(Ordering::Relaxed);
                    log::debug!(
                        "coord: J4 draw frame={}x{} tex={}x{} widget_physical={}x{} accel=false scale={:.3}",
                        s.frame.width, s.frame.height, tw2, th2, dbg_phys_w, dbg_phys_h, dbg_scale
                    );
                }
                drop(s);

                let prog = self.program.load(Ordering::Relaxed);
                gl::UseProgram(prog);
                let loc = gl::GetUniformLocation(prog, c"u_bgra".as_ptr() as *const _);
                if loc >= 0 {
                    gl::Uniform1i(loc, bgra);
                }
                gl::BindVertexArray(self.vao.load(Ordering::Relaxed));
                gl::ActiveTexture(gl::TEXTURE0);
                gl::BindTexture(gl::TEXTURE_2D, tex);
                gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);
                gl::BindVertexArray(0);
                gl::UseProgram(0);
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct ClickHistory {
        button: u32,
        x: f64,
        y: f64,
        time: u32,
        count: i32,
    }

    /// Button state shared by the real GTK legacy-event controller and motion
    /// controller. Unlike GestureClick, raw button releases are not cancelled
    /// when a pointer crosses GTK's drag threshold.
    #[derive(Debug, Default)]
    pub(super) struct MouseButtonTracker {
        pressed: u32,
        press_counts: [i32; 3],
        last_click: Option<ClickHistory>,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub(super) enum MouseInput {
        Press {
            button: u32,
            x: f64,
            y: f64,
            time: u32,
            double_time: u32,
            double_distance: f64,
            modifiers: u32,
        },
        Motion { x: f64, y: f64, modifiers: u32 },
        Release { button: u32, x: f64, y: f64, modifiers: u32 },
        Cancel { x: f64, y: f64 },
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub(super) enum MouseDispatch {
        Click {
            x: f64,
            y: f64,
            button: u32,
            down: bool,
            count: i32,
            modifiers: u32,
        },
        Move { x: f64, y: f64, modifiers: u32 },
    }

    impl MouseButtonTracker {
        pub(super) fn handle(&mut self, input: MouseInput) -> Vec<MouseDispatch> {
            match input {
                MouseInput::Press {
                    button,
                    x,
                    y,
                    time,
                    double_time,
                    double_distance,
                    modifiers,
                } => self
                    .press(button, x, y, time, double_time, double_distance)
                    .map(|count| MouseDispatch::Click {
                        x,
                        y,
                        button,
                        down: true,
                        count,
                        modifiers: modifiers | self.active_modifiers(),
                    })
                    .into_iter()
                    .collect(),
                MouseInput::Motion { x, y, modifiers } => vec![MouseDispatch::Move {
                    x,
                    y,
                    modifiers: modifiers | self.active_modifiers(),
                }],
                MouseInput::Release { button, x, y, modifiers } => self
                    .release(button)
                    .map(|count| MouseDispatch::Click {
                        x,
                        y,
                        button,
                        down: false,
                        count,
                        modifiers,
                    })
                    .into_iter()
                    .collect(),
                MouseInput::Cancel { x, y } => self
                    .cancel_all()
                    .into_iter()
                    .map(|(button, count)| MouseDispatch::Click {
                        x,
                        y,
                        button,
                        down: false,
                        count,
                        modifiers: 0,
                    })
                    .collect(),
            }
        }

        pub(super) fn press(
            &mut self,
            button: u32,
            x: f64,
            y: f64,
            time: u32,
            double_time: u32,
            double_distance: f64,
        ) -> Option<i32> {
            let mask = button_mask(button)?;
            if self.pressed & mask != 0 {
                return None;
            }
            let count = self
                .last_click
                .filter(|last| {
                    last.button == button
                        && time.wrapping_sub(last.time) <= double_time
                        && (x - last.x).abs() <= double_distance
                        && (y - last.y).abs() <= double_distance
                })
                .map_or(1, |last| (last.count % 3) + 1);
            self.pressed |= mask;
            self.press_counts[(button - 1) as usize] = count;
            self.last_click = Some(ClickHistory { button, x, y, time, count });
            Some(count)
        }

        pub(super) fn release(&mut self, button: u32) -> Option<i32> {
            let mask = button_mask(button)?;
            if self.pressed & mask == 0 {
                return None;
            }
            self.pressed &= !mask;
            Some(self.press_counts[(button - 1) as usize].max(1))
        }

        pub(super) fn active_modifiers(&self) -> u32 {
            self.pressed
        }

        pub(super) fn cancel_all(&mut self) -> Vec<(u32, i32)> {
            (1..=3)
                .filter_map(|button| self.release(button).map(|count| (button, count)))
                .collect()
        }
    }

    pub(super) fn should_forward_mouse(pointer_emulated: bool) -> bool {
        !pointer_emulated
    }

    fn button_mask(button: u32) -> Option<u32> {
        use sys::cef_event_flags_t as F;
        match button {
            1 => Some(F::EVENTFLAG_LEFT_MOUSE_BUTTON.0),
            2 => Some(F::EVENTFLAG_MIDDLE_MOUSE_BUTTON.0),
            3 => Some(F::EVENTFLAG_RIGHT_MOUSE_BUTTON.0),
            _ => None,
        }
    }

    fn dispatch_mouse(widget: &super::KarereWebView, event: MouseDispatch) {
        dispatch_mouse_with(
            event,
            |x, y, button, down, count, modifiers| {
                send_click(widget, x, y, button, down, count, modifiers)
            },
            |x, y, modifiers| send_move(widget, x, y, modifiers, false),
        );
    }

    /// Production dispatch seam shared by the installed GTK controllers and the
    /// headless regression harness. Keeping the sink injectable proves that the
    /// adapter calls the click/move forwarding path rather than merely mutating
    /// button state.
    pub(super) fn dispatch_mouse_with(
        event: MouseDispatch,
        mut click: impl FnMut(f64, f64, u32, bool, i32, u32),
        mut motion: impl FnMut(f64, f64, u32),
    ) {
        match event {
            MouseDispatch::Click {
                x,
                y,
                button,
                down,
                count,
                modifiers,
            } => click(x, y, button, down, count, modifiers),
            MouseDispatch::Move { x, y, modifiers } => motion(x, y, modifiers),
        }
    }

    fn release_stuck_mouse_buttons(
        widget: &super::KarereWebView,
        state: &std::rc::Rc<RefCell<MouseButtonTracker>>,
    ) {
        let scale = widget.scale_factor().max(1) as f64;
        let x = widget.imp().last_mouse_x.load(Ordering::Relaxed) as f64 / scale;
        let y = widget.imp().last_mouse_y.load(Ordering::Relaxed) as f64 / scale;
        for event in state.borrow_mut().handle(MouseInput::Cancel { x, y }) {
            dispatch_mouse(widget, event);
        }
    }

    fn install_input_controllers(widget: &super::KarereWebView) {
        use gtk::gdk;

        let im = gtk::IMMulticontext::new();
        im.set_client_widget(Some(widget));
        im.connect_commit(glib::clone!(
            #[weak]
            widget,
            move |_im, text| {
                for ch in text.chars() {
                    send_char(&widget, ch as u16, 0);
                }
            }
        ));
        // Stored so the widget timer can focus it in/out from CEF's editable-focus
        // signal — see the keyboard_request drain.
        widget.imp().im_context.replace(Some(im.clone()));

        // Touch --------------------------------------------------------------
        // Touchscreens (Phosh) need native touch events for scrolling — emulated
        // pointer events don't drive WhatsApp's touch scroll. A touch-only
        // GestureDrag gives clean widget-relative single-finger press/move/release
        // (covers scroll and tap); the mouse controllers below skip the emulated
        // pointer stream so the page isn't double-fed. Pinch is future work. (#162)
        let drag = gtk::GestureDrag::new();
        drag.set_touch_only(true);
        drag.connect_drag_begin(glib::clone!(
            #[weak]
            widget,
            move |_g, x, y| {
                widget.grab_focus();
                set_focus(&widget, true);
                send_touch(&widget, 0, x, y, TouchEventType::PRESSED);
            }
        ));
        drag.connect_drag_update(glib::clone!(
            #[weak]
            widget,
            move |g, ox, oy| {
                if let Some((sx, sy)) = g.start_point() {
                    send_touch(&widget, 0, sx + ox, sy + oy, TouchEventType::MOVED);
                }
            }
        ));
        drag.connect_drag_end(glib::clone!(
            #[weak]
            widget,
            move |g, ox, oy| {
                if let Some((sx, sy)) = g.start_point() {
                    send_touch(&widget, 0, sx + ox, sy + oy, TouchEventType::RELEASED);
                }
            }
        ));
        widget.add_controller(drag);

        // Mouse --------------------------------------------------------------
        // Keep raw button lifecycle separate from GTK gestures. GestureClick's
        // `released` signal is cancelled after a drag threshold crossing, which
        // used to leave CEF logically button-down and made text unselectable.
        let mouse_buttons = std::rc::Rc::new(RefCell::new(MouseButtonTracker::default()));

        let motion = gtk::EventControllerMotion::new();
        motion.connect_motion(glib::clone!(
            #[weak] widget,
            #[strong] mouse_buttons,
            move |ctrl, x, y| {
                if !should_forward_mouse(is_pointer_emulated(ctrl)) {
                    return; // touch handled above; don't double-feed as mouse
                }
                let modifiers = modifiers_from_state(ctrl.current_event_state());
                for event in mouse_buttons
                    .borrow_mut()
                    .handle(MouseInput::Motion { x, y, modifiers })
                {
                    dispatch_mouse(&widget, event);
                }
            }
        ));
        motion.connect_leave(glib::clone!(
            #[weak] widget,
            #[strong] mouse_buttons,
            move |ctrl| {
                let modifiers = modifiers_from_state(ctrl.current_event_state())
                    | mouse_buttons.borrow().active_modifiers();
                send_move(&widget, 0.0, 0.0, modifiers, true);
            }
        ));
        widget.add_controller(motion);

        let buttons = gtk::EventControllerLegacy::new();
        buttons.connect_event(glib::clone!(
            #[weak] widget,
            #[strong] im,
            #[strong] mouse_buttons,
            #[upgrade_or] glib::Propagation::Proceed,
            move |_ctrl, event| {
                use gtk::gdk::{ButtonEvent, EventType};
                if !should_forward_mouse(event.is_pointer_emulated()) {
                    return glib::Propagation::Proceed;
                }
                if event.event_type() == EventType::GrabBroken {
                    release_stuck_mouse_buttons(&widget, &mouse_buttons);
                    return glib::Propagation::Proceed;
                }
                let Some(button_event) = event.downcast_ref::<ButtonEvent>() else {
                    return glib::Propagation::Proceed;
                };
                let button = button_event.button();
                let Some((x, y)) = event.position() else {
                    return glib::Propagation::Proceed;
                };
                let modifiers = modifiers_from_state(event.modifier_state());
                match event.event_type() {
                    EventType::ButtonPress => {
                        let settings = widget.settings();
                        widget.grab_focus();
                        set_focus(&widget, true);
                        im.set_cursor_location(&gtk::gdk::Rectangle::new(
                            x as i32, y as i32, 1, 1,
                        ));
                        let events = mouse_buttons.borrow_mut().handle(MouseInput::Press {
                            button,
                            x,
                            y,
                            time: event.time(),
                            double_time: settings.gtk_double_click_time().max(0) as u32,
                            double_distance: settings.gtk_double_click_distance().max(0) as f64,
                            modifiers,
                        });
                        if !events.is_empty() && button == 2 {
                            read_primary_clipboard_paste(&widget, x, y);
                        }
                        for event in events {
                            dispatch_mouse(&widget, event);
                        }
                    }
                    EventType::ButtonRelease => {
                        for event in mouse_buttons.borrow_mut().handle(MouseInput::Release {
                            button,
                            x,
                            y,
                            modifiers,
                        }) {
                            dispatch_mouse(&widget, event);
                        }
                    }
                    _ => {}
                }
                glib::Propagation::Proceed
            }
        ));
        widget.add_controller(buttons);

        // A cancelled device grab or widget teardown must not strand CEF in a
        // button-down state. Normal releases have already removed their bit.
        widget.connect_unrealize(glib::clone!(
            #[strong] mouse_buttons,
            move |widget| release_stuck_mouse_buttons(widget, &mouse_buttons)
        ));

        // Scroll -------------------------------------------------------------
        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
        scroll.connect_scroll(glib::clone!(
            #[weak]
            widget,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |ctrl, dx, dy| {
                let modifiers = modifiers_from_state(ctrl.current_event_state());
                // Surface unit = pixel deltas (touchpad); Wheel unit = notch
                // clicks (mouse). Scaled differently in send_wheel. (#161)
                let precise = ctrl.unit() == gtk::gdk::ScrollUnit::Surface;
                send_wheel(&widget, dx, dy, modifiers, precise);
                glib::Propagation::Stop
            }
        ));
        widget.add_controller(scroll);

        // Keyboard -----------------------------------------------------------
        // Filter each key through the IM first; if it consumes the key, the
        // `commit` is the only source of text — don't also send a raw key/keyval.
        // Only keys the IM doesn't consume (Enter, arrows, Ctrl-combos) get a raw
        // key event. (#154)
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak]
            widget,
            #[strong]
            im,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |ctrl, keyval, keycode, state| {
                use gtk::gdk::{Key, ModifierType};
                // M17: intercept Ctrl+V before CEF. If GDK clipboard holds an image
                // or files, synthesize the paste and swallow the key so CEF's
                // GDK-blind native paste doesn't also fire. Text-only/empty falls through.
                if state.contains(ModifierType::CONTROL_MASK)
                    && !state.intersects(ModifierType::ALT_MASK | ModifierType::SUPER_MASK)
                    && matches!(keyval, Key::v | Key::V)
                    && try_intercept_paste(&widget)
                {
                    return glib::Propagation::Stop;
                }
                // OSR's native copy has no platform clipboard. Ask the renderer
                // for the current selection now, without racing the 50 ms PRIMARY
                // mirror. The key still forwards so page copy behavior is preserved.
                dispatch_explicit_copy(
                    ExplicitCopyTrigger::Keyboard { keyval, state },
                    || request_live_selection(&widget),
                );
                // A physical keypress means the user is typing — if the page's
                // editable-focus signal left the IM focused-out (post-send input
                // re-render), dead keys silently stop composing. Re-focus first. (#154)
                let imp = widget.imp();
                if !imp.im_focused.get() {
                    im.focus_in();
                    imp.im_focused.set(true);
                }
                // IM gets first crack. A consumed key is text (a letter, or a
                // dead-key composition still buffering) — the `commit` handler
                // emits the CHAR, so we send nothing else and swallow the key.
                let consumed = ctrl
                    .current_event()
                    .map(|e| im.filter_keypress(&e))
                    .unwrap_or(false);
                if consumed {
                    return glib::Propagation::Stop;
                }
                // Not text: deliver the raw key-down so the page sees Enter,
                // arrows, Ctrl-combos, F-keys, etc.
                send_key_raw(&widget, keyval, keycode, state, true);
                // Let accelerator combos (Ctrl/Alt/Super+key, F5/F11) bubble to
                // window/app shortcuts; consume the rest so typing stays in the webview.
                if is_accelerator_key(keyval, state) {
                    glib::Propagation::Proceed
                } else {
                    glib::Propagation::Stop
                }
            }
        ));
        keys.connect_key_released(glib::clone!(
            #[weak]
            widget,
            #[strong]
            im,
            move |ctrl, keyval, keycode, state| {
                // Mirror the press path: if the IM consumes the release, it
                // belongs to composition — don't also send a raw key-up.
                let consumed = ctrl
                    .current_event()
                    .map(|e| im.filter_keypress(&e))
                    .unwrap_or(false);
                if !consumed {
                    send_key_raw(&widget, keyval, keycode, state, false);
                }
            }
        ));
        widget.add_controller(keys);

        // Focus --------------------------------------------------------------
        let focus = gtk::EventControllerFocus::new();
        focus.connect_enter(glib::clone!(
            #[weak]
            widget,
            move |_| {
                // Note: no im.focus_in() here. The GLArea holds GTK focus almost
                // always, so focusing the IM on widget-focus kept Phosh's on-screen
                // keyboard up permanently. IM focus-in is driven by the page's
                // editable-focus signal instead (keyboard_request). focus_out stays
                // on leave so the keyboard hides when leaving the webview entirely.
                set_focus(&widget, true);
            }
        ));
        focus.connect_leave(glib::clone!(
            #[weak]
            widget,
            #[strong]
            im,
            move |_| {
                im.focus_out();
                widget.imp().im_focused.set(false);
                set_focus(&widget, false);
            }
        ));
        widget.add_controller(focus);

        // Drag-drop ----------------------------------------------------------
        // M17: accept file drops; surface each as a synthetic `drop` on the element
        // under the cursor (paste_bridge.js targets it via the envelope's coords).
        let drop_target = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        // Forward hover (enter/motion/leave) so the page's dropzone overlay mounts
        // DURING the hover — CEF only delivers the drop on release, too late to mount.
        drop_target.connect_enter(glib::clone!(
            #[weak]
            widget,
            #[upgrade_or]
            gdk::DragAction::COPY,
            move |_t, x, y| {
                send_drag_hover(&widget, "enter", x, y);
                gdk::DragAction::COPY
            }
        ));
        let last_motion = std::cell::Cell::new(std::time::Instant::now());
        drop_target.connect_motion(glib::clone!(
            #[weak]
            widget,
            #[upgrade_or]
            gdk::DragAction::COPY,
            move |_t, x, y| {
                // Throttle to ~10/s: keeps the dropzone alive without flooding IPC.
                let now = std::time::Instant::now();
                if now.duration_since(last_motion.get()) >= std::time::Duration::from_millis(100) {
                    last_motion.set(now);
                    send_drag_hover(&widget, "over", x, y);
                }
                gdk::DragAction::COPY
            }
        ));
        drop_target.connect_leave(glib::clone!(
            #[weak]
            widget,
            move |_t| send_drag_hover(&widget, "leave", 0.0, 0.0)
        ));
        drop_target.connect_drop(glib::clone!(
            #[weak]
            widget,
            #[upgrade_or]
            false,
            move |_target, value, x, y| {
                match value.get::<gdk::FileList>() {
                    Ok(list) => {
                        for file in list.files() {
                            load_and_send_file(&widget, file, "drop", Some((x, y)));
                        }
                        true
                    }
                    Err(_) => false,
                }
            }
        ));
        widget.add_controller(drop_target);

        let _ = gdk::ModifierType::SHIFT_MASK; // suppress unused-import warning
    }

    pub(super) fn is_copy_shortcut(
        keyval: gtk::gdk::Key,
        state: gtk::gdk::ModifierType,
    ) -> bool {
        use gtk::gdk::{Key, ModifierType};
        state.contains(ModifierType::CONTROL_MASK)
            && !state.intersects(ModifierType::ALT_MASK | ModifierType::SUPER_MASK)
            && matches!(keyval, Key::c | Key::C)
    }

    #[derive(Debug, Clone, Copy)]
    pub(super) enum ExplicitCopyTrigger {
        Keyboard {
            keyval: gtk::gdk::Key,
            state: gtk::gdk::ModifierType,
        },
        ContextMenu(i32),
    }

    /// Shared production trigger used by both the installed key controller and
    /// the CEF context-menu callback. The request sink is injectable so tests
    /// cover command dispatch, not just shortcut/command classification.
    pub(super) fn dispatch_explicit_copy(
        trigger: ExplicitCopyTrigger,
        request: impl FnOnce(),
    ) {
        let requested = match trigger {
            ExplicitCopyTrigger::Keyboard { keyval, state } => is_copy_shortcut(keyval, state),
            ExplicitCopyTrigger::ContextMenu(command_id) => {
                crate::handlers::context_menu::is_copy_command(command_id)
            }
        };
        if requested {
            request();
        }
    }

    fn is_accelerator_key(keyval: gtk::gdk::Key, state: gtk::gdk::ModifierType) -> bool {
        use gtk::gdk::{Key, ModifierType};
        if state.intersects(
            ModifierType::CONTROL_MASK | ModifierType::ALT_MASK | ModifierType::SUPER_MASK,
        ) {
            return true;
        }
        matches!(keyval, Key::F5 | Key::F11)
    }

    /// True when the controller's current event is a pointer event GTK
    /// synthesised from a touch sequence. Those are handled by the touch drag
    /// gesture, so the mouse controllers skip them to avoid double-feeding the
    /// page. (#162)
    fn is_pointer_emulated(ctrl: &impl IsA<gtk::EventController>) -> bool {
        ctrl.current_event()
            .map(|e| e.is_pointer_emulated())
            .unwrap_or(false)
    }

    fn modifiers_from_state(state: gtk::gdk::ModifierType) -> u32 {
        use gtk::gdk::ModifierType;
        use sys::cef_event_flags_t as F;
        let mut m = 0u32;
        if state.contains(ModifierType::SHIFT_MASK) {
            m |= F::EVENTFLAG_SHIFT_DOWN.0;
        }
        if state.contains(ModifierType::CONTROL_MASK) {
            m |= F::EVENTFLAG_CONTROL_DOWN.0;
        }
        if state.contains(ModifierType::ALT_MASK) {
            m |= F::EVENTFLAG_ALT_DOWN.0;
        }
        if state.contains(ModifierType::SUPER_MASK) {
            m |= F::EVENTFLAG_COMMAND_DOWN.0;
        }
        if state.contains(ModifierType::BUTTON1_MASK) {
            m |= F::EVENTFLAG_LEFT_MOUSE_BUTTON.0;
        }
        if state.contains(ModifierType::BUTTON2_MASK) {
            m |= F::EVENTFLAG_MIDDLE_MOUSE_BUTTON.0;
        }
        if state.contains(ModifierType::BUTTON3_MASK) {
            m |= F::EVENTFLAG_RIGHT_MOUSE_BUTTON.0;
        }
        m
    }

    /// Populate `container` per [`MenuEntry`]: flat `Button`s for commands (click →
    /// record id + popdown), `Separator`s, dim heading labels for (flattened)
    /// submenus. Disabled items are insensitive.
    fn build_menu_box(
        entries: &[crate::handlers::context_menu::MenuEntry],
        container: &gtk::Box,
        obj: &super::KarereWebView,
        popover: &gtk::Popover,
    ) {
        use crate::handlers::context_menu::MenuEntry;
        use gtk::prelude::*;

        for e in entries {
            match e {
                MenuEntry::Separator => {
                    container.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
                }
                MenuEntry::Submenu { label, items } => {
                    let heading = gtk::Label::new(Some(label));
                    heading.set_xalign(0.0);
                    heading.add_css_class("dim-label");
                    container.append(&heading);
                    build_menu_box(items, container, obj, popover);
                }
                MenuEntry::Item {
                    label,
                    command_id,
                    enabled,
                } => {
                    let btn = gtk::Button::with_label(label);
                    btn.set_has_frame(false);
                    btn.add_css_class("flat");
                    btn.set_sensitive(*enabled);
                    if let Some(lbl) = btn.child().and_downcast::<gtk::Label>() {
                        lbl.set_xalign(0.0);
                    }
                    let cmd = *command_id;
                    btn.connect_clicked(glib::clone!(
                        #[weak]
                        obj,
                        #[weak]
                        popover,
                        move |_| {
                            log::debug!("context menu: item activated id={cmd}");
                            // Record the choice; `closed` runs `cont` after popdown + refocus.
                            obj.imp().pending_menu_command.set(Some(cmd));
                            popover.popdown();
                        }
                    ));
                    container.append(&btn);
                }
            }
        }
    }

    // Bridges a per-account `RequestContext`'s init callback to the widget so the
    // browser is created only once the context is ready.
    wrap_request_context_handler! {
        pub struct ContextReadyHandler {
            widget: glib::object::WeakRef<super::KarereWebView>,
            account_id: String,
            make_foreground: bool,
        }

        impl RequestContextHandler {
            fn on_request_context_initialized(
                &self,
                request_context: Option<&mut RequestContext>,
            ) {
                if let Some(widget) = self.widget.upgrade() {
                    widget.imp().on_account_context_ready(
                        request_context,
                        &self.account_id,
                        self.make_foreground,
                    );
                }
            }
        }
    }

    fn resolved_browser(imp: &KarereWebView) -> Option<Browser> {
        if let Some(b) = imp.browser.lock().as_ref() {
            return Some(b.clone());
        }
        if let Some(life) = imp.life_span.lock().as_ref() {
            return life.state.lock().browser.clone();
        }
        None
    }

    pub(super) fn with_resolved<T>(resolve: impl FnOnce() -> Option<T>, send: impl FnOnce(T)) {
        if let Some(target) = resolve() {
            send(target);
        }
    }

    fn with_host<F: FnOnce(&cef::BrowserHost)>(widget: &super::KarereWebView, f: F) {
        with_resolved(
            || resolved_browser(widget.imp()).and_then(|browser| browser.host()),
            |host| f(&host),
        );
    }

    /// Scale (DIP→physical) the OSR paint buffer must use so it maps 1:1 onto the
    /// `GtkGLArea` framebuffer. `GtkGLArea` always allocates that framebuffer at the
    /// *integer* `gtk_widget_get_scale_factor()` (`width * scale`, `height * scale`)
    /// — it cannot render at a fractional scale — so CEF MUST paint at the same
    /// integer scale. Painting at the fractional surface scale (e.g. 1.5) leaves the
    /// texture smaller than the framebuffer, and the fullscreen-quad blit upscales it
    /// (GL_LINEAR), which is what blurred the web view under fractional scaling (#158).
    ///
    /// At 150 % this paints at 2× into the 2× framebuffer (crisp), and the compositor
    /// downscales the result to the 1.5× surface — a single high-quality downscale,
    /// the same path every GTK GLArea takes on a fractional display. View rect and
    /// mouse/wheel coords stay in DIP (CEF maps them via device_scale_factor). (#155, #158)
    /// GPU-accelerated OSR opt-in: env `KARERE_GPU_OSR=1` (or its setting) AND
    /// EGL dma-buf import must be available on the current GL context. Cached on
    /// first call; a prewarm before realization safely resolves to CPU OSR.
    /// (gpu-osr)
    pub(super) fn accel_osr_enabled() -> bool {
        use std::sync::OnceLock;
        static EN: OnceLock<bool> = OnceLock::new();
        *EN.get_or_init(|| {
            // Read once — the shared-texture flag is fixed for the browser's
            // lifetime, so this is restart-required. Env wins as a dev override /
            // kill-switch; otherwise the experimental `gpu-rendering` GSetting.
            // The visible-start path calls this after init_gl with the context
            // current. Background prewarm has no context, so capability probing
            // deliberately disables shared textures for that process. (gpu-osr)
            let requested = match std::env::var("KARERE_GPU_OSR").ok().as_deref() {
                Some("1") | Some("true") => true,
                Some("0") | Some("false") => false,
                _ => {
                    use gtk::prelude::SettingsExt;
                    let want = gtk::gio::Settings::new(crate::application::APP_ID)
                        .boolean("gpu-rendering");
                    // NVIDIA can't back CEF's exportable shared images — the GPU
                    // process fails SkSurface init and no accelerated frame ever
                    // arrives, leaving the chat area black (#167). Probe the GL
                    // context actually rendering (current here: realize →
                    // init_gl → browser creation) instead of the PCI driver
                    // list, so hybrid setups rendering on another GPU keep the
                    // accel path (#173). KARERE_GPU_OSR=1 forces.
                    if want && gl_vendor_is_nvidia() {
                        log::warn!(
                            "accel_osr: NVIDIA driver detected — GPU rendering is \
                             unsupported (CEF shared-image export fails, #167); \
                             using software rendering. KARERE_GPU_OSR=1 to force."
                        );
                        false
                    } else {
                        want
                    }
                }
            };
            // Do not ask CEF for shared textures unless this current GL/EGL
            // display can import them. Without this pre-browser fence CEF may
            // never call CPU on_paint, leaving a blank first frame.
            let enabled = requested && crate::gl_dmabuf::is_supported();
            log::info!("accel_osr: requested={requested} enabled={enabled}");
            enabled
        })
    }

    /// True when the current GL context is driven by the NVIDIA proprietary
    /// driver (GL_VENDOR contains "NVIDIA"; Mesa reports "Mesa"/"AMD" even on
    /// NVIDIA hardware via nouveau/NVK, where shared images are untested but
    /// not known-broken). Requires a current GL context — returns false when
    /// there is none. (gpu-osr)
    fn gl_vendor_is_nvidia() -> bool {
        let p = unsafe { gl::GetString(gl::VENDOR) };
        if p.is_null() {
            // No current GL context — prewarm (start-in-background) creates
            // browsers before the GLArea realizes. Fall back to the coarse
            // driver-bound check so NVIDIA still gets software rendering
            // instead of the broken accel path (#167); hybrid setups rendering
            // on the iGPU lose accel only in this prewarmed case.
            return std::path::Path::new("/sys/bus/pci/drivers/nvidia").exists();
        }
        let vendor = unsafe { std::ffi::CStr::from_ptr(p.cast()) }.to_string_lossy();
        log::info!("accel_osr: GL_VENDOR={vendor}");
        vendor.contains("NVIDIA")
    }

    fn paint_scale(widget: &super::KarereWebView) -> f64 {
        widget.scale_factor().max(1) as f64
    }

    /// Widget screen origin in physical pixels for `RenderHandler::screen_point`.
    ///
    /// - On Wayland returns `(0,0)` — global position is compositor-private and
    ///   must stay `(0,0)` so `screen==view` (correct Wayland fallback).
    /// - On X11 downcasts the native `GdkSurface` to `gdk4_x11::X11Surface` and
    ///   queries the root-relative position via `XTranslateCoordinates` (xlib).
    ///   `XTranslateCoordinates` returns X-server **physical** pixels already
    ///   (GDK creates X11 windows at physical size under `GDK_SCALE=2`,
    ///   `scale_factor()==2`), so the result is passed through clamped without
    ///   re-scaling — do NOT multiply by `widget.scale_factor()` (would double-
    ///   count and break `GetScreenPoint`/popup anchoring at 2×). (KARE-019)
    /// - Before realize or after unrealize (`native` or `surface` is `None`)
    ///   returns `(0,0)` without panicking; any query failure also falls back
    ///   to `(0,0)` at `debug` level. Main-thread only (callers are
    ///   `size_allocate` / `refresh_screen_scale`).
    ///
    /// Verified: no root `GdkSurface` exists in gdk4 0.11 to translate against,
    /// so the `translate_coordinates`/`compute_bounds` logical path would always
    /// return `false` — the shipped path is the xlib physical path.
    fn window_origin_for(widget: &super::KarereWebView) -> (i32, i32) {
        // Post-unrealize guard: no native/surface → (0,0), never panic.
        let Some(native) = widget.native() else {
            return (0, 0);
        };
        let Some(surface) = native.surface() else {
            return (0, 0);
        };
        // Try X11 downcast; Wayland/Broadway/missing X11 backend → (0,0)
        // Wayland fallback must stay (0,0) by design, not a partial value.
        let x11_surface = match surface.downcast_ref::<gdk4_x11::X11Surface>() {
            Some(s) => s,
            None => return (0, 0),
        };
        // X11 path — XTranslateCoordinates returns physical pixels — do not scale; already in screen physical space.
        let display = surface.display();
        let Some(x11_display) = display.downcast_ref::<gdk4_x11::X11Display>() else {
            log::debug!("window_origin_for: display is not X11");
            return (0, 0);
        };
        let xid = x11_surface.xid();
        // SAFETY: xdisplay is valid on the GTK main thread while the widget
        // is realized; we guard null and the Xlib call is main-thread-only.
        let xdisplay = unsafe { x11_display.xdisplay() };
        if xdisplay.is_null() {
            log::debug!("window_origin_for: xdisplay is null");
            return (0, 0);
        }
        let root = x11_display.xrootwindow();
        let mut rx: ::std::os::raw::c_int = 0;
        let mut ry: ::std::os::raw::c_int = 0;
        let mut child: ::std::os::raw::c_ulong = 0;
        // x11-dl loads libX11 dynamically; opening can fail without libX11.
        let xlib = match x11::xlib::Xlib::open() {
            Ok(lib) => lib,
            Err(_) => {
                log::debug!("window_origin_for: Xlib::open failed");
                return (0, 0);
            }
        };
        let ok = unsafe {
            (xlib.XTranslateCoordinates)(xdisplay as *mut x11::xlib::Display, xid, root, 0, 0, &mut rx, &mut ry, &mut child)
        };
        if ok == 0 {
            log::debug!("window_origin_for: XTranslateCoordinates failed");
            return (0, 0);
        }
        // XTranslateCoordinates returns physical pixels — do not scale; already in screen physical space.
        (rx, ry)
    }

    pub(super) fn physical_mouse_coordinates(x: f64, y: f64, scale: i32) -> (i32, i32) {
        let s = scale.max(1) as f64;
        ((x * s).round() as i32, (y * s).round() as i32)
    }

    /// CEF mouse-host operations. Production implements this on BrowserHost;
    /// tests inject an in-memory host only at this final API boundary.
    pub(super) trait MouseEventSink {
        fn send_mouse_move(&mut self, event: &MouseEvent, leave: bool);
        fn send_mouse_click(
            &mut self,
            event: &MouseEvent,
            button: MouseButtonType,
            mouse_up: bool,
            click_count: i32,
        );
    }

    impl MouseEventSink for cef::BrowserHost {
        fn send_mouse_move(&mut self, event: &MouseEvent, leave: bool) {
            self.send_mouse_move_event(Some(event), leave as i32);
        }

        fn send_mouse_click(
            &mut self,
            event: &MouseEvent,
            button: MouseButtonType,
            mouse_up: bool,
            click_count: i32,
        ) {
            self.send_mouse_click_event(Some(event), button, mouse_up as i32, click_count);
        }
    }

    pub(super) fn send_move_with<S: MouseEventSink>(
        x: f64,
        y: f64,
        scale: i32,
        modifiers: u32,
        leave: bool,
        resolve: impl FnOnce() -> Option<S>,
        mut update_position: impl FnMut(i32, i32),
    ) {
        // CEF view coords are physical pixels (view_rect is physical, #158).
        let (px, py) = physical_mouse_coordinates(x, y, scale);
        log::debug!(
            "coord: J1/J6 move logical={:.1},{:.1} physical={},{} scale={} leave={}",
            x, y, px, py, scale, leave
        );
        if !leave {
            update_position(px, py);
        }
        let event = MouseEvent {
            x: px,
            y: py,
            modifiers,
        };
        with_resolved(resolve, |mut sink| sink.send_mouse_move(&event, leave));
    }

    fn send_move(widget: &super::KarereWebView, x: f64, y: f64, modifiers: u32, leave: bool) {
        let imp = widget.imp();
        send_move_with(
            x,
            y,
            widget.scale_factor(),
            modifiers,
            leave,
            || resolved_browser(imp).and_then(|browser| browser.host()),
            |px, py| {
                imp.last_mouse_x.store(px, Ordering::Relaxed);
                imp.last_mouse_y.store(py, Ordering::Relaxed);
            },
        );
    }

    pub(super) fn send_click_with<S: MouseEventSink>(
        position: (f64, f64),
        scale: i32,
        button: (u32, bool, i32),
        modifiers: u32,
        resolve: impl FnOnce() -> Option<S>,
    ) {
        let (x, y) = position;
        let (button, down, n_press) = button;
        let btn = match button {
            1 => MouseButtonType::LEFT,
            2 => MouseButtonType::MIDDLE,
            3 => MouseButtonType::RIGHT,
            _ => return,
        };
        let (px, py) = physical_mouse_coordinates(x, y, scale);
        log::debug!(
            "coord: J1/J6 click logical={:.1},{:.1} physical={},{} scale={} button={} down={} count={}",
            x, y, px, py, scale, button, down, n_press
        );
        let (x, y) = (px, py);
        let event = MouseEvent { x, y, modifiers };
        with_resolved(resolve, |mut sink| {
            sink.send_mouse_click(&event, btn, !down, n_press.max(1));
        });
    }

    fn send_click(
        widget: &super::KarereWebView,
        x: f64,
        y: f64,
        button: u32,
        down: bool,
        n_press: i32,
        modifiers: u32,
    ) {
        send_click_with(
            (x, y),
            widget.scale_factor(),
            (button, down, n_press),
            modifiers,
            || resolved_browser(widget.imp()).and_then(|browser| browser.host()),
        );
    }

    /// Live pointer position in physical widget coords, for wheel hit-testing —
    /// touchpad scrolls arrive without motion events, so cached coords go stale. (#161)
    fn pointer_pos_physical(widget: &super::KarereWebView) -> Option<(i32, i32)> {
        let pointer = widget.display().default_seat()?.pointer()?;
        let native = widget.native()?;
        let (sx, sy, _) = native.surface()?.device_position(&pointer)?;
        // Surface → widget coords: strip the native's window-decoration offset,
        // then map through the widget tree.
        let (tx, ty) = native.surface_transform();
        let p = native.compute_point(
            widget,
            &gtk::graphene::Point::new((sx - tx) as f32, (sy - ty) as f32),
        )?;
        let s = widget.scale_factor().max(1) as f64;
        Some(((p.x() as f64 * s).round() as i32, (p.y() as f64 * s).round() as i32))
    }

    fn send_wheel(widget: &super::KarereWebView, dx: f64, dy: f64, modifiers: u32, precise: bool) {
        // CEF wants pixel deltas. A mouse wheel reports notch clicks (±1) → use a
        // browser-like notch size. A touchpad (Surface unit) reports surface-unit
        // deltas; Chromium's own Wayland backend scales those by
        // kWheelDelta(53) / kAxisValueScale(10) = 5.3 (wayland_pointer.cc), so
        // match it — 1:1 passthrough was ~5x slower than every browser. (#161)
        let step = if precise { 5.3 } else { 100.0 };
        let s = widget.scale_factor().max(1) as f64;
        let imp = widget.imp();

        // Accumulate sub-pixel deltas so fine touchpad motion isn't truncated to
        // zero (the old `as i32` dropped small deltas → jumpy/slow scroll). Keep
        // the fractional remainder for the next event. (#161)
        let ax = imp.scroll_accum_x.get() + (-dx * step * s);
        let ay = imp.scroll_accum_y.get() + (-dy * step * s);
        let ix = ax.trunc();
        let iy = ay.trunc();
        imp.scroll_accum_x.set(ax - ix);
        imp.scroll_accum_y.set(ay - iy);
        if ix == 0.0 && iy == 0.0 {
            return; // no whole pixel yet — keep accumulating
        }

        // CEF hit-tests the cursor to pick the scroll target; (0,0) scrolls nothing.
        // Touchpad scrolling doesn't move the pointer, so the cached motion coords
        // can be stale (pointer entered without motion — window presented under
        // the cursor, workspace switch): query the live position, fall back to
        // last_mouse_* (already physical). (#161)
        let (hx, hy) = pointer_pos_physical(widget).unwrap_or_else(|| {
            (
                imp.last_mouse_x.load(Ordering::Relaxed),
                imp.last_mouse_y.load(Ordering::Relaxed),
            )
        });
        let event = MouseEvent {
            x: hx,
            y: hy,
            modifiers,
        };
        with_host(widget, |host| {
            host.send_mouse_wheel_event(Some(&event), ix as i32, iy as i32);
        });
    }

    /// Forward a touch point to CEF so the page gets native touchstart/move/end
    /// — required for touch scrolling on touchscreens (Phosh), which emulated
    /// pointer events don't drive. Coords are physical (× scale), like the mouse
    /// path. (#162)
    fn send_touch(widget: &super::KarereWebView, id: i32, x: f64, y: f64, type_: TouchEventType) {
        let s = widget.scale_factor().max(1) as f32;
        let event = TouchEvent {
            id,
            x: x as f32 * s,
            y: y as f32 * s,
            radius_x: 0.0,
            radius_y: 0.0,
            rotation_angle: 0.0,
            pressure: 1.0,
            type_,
            modifiers: 0,
            pointer_type: PointerType::TOUCH,
        };
        with_host(widget, |host| host.send_touch_event(Some(&event)));
    }

    /// Send only the raw key-down/up (RAWKEYDOWN / KEYUP) — no CHAR. Character
    /// insertion is driven separately by the IM `commit` (`send_char`), so dead
    /// keys compose and there's no double-insert on the first key. (#154)
    fn send_key_raw(
        widget: &super::KarereWebView,
        keyval: gtk::gdk::Key,
        keycode: u32,
        state: gtk::gdk::ModifierType,
        down: bool,
    ) {
        let evt = KeyEvent {
            size: std::mem::size_of::<sys::_cef_key_event_t>(),
            type_: if down {
                KeyEventType::RAWKEYDOWN
            } else {
                KeyEventType::KEYUP
            },
            modifiers: modifiers_from_state(state),
            windows_key_code: gdk_key_to_vk(keyval),
            native_key_code: keycode as i32,
            is_system_key: 0,
            character: 0,
            unmodified_character: 0,
            focus_on_editable_field: 0,
        };
        with_host(widget, |host| {
            host.send_key_event(Some(&evt));
        });
    }

    /// Send a single composed UTF-16 unit as a CEF CHAR event. Used for IM
    /// `commit` text (dead-key / IME composition). (#154)
    fn send_char(widget: &super::KarereWebView, code: u16, modifiers: u32) {
        if code == 0 {
            return;
        }
        let evt = KeyEvent {
            size: std::mem::size_of::<sys::_cef_key_event_t>(),
            type_: KeyEventType::CHAR,
            modifiers,
            windows_key_code: code as i32,
            native_key_code: 0,
            is_system_key: 0,
            character: code,
            unmodified_character: code,
            focus_on_editable_field: 0,
        };
        with_host(widget, |host| {
            host.send_key_event(Some(&evt));
        });
    }

    fn set_focus(widget: &super::KarereWebView, focused: bool) {
        with_host(widget, |host| host.set_focus(focused as i32));
    }

    /// Synthesize Ctrl+Shift+C to toggle the embedded DevTools "inspect element" picker.
    pub fn send_inspect_shortcut(widget: &super::KarereWebView) {
        use sys::cef_event_flags_t as F;
        let modifiers = F::EVENTFLAG_CONTROL_DOWN.0 | F::EVENTFLAG_SHIFT_DOWN.0;
        let down = KeyEvent {
            size: std::mem::size_of::<sys::_cef_key_event_t>(),
            type_: KeyEventType::RAWKEYDOWN,
            modifiers,
            windows_key_code: 0x43, // VK_C
            native_key_code: 0,
            is_system_key: 0,
            character: 0,
            unmodified_character: 0,
            focus_on_editable_field: 0,
        };
        let up = KeyEvent {
            type_: KeyEventType::KEYUP,
            ..down.clone()
        };
        with_host(widget, |host| {
            host.set_focus(1);
            host.send_key_event(Some(&down));
            host.send_key_event(Some(&up));
        });
    }

    // ---- M17 paste / drop bridge ---------------------------------------

    /// Send a [`crate::ipc::BrowserMessage`] to the renderer's main frame.
    fn send_browser_message(widget: &super::KarereWebView, msg: crate::ipc::BrowserMessage) {
        if let Some(browser) = resolved_browser(widget.imp())
            && let Some(frame) = browser.main_frame()
            && let Some(mut m) = msg.to_cef_message()
        {
            frame.send_process_message(cef::ProcessId::RENDERER, Some(&mut m));
        }
    }

    /// Marshal a binary clipboard/drop payload into a `DispatchPasteEvent` (`kind` =
    /// `"paste"` or `"drop"`). Large payloads round-trip via a tempfile (see
    /// [`crate::paste::make_blob`]).
    fn send_blob_paste(
        widget: &super::KarereWebView,
        mime: String,
        kind: &str,
        bytes: &[u8],
        name: Option<String>,
        coords: Option<(f64, f64)>,
    ) {
        let payload = crate::paste::make_blob(bytes);
        let (x, y) = match coords {
            Some((x, y)) => (Some(x), Some(y)),
            None => (None, None),
        };
        send_browser_message(
            widget,
            crate::ipc::BrowserMessage::DispatchPasteEvent {
                mime,
                kind: kind.to_string(),
                payload,
                name,
                x,
                y,
            },
        );
    }

    /// Notify the page that a file drag is hovering so it can pre-mount its
    /// dropzone before the drop commits.
    fn send_drag_hover(widget: &super::KarereWebView, phase: &str, x: f64, y: f64) {
        send_browser_message(
            widget,
            crate::ipc::BrowserMessage::DragHover {
                phase: phase.to_string(),
                x,
                y,
            },
        );
    }

    /// Marshal primary-clipboard text (middle-click) as a `text/plain` paste with
    /// click coords so the page targets the element under the cursor (editable only).
    fn send_text_paste(widget: &super::KarereWebView, text: &str, coords: Option<(f64, f64)>) {
        let (x, y) = match coords {
            Some((x, y)) => (Some(x), Some(y)),
            None => (None, None),
        };
        send_browser_message(
            widget,
            crate::ipc::BrowserMessage::DispatchPasteEvent {
                mime: "text/plain".to_string(),
                kind: "paste".to_string(),
                payload: crate::ipc::PasteBlob::Base64(crate::paste::b64(text.as_bytes())),
                name: None,
                x,
                y,
            },
        );
    }

    /// On Ctrl+V, inspect the GDK clipboard. If it holds an image or files, async-read
    /// and dispatch a synthetic paste, returning `true` so the caller swallows the key
    /// (CEF's GDK-blind native paste must not also fire). Text-only/empty → `false`.
    fn try_intercept_paste(widget: &super::KarereWebView) -> bool {
        let Some(display) = gtk::gdk::Display::default() else {
            return false;
        };
        let clipboard = display.clipboard();
        let formats = clipboard.formats();

        let has_image = formats.contains_type(gtk::gdk::Texture::static_type())
            || formats.contain_mime_type("image/png")
            || formats.contain_mime_type("image/jpeg")
            || formats.contain_mime_type("image/gif")
            || formats.contain_mime_type("image/webp");
        if has_image {
            read_clipboard_image(widget, &clipboard);
            return true;
        }

        let has_files = formats.contains_type(gtk::gdk::FileList::static_type())
            || formats.contain_mime_type("text/uri-list");
        if has_files {
            read_clipboard_files(widget, &clipboard);
            return true;
        }

        // CEF's windowless clipboard doesn't consult GDK, so native Ctrl+V pastes
        // nothing. Read GDK text ourselves and synthesize, like the image/file paths.
        let has_text = formats.contains_type(glib::Type::STRING)
            || formats.contain_mime_type("text/plain")
            || formats.contain_mime_type("text/plain;charset=utf-8")
            || formats.contain_mime_type("UTF8_STRING");
        if has_text {
            read_clipboard_text(widget, &clipboard);
            return true;
        }

        false
    }

    fn read_clipboard_text(widget: &super::KarereWebView, clipboard: &gtk::gdk::Clipboard) {
        clipboard.read_text_async(
            gtk::gio::Cancellable::NONE,
            glib::clone!(
                #[weak]
                widget,
                move |res| match res {
                    Ok(Some(text)) if !text.is_empty() => {
                        send_text_paste(&widget, text.as_str(), None)
                    }
                    Ok(_) => {}
                    Err(err) => log::warn!("clipboard text read failed: {err}"),
                }
            ),
        );
    }

    fn read_clipboard_image(widget: &super::KarereWebView, clipboard: &gtk::gdk::Clipboard) {
        clipboard.read_texture_async(
            gtk::gio::Cancellable::NONE,
            glib::clone!(
                #[weak]
                widget,
                move |res| match res {
                    Ok(Some(texture)) => {
                        let bytes = texture.save_to_png_bytes();
                        send_blob_paste(
                            &widget,
                            "image/png".to_string(),
                            "paste",
                            &bytes,
                            None,
                            None,
                        );
                    }
                    Ok(None) => log::debug!("clipboard image read: empty"),
                    Err(err) => log::warn!("clipboard image read failed: {err}"),
                }
            ),
        );
    }

    fn read_clipboard_files(widget: &super::KarereWebView, clipboard: &gtk::gdk::Clipboard) {
        clipboard.read_value_async(
            gtk::gdk::FileList::static_type(),
            glib::Priority::DEFAULT,
            gtk::gio::Cancellable::NONE,
            glib::clone!(
                #[weak]
                widget,
                move |res| match res {
                    Ok(value) => match value.get::<gtk::gdk::FileList>() {
                        Ok(list) => {
                            for file in list.files() {
                                load_and_send_file(&widget, file, "paste", None);
                            }
                        }
                        Err(err) => log::warn!("clipboard file list decode failed: {err}"),
                    },
                    Err(err) => log::warn!("clipboard file read failed: {err}"),
                }
            ),
        );
    }

    /// Load one file's contents asynchronously and dispatch it as a paste/drop.
    fn load_and_send_file(
        widget: &super::KarereWebView,
        file: gtk::gio::File,
        kind: &'static str,
        coords: Option<(f64, f64)>,
    ) {
        let name = file.basename().map(|p| p.to_string_lossy().into_owned());
        file.load_contents_async(
            gtk::gio::Cancellable::NONE,
            glib::clone!(
                #[weak]
                widget,
                move |res| match res {
                    Ok((bytes, _etag)) => {
                        let mime = guess_mime(name.as_deref(), &bytes);
                        send_blob_paste(&widget, mime, kind, &bytes, name, coords);
                    }
                    Err(err) => log::warn!("read dropped/pasted file failed: {err}"),
                }
            ),
        );
    }

    /// Best-effort MIME from filename + leading bytes; defaults to octet-stream.
    fn guess_mime(name: Option<&str>, bytes: &[u8]) -> String {
        let (content_type, _certain) = gtk::gio::content_type_guess(name, bytes);
        gtk::gio::content_type_get_mime_type(content_type.as_str())
            .map(|m| m.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string())
    }

    /// Final browser→renderer transport used by explicit Copy. Keeping the seam
    /// at `Frame::send_process_message` lets the headless regression capture the
    /// real typed CEF envelope while production still resolves the live frame.
    pub(super) trait RendererMessageSink {
        fn send_to_renderer(&mut self, message: crate::ipc::BrowserMessage);
    }

    impl RendererMessageSink for cef::Frame {
        fn send_to_renderer(&mut self, message: crate::ipc::BrowserMessage) {
            if let Some(mut message) = message.to_cef_message() {
                self.send_process_message(ProcessId::RENDERER, Some(&mut message));
            }
        }
    }

    pub(super) fn request_live_selection_with<S: RendererMessageSink>(
        resolve: impl FnOnce() -> Option<S>,
    ) -> bool {
        let Some(mut sink) = resolve() else {
            return false;
        };
        sink.send_to_renderer(crate::ipc::BrowserMessage::CopySelection);
        true
    }

    /// Ask only the current foreground browser's main renderer frame to copy its
    /// live selection. The response returns through RendererMessage::SetClipboard.
    fn request_live_selection(widget: &super::KarereWebView) {
        request_live_selection_with(|| {
            resolved_browser(widget.imp()).and_then(|browser| browser.main_frame())
        });
    }

    /// Middle-click: paste primary-clipboard text into the element at `(x, y)` if
    /// editable (page-enforced). Empty selection sends nothing.
    fn read_primary_clipboard_paste(widget: &super::KarereWebView, x: f64, y: f64) {
        let Some(display) = gtk::gdk::Display::default() else {
            return;
        };
        display.primary_clipboard().read_text_async(
            gtk::gio::Cancellable::NONE,
            glib::clone!(
                #[weak]
                widget,
                move |res| match res {
                    Ok(Some(text)) if !text.is_empty() => {
                        send_text_paste(&widget, text.as_str(), Some((x, y)))
                    }
                    Ok(_) => {}
                    Err(err) => log::debug!("primary clipboard read failed: {err}"),
                }
            ),
        );
    }

    fn gdk_key_to_vk(keyval: gtk::gdk::Key) -> i32 {
        use gtk::gdk::Key;
        // Common keys only; everything else falls back to the unicode CHAR event.
        match keyval {
            Key::BackSpace => 0x08,
            Key::Tab => 0x09,
            Key::Return | Key::KP_Enter => 0x0D,
            Key::Escape => 0x1B,
            Key::Page_Up => 0x21,
            Key::Page_Down => 0x22,
            Key::End => 0x23,
            Key::Home => 0x24,
            Key::Left => 0x25,
            Key::Up => 0x26,
            Key::Right => 0x27,
            Key::Down => 0x28,
            Key::Insert => 0x2D,
            Key::Delete => 0x2E,
            // Only letters/digits map cleanly to a Windows VK via ASCII (A–Z, 0–9).
            // Punctuation collides with named VKs (e.g. '.'=0x2E=VK_DELETE), making
            // Chromium treat the keydown as a command and drop the char. Use 0; the
            // separate CHAR event (IM commit / keyval fallback) still inserts it.
            _ => keyval
                .to_unicode()
                .filter(|c| c.is_ascii_alphanumeric())
                .map(|c| c.to_ascii_uppercase() as i32)
                .unwrap_or(0),
        }
    }

    const VS: &str = r#"#version 300 es
precision highp float;
layout(location=0) in vec2 a_pos;
layout(location=1) in vec2 a_uv;
out vec2 v_uv;
void main() {
    v_uv = a_uv;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}
"#;

    pub(super) const FS: &str = r#"#version 300 es
precision highp float;
in vec2 v_uv;
out vec4 frag;
uniform sampler2D u_tex;
// 1 = software path (CEF BGRA bytes uploaded as RGBA → swizzle back).
// 0 = GPU dma-buf path (EGL imports the real format → sample as RGBA). (gpu-osr)
uniform int u_bgra;
void main() {
    vec4 c = texture(u_tex, v_uv);
    frag = (u_bgra == 1) ? c.bgra : c.rgba;
}
"#;

    unsafe fn compile_program(vsrc: &CString, fsrc: &CString) -> GLuint {
        unsafe {
            let vs = compile_shader(gl::VERTEX_SHADER, vsrc);
            let fs = compile_shader(gl::FRAGMENT_SHADER, fsrc);
            let p = gl::CreateProgram();
            gl::AttachShader(p, vs);
            gl::AttachShader(p, fs);
            gl::LinkProgram(p);
            let mut ok = 0;
            gl::GetProgramiv(p, gl::LINK_STATUS, &mut ok);
            if ok == 0 {
                let mut log_len = 0;
                gl::GetProgramiv(p, gl::INFO_LOG_LENGTH, &mut log_len);
                let mut buf = vec![0u8; log_len as usize];
                gl::GetProgramInfoLog(
                    p,
                    log_len,
                    std::ptr::null_mut(),
                    // c_char differs per arch (i8 x86_64 / u8 aarch64); cast per-arch
                    // (hardcoding i8 broke the aarch64 Flatpak build).
                    buf.as_mut_ptr() as *mut std::os::raw::c_char,
                );
                log::error!("program link: {}", String::from_utf8_lossy(&buf));
            }
            gl::DeleteShader(vs);
            gl::DeleteShader(fs);
            p
        }
    }

    unsafe fn compile_shader(kind: GLenum, src: &CString) -> GLuint {
        unsafe {
            let s = gl::CreateShader(kind);
            let p = src.as_ptr();
            gl::ShaderSource(s, 1, &p, std::ptr::null());
            gl::CompileShader(s);
            let mut ok = 0;
            gl::GetShaderiv(s, gl::COMPILE_STATUS, &mut ok);
            if ok == 0 {
                let mut log_len = 0;
                gl::GetShaderiv(s, gl::INFO_LOG_LENGTH, &mut log_len);
                let mut buf = vec![0u8; log_len as usize];
                gl::GetShaderInfoLog(
                    s,
                    log_len,
                    std::ptr::null_mut(),
                    buf.as_mut_ptr() as *mut std::os::raw::c_char,
                );
                log::error!("shader compile: {}", String::from_utf8_lossy(&buf));
            }
            s
        }
    }
}

#[cfg(test)]
mod tests {
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

        // Force GTK's production create-context signal to fail. The subclass
        // realize implementation must preserve that error and must not cross
        // the browser-bootstrap fence.
        let failed = KarereWebView::new();
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

        let error = failed
            .error()
            .expect("injected context failure must remain observable");
        assert!(error.message().contains("injected GL context failure"));
        assert!(failed.imp().browser.lock().is_none());
        assert!(failed.imp().browsers.lock().is_empty());
        assert!(failed.imp().pending_contexts.lock().is_empty());
        window.destroy();

        assert_rejected_accelerated_frame_restarts_pool_then_renders_cpu_callback();
    }

    #[test]
    fn cpu_upload_covers_empty_first_reuse_update_and_resize() {
        let mut frame = crate::handlers::FrameBuffer::default();
        assert_eq!(cpu_upload(&frame, (0, 0)), CpuUpload::Empty);

        // CEF supplies BGRA bytes; the fragment shader performs the matching
        // BGRA swizzle on the software path.
        frame.width = 2;
        frame.height = 1;
        frame.pixels = vec![1, 2, 3, 255, 4, 5, 6, 255];
        frame.dirty = true;
        assert_eq!(cpu_upload(&frame, (0, 0)), CpuUpload::Allocate);
        assert!(super::imp::FS.contains("c.bgra"));

        frame.dirty = false;
        assert_eq!(cpu_upload(&frame, (2, 1)), CpuUpload::Reuse);
        frame.dirty = true;
        assert_eq!(cpu_upload(&frame, (2, 1)), CpuUpload::Update);

        frame.width = 1;
        frame.height = 2;
        assert_eq!(cpu_upload(&frame, (2, 1)), CpuUpload::Allocate);
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

        window.destroy();
    }
}

#[cfg(test)]
mod input_tests {
    use super::imp::{
        ExplicitCopyTrigger, MouseButtonTracker, MouseDispatch, MouseEventSink, MouseInput,
        RendererMessageSink, dispatch_explicit_copy, dispatch_mouse_with,
        physical_mouse_coordinates, request_live_selection_with, send_click_with, send_move_with,
        should_forward_mouse,
    };
    use crate::handlers::{
        client::dispatch_renderer_message_with,
        render_process::{
            RendererDispatchSink, dispatch_browser_message_with, renderer_message_from_v8_args,
        },
    };
    use base64::Engine;
    use cef::sys::cef_event_flags_t as F;
    use cef::{MouseButtonType, MouseEvent};
    use gtk::gdk::{Key, ModifierType};
    use std::cell::{Cell, RefCell};
    use std::process::Command;

    #[derive(Debug, PartialEq)]
    enum Observed {
        Click {
            browser: i32,
            x: i32,
            y: i32,
            button: u32,
            down: bool,
            count: i32,
            modifiers: u32,
        },
        Move {
            browser: i32,
            x: i32,
            y: i32,
            modifiers: u32,
        },
    }

    #[derive(Clone)]
    struct FakeMouseHost<'a> {
        browser: i32,
        out: &'a RefCell<Vec<Observed>>,
    }

    impl MouseEventSink for FakeMouseHost<'_> {
        fn send_mouse_move(&mut self, event: &MouseEvent, _leave: bool) {
            self.out.borrow_mut().push(Observed::Move {
                browser: self.browser,
                x: event.x,
                y: event.y,
                modifiers: event.modifiers,
            });
        }

        fn send_mouse_click(
            &mut self,
            event: &MouseEvent,
            button: MouseButtonType,
            mouse_up: bool,
            click_count: i32,
        ) {
            let button = if button == MouseButtonType::LEFT {
                1
            } else if button == MouseButtonType::MIDDLE {
                2
            } else {
                3
            };
            self.out.borrow_mut().push(Observed::Click {
                browser: self.browser,
                x: event.x,
                y: event.y,
                button,
                down: !mouse_up,
                count: click_count,
                modifiers: event.modifiers,
            });
        }
    }

    fn route_mouse(
        event: MouseDispatch,
        active: &Cell<i32>,
        scale: i32,
        out: &RefCell<Vec<Observed>>,
    ) {
        dispatch_mouse_with(
            event,
            |x, y, button, down, count, modifiers| {
                send_click_with((x, y), scale, (button, down, count), modifiers, || {
                    Some(FakeMouseHost {
                        browser: active.get(),
                        out,
                    })
                });
            },
            |x, y, modifiers| {
                send_move_with(
                    x,
                    y,
                    scale,
                    modifiers,
                    false,
                    || {
                        Some(FakeMouseHost {
                            browser: active.get(),
                            out,
                        })
                    },
                    |_, _| {},
                );
            },
        );
    }

    struct JavaScriptRenderer<'a> {
        selection: &'a str,
        collapsed: bool,
        clipboard: &'a RefCell<Vec<(bool, String)>>,
    }

    impl RendererDispatchSink for JavaScriptRenderer<'_> {
        fn execute_java_script(&mut self, script: &str, source_url: &str) {
            assert_eq!(source_url, "karere://copy-selection");
            let b64 = base64::engine::general_purpose::STANDARD;
            let output = Command::new("node")
                .args([
                    "tests/copy_bridge_pipeline.js",
                    &b64.encode(self.selection),
                    if self.collapsed { "true" } else { "false" },
                    &b64.encode(script),
                ])
                .output()
                .expect("node must execute the production copy bridge");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            if result.is_null() {
                return;
            }
            let message = renderer_message_from_v8_args(
                result["name"].as_str().unwrap(),
                result["innerJson"].as_str().unwrap(),
            )
            .unwrap();
            dispatch_renderer_message_with(&message, |text, primary| {
                self.clipboard.borrow_mut().push((primary, text.to_owned()));
            });
        }

        fn send_to_browser(&mut self, _message: crate::ipc::RendererMessage) {
            panic!("copy selection must execute JavaScript, not send a renderer reply");
        }
    }

    struct ForegroundFrame<'a> {
        browser: i32,
        requested_browsers: &'a RefCell<Vec<i32>>,
        renderer: JavaScriptRenderer<'a>,
    }

    impl RendererMessageSink for ForegroundFrame<'_> {
        fn send_to_renderer(&mut self, message: crate::ipc::BrowserMessage) {
            self.requested_browsers.borrow_mut().push(self.browser);
            // This is the typed value delivered by Frame::send_process_message
            // after the production renderer callback decodes its CEF envelope.
            dispatch_browser_message_with(message, &mut self.renderer);
        }
    }

    fn copy_through_production_adapters(
        trigger: ExplicitCopyTrigger,
        active: &Cell<i32>,
        selection: &str,
        collapsed: bool,
        requested_browsers: &RefCell<Vec<i32>>,
        clipboard: &RefCell<Vec<(bool, String)>>,
    ) {
        dispatch_explicit_copy(trigger, || {
            assert!(request_live_selection_with(|| Some(ForegroundFrame {
                browser: active.get(),
                requested_browsers,
                renderer: JavaScriptRenderer {
                    selection,
                    collapsed,
                    clipboard,
                },
            })));
        });
    }

    #[test]
    fn drag_then_immediate_keyboard_and_command_copy_use_foreground_pipeline() {
        let active = Cell::new(7);
        let observed = RefCell::new(Vec::new());
        let mut adapter = MouseButtonTracker::default();
        let inputs = [
            MouseInput::Press {
                button: 1,
                x: 10.0,
                y: 10.0,
                time: 100,
                double_time: 250,
                double_distance: 5.0,
                modifiers: 0,
            },
            // Far beyond the historical GTK GestureClick threshold.
            MouseInput::Motion {
                x: 110.0,
                y: 42.0,
                modifiers: 0,
            },
            MouseInput::Release {
                button: 1,
                x: 110.0,
                y: 42.0,
                modifiers: 0,
            },
        ];
        for input in inputs {
            for event in adapter.handle(input) {
                route_mouse(event, &active, 2, &observed);
            }
        }
        assert_eq!(
            observed.into_inner(),
            vec![
                Observed::Click {
                    browser: 7,
                    x: 20,
                    y: 20,
                    button: 1,
                    down: true,
                    count: 1,
                    modifiers: F::EVENTFLAG_LEFT_MOUSE_BUTTON.0,
                },
                Observed::Move {
                    browser: 7,
                    x: 220,
                    y: 84,
                    modifiers: F::EVENTFLAG_LEFT_MOUSE_BUTTON.0,
                },
                Observed::Click {
                    browser: 7,
                    x: 220,
                    y: 84,
                    button: 1,
                    down: false,
                    count: 1,
                    modifiers: 0,
                },
            ]
        );

        let clipboard = RefCell::new(vec![(false, "existing".to_owned())]);
        let requested_browsers = RefCell::new(Vec::new());
        let selection = "first line\nZażółć 🙂\x1b";
        let sanitized_selection = "first line\nZażółć 🙂";
        copy_through_production_adapters(
            ExplicitCopyTrigger::Keyboard {
                keyval: Key::c,
                state: ModifierType::CONTROL_MASK,
            },
            &active,
            selection,
            false,
            &requested_browsers,
            &clipboard,
        );
        // Switching pooled accounts between copy actions must re-resolve the
        // foreground frame; browser 7 must not receive later requests.
        active.set(8);
        copy_through_production_adapters(
            ExplicitCopyTrigger::ContextMenu(cef::sys::cef_menu_id_t::MENU_ID_COPY as i32),
            &active,
            selection,
            false,
            &requested_browsers,
            &clipboard,
        );
        assert_eq!(
            clipboard.borrow().as_slice(),
            [
                (false, "existing".to_owned()),
                (false, sanitized_selection.to_owned()),
                (false, sanitized_selection.to_owned()),
            ]
        );

        // Empty/collapsed live selection produces no SetClipboard in JavaScript;
        // even a defensive empty envelope cannot clobber the existing value.
        copy_through_production_adapters(
            ExplicitCopyTrigger::Keyboard {
                keyval: Key::C,
                state: ModifierType::CONTROL_MASK,
            },
            &active,
            "",
            true,
            &requested_browsers,
            &clipboard,
        );
        assert_eq!(clipboard.borrow().len(), 3);
        assert_eq!(requested_browsers.borrow().as_slice(), [7, 8, 8]);
    }

    #[test]
    fn every_event_re_resolves_the_foreground_after_account_switch() {
        let active = Cell::new(1);
        let observed = RefCell::new(Vec::new());
        let mut adapter = MouseButtonTracker::default();
        for event in adapter.handle(MouseInput::Press {
            button: 1,
            x: 1.0,
            y: 1.0,
            time: 1,
            double_time: 250,
            double_distance: 5.0,
            modifiers: 0,
        }) {
            route_mouse(event, &active, 1, &observed);
        }
        active.set(2);
        for input in [
            MouseInput::Motion {
                x: 20.0,
                y: 20.0,
                modifiers: 0,
            },
            MouseInput::Release {
                button: 1,
                x: 20.0,
                y: 20.0,
                modifiers: 0,
            },
        ] {
            for event in adapter.handle(input) {
                route_mouse(event, &active, 1, &observed);
            }
        }
        let observed = observed.into_inner();
        assert!(matches!(observed[0], Observed::Click { browser: 1, .. }));
        assert!(observed[1..].iter().all(|event| matches!(
            event,
            Observed::Move { browser: 2, .. } | Observed::Click { browser: 2, .. }
        )));
    }

    #[test]
    fn preserves_multiclick_buttons_touch_suppression_layouts_and_cancellation() {
        assert!(should_forward_mouse(false));
        assert!(!should_forward_mouse(true));
        for width in [600.0, 1280.0] {
            let x = width - 10.1;
            // physical_mouse_coordinates now rounds (x*scale).round() to avoid
            // truncation bias at HiDPI (bounded fix #158/#176).
            assert_eq!(
                physical_mouse_coordinates(x, 7.1, 1),
                ((x * 1.0).round() as i32, 7)
            );
            assert_eq!(
                physical_mouse_coordinates(x, 7.1, 2),
                ((x * 2.0).round() as i32, 14)
            );
        }

        let mut adapter = MouseButtonTracker::default();
        for (time, expected) in [(100, 1), (200, 2), (300, 3), (400, 1)] {
            assert_eq!(adapter.press(1, 4.0, 5.0, time, 250, 5.0), Some(expected));
            assert_eq!(adapter.press(1, 4.0, 5.0, time, 250, 5.0), None);
            assert_eq!(adapter.release(1), Some(expected));
        }
        adapter.press(2, 1.0, 1.0, 1_000, 250, 5.0).unwrap();
        adapter.press(3, 1.0, 1.0, 1_001, 250, 5.0).unwrap();
        let cancelled = adapter.handle(MouseInput::Cancel { x: 9.0, y: 8.0 });
        assert_eq!(cancelled.len(), 2);
        assert_eq!(adapter.active_modifiers(), 0);
    }

    #[test]
    fn hidpi_physical_coordinates_round_instead_of_truncating() {
        // Truncation at HiDPI could bias by up to scale-1 px (e.g. 10.9*2=20 vs 22).
        // Bounded fix #158 rounds to nearest, keeping error ≤0.5 px.
        assert_eq!(physical_mouse_coordinates(10.9, 20.9, 1), (11, 21));
        assert_eq!(physical_mouse_coordinates(10.9, 20.9, 2), (22, 42));
        assert_eq!(physical_mouse_coordinates(10.1, 20.1, 2), (20, 40));
        assert_eq!(physical_mouse_coordinates(0.5, 0.5, 2), (1, 1));
        // Send path uses same helper, so dispatch stays within 1 px of expected
        // physical after rounding, even for fractional GTK gesture coords.
        let observed = std::cell::RefCell::new(Vec::new());
        let active = std::cell::Cell::new(99);
        route_mouse(
            crate::web_view::imp::MouseDispatch::Click {
                x: 10.9,
                y: 20.9,
                button: 1,
                down: true,
                count: 1,
                modifiers: 0,
            },
            &active,
            2,
            &observed,
        );
        assert_eq!(
            observed.borrow()[0],
            Observed::Click {
                browser: 99,
                x: 22,
                y: 42,
                button: 1,
                down: true,
                count: 1,
                modifiers: 0
            }
        );
    }

    #[test]
    fn h7_origin_and_fractional_rounding() {
        // KARE-018 H7: origin-calibrated mapping and fractional Wayland scales.
        // Host dispatch is (logical * scale).round(); chrome offset is subtracted
        // before scaling via calibration (observed - origin), so calibrated physical
        // equals ((x - originX)*scale).round().
        fn calibrated_physical(x: f64, origin: f64, scale: f64) -> i32 {
            ((x - origin) * scale).round() as i32
        }
        // Origin case: header 60px offset at 2×
        assert_eq!(calibrated_physical(170.0, 60.0, 2.0), 220);
        assert_eq!(calibrated_physical(230.0, 60.0, 2.0), 340);
        // Fractional cases: 110.5 at 1.25× →138, at 1.5× →166
        assert_eq!(((110.5_f64 * 1.25).round() as i32), 138);
        assert_eq!(((110.5_f64 * 1.5).round() as i32), 166);
        assert_eq!(physical_mouse_coordinates(110.5, 20.0, 1), (111, 20));
        // Transform + scroll origin: translate(10,20)+scrollTop 80 shifts origin by 10,100
        let origin_x = 360.0 + 10.0; // panel + translateX
        let origin_y = 60.0 + 20.0 + 80.0; // header + translateY + scroll
        assert_eq!(calibrated_physical(500.0, origin_x, 1.0), 130);
        assert_eq!(calibrated_physical(300.0, origin_y, 2.0), 280);
        // pointer_pos_physical stripping + origin: native 200 at scale 2 with surface xform stripped → logical 100, minus origin 60 → 40*2=80
        // show_context_menu: x_dev 220 at scale 2 → logical 110, origin-corrected 50 → verifies divide symmetry
        assert_eq!((220.0_f64 / 2.0).round() as i32, 110);
        assert_eq!(((110.0_f64 - 60.0) * 2.0).round() as i32, 100);
        // Dispatch with origin-corrected coords stays within 1px
        let observed = std::cell::RefCell::new(Vec::new());
        let active = std::cell::Cell::new(7);
        route_mouse(
            crate::web_view::imp::MouseDispatch::Move { x: 110.0, y: 110.0, modifiers: 0 },
            &active, 2, &observed,
        );
        assert_eq!(observed.borrow()[0], Observed::Move { browser: 7, x: 220, y: 220, modifiers: 0 });
        // Calibrated dispatch: logical 170 with origin 60 at 2× should hit physical 220
        let cx = calibrated_physical(170.0, 60.0, 2.0);
        assert_eq!(cx, 220);
        assert_eq!(physical_mouse_coordinates(170.0 - 60.0, 0.0, 2), (220, 0));
    }

    #[test]
    fn non_copy_commands_and_modified_shortcuts_do_not_request_selection() {
        let requests = Cell::new(0);
        for trigger in [
            ExplicitCopyTrigger::Keyboard {
                keyval: Key::c,
                state: ModifierType::empty(),
            },
            ExplicitCopyTrigger::Keyboard {
                keyval: Key::c,
                state: ModifierType::CONTROL_MASK | ModifierType::ALT_MASK,
            },
            ExplicitCopyTrigger::ContextMenu(cef::sys::cef_menu_id_t::MENU_ID_PASTE as i32),
        ] {
            dispatch_explicit_copy(trigger, || requests.set(requests.get() + 1));
        }
        assert_eq!(requests.get(), 0);
    }
}

#[cfg(test)]
mod zoom_tests {
    use super::{cef_to_linear, linear_to_cef, ZOOM_MAX};

    /// linear → CEF → linear round-trips within 1e-9 across the range.
    #[test]
    fn round_trip_linear() {
        for x in [0.5_f64, 1.0, 1.1, 1.331, 3.0] {
            let back = cef_to_linear(linear_to_cef(x));
            assert!((back - x).abs() < 1e-9, "round-trip {x} -> {back}");
        }
    }

    /// out-of-range input clamps to the max linear factor (3.0).
    #[test]
    fn clamp_above_max() {
        let back = cef_to_linear(linear_to_cef(5.0));
        assert!((back - ZOOM_MAX).abs() < 1e-9, "clamp 5.0 -> {back}");
    }
}
