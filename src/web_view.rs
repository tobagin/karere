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
        obj.imp().devtools.store(true, std::sync::atomic::Ordering::Relaxed);
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
        self.imp().spawn_browser(Some(account_id.to_owned()), foreground);
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
                if ["phosh", "plasma-mobile", "lomiri"].iter().any(|m| d.contains(m)) {
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
pub(crate) fn apply_zoom_from_account(browser: &cef::Browser) {
    use cef::{ImplBrowser, ImplBrowserHost};
    let Some(id) = crate::accounts::account_for_browser(browser.identifier()) else {
        return;
    };
    let persisted = crate::accounts::manager()
        .get(&id)
        .map(|a| a.zoom_level)
        .unwrap_or(1.0);
    let effective = persisted.max(zoom_floor());
    if let Some(host) = browser.host() {
        host.set_zoom_level(linear_to_cef(effective));
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
    let Some(frame) = browser.main_frame() else { return };
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

    let Some(host) = browser.host() else { return false };
    let Some(ctx) = host.request_context() else { return false };
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
    let view = CTX_MENU_WIDGETS.with(|m| {
        m.borrow()
            .get(&browser_id)
            .and_then(|w| w.upgrade())
    });
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
        MouseButtonType, MouseEvent, RequestContext, RequestContextHandler, RequestContextSettings,
        RunContextMenuCallback, WindowInfo, WrapRequestContextHandler,
        browser_host_create_browser_sync, rc::Rc, request_context_create_context, sys,
        wrap_request_context_handler,
    };
    use std::cell::RefCell;
    use std::collections::HashMap;
    use gl::types::{GLenum, GLint, GLuint};
    use glib::subclass::Signal;
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use once_cell::sync::Lazy;
    use parking_lot::Mutex;
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
        /// Last chrome-window visibility (recorded; sound gating no longer uses it).
        #[allow(dead_code)]
        pub window_visible: AtomicBool,
        program: AtomicU32,
        vao: AtomicU32,
        vbo: AtomicU32,
        texture: AtomicU32,
        tex_w: AtomicI32,
        tex_h: AtomicI32,
        /// Last pointer position (logical px) so wheel events hit the element
        /// under the cursor, not the top-left corner.
        last_mouse_x: AtomicI32,
        last_mouse_y: AtomicI32,
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
            widget.set_has_depth_buffer(false);
            widget.set_has_stencil_buffer(false);
            widget.set_auto_render(false);

            let scale = widget.scale_factor() as f32;
            // Default viewport so prewarm browsers (created before sizing) lay out
            // usably; the real allocation replaces it on first show.
            let shared = new_shared((1280, 800), scale);
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
                let cursor_name = {
                    let mut s = shared.lock();
                    if s.frame.dirty {
                        w.queue_render();
                    }
                    if s.cursor_dirty {
                        s.cursor_dirty = false;
                        Some(s.cursor_name)
                    } else {
                        None
                    }
                };
                if let Some(name) = cursor_name {
                    w.set_cursor_from_name(Some(name));
                }
                glib::ControlFlow::Continue
            });

            widget.set_focusable(true);
            widget.set_can_focus(true);
            install_input_controllers(&widget);
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| vec![
                Signal::builder("title-changed")
                    .param_types([String::static_type()])
                    .build(),
            ]);
            SIGNALS.as_ref()
        }
    }

    impl WidgetImpl for KarereWebView {
        fn realize(&self) {
            self.parent_realize();
            let widget = self.obj();
            widget.make_current();
            if let Some(err) = widget.error() {
                log::error!("GLArea realize error: {err}");
                return;
            }
            unsafe {
                self.init_gl();
            }
            self.bootstrap_pool();

            // Follow fractional scale changes (e.g. dragging between monitors of
            // different scale) so device_scale_factor / paint buffer track. A
            // pure scale change need not re-run size_allocate, so watch the
            // surface's `scale` property directly. (#155)
            if let Some(surface) = widget.native().and_then(|n| n.surface()) {
                surface.connect_scale_notify(glib::clone!(
                    #[weak] widget,
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
            // CEF's view rect and mouse events are in DIP (logical) units; its
            // GetScreenInfo.device_scale_factor maps them to the physical paint
            // buffer (on_paint dimensions). Pass the *logical* size plus the
            // *fractional* surface scale so content lays out at the correct size
            // on fractional displays — e.g. 150 % GNOME scaling (#155).
            let scale = surface_scale(&self.obj());

            if let Some(shared) = self.shared.lock().as_ref() {
                let mut s = shared.lock();
                s.size = (width, height);
                s.scale_factor = scale as f32;
            }

            if let Some(browser) = resolved_browser(self)
                && let Some(host) = browser.host()
            {
                host.notify_screen_info_changed();
                host.was_resized();
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

        pub fn close_browser(&self) {
            // Cancel any in-flight OSR menu so CEF doesn't leak pending-menu state
            // on mid-menu teardown, and drop the popover.
            if let Some(cb) = self.pending_menu_callback.borrow_mut().take() {
                cb.cancel();
            }
            if let Some(pop) = self.context_popover.borrow_mut().take() {
                pop.unparent();
            }
            let id = self.browser_id.swap(0, Ordering::Relaxed);
            if id != 0 {
                super::unregister_context_menu_widget(id);
            }

            // Close every pooled account browser, then the legacy/DevTools single
            // browser if it's outside the pool.
            let pooled: Vec<Browser> = self.browsers.lock().drain().map(|(_, b)| b).collect();
            self.life_spans.lock().clear();
            *self.foreground.lock() = None;
            for browser in &pooled {
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
        }

        /// Push the current fractional surface scale into the shared state and
        /// tell CEF its screen info changed, so the device_scale_factor and
        /// paint buffer follow a live scale change. (#155)
        fn refresh_screen_scale(&self) {
            let scale = surface_scale(&self.obj()) as f32;
            if let Some(shared) = self.shared.lock().as_ref() {
                shared.lock().scale_factor = scale;
            }
            if let Some(browser) = resolved_browser(self)
                && let Some(host) = browser.host()
            {
                host.notify_screen_info_changed();
                host.was_resized();
            }
            self.obj().queue_render();
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

            // CEF reports the cursor in view coords, which are DIP now that we
            // forward mouse events in DIP (#155) — the same space as GTK widget
            // coords, so use them directly for the popover anchor.
            let rect = gtk::gdk::Rectangle::new(x_dev, y_dev, 1, 1);
            popover.set_pointing_to(Some(&rect));

            // Resolve the callback AFTER popdown (webview re-focused): dispatch the
            // activated command, or cancel if dismissed without a selection.
            popover.connect_closed(glib::clone!(
                #[weak] obj,
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

        /// Set the foreground browser's zoom from a linear factor (clamped + → CEF
        /// log level). CEF UI thread only. (M18)
        pub fn set_zoom_linear(&self, linear: f64) {
            if let Some(browser) = self.browser.lock().as_ref().cloned()
                && let Some(host) = browser.host()
            {
                host.set_zoom_level(super::linear_to_cef(linear));
            }
        }

        /// Foreground browser's zoom as a linear factor, or 1.0 if none. (M18)
        pub fn get_zoom_linear(&self) -> f64 {
            if let Some(browser) = self.browser.lock().as_ref().cloned()
                && let Some(host) = browser.host()
            {
                super::cef_to_linear(host.zoom_level())
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
            let Some(first) = accounts.first().cloned() else {
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
            if let Some(id) = account_id.as_ref()
                && self.browsers.lock().contains_key(id)
            {
                if make_foreground {
                    self.switch_to(id);
                }
                return;
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
                ..Default::default()
            };
            let settings = BrowserSettings {
                // 30fps OSR: ample for a chat UI and halves the idle compositor/
                // paint load vs 60 (WhatsApp's idle animations keep invalidating).
                windowless_frame_rate: 30,
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
        }

        unsafe fn teardown_gl(&self) {
            unsafe {
                let tex = self.texture.load(Ordering::Relaxed);
                if tex != 0 {
                    gl::DeleteTextures(1, &tex);
                    self.texture.store(0, Ordering::Relaxed);
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

        unsafe fn draw(&self) {
            let shared = match self.shared.lock().as_ref() {
                Some(s) => s.clone(),
                None => return,
            };
            let mut s = shared.lock();

            unsafe {
                gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);

                if s.frame.width == 0 || s.frame.height == 0 || s.frame.pixels.is_empty() {
                    return;
                }

                let tex = self.texture.load(Ordering::Relaxed);
                gl::BindTexture(gl::TEXTURE_2D, tex);
                let tw = self.tex_w.load(Ordering::Relaxed);
                let th = self.tex_h.load(Ordering::Relaxed);
                if (tw, th) != (s.frame.width, s.frame.height) {
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
                } else if s.frame.dirty {
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
                drop(s);

                gl::UseProgram(self.program.load(Ordering::Relaxed));
                gl::BindVertexArray(self.vao.load(Ordering::Relaxed));
                gl::ActiveTexture(gl::TEXTURE0);
                gl::BindTexture(gl::TEXTURE_2D, tex);
                gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);
                gl::BindVertexArray(0);
                gl::UseProgram(0);
            }
        }
    }

    fn install_input_controllers(widget: &super::KarereWebView) {
        use gtk::gdk;

        // Mouse motion -------------------------------------------------------
        let motion = gtk::EventControllerMotion::new();
        motion.connect_motion(glib::clone!(
            #[weak] widget,
            move |ctrl, x, y| {
                let modifiers = modifiers_from_state(ctrl.current_event_state());
                send_move(&widget, x, y, modifiers, false);
            }
        ));
        motion.connect_leave(glib::clone!(
            #[weak] widget,
            move |ctrl| {
                let modifiers = modifiers_from_state(ctrl.current_event_state());
                send_move(&widget, 0.0, 0.0, modifiers, true);
            }
        ));
        widget.add_controller(motion);

        // Mouse buttons ------------------------------------------------------
        for button in 1..=3 {
            let click = gtk::GestureClick::builder().button(button).build();
            click.connect_pressed(glib::clone!(
                #[weak] widget,
                move |gesture, n_press, x, y| {
                    widget.grab_focus();
                    // Focus CEF on every click: at launch the GLArea may already hold
                    // GTK focus, so `grab_focus` is a no-op and the enter signal never
                    // fires, leaving CEF unfocused (no caret until refocus).
                    set_focus(&widget, true);
                    let modifiers = modifiers_from_state(gesture.current_event_state());
                    send_click(&widget, x, y, button, true, n_press, modifiers);
                    // M17: middle-click also pastes primary. Not claimed, so CEF still
                    // gets the middle button (preserves middle-click-to-open-link).
                    if button == 2 {
                        read_primary_clipboard_paste(&widget, x, y);
                    }
                }
            ));
            click.connect_released(glib::clone!(
                #[weak] widget,
                move |gesture, n_press, x, y| {
                    let modifiers = modifiers_from_state(gesture.current_event_state());
                    send_click(&widget, x, y, button, false, n_press, modifiers);
                }
            ));
            widget.add_controller(click);
        }

        // Scroll -------------------------------------------------------------
        let scroll = gtk::EventControllerScroll::new(
            gtk::EventControllerScrollFlags::BOTH_AXES,
        );
        scroll.connect_scroll(glib::clone!(
            #[weak] widget,
            #[upgrade_or] glib::Propagation::Proceed,
            move |ctrl, dx, dy| {
                let modifiers = modifiers_from_state(ctrl.current_event_state());
                send_wheel(&widget, dx, dy, modifiers);
                glib::Propagation::Stop
            }
        ));
        widget.add_controller(scroll);

        // Keyboard -----------------------------------------------------------
        // An input-method context turns raw key events into composed text:
        // dead keys (US-International ``+a → à``) and full IMEs only yield the
        // final character through `commit`, never from a single keyval. Route
        // keys through it and forward the committed text to CEF as CHAR events
        // (#154). Keys the IM doesn't consume fall back to the keyval below.
        let im = gtk::IMMulticontext::new();
        im.set_client_widget(Some(widget));
        im.connect_commit(glib::clone!(
            #[weak] widget,
            move |_im, text| {
                for ch in text.chars() {
                    send_char(&widget, ch as u16, 0);
                }
            }
        ));

        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak] widget,
            #[strong] im,
            #[upgrade_or] glib::Propagation::Proceed,
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
                // M17 outbound: CEF's offscreen copy never reaches the system clipboard
                // (DOM `copy` doesn't fire in OSR). Page selection is mirrored to PRIMARY
                // (50-copy-bridge), so on Ctrl+C promote PRIMARY → CLIPBOARD at the GTK
                // layer. Key still forwards to CEF below.
                if state.contains(ModifierType::CONTROL_MASK)
                    && !state.intersects(ModifierType::ALT_MASK | ModifierType::SUPER_MASK)
                    && matches!(keyval, Key::c | Key::C)
                {
                    promote_primary_to_clipboard();
                }
                // Always deliver the raw key-down so the page sees navigation,
                // Enter-to-send, and shortcut keys.
                send_key_raw(&widget, keyval, keycode, state, true);
                // Let the IM compose. On a dead key it buffers and returns true
                // (no commit yet); on the next key it fires `commit` with the
                // composed text. Plain keys also commit here.
                let consumed = ctrl
                    .current_event()
                    .map(|e| im.filter_keypress(&e))
                    .unwrap_or(false);
                if !consumed {
                    // IM produced no text (no IM running, or a combo like Ctrl+C);
                    // emit the keyval's character directly so typing still works.
                    send_char_from_keyval(&widget, keyval, state);
                }
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
            #[weak] widget,
            #[strong] im,
            move |ctrl, keyval, keycode, state| {
                if let Some(e) = ctrl.current_event() {
                    im.filter_keypress(&e);
                }
                send_key_raw(&widget, keyval, keycode, state, false);
            }
        ));
        widget.add_controller(keys);

        // Focus --------------------------------------------------------------
        let focus = gtk::EventControllerFocus::new();
        focus.connect_enter(glib::clone!(
            #[weak] widget,
            #[strong] im,
            move |_| {
                im.focus_in();
                set_focus(&widget, true);
            }
        ));
        focus.connect_leave(glib::clone!(
            #[weak] widget,
            #[strong] im,
            move |_| {
                im.focus_out();
                set_focus(&widget, false);
            }
        ));
        widget.add_controller(focus);

        // Drag-drop ----------------------------------------------------------
        // M17: accept file drops; surface each as a synthetic `drop` on the element
        // under the cursor (paste_bridge.js targets it via the envelope's coords).
        let drop_target = gtk::DropTarget::new(
            gdk::FileList::static_type(),
            gdk::DragAction::COPY,
        );
        // Forward hover (enter/motion/leave) so the page's dropzone overlay mounts
        // DURING the hover — CEF only delivers the drop on release, too late to mount.
        drop_target.connect_enter(glib::clone!(
            #[weak] widget,
            #[upgrade_or] gdk::DragAction::COPY,
            move |_t, x, y| {
                send_drag_hover(&widget, "enter", x, y);
                gdk::DragAction::COPY
            }
        ));
        let last_motion = std::cell::Cell::new(std::time::Instant::now());
        drop_target.connect_motion(glib::clone!(
            #[weak] widget,
            #[upgrade_or] gdk::DragAction::COPY,
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
            #[weak] widget,
            move |_t| send_drag_hover(&widget, "leave", 0.0, 0.0)
        ));
        drop_target.connect_drop(glib::clone!(
            #[weak] widget,
            #[upgrade_or] false,
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

        let _ = gdk::ModifierType::SHIFT_MASK;  // suppress unused-import warning
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

    fn modifiers_from_state(state: gtk::gdk::ModifierType) -> u32 {
        use gtk::gdk::ModifierType;
        use sys::cef_event_flags_t as F;
        let mut m = 0u32;
        if state.contains(ModifierType::SHIFT_MASK) { m |= F::EVENTFLAG_SHIFT_DOWN.0; }
        if state.contains(ModifierType::CONTROL_MASK) { m |= F::EVENTFLAG_CONTROL_DOWN.0; }
        if state.contains(ModifierType::ALT_MASK) { m |= F::EVENTFLAG_ALT_DOWN.0; }
        if state.contains(ModifierType::SUPER_MASK) { m |= F::EVENTFLAG_COMMAND_DOWN.0; }
        if state.contains(ModifierType::BUTTON1_MASK) { m |= F::EVENTFLAG_LEFT_MOUSE_BUTTON.0; }
        if state.contains(ModifierType::BUTTON2_MASK) { m |= F::EVENTFLAG_MIDDLE_MOUSE_BUTTON.0; }
        if state.contains(ModifierType::BUTTON3_MASK) { m |= F::EVENTFLAG_RIGHT_MOUSE_BUTTON.0; }
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
                        #[weak] obj,
                        #[weak] popover,
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

    fn with_host<F: FnOnce(&cef::BrowserHost)>(widget: &super::KarereWebView, f: F) {
        if let Some(b) = resolved_browser(widget.imp())
            && let Some(host) = b.host()
        {
            f(&host);
        }
    }

    /// Fractional display scale (DIP→physical) read from the widget's surface,
    /// e.g. 1.5 for 150 % GNOME fractional scaling. Falls back to the integer
    /// scale factor before the surface exists. CEF's screen-info scale and the
    /// paint buffer are sized from this; mouse/view coords stay in DIP. (#155)
    fn surface_scale(widget: &super::KarereWebView) -> f64 {
        widget
            .native()
            .and_then(|n| n.surface())
            .map(|s| s.scale())
            .filter(|s| *s > 0.0)
            .unwrap_or_else(|| widget.scale_factor() as f64)
    }

    fn send_move(widget: &super::KarereWebView, x: f64, y: f64, modifiers: u32, leave: bool) {
        if !leave {
            // Remember cursor so wheel events scroll the element under it.
            widget.imp().last_mouse_x.store(x as i32, Ordering::Relaxed);
            widget.imp().last_mouse_y.store(y as i32, Ordering::Relaxed);
        }
        // DIP (logical) coords — CEF maps to physical via device_scale_factor.
        let event = MouseEvent {
            x: x as i32,
            y: y as i32,
            modifiers,
        };
        with_host(widget, |host| {
            host.send_mouse_move_event(Some(&event), leave as i32);
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
        let event = MouseEvent {
            x: x as i32,
            y: y as i32,
            modifiers,
        };
        let btn = match button {
            1 => MouseButtonType::LEFT,
            2 => MouseButtonType::MIDDLE,
            3 => MouseButtonType::RIGHT,
            _ => return,
        };
        with_host(widget, |host| {
            host.send_mouse_click_event(Some(&event), btn, (!down) as i32, n_press.max(1));
        });
    }

    fn send_wheel(widget: &super::KarereWebView, dx: f64, dy: f64, modifiers: u32) {
        // GTK deltas are wheel ticks (1.0 = one notch); CEF wants pixel deltas.
        const STEP: f64 = 40.0;
        // CEF hit-tests the cursor to pick the scroll target; (0,0) scrolls nothing.
        // Coords/deltas in DIP — CEF maps to physical via device_scale_factor.
        let imp = widget.imp();
        let event = MouseEvent {
            x: imp.last_mouse_x.load(Ordering::Relaxed),
            y: imp.last_mouse_y.load(Ordering::Relaxed),
            modifiers,
        };
        with_host(widget, |host| {
            host.send_mouse_wheel_event(
                Some(&event),
                (-dx * STEP) as i32,
                (-dy * STEP) as i32,
            );
        });
    }

    /// Send only the raw key-down/up (RAWKEYDOWN / KEYUP) — no CHAR. Character
    /// insertion is driven separately by the IM `commit` (`send_char`) or the
    /// keyval fallback (`send_char_from_keyval`), so dead keys compose. (#154)
    fn send_key_raw(
        widget: &super::KarereWebView,
        keyval: gtk::gdk::Key,
        keycode: u32,
        state: gtk::gdk::ModifierType,
        down: bool,
    ) {
        let evt = KeyEvent {
            size: std::mem::size_of::<sys::_cef_key_event_t>(),
            type_: if down { KeyEventType::RAWKEYDOWN } else { KeyEventType::KEYUP },
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

    /// CHAR fallback for keys the IM did not consume (no IM running, or a combo
    /// like Ctrl+C): derive the character straight from the keyval. (#154)
    fn send_char_from_keyval(
        widget: &super::KarereWebView,
        keyval: gtk::gdk::Key,
        state: gtk::gdk::ModifierType,
    ) {
        if let Some(ch) = keyval.to_unicode() {
            send_char(widget, ch as u16, modifiers_from_state(state));
        }
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
    /// + dispatch a synthetic paste and return `true` so the caller swallows the key
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
                #[weak] widget,
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
                #[weak] widget,
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
                #[weak] widget,
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
                #[weak] widget,
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

    /// Ctrl+C: copy PRIMARY (synced to the page selection by `50-copy-bridge.js`)
    /// into CLIPBOARD, since CEF's offscreen copy never reaches the system clipboard.
    fn promote_primary_to_clipboard() {
        let Some(display) = gtk::gdk::Display::default() else {
            return;
        };
        let clipboard = display.clipboard();
        display.primary_clipboard().read_text_async(
            gtk::gio::Cancellable::NONE,
            move |res| {
                if let Ok(Some(text)) = res
                    && !text.is_empty()
                {
                    clipboard.set_text(text.as_str());
                }
            },
        );
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
                #[weak] widget,
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

    const FS: &str = r#"#version 300 es
precision highp float;
in vec2 v_uv;
out vec4 frag;
uniform sampler2D u_tex;
void main() {
    frag = texture(u_tex, v_uv).bgra;
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
