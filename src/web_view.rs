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

/// Monotonic time (µs, `glib::monotonic_time`) of the last host-observed user
/// input on the web view: key press, button press, drag motion, touch, or a
/// context-menu activation. Gates page-initiated clipboard writes. (S9)
static LAST_USER_INPUT_US: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(i64::MIN);

/// Record that the user just interacted with the web view. Main thread.
pub(crate) fn note_user_input() {
    LAST_USER_INPUT_US.store(glib::monotonic_time(), std::sync::atomic::Ordering::Relaxed);
}

/// True when the user interacted with the web view within the last `within_ms`.
pub fn recent_user_input(within_ms: i64) -> bool {
    let last = LAST_USER_INPUT_US.load(std::sync::atomic::Ordering::Relaxed);
    last != i64::MIN && glib::monotonic_time().saturating_sub(last) <= within_ms * 1000
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

/// UV transform `[x, y, w, h]` (fractions of the texture) that samples just
/// `visible` out of a `coded`-sized buffer. Fed to the vertex shader's `u_uv`.
fn texture_uv(visible: (i32, i32, i32, i32), coded: (i32, i32)) -> [f32; 4] {
    let cw = coded.0.max(1) as f32;
    let ch = coded.1.max(1) as f32;
    let (x, y, w, h) = visible;
    [x as f32 / cw, y as f32 / ch, w as f32 / cw, h as f32 / ch]
}

/// GL viewport `(x, y, w, h)` that puts `rect` — top-left origin, in CEF view
/// (physical) pixels — on a `framebuffer`-sized target while the view is
/// `view` pixels (`SharedState::size`). GL counts y from the bottom. A frame
/// of the current view size fills the framebuffer exactly; a stale
/// previous-size frame during a resize keeps its own size anchored top-left
/// instead of being stretched over the new size. Popups use the same mapping.
fn gl_viewport_for(
    framebuffer: (i32, i32),
    view: (i32, i32),
    rect: (i32, i32, i32, i32),
) -> (i32, i32, i32, i32) {
    let sx = if view.0 > 0 { framebuffer.0 as f64 / view.0 as f64 } else { 1.0 };
    let sy = if view.1 > 0 { framebuffer.1 as f64 / view.1 as f64 } else { 1.0 };
    let (x, y, w, h) = rect;
    let x0 = (x as f64 * sx).round() as i32;
    let y0 = (y as f64 * sy).round() as i32;
    let x1 = ((x + w) as f64 * sx).round() as i32;
    let y1 = ((y + h) as f64 * sy).round() as i32;
    (x0, framebuffer.1 - y1, x1 - x0, y1 - y0)
}

/// WhatsApp Web's page background, the same pair the shell tints itself with
/// (`match-whatsapp-colors` in `application.rs`). Shown around a frame that
/// does not cover the widget yet (resize, first paint) instead of black.
const PAGE_BG_DARK: [f32; 3] = [0x1d as f32 / 255.0, 0x1f as f32 / 255.0, 0x1f as f32 / 255.0];
const PAGE_BG_LIGHT: [f32; 3] = [0xf7 as f32 / 255.0, 0xf5 as f32 / 255.0, 0xf3 as f32 / 255.0];

/// Page background for the current theme. Read off the widget's CSS
/// foreground colour, which libadwaita drives from Karere's theme setting: a
/// light foreground means a dark theme.
fn page_background(widget: &impl gtk::prelude::WidgetExt) -> [f32; 3] {
    let fg = widget.color();
    page_background_for_foreground(fg.red(), fg.green(), fg.blue())
}

/// Physical screen origin of the window showing `area` (X11; `(0, 0)` on
/// Wayland), queried now rather than at the last allocation so a moved window
/// reports its new position. `None` when `area` is not a web view. Main thread
/// only.
pub(crate) fn current_window_origin(area: &gtk::GLArea) -> Option<(i32, i32)> {
    use gtk::prelude::Cast;
    area.downcast_ref::<KarereWebView>()
        .map(imp::window_origin_for)
}

fn page_background_for_foreground(r: f32, g: f32, b: f32) -> [f32; 3] {
    if 0.299 * r + 0.587 * g + 0.114 * b > 0.5 {
        PAGE_BG_DARK
    } else {
        PAGE_BG_LIGHT
    }
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
    use gtk::prelude::SettingsExt;
    with_app_settings(|s| {
        if s.boolean("webview-zoom") {
            s.double("zoom-level").clamp(ZOOM_MIN, ZOOM_MAX)
        } else {
            ZOOM_MIN
        }
    })
}

thread_local! {
    /// One app `GSettings` for the hot-path readers below (allocation, load
    /// end, mute pushes) instead of constructing a new object per call. Reads
    /// still go to the backend, so values are always current. Main thread only
    /// (`gio::Settings` is not `Send`).
    static APP_SETTINGS: gtk::gio::Settings = gtk::gio::Settings::new(crate::application::APP_ID);
}

fn with_app_settings<R>(f: impl FnOnce(&gtk::gio::Settings) -> R) -> R {
    APP_SETTINGS.with(f)
}

/// Running on a phone shell, where `auto` always means the mobile layout.
/// The session's desktop cannot change under a running process.
fn on_mobile_desktop() -> bool {
    static MOBILE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *MOBILE.get_or_init(|| {
        std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|desktop| {
            let d = desktop.to_lowercase();
            ["phosh", "plasma-mobile", "lomiri"]
                .iter()
                .any(|m| d.contains(m))
        })
    })
}

/// The width (logical px) below which `auto` mode switches WhatsApp Web to the
/// single-pane mobile layout. (M21)
pub(crate) const MOBILE_WIDTH_THRESHOLD: i32 = 768;

/// The upstream Whatslectron-UT mobile layout script (with local patches),
/// embedded for on-demand injection (NOT in the always-run M13 bundle). Applies the single-pane
/// layout unconditionally when run, so the host gates *when* it runs (see
/// [`should_use_mobile_layout`] / [`apply_mobile_layout`]). (M21)
const EMBED_MOBILE: &str = include_str!("../data/js-deferred/mobile_responsive.js");

/// Whether WhatsApp Web should use the single-pane mobile layout (mirrors v3):
/// `mobile-layout` GSetting forces `enabled`/`disabled`; `auto` is true on a mobile
/// desktop (phosh/plasma-mobile/lomiri) or width in `(0, MOBILE_WIDTH_THRESHOLD)`. (M21)
pub(crate) fn should_use_mobile_layout(width_logical_px: i32) -> bool {
    use gtk::prelude::SettingsExt;

    let mode = with_app_settings(|s| s.string("mobile-layout"));
    match mode.as_str() {
        "enabled" => true,
        "disabled" => false,
        _ => {
            on_mobile_desktop()
                || (width_logical_px > 0 && width_logical_px < MOBILE_WIDTH_THRESHOLD)
        }
    }
}

thread_local! {
    /// Layout each browser's current page was loaded with (CEF browser id →
    /// mobile), recorded when `on_load_end` runs the gate. The v3 script cannot
    /// un-apply itself, so a page whose recorded layout no longer matches the
    /// width needs a reload; tracking it per browser lets a background account
    /// be corrected when it comes to the foreground. Main thread only.
    static MOBILE_LOADED: std::cell::RefCell<std::collections::HashMap<i32, bool>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// True when `browser_id`'s loaded page uses the other layout than
/// `width_logical_px` calls for. `false` while no page has finished loading
/// (its `on_load_end` will pick the right one).
fn mobile_layout_mismatch(browser_id: i32, width_logical_px: i32) -> bool {
    let wanted = should_use_mobile_layout(width_logical_px);
    MOBILE_LOADED.with(|m| m.borrow().get(&browser_id).is_some_and(|&loaded| loaded != wanted))
}

fn record_mobile_layout(browser_id: i32, mobile: bool) {
    MOBILE_LOADED.with(|m| {
        m.borrow_mut().insert(browser_id, mobile);
    });
}

/// Inject the mobile-responsive script into `browser`'s main frame when the layout
/// is mobile for `width_logical_px`. Idempotent per page via `window.__karereMobileApplied`.
/// Called from `on_load_end`; the script only un-applies via a full reload (driven
/// by the debounced gate in `KarereWebView::check_mobile_layout`). (M21)
pub(crate) fn apply_mobile_layout(browser: &cef::Browser, width_logical_px: i32) {
    use cef::{CefString, ImplBrowser, ImplFrame};
    let mobile = should_use_mobile_layout(width_logical_px);
    record_mobile_layout(browser.identifier(), mobile);
    if !mobile {
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
    let globally_off = with_app_settings(|s| {
        !s.boolean("notifications-enabled") || !s.boolean("notify-sound-enabled")
    });
    if globally_off {
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

/// Run `js` in `browser`'s main frame (no-op without one).
fn run_page_script(browser: &cef::Browser, js: &str, name: &str) {
    use cef::{CefString, ImplBrowser, ImplFrame};
    if let Some(frame) = browser.main_frame() {
        frame.execute_java_script(
            Some(&CefString::from(js)),
            Some(&CefString::from(name)),
            0,
        );
    }
}

/// Script telling `30-autocorrect.js` whether English autocorrect applies:
/// `true` when the first effective spellcheck language is English, `false`
/// for any other language, `null` (page auto-detect) when there is none. (J2)
pub(crate) fn autocorrect_language_script(langs: &[String]) -> String {
    let v = match langs.first() {
        None => "null",
        Some(l) if l.split(['-', '_']).next().is_some_and(|p| p.eq_ignore_ascii_case("en")) => {
            "true"
        }
        Some(_) => "false",
    };
    format!("if(window.__karereAutocorrectSetEnabled)window.__karereAutocorrectSetEnabled({v});")
}

/// Push the autocorrect language gate for `langs` (the resolved spellcheck
/// languages) to `browser`.
pub(crate) fn apply_autocorrect_language(browser: &cef::Browser, langs: &[String]) {
    run_page_script(
        browser,
        &autocorrect_language_script(langs),
        "karere://autocorrect-lang",
    );
}

/// Largest `user.css` that is injected.
const USER_CSS_MAX_BYTES: u64 = 256 * 1024;

/// `<user config dir>/<app id>/user.css` (inside Flatpak: `~/.var/app/<id>/config/...`).
pub(crate) fn user_css_path() -> std::path::PathBuf {
    glib::user_config_dir()
        .join(crate::application::APP_ID)
        .join("user.css")
}

fn read_user_css() -> Option<String> {
    use std::io::Read;
    let path = user_css_path();
    // A FIFO or device here would block the main thread on open.
    if !std::fs::metadata(&path).ok()?.is_file() {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > USER_CSS_MAX_BYTES {
        log::warn!("user.css is larger than {USER_CSS_MAX_BYTES} bytes; ignoring it");
        return None;
    }
    let mut css = String::new();
    file.take(USER_CSS_MAX_BYTES).read_to_string(&mut css).ok()?;
    Some(css)
}

/// Script creating/updating `<style id="karere-user-css">`. The CSS travels as
/// a JSON string literal and is assigned via `textContent`, so nothing in it can
/// break out of the literal or be parsed as markup. (F9)
pub(crate) fn user_css_script(css: &str) -> String {
    let lit = serde_json::to_string(css).unwrap_or_else(|_| "\"\"".to_owned());
    format!(
        "(function(){{var e=document.getElementById('karere-user-css');\
         if(!e){{e=document.createElement('style');e.id='karere-user-css';\
         (document.head||document.documentElement).appendChild(e);}}\
         e.textContent={lit};}})();"
    )
}

/// Inject the user stylesheet, if the file exists. Called from `on_load_end`,
/// so it applies to every account and again after each reload. (F9)
pub(crate) fn apply_user_css(browser: &cef::Browser) {
    if let Some(css) = read_user_css() {
        run_page_script(browser, &user_css_script(&css), "karere://user-css");
    }
}

/// Apply the per-account persisted zoom (lifted to the accessibility floor) to
/// `browser`, rewriting the persisted value if the floor lifted it. Called from
/// `on_load_end` so each account restores its zoom on first paint and after
/// navigation. (M18 4.1 / 5.2)
pub(crate) fn apply_zoom_from_account(browser: &cef::Browser, display_scale: f64) {
    use cef::{ImplBrowser, ImplBrowserHost};
    let Some(id) = crate::accounts::account_for_browser(browser.identifier()) else {
        log::trace!(
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
    log::trace!(
        "coord: J7 apply_zoom account={} user_linear={:.3} display_scale={:.3} cef_level={:.3}",
        id,
        effective,
        display_scale,
        cef_level
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
/// The list is always written directly. Never write an intermediate `[]`: once
/// spellcheck is running in OSR, a clear tears the service down and the re-set
/// doesn't revive it (dead until restart). At startup the service is already
/// running from `--spell-check-languages`, so a clear there killed it too (#203).
pub(crate) fn apply_spellcheck_to_browser(browser: &cef::Browser, langs: &[String], enabled: bool) {
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

    apply_autocorrect_language(browser, langs);

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

/// Apply the cursor / IM focus / announcement requests pending in `area`'s
/// shared state. Called from the idle callback `handlers::wake_view` queues.
/// Main-thread only.
pub(crate) fn flush_view_requests(area: &gtk::GLArea) {
    use gtk::prelude::Cast;
    if let Some(view) = area.downcast_ref::<KarereWebView>() {
        view.imp().flush_requests(false);
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
    use std::sync::LazyLock;
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
        /// Account the user last asked to show while its context was still
        /// initialising; consumed by `on_account_context_ready`.
        pub wanted_foreground: Mutex<Option<String>>,
        /// Life-span handlers of browsers asked to close, held until each reports
        /// `closed` so quit can wait for every one of them.
        pub closing_life_spans: Mutex<Vec<ShellLifeSpanHandler>>,
        /// Set by quit: a context that finishes initialising afterwards must not
        /// create a browser nobody will close.
        pub shutting_down: AtomicBool,
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
        /// Last chrome-window visibility. `adopt_foreground` reads it to keep a
        /// new foreground paused while the window is hidden (#151).
        pub window_visible: AtomicBool,
        program: AtomicU32,
        /// No usable GL context, so frames are presented through GTK's normal
        /// (software) render path instead of the GLArea framebuffer. See
        /// `snapshot`. (#177)
        pub(super) software_present: AtomicBool,
        vao: AtomicU32,
        vbo: AtomicU32,
        texture: AtomicU32,
        tex_w: AtomicI32,
        tex_h: AtomicI32,
        /// GPU-OSR: dedicated texture the imported DMA-BUF EGLImage binds to, and
        /// the live EGLImage kept alive while that texture is sampled. (gpu-osr)
        accel_tex: AtomicU32,
        imported: RefCell<Option<crate::gl_dmabuf::ImportedImage>>,
        /// Popup layer (`<select>` dropdowns) textures: CPU upload target with
        /// its current size, and the accelerated import target + live
        /// EGLImage. Composited over the view in `draw`.
        popup_tex: AtomicU32,
        pub(super) popup_tex_w: AtomicI32,
        pub(super) popup_tex_h: AtomicI32,
        accel_popup_tex: AtomicU32,
        imported_popup: RefCell<Option<crate::gl_dmabuf::ImportedImage>>,
        /// Software present path: textures built from the last view / popup
        /// frame, reused while CEF has not painted again so an unrelated
        /// redraw does not copy the whole frame into a new texture. (P6)
        sw_texture: RefCell<Option<gtk::gdk::MemoryTexture>>,
        sw_popup_texture: RefCell<Option<gtk::gdk::MemoryTexture>>,
        /// Surface whose `scale` we follow and the handler id, so a realize
        /// connects once per surface and unrealize disconnects. (I11)
        scale_notify: RefCell<Option<(gtk::gdk::Surface, glib::SignalHandlerId)>>,
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
        /// M21 mobile-layout gate: pending debounced re-check after a resize or
        /// foreground switch. When it fires, a foreground page loaded with the
        /// other layout is reloaded so `on_load_end` re-evaluates. (D5)
        mobile_check: RefCell<Option<glib::SourceId>>,
        /// The GLArea's IM context, stored so `flush_requests` can focus it in/out
        /// in response to CEF's editable-focus signal (keyboard_request). Set once
        /// in `install_input_controllers`.
        pub im_context: RefCell<Option<gtk::IMMulticontext>>,
        /// Mirror of the IM context's focus state. The page can leave the IM
        /// focused-out (a send re-renders the input → virtual-keyboard NONE with
        /// no follow-up), which kills dead-key composition; the key handler uses
        /// this to re-focus the IM on the next physical keypress. (#154)
        pub im_focused: std::cell::Cell<bool>,
        /// Set while the attach file chooser is up, so a drop of several
        /// unreadable files opens it once rather than once per file. (#192)
        pub attach_chooser_open: std::cell::Cell<bool>,
        /// Ticks left before a pending focus-out is actually applied, or 0 when
        /// none is queued. A composer re-render emits editable-focus false then
        /// true; applying the false immediately unfocused the IM mid-typing and
        /// dropped the next key (#180). Deferring it lets the paired true cancel
        /// it, while a genuine blur still settles. See `im_focus_step`.
        pub im_focus_out_pending: std::cell::Cell<u8>,
        /// The focus-out debounce timer is running (`arm_im_focus_timer`).
        im_focus_timer_armed: std::cell::Cell<bool>,
        /// Input-method composition (preedit) state; decides whether a key press
        /// goes to the IM before CEF and how committed text is delivered. (I1)
        pub ime_state: std::cell::Cell<ImeState>,
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
            shared.lock().redraw = Some(widget.upcast_ref::<gtk::GLArea>().downgrade().into());

            // No periodic timer here. Paints queue their own render
            // (`request_redraw` in the render handler), and the cursor / IM /
            // announcement handlers queue one coalesced flush through
            // `handlers::wake_view` (`flush_requests`). Only a pending IM
            // focus-out runs a short-lived timer, for its debounce. A hidden
            // window therefore costs no wakeups from this view.
            // (Never add_tick_callback: the frame clock free-runs when vsync is
            // unreliable, pinning a core — issue #151.)

            widget.set_focusable(true);
            widget.set_can_focus(true);
            install_input_controllers(&widget);
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: LazyLock<Vec<Signal>> = LazyLock::new(|| {
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
        /// Present the CEF frame without GL when `realize` could not get a
        /// context (#177). GTK has already fallen back to its software renderer
        /// in that situation, so handing it a memory texture is all that is
        /// left to do; on every normal machine this defers to `GtkGLArea`,
        /// which drives `render` -> `draw` as before.
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            if !self.software_present.load(Ordering::Relaxed) {
                self.parent_snapshot(snapshot);
                return;
            }
            let widget = self.obj();
            let (w, h) = (widget.width() as f32, widget.height() as f32);
            let bounds = gtk::graphene::Rect::new(0.0, 0.0, w, h);
            let [r, g, b] = super::page_background(&*widget);
            snapshot.append_color(&gtk::gdk::RGBA::new(r, g, b, 1.0), &bounds);
            let Some(shared) = self.shared.lock().as_ref().cloned() else {
                return;
            };
            let mut s = shared.lock();
            let view = cached_frame_texture(&self.sw_texture, &mut s.frame);
            let popup_rect = s.popup.visible_rect(s.foreground_browser_id);
            let popup = match popup_rect {
                Some(_) => cached_frame_texture(&self.sw_popup_texture, &mut s.popup.frame),
                None => {
                    *self.sw_popup_texture.borrow_mut() = None;
                    None
                }
            };
            drop(s);
            let Some(view) = view else { return };
            // Frame and popup rect are physical pixels, widget bounds logical.
            // Draw the frame at its own size, anchored top-left: during a
            // resize the previous-size frame stays 1:1 instead of stretching.
            let scale = paint_scale(&widget) as f32;
            snapshot.push_clip(&bounds);
            snapshot.append_texture(
                &view,
                &gtk::graphene::Rect::new(
                    0.0,
                    0.0,
                    view.width() as f32 / scale,
                    view.height() as f32 / scale,
                ),
            );
            if let (Some(popup), Some((px, py, pw, ph))) = (popup, popup_rect) {
                snapshot.append_texture(
                    &popup,
                    &gtk::graphene::Rect::new(
                        px as f32 / scale,
                        py as f32 / scale,
                        pw as f32 / scale,
                        ph as f32 / scale,
                    ),
                );
            }
            snapshot.pop();
        }

        fn realize(&self) {
            self.parent_realize();
            // Both present paths (GL and software) must follow scale changes.
            self.watch_surface_scale();
            let widget = self.obj();
            widget.make_current();
            if let Some(err) = widget.error() {
                // No GL context. GDK cannot produce one below GLES 3.0, which is
                // all a Mali-400 (PinePhone, lima) offers, so this is terminal for
                // the GL path -- but not for the app: CEF's software OSR still
                // hands us BGRA frames, and `snapshot` can present them through
                // GTK's own renderer. Clear GTK's error so the GLArea shows our
                // frames rather than its built-in error label, and carry on to
                // the browser bootstrap. (#177)
                log::warn!("no GL context ({err}) — presenting frames in software");
                widget.set_error(None);
                self.software_present.store(true, Ordering::Relaxed);
                NO_GL.store(true, Ordering::Relaxed);
                self.bootstrap_pool();
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
            // The one point where this GLArea's context is known current and
            // before any browser of a visible start is created. (I6)
            probe_accel_osr();
            self.bootstrap_pool();
        }

        fn unrealize(&self) {
            self.close_browser();
            if let Some((surface, id)) = self.scale_notify.take() {
                surface.disconnect(id);
            }
            if let Some(id) = self.mobile_check.take() {
                id.remove();
            }
            *self.sw_texture.borrow_mut() = None;
            *self.sw_popup_texture.borrow_mut() = None;
            // glDelete* act on whatever context is current; deleting by name
            // with another view's context current (DevTools closing next to the
            // main view) would free that view's objects. Only tear down with
            // our own context current. (I4)
            let widget = self.obj();
            widget.make_current();
            if widget.error().is_none() && widget.context().is_some() {
                unsafe {
                    self.teardown_gl();
                }
            } else {
                // Our names die with the context GTK is about to destroy.
                self.forget_gl_names();
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
            self.sync_viewport(width, height, "size_allocate");

            // M21: gate the mobile single-pane layout on logical width. The v3
            // script can't un-apply its DOM/CSS, so a threshold crossing reloads
            // and `on_load_end` re-injects (or not) — debounced, so dragging
            // across the threshold reloads once at the end, not per frame.
            if width > 0
                && let Some(browser) = resolved_browser(self)
                && super::mobile_layout_mismatch(browser.identifier(), width)
            {
                self.schedule_mobile_layout_check();
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
            {
                // Keep every handler until it reports closed (`is_browser_closed`).
                let mut closing = self.closing_life_spans.lock();
                closing.extend(self.life_spans.lock().drain().map(|(_, l)| l));
                if let Some(l) = self.life_span.lock().take() {
                    closing.push(l);
                }
            }
            self.cdp_registrations.borrow_mut().clear();
            self.pending_contexts.lock().clear();
            *self.foreground.lock() = None;
            *self.wanted_foreground.lock() = None;
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
            self.sync_viewport(widget.width(), widget.height(), "refresh_scale");
            widget.queue_render();
        }

        /// Follow the native surface's scale (e.g. dragging between monitors of
        /// different scale) so the physical paint buffer tracks it. The `scale`
        /// notify fires across the integer boundaries the OSR buffer cares
        /// about; a pure fractional change leaves the integer paint scale
        /// unchanged, so `refresh_screen_scale` is then a no-op. Connected once
        /// per surface — a re-realize on the same surface must not stack a
        /// second handler — and disconnected on unrealize. (#155, #158, I11)
        fn watch_surface_scale(&self) {
            let widget = self.obj();
            let Some(surface) = widget.native().and_then(|n| n.surface()) else {
                return;
            };
            let mut slot = self.scale_notify.borrow_mut();
            if slot.as_ref().is_some_and(|(watched, _)| *watched == surface) {
                return;
            }
            if let Some((old, id)) = slot.take() {
                old.disconnect(id);
            }
            let id = surface.connect_scale_notify(glib::clone!(
                #[weak]
                widget,
                move |_s| widget.imp().refresh_screen_scale()
            ));
            *slot = Some((surface, id));
        }

        /// Publish the physical viewport for a logical `width`×`height` at the
        /// current integer paint scale (a non-positive size keeps the old one)
        /// and tell CEF. CEF is left alone when neither the physical size nor
        /// the scale changed — GTK re-allocates far more often than either
        /// moves (P3). A scale change re-applies zoom so the display-scale term
        /// tracks it, keeping buffer and input transform coupled (#158/#176).
        fn sync_viewport(&self, width: i32, height: i32, why: &str) {
            let widget = self.obj();
            let scale = paint_scale(&widget);
            let Some(shared) = self.shared.lock().clone() else {
                return;
            };
            let (old_size, old_scale, new_size) = {
                let mut s = shared.lock();
                let old = (s.size, s.scale_factor as f64);
                if width > 0 && height > 0 {
                    s.size = (
                        (width as f64 * scale).round() as i32,
                        (height as f64 * scale).round() as i32,
                    );
                }
                s.scale_factor = scale as f32;
                (old.0, old.1, s.size)
            };
            let scale_changed = (scale - old_scale).abs() > f64::EPSILON;
            log::trace!(
                "coord: {why} logical={width}x{height} scale={scale:.3} physical={}x{} old_scale={old_scale:.3} old_physical={}x{}",
                new_size.0,
                new_size.1,
                old_size.0,
                old_size.1
            );
            if new_size == old_size && !scale_changed {
                return;
            }
            // Lock released: the X11 query talks to the server.
            let origin = window_origin_for(&widget);
            shared.lock().window_origin = origin;
            if let Some(browser) = resolved_browser(self)
                && let Some(host) = browser.host()
            {
                host.notify_screen_info_changed();
                host.was_resized();
                if scale_changed {
                    super::apply_zoom_from_account(&browser, scale);
                }
            }
        }

        /// Debounced mobile-layout re-check (D5): ~400 ms after the last call,
        /// reload the foreground page if it was loaded with the other layout.
        /// Every call restarts the wait, so a window dragged across the
        /// threshold reloads at most once, after it settles.
        fn schedule_mobile_layout_check(&self) {
            if let Some(id) = self.mobile_check.take() {
                id.remove();
            }
            let weak = self.obj().downgrade();
            let id = glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
                if let Some(widget) = weak.upgrade() {
                    let imp = widget.imp();
                    // Fired: the id is spent, never `remove()` it.
                    drop(imp.mobile_check.take());
                    imp.check_mobile_layout();
                }
            });
            *self.mobile_check.borrow_mut() = Some(id);
        }

        fn check_mobile_layout(&self) {
            let width = self.obj().width();
            if width <= 0 {
                return;
            }
            let Some(browser) = resolved_browser(self) else {
                return;
            };
            let id = browser.identifier();
            if super::mobile_layout_mismatch(id, width) {
                log::info!("mobile layout changed at {width}px; reloading browser {id}");
                // Record the new layout now so another check before the reload
                // finishes does not reload again; on_load_end re-records it.
                super::record_mobile_layout(id, super::should_use_mobile_layout(width));
                browser.reload();
            }
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
                            // CEF's windowless Paste only sees Chromium's internal
                            // clipboard, never GDK's. Route it through the same host
                            // paste path as Ctrl+V; cancel so CEF doesn't paste
                            // stale internal content on top. (#189)
                            Some(id)
                                if crate::handlers::context_menu::is_paste_command(id)
                                    && try_intercept_paste(&obj) =>
                            {
                                log::debug!("context menu: host paste, cancel(id={id})");
                                cb.cancel();
                            }
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
                log::trace!(
                    "coord: J7 set_zoom_linear account={} user_linear={:.3} display_scale={:.3} cef_level={:.3}",
                    acct,
                    linear,
                    display_scale,
                    cef_level
                );
                if let Some(host) = browser.host() {
                    host.set_zoom_level(cef_level);
                }
            } else {
                log::trace!(
                    "coord: J7 set_zoom_linear account=none user_linear={:.3} display_scale={:.3} cef_level={:.3} (no browser)",
                    linear,
                    display_scale,
                    cef_level
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
            // Every pooled account: each has its own RequestContext, so
            // applying only to the foreground left background accounts on the
            // old dictionary until restart. (N7)
            let mut targets: Vec<Browser> = self.browsers.lock().values().cloned().collect();
            if targets.is_empty()
                && let Some(browser) = self.browser.lock().as_ref()
            {
                targets.push(browser.clone()); // DevTools / legacy single browser
            }
            if targets.is_empty() {
                log::warn!("set_spellcheck_languages: no live browser");
                return;
            }
            // Live switch: never use the [] force-clear (tears down OSR spellcheck).
            for browser in &targets {
                super::apply_spellcheck_to_browser(browser, langs, enabled);
            }
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

        /// Apply the cursor, screen-reader and IM-focus requests the CEF
        /// handlers left in the shared state. Runs from the idle callback
        /// `handlers::wake_view` queues (`debounce_tick` false), and from the
        /// IM focus-out debounce timer (`debounce_tick` true), never inside a
        /// CEF callback or with the shared lock held across GTK calls.
        pub(super) fn flush_requests(&self, debounce_tick: bool) {
            let w = self.obj();
            let Some(shared) = self.shared.lock().clone() else {
                return;
            };
            let (dirty, cursor_name, keyboard_request, announce) = {
                let mut s = shared.lock();
                if !debounce_tick {
                    s.view_wake_queued = false;
                }
                let dirty = s.frame.dirty || s.accel.as_ref().is_some_and(|a| a.dirty);
                let cursor = if s.cursor_dirty {
                    s.cursor_dirty = false;
                    Some(s.cursor_name)
                } else {
                    None
                };
                (dirty, cursor, s.keyboard_request.take(), s.a11y_announce.take())
            };
            // Paints queue their own render; this only covers a frame still
            // waiting for one.
            if dirty {
                w.queue_render();
            }
            if let Some(name) = cursor_name {
                w.set_cursor_from_name(Some(name));
            }
            // Page focus moved under a screen reader (#204).
            if let Some(text) = announce {
                gtk::prelude::AccessibleExt::announce(
                    &*w,
                    &text,
                    gtk::AccessibleAnnouncementPriority::Medium,
                );
            }
            // Editable focus changed in the page → focus the IM context so the
            // on-screen keyboard shows only for real text fields. Focus-out is
            // debounced so a composer re-render's false/true pair never leaves
            // the IM unfocused across the next keystroke (#180). The countdown
            // only advances on debounce ticks, so an unrelated flush (cursor,
            // announcement) cannot shorten it.
            if keyboard_request.is_none() && !debounce_tick {
                return;
            }
            let Some(im) = self.im_context.borrow().clone() else {
                return;
            };
            let mut pending = self.im_focus_out_pending.get();
            let action = im_focus_step(keyboard_request, self.im_focused.get(), &mut pending);
            self.im_focus_out_pending.set(pending);
            match action {
                ImFocusAction::FocusIn => {
                    im.focus_in();
                    self.im_focused.set(true);
                }
                ImFocusAction::FocusOut => {
                    im.focus_out();
                    self.im_focused.set(false);
                }
                ImFocusAction::Idle => {}
            }
            if pending > 0 {
                self.arm_im_focus_timer();
            }
        }

        /// Tick the IM focus-out debounce every `IM_FOCUS_TICK` until it has
        /// settled (applied or cancelled), then stop. At most one runs.
        fn arm_im_focus_timer(&self) {
            if self.im_focus_timer_armed.replace(true) {
                return;
            }
            let weak = self.obj().downgrade();
            glib::timeout_add_local(IM_FOCUS_TICK, move || {
                let Some(w) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                let imp = w.imp();
                imp.flush_requests(true);
                if imp.im_focus_out_pending.get() > 0 {
                    glib::ControlFlow::Continue
                } else {
                    // Cleared before returning Break: the source is then gone,
                    // so nothing may hold or remove its id.
                    imp.im_focus_timer_armed.set(false);
                    glib::ControlFlow::Break
                }
            });
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

            if self.shutting_down.load(Ordering::Acquire) {
                return; // quitting: a late spawn would never be closed
            }

            if let Some(id) = account_id.as_ref() {
                if make_foreground {
                    // Remembered until the browser exists (switch_to/adopt clears
                    // it), so a still-initialising context still takes over.
                    *self.wanted_foreground.lock() = Some(id.clone());
                }
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
            if self.shutting_down.load(Ordering::Acquire) {
                log::info!("context ready for {account_id} during quit; not creating a browser");
                self.pending_contexts.lock().remove(account_id);
                return;
            }
            // The latest foreground request wins over the flag captured at spawn
            // time: a switch to this account while it was initialising must show
            // it, and a switch away from it must not be undone now.
            let make_foreground = {
                let mut wanted = self.wanted_foreground.lock();
                if wanted.as_deref() == Some(account_id) {
                    *wanted = None;
                    true
                } else {
                    make_foreground && wanted.is_none() && self.foreground.lock().is_none()
                }
            };
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

            // A launch URL (deep link, --url) goes to the foreground browser only.
            let url_string = if make_foreground || account_id.is_none() {
                self.pending_url.lock().take()
            } else {
                None
            }
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
                // An open composition belongs to the outgoing page too. (I1)
                reset_ime(&self.obj());
            }

            // A launch URL (deep link) that arrived while no foreground browser
            // existed and was not consumed at creation.
            if account_id.is_some()
                && let Some(url) = self.pending_url.lock().take()
                && let Some(frame) = browser.main_frame()
            {
                frame.load_url(Some(&CefString::from(url.as_str())));
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
            {
                let mut wanted = self.wanted_foreground.lock();
                if wanted.is_some() && *wanted == account_id {
                    *wanted = None;
                }
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
            // A background account may have loaded at another width; bring
            // its layout in line now that it is the one on screen. (D5)
            self.schedule_mobile_layout_check();
        }

        /// Switch the foreground to the account `new_id`, pausing the previous
        /// foreground and resuming the target. No-op if already foreground or the
        /// target has no spawned browser.
        pub fn switch_to(&self, new_id: &str) {
            if self.foreground.lock().as_deref() == Some(new_id) {
                let mut wanted = self.wanted_foreground.lock();
                if wanted.as_deref() == Some(new_id) {
                    *wanted = None;
                }
                return;
            }
            // Resolve the target before touching the current foreground, so a
            // missing target leaves the visible account showing.
            let Some(browser) = self.browsers.lock().get(new_id).cloned() else {
                log::warn!("switch_to: no spawned browser for account {new_id}");
                return;
            };
            let life = self.life_spans.lock().get(new_id).cloned();
            let Some(life) = life else {
                log::warn!("switch_to: no life-span handler for account {new_id}");
                return;
            };
            if let Some(prev) = self.browser.lock().as_ref()
                && let Some(host) = prev.host()
            {
                host.was_hidden(1);
            }
            self.adopt_foreground(Some(new_id.to_owned()), browser, life);
        }

        /// Close and drop the browser for `account_id` (the `remove(id)` path).
        pub fn close_account_browser(&self, account_id: &str) {
            let was_foreground = self.foreground.lock().as_deref() == Some(account_id);
            {
                let mut wanted = self.wanted_foreground.lock();
                if wanted.as_deref() == Some(account_id) {
                    *wanted = None;
                }
            }
            // Quit still waits on this browser if it has not finished closing.
            let life = self.life_spans.lock().remove(account_id);
            if let Some(life) = life {
                self.closing_life_spans.lock().push(life);
            }
            // Detach the notification bridge (drop = remove observer).
            self.cdp_registrations.borrow_mut().remove(account_id);
            if let Some(browser) = self.browsers.lock().remove(account_id) {
                crate::accounts::unregister_browser(browser.identifier());
                if let Some(host) = browser.host() {
                    host.close_browser(1);
                }
            }
            if was_foreground {
                reset_ime(&self.obj());
                let id = self.browser_id.swap(0, Ordering::Relaxed);
                if id != 0 {
                    super::unregister_context_menu_widget(id);
                }
                *self.browser.lock() = None;
                *self.life_span.lock() = None;
                *self.foreground.lock() = None;
                if let Some(shared) = self.shared.lock().as_ref() {
                    let mut s = shared.lock();
                    s.foreground_browser_id = 0;
                    s.popup = Default::default();
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

            // GPU-OSR: a second texture the imported DMA-BUF EGLImage targets,
            // plus the popup layer's CPU and DMA-BUF targets.
            unsafe {
                self.accel_tex.store(new_texture(), Ordering::Relaxed);
                self.popup_tex.store(new_texture(), Ordering::Relaxed);
                self.accel_popup_tex.store(new_texture(), Ordering::Relaxed);
            }
            self.popup_tex_w.store(0, Ordering::Relaxed);
            self.popup_tex_h.store(0, Ordering::Relaxed);
        }

        /// Drop every GL name without deleting it, for when our context cannot
        /// be made current (the objects go with the context). EGLImages need
        /// only their display, so they are still released properly. (I4)
        fn forget_gl_names(&self) {
            *self.imported.borrow_mut() = None;
            *self.imported_popup.borrow_mut() = None;
            for name in [
                &self.texture,
                &self.accel_tex,
                &self.popup_tex,
                &self.accel_popup_tex,
                &self.vbo,
                &self.vao,
                &self.program,
            ] {
                name.store(0, Ordering::Relaxed);
            }
            for size in [&self.tex_w, &self.tex_h, &self.popup_tex_w, &self.popup_tex_h] {
                size.store(0, Ordering::Relaxed);
            }
        }

        /// Delete our GL objects. The GLArea context must be current.
        unsafe fn teardown_gl(&self) {
            // Release the live EGLImages while the GL context is still current.
            *self.imported.borrow_mut() = None;
            *self.imported_popup.borrow_mut() = None;
            unsafe {
                for name in [
                    &self.texture,
                    &self.accel_tex,
                    &self.popup_tex,
                    &self.accel_popup_tex,
                ] {
                    let tex = name.swap(0, Ordering::Relaxed);
                    if tex != 0 {
                        gl::DeleteTextures(1, &tex);
                    }
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
                // GtkGLArea's default `resize` handler sets the viewport to its
                // framebuffer. Read it back so layers can be placed (and the
                // full viewport restored) without re-deriving GTK's scale.
                let mut viewport: [GLint; 4] = [0; 4];
                gl::GetIntegerv(gl::VIEWPORT, viewport.as_mut_ptr());

                // Theme-matched page background, not black: it shows around a
                // frame that does not cover the widget yet (resize, startup).
                let [r, g, b] = super::page_background(&*self.obj());
                gl::ClearColor(r, g, b, 1.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);

                // A CPU paint drops the accelerated frame (render.rs on_paint);
                // release its EGLImage here, with the context current, so the
                // CPU frame is what gets drawn rather than the last DMA-BUF. A
                // hidden popup drops its import the same way. (I5)
                if s.accel.is_none() {
                    *self.imported.borrow_mut() = None;
                }
                if s.popup.accel.is_none() {
                    *self.imported_popup.borrow_mut() = None;
                }

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
                    s.accel = None;
                    *self.imported.borrow_mut() = None;
                    // CEF's shared-texture choice is fixed at browser creation.
                    // Recreate the pool after this render callback so subsequent
                    // callbacks are CPU on_paint rather than more unusable DMA-BUFs.
                    self.schedule_software_osr_fallback();
                }

                // Popup DMA-BUF. A rejected one only costs the popup; the view
                // import above is what decides the software fallback.
                let aptex = self.accel_popup_tex.load(Ordering::Relaxed);
                let mut popup_failed = false;
                if let Some(pf) = s.popup.accel.as_mut()
                    && pf.dirty
                {
                    if let Some(img) = crate::gl_dmabuf::import_to_texture(
                        aptex,
                        pf.width,
                        pf.height,
                        pf.fourcc,
                        pf.modifier,
                        &pf.planes,
                    ) {
                        *self.imported_popup.borrow_mut() = Some(img);
                        pf.dirty = false;
                    } else {
                        popup_failed = true;
                    }
                }
                if popup_failed {
                    log::warn!("accelerated popup import failed; popup not shown");
                    s.popup.accel = None;
                    *self.imported_popup.borrow_mut() = None;
                }

                let accel_view = if self.imported.borrow().is_some() {
                    s.accel.as_ref().map(|af| {
                        let layer = Layer {
                            tex: atex,
                            bgra: 0,
                            uv: super::texture_uv(af.visible, (af.width, af.height)),
                        };
                        (layer, (af.visible.2, af.visible.3))
                    })
                } else {
                    None
                };
                let (view, view_size) = match accel_view {
                    Some(v) => v,
                    None => {
                        // Software path: upload CEF's CPU BGRA buffer through the
                        // valid GLES context (software OSR does not eliminate GLArea).
                        let tex = self.texture.load(Ordering::Relaxed);
                        if !upload_cpu_frame(tex, &self.tex_w, &self.tex_h, &mut s.frame) {
                            return;
                        }
                        let layer = Layer {
                            tex,
                            bgra: 1,
                            uv: FULL_UV,
                        };
                        (layer, (s.frame.width, s.frame.height))
                    }
                };

                // Popup layer (`<select>` dropdowns), CPU or DMA-BUF, drawn over
                // the view at CEF's popup rect.
                let mut popup = None;
                if let Some(rect) = s.popup.visible_rect(s.foreground_browser_id) {
                    let accel_popup = if self.imported_popup.borrow().is_some() {
                        s.popup.accel.as_ref().map(|pf| Layer {
                            tex: aptex,
                            bgra: 0,
                            uv: super::texture_uv(pf.visible, (pf.width, pf.height)),
                        })
                    } else {
                        None
                    };
                    popup = match accel_popup {
                        Some(layer) => Some((layer, rect)),
                        None => {
                            let ptex = self.popup_tex.load(Ordering::Relaxed);
                            upload_cpu_frame(
                                ptex,
                                &self.popup_tex_w,
                                &self.popup_tex_h,
                                &mut s.popup.frame,
                            )
                            .then_some((
                                Layer {
                                    tex: ptex,
                                    bgra: 1,
                                    uv: FULL_UV,
                                },
                                rect,
                            ))
                        }
                    };
                }
                let view_rect_size = s.size;
                drop(s);

                let framebuffer = if viewport[2] > 0 && viewport[3] > 0 {
                    (viewport[2], viewport[3])
                } else {
                    view_size
                };
                let prog = self.program.load(Ordering::Relaxed);
                gl::UseProgram(prog);
                let locs = (
                    gl::GetUniformLocation(prog, c"u_bgra".as_ptr() as *const _),
                    gl::GetUniformLocation(prog, c"u_uv".as_ptr() as *const _),
                );
                gl::BindVertexArray(self.vao.load(Ordering::Relaxed));
                gl::ActiveTexture(gl::TEXTURE0);
                let place = |rect| super::gl_viewport_for(framebuffer, view_rect_size, rect);
                draw_layer(&view, place((0, 0, view_size.0, view_size.1)), locs);
                if let Some((layer, rect)) = popup {
                    draw_layer(&layer, place(rect), locs);
                }
                gl::BindVertexArray(0);
                gl::UseProgram(0);
                if viewport[2] > 0 && viewport[3] > 0 {
                    gl::Viewport(viewport[0], viewport[1], viewport[2], viewport[3]);
                }
            }
        }
    }

    /// One textured quad of the GL present path.
    struct Layer {
        tex: GLuint,
        /// `u_bgra`: 1 = CPU BGRA bytes uploaded as RGBA, 0 = DMA-BUF import.
        bgra: GLint,
        /// `u_uv`: sampled sub-rect of the texture, `[x, y, w, h]` fractions.
        uv: [f32; 4],
    }

    const FULL_UV: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

    /// Draw `layer` into the GL `viewport` rect. Program + VAO bound by the caller.
    unsafe fn draw_layer(layer: &Layer, viewport: (i32, i32, i32, i32), locs: (GLint, GLint)) {
        let (x, y, w, h) = viewport;
        unsafe {
            gl::Viewport(x, y, w, h);
            if locs.0 >= 0 {
                gl::Uniform1i(locs.0, layer.bgra);
            }
            if locs.1 >= 0 {
                let [u, v, uw, vh] = layer.uv;
                gl::Uniform4f(locs.1, u, v, uw, vh);
            }
            gl::BindTexture(gl::TEXTURE_2D, layer.tex);
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);
        }
    }

    /// Bring `tex` (currently holding `tex_w`×`tex_h`) up to date with the CPU
    /// `frame`, uploading only what changed. Returns false when there is no
    /// frame to draw. Context must be current.
    unsafe fn upload_cpu_frame(
        tex: GLuint,
        tex_w: &AtomicI32,
        tex_h: &AtomicI32,
        frame: &mut crate::handlers::FrameBuffer,
    ) -> bool {
        let size = (tex_w.load(Ordering::Relaxed), tex_h.load(Ordering::Relaxed));
        unsafe {
            match super::cpu_upload(frame, size) {
                super::CpuUpload::Empty => return false,
                super::CpuUpload::Allocate => {
                    gl::BindTexture(gl::TEXTURE_2D, tex);
                    gl::TexImage2D(
                        gl::TEXTURE_2D,
                        0,
                        gl::RGBA8 as GLint,
                        frame.width,
                        frame.height,
                        0,
                        gl::RGBA,
                        gl::UNSIGNED_BYTE,
                        frame.pixels.as_ptr() as *const c_void,
                    );
                    tex_w.store(frame.width, Ordering::Relaxed);
                    tex_h.store(frame.height, Ordering::Relaxed);
                    frame.dirty = false;
                }
                super::CpuUpload::Update => {
                    gl::BindTexture(gl::TEXTURE_2D, tex);
                    // Upload only what CEF reported as damaged. Typing
                    // dirties a caret-sized rect; re-sending the whole
                    // window every frame is what makes CPU OSR stutter
                    // on integrated GPUs (#179/#180). GLES 3.0 gives us
                    // the UNPACK_* row addressing to do it in one call.
                    let (x, y, w, h) = frame.damage.unwrap_or((0, 0, frame.width, frame.height));
                    if (x, y, w, h) != (0, 0, frame.width, frame.height) {
                        gl::PixelStorei(gl::UNPACK_ROW_LENGTH, frame.width);
                        gl::PixelStorei(gl::UNPACK_SKIP_PIXELS, x);
                        gl::PixelStorei(gl::UNPACK_SKIP_ROWS, y);
                    }
                    gl::TexSubImage2D(
                        gl::TEXTURE_2D,
                        0,
                        x,
                        y,
                        w,
                        h,
                        gl::RGBA,
                        gl::UNSIGNED_BYTE,
                        frame.pixels.as_ptr() as *const c_void,
                    );
                    // Leave the unpack state as every other caller expects.
                    gl::PixelStorei(gl::UNPACK_ROW_LENGTH, 0);
                    gl::PixelStorei(gl::UNPACK_SKIP_PIXELS, 0);
                    gl::PixelStorei(gl::UNPACK_SKIP_ROWS, 0);
                    frame.dirty = false;
                }
                super::CpuUpload::Reuse => {}
            }
        }
        true
    }

    /// A linear-filtered, edge-clamped `GL_TEXTURE_2D`. Context must be current.
    unsafe fn new_texture() -> GLuint {
        let mut tex = 0;
        unsafe {
            gl::GenTextures(1, &mut tex);
            gl::BindTexture(gl::TEXTURE_2D, tex);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
        tex
    }

    /// Software present path: the cached texture for `frame`, rebuilt only
    /// when CEF painted since the last snapshot. CEF hands back premultiplied
    /// BGRA, which is a GdkMemoryTexture format verbatim, so a rebuild is the
    /// one copy `Bytes` makes. (P6)
    fn cached_frame_texture(
        cache: &RefCell<Option<gtk::gdk::MemoryTexture>>,
        frame: &mut crate::handlers::FrameBuffer,
    ) -> Option<gtk::gdk::MemoryTexture> {
        let mut slot = cache.borrow_mut();
        if frame.width <= 0 || frame.height <= 0 || frame.pixels.is_empty() {
            *slot = None;
            return None;
        }
        if frame.dirty || slot.is_none() {
            *slot = Some(gtk::gdk::MemoryTexture::new(
                frame.width,
                frame.height,
                gtk::gdk::MemoryFormat::B8g8r8a8Premultiplied,
                &glib::Bytes::from(&frame.pixels[..]),
                (frame.width * 4) as usize,
            ));
            frame.dirty = false;
        }
        slot.clone()
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
        Motion {
            x: f64,
            y: f64,
            modifiers: u32,
        },
        Release {
            button: u32,
            x: f64,
            y: f64,
            modifiers: u32,
        },
        Cancel {
            x: f64,
            y: f64,
        },
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
        Move {
            x: f64,
            y: f64,
            modifiers: u32,
        },
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
                MouseInput::Release {
                    button,
                    x,
                    y,
                    modifiers,
                } => self
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
            self.last_click = Some(ClickHistory {
                button,
                x,
                y,
                time,
                count,
            });
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
                let imp = widget.imp();
                let mut state = imp.ime_state.get();
                let route = state.commit();
                imp.ime_state.set(state);
                match route {
                    // Text that ends a composition replaces the page's composing
                    // range (or inserts at the caret if it was already cleared).
                    ImeCommit::Composition => {
                        let text = CefString::from(text);
                        with_host(&widget, |host| {
                            host.ime_commit_text(Some(&text), Some(&ime_invalid_range()), 0)
                        });
                    }
                    ImeCommit::Chars => {
                        // CEF's `character` field is UTF-16. `ch as u16` truncated
                        // anything above the BMP — an emoji committed by the IM
                        // (U+1F600) arrived as U+F600 — so encode properly and send
                        // each surrogate in turn.
                        let mut buf = [0u16; 2];
                        for ch in text.chars() {
                            for unit in ch.encode_utf16(&mut buf) {
                                send_char(&widget, *unit, 0);
                            }
                        }
                    }
                }
            }
        ));
        // Composition (ibus/fcitx CJK, dead keys): mirror the IM's preedit into
        // the page as a CEF IME composition so it renders inline, and so the
        // key handler knows to hand keys to the IM first while it is active. (I1)
        im.connect_preedit_start(glib::clone!(
            #[weak]
            widget,
            move |_im| {
                let imp = widget.imp();
                let mut state = imp.ime_state.get();
                state.preedit_start();
                imp.ime_state.set(state);
            }
        ));
        im.connect_preedit_changed(glib::clone!(
            #[weak]
            widget,
            move |im| {
                let (text, _attrs, cursor) = im.preedit_string();
                let imp = widget.imp();
                let mut state = imp.ime_state.get();
                let action = state.preedit_changed(text.is_empty());
                imp.ime_state.set(state);
                apply_ime_action(&widget, action, &text, cursor);
                update_ime_cursor_location(&widget);
            }
        ));
        im.connect_preedit_end(glib::clone!(
            #[weak]
            widget,
            move |_im| {
                let imp = widget.imp();
                let mut state = imp.ime_state.get();
                let action = state.preedit_end();
                imp.ime_state.set(state);
                apply_ime_action(&widget, action, "", 0);
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
        // Whether CEF holds a pressed touch point: a cancelled sequence is
        // followed by GTK's drag-end, which must not also send RELEASED. (I7)
        let touch_down = std::rc::Rc::new(std::cell::Cell::new(false));
        drag.connect_drag_begin(glib::clone!(
            #[weak]
            widget,
            #[strong]
            touch_down,
            move |_g, x, y| {
                super::note_user_input();
                widget.grab_focus();
                set_focus(&widget, true);
                touch_down.set(true);
                send_touch(&widget, 0, x, y, TouchEventType::PRESSED);
            }
        ));
        drag.connect_drag_update(glib::clone!(
            #[weak]
            widget,
            #[strong]
            touch_down,
            move |g, ox, oy| {
                if !touch_down.get() {
                    return;
                }
                super::note_user_input();
                if let Some((sx, sy)) = g.start_point() {
                    send_touch(&widget, 0, sx + ox, sy + oy, TouchEventType::MOVED);
                }
            }
        ));
        drag.connect_drag_end(glib::clone!(
            #[weak]
            widget,
            #[strong]
            touch_down,
            move |g, ox, oy| {
                if !touch_down.replace(false) {
                    return;
                }
                if let Some((sx, sy)) = g.start_point() {
                    send_touch(&widget, 0, sx + ox, sy + oy, TouchEventType::RELEASED);
                }
            }
        ));
        // A sequence GTK takes back (another gesture claimed it, the grab
        // broke) must be cancelled in CEF too, or the page keeps a stuck
        // pressed touch. (I7)
        drag.connect_cancel(glib::clone!(
            #[weak]
            widget,
            #[strong]
            touch_down,
            move |g, _sequence| {
                if !touch_down.replace(false) {
                    return;
                }
                let (sx, sy) = g.start_point().unwrap_or((0.0, 0.0));
                let (ox, oy) = g.offset().unwrap_or((0.0, 0.0));
                send_touch(&widget, 0, sx + ox, sy + oy, TouchEventType::CANCELLED);
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
            #[weak]
            widget,
            #[strong]
            mouse_buttons,
            move |ctrl, x, y| {
                if !should_forward_mouse(is_pointer_emulated(ctrl)) {
                    return; // touch handled above; don't double-feed as mouse
                }
                let modifiers = modifiers_from_state(ctrl.current_event_state());
                // A drag-select is user input for as long as it lasts: the PRIMARY
                // mirror it triggers must pass the clipboard gesture gate. (S9)
                if mouse_buttons.borrow().active_modifiers() != 0 {
                    super::note_user_input();
                }
                for event in
                    mouse_buttons
                        .borrow_mut()
                        .handle(MouseInput::Motion { x, y, modifiers })
                {
                    dispatch_mouse(&widget, event);
                }
            }
        ));
        motion.connect_leave(glib::clone!(
            #[weak]
            widget,
            #[strong]
            mouse_buttons,
            move |ctrl| {
                let modifiers = modifiers_from_state(ctrl.current_event_state())
                    | mouse_buttons.borrow().active_modifiers();
                send_move(&widget, 0.0, 0.0, modifiers, true);
            }
        ));
        widget.add_controller(motion);

        let buttons = gtk::EventControllerLegacy::new();
        buttons.connect_event(glib::clone!(
            #[weak]
            widget,
            #[strong]
            im,
            #[strong]
            mouse_buttons,
            #[upgrade_or]
            glib::Propagation::Proceed,
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
                // EventControllerLegacy hands out raw surface coords; CEF needs
                // widget coords (GestureClick used to do this mapping; KARE-002 regression).
                let Some((x, y)) = event
                    .position()
                    .and_then(|(sx, sy)| surface_to_widget(&widget, sx, sy))
                else {
                    return glib::Propagation::Proceed;
                };
                let modifiers = modifiers_from_state(event.modifier_state());
                match event.event_type() {
                    EventType::ButtonPress => {
                        super::note_user_input();
                        let settings = widget.settings();
                        widget.grab_focus();
                        set_focus(&widget, true);
                        // Clicking moves the caret away from an open composition;
                        // end it instead of leaving the IM composing elsewhere. (I1)
                        if widget.imp().ime_state.get().filter_first() {
                            reset_ime(&widget);
                        }
                        im.set_cursor_location(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1));
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
                            // The page hit-tests in CSS px, not widget px. (I8)
                            let (px, py) = widget_to_page(&widget, x, y);
                            read_primary_clipboard_paste(&widget, px, py);
                        }
                        for event in events {
                            dispatch_mouse(&widget, event);
                        }
                    }
                    EventType::ButtonRelease => {
                        super::note_user_input();
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
            #[strong]
            mouse_buttons,
            move |widget| release_stuck_mouse_buttons(widget, &mouse_buttons)
        ));

        // Scroll -------------------------------------------------------------
        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
        let zoom_accum = std::cell::Cell::new(0.0_f64);
        scroll.connect_scroll(glib::clone!(
            #[weak]
            widget,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |ctrl, dx, dy| {
                let state = ctrl.current_event_state();
                // Surface unit = pixel deltas (touchpad); Wheel unit = notch
                // clicks (mouse). Scaled differently in send_wheel. (#161)
                let precise = ctrl.unit() == gtk::gdk::ScrollUnit::Surface;
                // Ctrl+wheel is the window's zoom, not the page's: CEF would zoom
                // the browser behind the per-account level. One action per notch;
                // smooth deltas accumulate to a notch's worth first. (I10)
                if state.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                    let threshold = if precise { ZOOM_SCROLL_SURFACE_STEP } else { 1.0 };
                    let mut accum = zoom_accum.get();
                    let steps = zoom_scroll_steps(&mut accum, dy, threshold);
                    zoom_accum.set(accum);
                    let action = if steps > 0 { "win.zoom-in" } else { "win.zoom-out" };
                    for _ in 0..steps.unsigned_abs() {
                        if let Err(err) = widget.activate_action(action, None) {
                            log::debug!("ctrl+scroll: {action}: {err}");
                            break;
                        }
                    }
                    return glib::Propagation::Stop;
                }
                zoom_accum.set(0.0);
                send_wheel(&widget, dx, dy, modifiers_from_state(state), precise);
                glib::Propagation::Stop
            }
        ));
        widget.add_controller(scroll);

        // Keyboard -----------------------------------------------------------
        // Every press sends its raw key-down first, then goes through the IM,
        // whose `commit` is the only source of text for a key it consumes. (#154)
        // While a composition is open the IM gets the key first instead, and a
        // key it consumes never reaches the page (Enter picks a candidate, it
        // does not send the message). (I1)
        let keys = gtk::EventControllerKey::new();
        // Hardware keycodes whose press the IM swallowed during composition;
        // their release is not forwarded either, so CEF sees no orphan key-up.
        let swallowed_keys = std::rc::Rc::new(RefCell::new(Vec::<u32>::new()));
        keys.connect_key_pressed(glib::clone!(
            #[weak]
            widget,
            #[strong]
            im,
            #[strong]
            swallowed_keys,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |ctrl, keyval, keycode, state| {
                use gtk::gdk::{Key, ModifierType};
                super::note_user_input();
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
                dispatch_explicit_copy(ExplicitCopyTrigger::Keyboard { keyval, state }, || {
                    request_live_selection(&widget)
                });
                // A physical keypress means the user is typing — if the page's
                // editable-focus signal left the IM focused-out (post-send input
                // re-render), dead keys silently stop composing. Re-focus first. (#154)
                let imp = widget.imp();
                // A keystroke means the field is live: drop any focus-out the page
                // queued, so the debounce window can't expire mid-word (#180).
                imp.im_focus_out_pending.set(0);
                if !imp.im_focused.get() {
                    im.focus_in();
                    imp.im_focused.set(true);
                }
                let layout = ctrl.group();
                let mut ime = imp.ime_state.get();
                ime.key_press_begin();
                imp.ime_state.set(ime);
                if ime.filter_first() {
                    // Composing: the IM drives the candidate window with this
                    // key (Enter, Backspace, arrows, Space). Only a key it
                    // declines is the page's.
                    update_ime_cursor_location(&widget);
                    let consumed = ctrl
                        .current_event()
                        .map(|e| im.filter_keypress(&e))
                        .unwrap_or(false);
                    if consumed {
                        let mut keys = swallowed_keys.borrow_mut();
                        if !keys.contains(&keycode) {
                            keys.push(keycode);
                        }
                        drop(keys);
                        return glib::Propagation::Stop;
                    }
                    send_key_raw(&widget, keyval, keycode, layout, state, true);
                } else {
                    // The key-down goes first, for text keys too. Chromium drops
                    // the CHAR that follows a key-down the page handled, and only
                    // the next key-down lifts that; a letter sent as a bare CHAR
                    // while Backspace or an arrow was still held was therefore
                    // swallowed. It carries no character, so it inserts nothing
                    // by itself. (#180)
                    send_key_raw(&widget, keyval, keycode, layout, state, true);
                    // A consumed key is text (a letter, or a dead-key composition
                    // starting) — the `commit` handler emits the CHAR, which runs
                    // inside this call, so nothing else is sent.
                    let consumed = ctrl
                        .current_event()
                        .map(|e| im.filter_keypress(&e))
                        .unwrap_or(false);
                    if consumed {
                        return glib::Propagation::Stop;
                    }
                }
                // A declined printable key means no `commit` is coming, and a
                // RAWKEYDOWN alone inserts nothing — the character is silently
                // dropped. Happens when the IM is momentarily not focused: the
                // page's editable-focus signal focuses it out on composer
                // re-renders, and re-focusing an async IM (ibus) can land after
                // this key. Synthesize the CHAR ourselves. (#180)
                if let Some(unit) = printable_char(keyval, state) {
                    send_char(&widget, unit, modifiers_from_state(state));
                } else if let Some(unit) = enter_char(keyval, state) {
                    // Enter's CHAR is what inserts a newline in a <textarea> and
                    // submits an <input>; no IM commits one. If the page handled
                    // the key-down (the composer's send), Chromium drops it.
                    send_char(&widget, unit, modifiers_from_state(state));
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
            #[weak]
            widget,
            #[strong]
            im,
            #[strong]
            swallowed_keys,
            move |ctrl, keyval, keycode, state| {
                // Mirror the press path: if the IM consumes the release, it
                // belongs to composition — don't also send a raw key-up.
                let consumed = ctrl
                    .current_event()
                    .map(|e| im.filter_keypress(&e))
                    .unwrap_or(false);
                let swallowed = {
                    let mut keys = swallowed_keys.borrow_mut();
                    match keys.iter().position(|k| *k == keycode) {
                        Some(i) => {
                            keys.swap_remove(i);
                            true
                        }
                        None => false,
                    }
                };
                if !consumed && !swallowed {
                    send_key_raw(&widget, keyval, keycode, ctrl.group(), state, false);
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
            #[strong]
            swallowed_keys,
            move |_| {
                // Releases of keys pressed here now go elsewhere.
                swallowed_keys.borrow_mut().clear();
                reset_ime(&widget);
                im.focus_out();
                widget.imp().im_focused.set(false);
                set_focus(&widget, false);
            }
        ));
        widget.add_controller(focus);

        // Drag-drop ----------------------------------------------------------
        // M17: accept file drops; surface each as a synthetic `drop` on the element
        // under the cursor (paste_bridge.js targets it via the envelope's coords).
        // Async target: the drop is read by hand so a portal transfer can be
        // resolved here rather than by GTK's own conversion. (#192)
        let drop_formats = gdk::ContentFormatsBuilder::new()
            .add_mime_type(PORTAL_FILE_TRANSFER)
            // Matched against the offer as mime types, so the plain-path form
            // has to be named: the file list GType alone accepts nothing.
            .add_mime_type("text/uri-list")
            .add_type(gdk::FileList::static_type())
            .build();
        let drop_target = gtk::DropTargetAsync::new(Some(drop_formats), gdk::DragAction::COPY);
        // Decide on the offered types alone. The default check also wants the
        // drop's current action to be one of ours, but on entry that is whatever
        // the compositor picked for the source (Hyprland + Nautilus: MOVE), so a
        // COPY-only target refused every drag before it could ask for COPY.
        drop_target.connect_accept(|_t, drop| {
            let formats = drop.formats();
            log::debug!(
                "drag accept? actions={:?} offered {formats}",
                drop.actions()
            );
            formats.contain_mime_type(PORTAL_FILE_TRANSFER)
                || formats.contain_mime_type("text/uri-list")
        });
        // Forward hover (enter/motion/leave) so the page's dropzone overlay mounts
        // DURING the hover — CEF only delivers the drop on release, too late to mount.
        drop_target.connect_drag_enter(glib::clone!(
            #[weak]
            widget,
            #[upgrade_or]
            gdk::DragAction::COPY,
            move |_t, drop, x, y| {
                log::debug!("drag enter: offered {}", drop.formats());
                send_drag_hover(&widget, "enter", x, y);
                gdk::DragAction::COPY
            }
        ));
        let last_motion = std::cell::Cell::new(std::time::Instant::now());
        drop_target.connect_drag_motion(glib::clone!(
            #[weak]
            widget,
            #[upgrade_or]
            gdk::DragAction::COPY,
            move |_t, _drop, x, y| {
                // Throttle to ~10/s: keeps the dropzone alive without flooding IPC.
                let now = std::time::Instant::now();
                if now.duration_since(last_motion.get()) >= std::time::Duration::from_millis(100) {
                    last_motion.set(now);
                    send_drag_hover(&widget, "over", x, y);
                }
                gdk::DragAction::COPY
            }
        ));
        drop_target.connect_drag_leave(glib::clone!(
            #[weak]
            widget,
            move |_t, _drop| send_drag_hover(&widget, "leave", 0.0, 0.0)
        ));
        drop_target.connect_drop(glib::clone!(
            #[weak]
            widget,
            #[upgrade_or]
            false,
            move |_target, drop, x, y| {
                // Taken now, at release, and in page CSS px for the bridge. (I8)
                let coords = Some(widget_to_page(&widget, x, y));
                if drop.formats().contain_mime_type(PORTAL_FILE_TRANSFER) {
                    drop.read_async(
                        &[PORTAL_FILE_TRANSFER],
                        glib::Priority::DEFAULT,
                        gtk::gio::Cancellable::NONE,
                        glib::clone!(
                            #[weak]
                            widget,
                            #[strong]
                            drop,
                            move |res| {
                                match res {
                                    Ok((stream, _mime)) => {
                                        send_portal_files(&widget, stream, "drop", coords)
                                    }
                                    Err(err) => log::warn!("drop transfer read failed: {err}"),
                                }
                                drop.finish(gdk::DragAction::COPY);
                            }
                        ),
                    );
                } else {
                    drop.read_value_async(
                        gdk::FileList::static_type(),
                        glib::Priority::DEFAULT,
                        gtk::gio::Cancellable::NONE,
                        glib::clone!(
                            #[weak]
                            widget,
                            #[strong]
                            drop,
                            move |res| {
                                match res.map(|value| value.get::<gdk::FileList>()) {
                                    Ok(Ok(list)) => {
                                        load_and_send_files(&widget, list.files(), "drop", coords, true)
                                    }
                                    Ok(Err(err)) => {
                                        log::warn!("drop file list decode failed: {err}")
                                    }
                                    Err(err) => log::warn!("drop file read failed: {err}"),
                                }
                                drop.finish(gdk::DragAction::COPY);
                            }
                        ),
                    );
                }
                true
            }
        ));
        widget.add_controller(drop_target);
    }

    pub(super) fn is_copy_shortcut(keyval: gtk::gdk::Key, state: gtk::gdk::ModifierType) -> bool {
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
    pub(super) fn dispatch_explicit_copy(trigger: ExplicitCopyTrigger, request: impl FnOnce()) {
        // A menu item is picked in the popover, not on the web view, so no
        // press handler saw it; its Copy reply must still pass the gate. (S9)
        if matches!(trigger, ExplicitCopyTrigger::ContextMenu(_)) {
            super::note_user_input();
        }
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

    /// Interval of the IM focus-out debounce timer (`arm_im_focus_timer`).
    pub(super) const IM_FOCUS_TICK: std::time::Duration = std::time::Duration::from_millis(16);

    /// Ticks a focus-out waits before it is applied. The debounce timer ticks
    /// every `IM_FOCUS_TICK`, so this is ~64 ms — longer than the gap between the editable-focus false
    /// and true a composer re-render emits, and short enough that a real blur
    /// still hides an on-screen keyboard promptly. (#180)
    pub(super) const IM_FOCUS_OUT_DEBOUNCE_TICKS: u8 = 4;

    /// What one drain tick should do to the IM context's focus.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) enum ImFocusAction {
        /// Leave the IM as it is.
        Idle,
        /// Call `focus_in` and mark the IM focused.
        FocusIn,
        /// Call `focus_out` and mark the IM unfocused.
        FocusOut,
    }

    /// Decide one tick of the IM focus debounce.
    ///
    /// `request` is the editable-focus value drained from CEF this tick (`None`
    /// when the page said nothing), `focused` mirrors `im_focused`, and `pending`
    /// is the focus-out countdown, updated in place.
    ///
    /// A `true` wins immediately and cancels any queued focus-out — that pairing
    /// is what a composer re-render produces, and acting on its `false` is what
    /// dropped keys in #180. A `false` only arms the countdown; the focus-out
    /// lands on a later tick if nothing re-focuses first.
    pub(super) fn im_focus_step(
        request: Option<bool>,
        focused: bool,
        pending: &mut u8,
    ) -> ImFocusAction {
        match request {
            Some(true) => {
                *pending = 0;
                if focused {
                    ImFocusAction::Idle
                } else {
                    ImFocusAction::FocusIn
                }
            }
            Some(false) => {
                // Nothing to defer if the IM is already out.
                if focused && *pending == 0 {
                    *pending = IM_FOCUS_OUT_DEBOUNCE_TICKS;
                }
                ImFocusAction::Idle
            }
            None => {
                if *pending == 0 {
                    return ImFocusAction::Idle;
                }
                *pending -= 1;
                if *pending == 0 && focused {
                    ImFocusAction::FocusOut
                } else {
                    ImFocusAction::Idle
                }
            }
        }
    }

    /// UTF-16 unit to synthesize as a CEF `CHAR` for a key the IM declined, or
    /// `None` when the key is not text. Ctrl/Alt/Super combos are shortcuts, and
    /// C0/DEL keyvals (Enter, Tab, Backspace, Escape) keep their raw-key-only
    /// handling — the page acts on those from the key-down. (#180)
    pub(super) fn printable_char(
        keyval: gtk::gdk::Key,
        state: gtk::gdk::ModifierType,
    ) -> Option<u16> {
        use gtk::gdk::ModifierType;
        if state.intersects(
            ModifierType::CONTROL_MASK | ModifierType::ALT_MASK | ModifierType::SUPER_MASK,
        ) {
            return None;
        }
        let ch = keyval.to_unicode()?;
        if (ch as u32) < 0x20 || ch == '\u{7f}' {
            return None;
        }
        let mut buf = [0u16; 2];
        match ch.encode_utf16(&mut buf) {
            [unit] => Some(*unit),
            // Astral-plane keyvals can't be expressed as one UTF-16 unit; the IM
            // commit path handles those (no physical key produces them directly).
            _ => None,
        }
    }

    /// CHAR (`\r`) to send after Enter's key-down. Blink inserts a newline in a
    /// `<textarea>` and submits an `<input>` from this CHAR, not the key-down,
    /// and no IM commits one. Ctrl/Alt/Super+Enter are shortcuts. (I2)
    pub(super) fn enter_char(keyval: gtk::gdk::Key, state: gtk::gdk::ModifierType) -> Option<u16> {
        use gtk::gdk::{Key, ModifierType};
        if state.intersects(
            ModifierType::CONTROL_MASK | ModifierType::ALT_MASK | ModifierType::SUPER_MASK,
        ) {
            return None;
        }
        matches!(keyval, Key::Return | Key::KP_Enter | Key::ISO_Enter).then_some(0x0D)
    }

    /// Input-method composition state, kept on the widget. `preedit`: the IM
    /// shows a non-empty preedit. `cef_composing`: the page holds a composition
    /// we set. `session`: a composition ran since the last key that started
    /// outside one, so its commit must replace that composition. (I1)
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    pub struct ImeState {
        preedit: bool,
        cef_composing: bool,
        session: bool,
    }

    /// What a preedit change asks of CEF.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) enum ImeAction {
        Idle,
        /// `ime_set_composition` with the current preedit.
        SetComposition,
        /// `ime_cancel_composition`: the preedit emptied without a commit.
        Cancel,
    }

    /// How committed IM text reaches the page.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) enum ImeCommit {
        /// `ime_commit_text`: ends (replaces) a composition.
        Composition,
        /// CHAR events: ordinary typing through the IM, as before. (#154/#180)
        Chars,
    }

    impl ImeState {
        /// Whether a key press goes to the IM before CEF sees its raw key-down.
        /// Only while composing; otherwise the #180 order (raw key-down first)
        /// stays.
        pub(super) fn filter_first(&self) -> bool {
            self.preedit || self.cef_composing
        }

        /// A key press outside any composition closes the previous session, so
        /// ordinary typing commits as CHARs again. An async IM (ibus) may commit
        /// after its key's handler returned; the session survives until here.
        pub(super) fn key_press_begin(&mut self) {
            if !self.filter_first() {
                self.session = false;
            }
        }

        pub(super) fn preedit_start(&mut self) {
            self.session = true;
        }

        pub(super) fn preedit_changed(&mut self, empty: bool) -> ImeAction {
            self.preedit = !empty;
            if !empty {
                self.session = true;
                self.cef_composing = true;
                ImeAction::SetComposition
            } else if std::mem::take(&mut self.cef_composing) {
                ImeAction::Cancel
            } else {
                ImeAction::Idle
            }
        }

        pub(super) fn preedit_end(&mut self) -> ImeAction {
            self.preedit_changed(true)
        }

        /// GtkIMContextSimple ends a dead-key sequence with preedit-end then
        /// commit; ibus may commit first. Either order lands the text exactly
        /// once: through `ime_commit_text` while in a session.
        pub(super) fn commit(&mut self) -> ImeCommit {
            let route = if self.session || self.cef_composing {
                ImeCommit::Composition
            } else {
                ImeCommit::Chars
            };
            // CEF ends the composition on commit; a preedit that continues
            // (partial commit) opens a new one on its next change.
            self.cef_composing = false;
            route
        }
    }

    /// `CefRange::InvalidRange()`: "no replacement range", i.e. the current
    /// composition or selection.
    fn ime_invalid_range() -> cef::Range {
        cef::Range {
            from: u32::MAX,
            to: u32::MAX,
        }
    }

    fn apply_ime_action(widget: &super::KarereWebView, action: ImeAction, text: &str, cursor: i32) {
        match action {
            ImeAction::Idle => {}
            ImeAction::SetComposition => send_ime_composition(widget, text, cursor),
            ImeAction::Cancel => with_host(widget, |host| host.ime_cancel_composition()),
        }
    }

    /// Show `text` as the page's composition, underlined, with the caret at
    /// `cursor_chars` (GTK counts characters; CEF counts UTF-16 units).
    fn send_ime_composition(widget: &super::KarereWebView, text: &str, cursor_chars: i32) {
        let (len, caret) = ime_utf16_extent(text, cursor_chars);
        let underline = cef::CompositionUnderline {
            range: cef::Range { from: 0, to: len },
            // Transparent = drawn in the text colour, so it shows in dark mode.
            color: 0,
            background_color: 0,
            thick: 0,
            style: cef::CompositionUnderlineStyle::SOLID,
            ..Default::default()
        };
        let text = CefString::from(text);
        with_host(widget, |host| {
            host.ime_set_composition(
                Some(&text),
                Some(std::slice::from_ref(&underline)),
                Some(&ime_invalid_range()),
                Some(&cef::Range {
                    from: caret,
                    to: caret,
                }),
            )
        });
    }

    /// UTF-16 length of `text` and the UTF-16 offset of character `cursor_chars`.
    pub(super) fn ime_utf16_extent(text: &str, cursor_chars: i32) -> (u32, u32) {
        let len = text.encode_utf16().count() as u32;
        let caret = text
            .chars()
            .take(cursor_chars.max(0) as usize)
            .map(char::len_utf16)
            .sum::<usize>() as u32;
        (len, caret.min(len))
    }

    /// Drop any open composition: the IM's (it may commit it, which then
    /// replaces the page's composition) and then the page's. Focus-out,
    /// account switch, browser change, and a click while composing. (I1)
    fn reset_ime(widget: &super::KarereWebView) {
        let imp = widget.imp();
        let im = imp.im_context.borrow().clone();
        if let Some(im) = im {
            im.reset();
        }
        if imp.ime_state.replace(ImeState::default()).cef_composing {
            with_host(widget, |host| host.ime_cancel_composition());
        }
        // The next composition reports its own range; until then the click
        // position is the better guess.
        if let Some(shared) = imp.shared.lock().as_ref() {
            shared.lock().ime_caret_rect = None;
        }
    }

    /// Place the IM candidate window at the page's composition, from the last
    /// range CEF reported (`on_ime_composition_range_changed`, view = physical
    /// px). Without one, the location set on the last click stays. (I1)
    fn update_ime_cursor_location(widget: &super::KarereWebView) {
        let imp = widget.imp();
        let rect = imp
            .shared
            .lock()
            .as_ref()
            .and_then(|shared| shared.lock().ime_caret_rect);
        let Some((x, y, w, h)) = rect else {
            return;
        };
        let s = widget.scale_factor().max(1);
        let im = imp.im_context.borrow().clone();
        if let Some(im) = im {
            im.set_cursor_location(&gtk::gdk::Rectangle::new(
                x / s,
                y / s,
                (w / s).max(1),
                (h / s).max(1),
            ));
        }
    }

    /// Surface-unit (touchpad) scroll distance worth one Ctrl+scroll zoom step.
    const ZOOM_SCROLL_SURFACE_STEP: f64 = 40.0;

    /// Whole zoom steps (+ = in) for a Ctrl+scroll `dy`, accumulating smooth
    /// deltas in `accum` until they reach `threshold`. A direction change
    /// drops the partial step. (I10)
    pub(super) fn zoom_scroll_steps(accum: &mut f64, dy: f64, threshold: f64) -> i32 {
        let delta = -dy;
        if delta == 0.0 || threshold <= 0.0 {
            return 0;
        }
        if *accum != 0.0 && accum.signum() != delta.signum() {
            *accum = 0.0;
        }
        *accum += delta;
        let steps = (*accum / threshold).trunc();
        *accum -= steps * threshold;
        steps as i32
    }

    /// Widget logical px → page CSS px. CEF's view is physical px and the page
    /// zoom folds the display scale in (#158), so CSS = logical × scale / zoom.
    /// The mouse path needs no such step: CEF maps its physical coords itself.
    /// Only coordinates handed to page script (`elementFromPoint`, synthetic
    /// event `clientX/Y`) go through here. (I8)
    pub(super) fn page_coords(x: f64, y: f64, scale: f64, page_zoom: f64) -> (f64, f64) {
        if !(page_zoom.is_finite() && page_zoom > 0.0) {
            return (x, y);
        }
        let s = scale.max(1.0);
        (x * s / page_zoom, y * s / page_zoom)
    }

    fn widget_to_page(widget: &super::KarereWebView, x: f64, y: f64) -> (f64, f64) {
        let scale = widget.scale_factor().max(1) as f64;
        match resolved_browser(widget.imp()).and_then(|browser| browser.host()) {
            Some(host) => page_coords(x, y, scale, super::cef_to_linear(host.zoom_level())),
            None => (x, y),
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
    fn paint_scale(widget: &super::KarereWebView) -> f64 {
        widget.scale_factor().max(1) as f64
    }

    /// Set once `realize` gives up on GL (#177). Process-global to reach
    /// `accel_osr_enabled`, whose answer is fixed for the browser's lifetime.
    pub(super) static NO_GL: AtomicBool = AtomicBool::new(false);

    /// GPU-OSR capability, decided once by [`probe_accel_osr`] with a GLArea
    /// context current. Unset until then. (gpu-osr, I6)
    static ACCEL_PROBE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

    /// Decide GPU-accelerated OSR: env `KARERE_GPU_OSR=1` (or the
    /// `gpu-rendering` setting) AND EGL dma-buf import on this context. Call
    /// only with the GLArea context current — `realize` after `init_gl`. An
    /// answer is cached only when it was actually measured: without a current
    /// EGL display the probe leaves the slot empty for the next realize.
    /// Restart-required once cached, as the shared-texture flag is fixed per
    /// browser anyway. (gpu-osr, I6)
    pub(super) fn probe_accel_osr() {
        if ACCEL_PROBE.get().is_some() || NO_GL.load(Ordering::Relaxed) {
            return;
        }
        // Env wins as a dev override / kill-switch; otherwise the experimental
        // `gpu-rendering` GSetting.
        let requested = match std::env::var("KARERE_GPU_OSR").ok().as_deref() {
            Some("1") | Some("true") => true,
            Some("0") | Some("false") => false,
            _ => {
                use gtk::prelude::SettingsExt;
                let want = super::with_app_settings(|s| s.boolean("gpu-rendering"));
                // NVIDIA can't back CEF's exportable shared images — the GPU
                // process fails SkSurface init and no accelerated frame ever
                // arrives, leaving the chat area black (#167). Probe the GL
                // context actually rendering (current here) instead of the PCI
                // driver list, so hybrid setups rendering on another GPU keep
                // the accel path (#173). KARERE_GPU_OSR=1 forces.
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
        // Do not ask CEF for shared textures unless this GL/EGL display can
        // import them. Without this pre-browser fence CEF may never call CPU
        // on_paint, leaving a blank first frame.
        let enabled = if requested {
            match crate::gl_dmabuf::dmabuf_import_supported() {
                Some(ok) => ok,
                None => return, // not measurable here; keep CPU OSR, retry later
            }
        } else {
            false
        };
        log::info!("accel_osr: requested={requested} enabled={enabled}");
        let _ = ACCEL_PROBE.set(enabled);
    }

    /// Whether a browser created now should ask CEF for shared textures.
    ///
    /// Before the first successful [`probe_accel_osr`] — a background (tray)
    /// start creates every account's browser before any GLArea is realized —
    /// this answers CPU OSR without caching it, so browsers created after the
    /// window first shows (new accounts, DevTools, a software restart) still
    /// get the probed answer. Pre-probe browsers stay on CPU paint for their
    /// lifetime: requesting shared textures blind risks a never-painting view
    /// on machines that can't export them (#167) or have no GL at all (#177).
    pub(super) fn accel_osr_enabled() -> bool {
        // Nothing can import a shared texture without a GL context, and asking
        // for one stops CEF ever calling CPU on_paint -- which is the only
        // thing the software present path has to draw. (#177)
        if NO_GL.load(Ordering::Relaxed) {
            return false;
        }
        match ACCEL_PROBE.get() {
            Some(&enabled) => enabled,
            None => {
                log::debug!("accel_osr: not probed yet (no GL context) — CPU OSR for this browser");
                false
            }
        }
    }

    /// True when the current GL context is driven by the NVIDIA proprietary
    /// driver (GL_VENDOR contains "NVIDIA"; Mesa reports "Mesa"/"AMD" even on
    /// NVIDIA hardware via nouveau/NVK, where shared images are untested but
    /// not known-broken). Expects a current GL context; without one it falls
    /// back to whether the nvidia kernel driver is bound. (gpu-osr)
    fn gl_vendor_is_nvidia() -> bool {
        let p = unsafe { gl::GetString(gl::VENDOR) };
        if p.is_null() {
            // No current GL context (should not happen from the realize
            // probe). Fall back to the coarse driver-bound check so NVIDIA
            // still gets software rendering instead of the broken accel path.
            return std::path::Path::new("/sys/bus/pci/drivers/nvidia").exists();
        }
        let vendor = unsafe { std::ffi::CStr::from_ptr(p.cast()) }.to_string_lossy();
        log::info!("accel_osr: GL_VENDOR={vendor}");
        vendor.contains("NVIDIA")
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
    ///   `sync_viewport` and, per query so window moves are seen,
    ///   `RenderHandler::screen_point` via `current_window_origin`).
    ///
    /// Verified: no root `GdkSurface` exists in gdk4 0.11 to translate against,
    /// so the `translate_coordinates`/`compute_bounds` logical path would always
    /// return `false` — the shipped path is the xlib physical path.
    pub(super) fn window_origin_for(widget: &super::KarereWebView) -> (i32, i32) {
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
        // Resolved once: `open()` rebuilds the whole symbol table per call.
        static XLIB: std::sync::OnceLock<Option<x11::xlib::Xlib>> = std::sync::OnceLock::new();
        let Some(xlib) = XLIB
            .get_or_init(|| {
                x11::xlib::Xlib::open()
                    .map_err(|e| log::debug!("window_origin_for: Xlib::open failed: {e}"))
                    .ok()
            })
            .as_ref()
        else {
            return (0, 0);
        };
        let ok = unsafe {
            (xlib.XTranslateCoordinates)(
                xdisplay as *mut x11::xlib::Display,
                xid,
                root,
                0,
                0,
                &mut rx,
                &mut ry,
                &mut child,
            )
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
            x,
            y,
            px,
            py,
            scale,
            leave
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
            x,
            y,
            px,
            py,
            scale,
            button,
            down,
            n_press
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
        let (sx, sy, _) = widget.native()?.surface()?.device_position(&pointer)?;
        let (x, y) = surface_to_widget(widget, sx, sy)?;
        let s = widget.scale_factor().max(1) as f64;
        Some(((x * s).round() as i32, (y * s).round() as i32))
    }

    /// Surface → widget coords: strip the native's window-decoration offset, then
    /// map through the widget tree. Raw `GdkEvent` positions (and
    /// `device_position`) are surface-relative — feeding them to CEF unmapped
    /// offsets every click by the header-bar height (KARE-002 regression).
    fn surface_to_widget(widget: &super::KarereWebView, sx: f64, sy: f64) -> Option<(f64, f64)> {
        let native = widget.native()?;
        let (tx, ty) = native.surface_transform();
        let p = native.compute_point(
            widget,
            &gtk::graphene::Point::new((sx - tx) as f32, (sy - ty) as f32),
        )?;
        Some((p.x() as f64, p.y() as f64))
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
        layout: u32,
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
            windows_key_code: gdk_key_to_vk(
                keyval,
                unshifted_keyval(widget, keycode, layout),
                keycode,
            ),
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
        let payload = match crate::paste::make_blob(bytes) {
            Ok(p) => p,
            Err(err) => {
                log::warn!("paste: {err}");
                toast_paste_too_large(widget);
                return;
            }
        };
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

    /// Tell the user an oversized paste/drop was refused (`make_blob` limit).
    fn toast_paste_too_large(widget: &super::KarereWebView) {
        if let Some(window) = widget.root().and_downcast::<crate::window::KarereWindow>() {
            window.show_toast(&gettextrs::gettext("File is too large to attach (limit 48 MB)"));
        }
    }

    /// Notify the page that a file drag is hovering so it can pre-mount its
    /// dropzone before the drop commits.
    fn send_drag_hover(widget: &super::KarereWebView, phase: &str, x: f64, y: f64) {
        // The bridge hit-tests these with elementFromPoint: CSS px. (I8)
        let (x, y) = widget_to_page(widget, x, y);
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
        if clipboard.formats().contain_mime_type(PORTAL_FILE_TRANSFER) {
            clipboard.read_async(
                &[PORTAL_FILE_TRANSFER],
                glib::Priority::DEFAULT,
                gtk::gio::Cancellable::NONE,
                glib::clone!(
                    #[weak]
                    widget,
                    move |res| match res {
                        Ok((stream, _mime)) => send_portal_files(&widget, stream, "paste", None),
                        Err(err) => log::warn!("clipboard transfer read failed: {err}"),
                    }
                ),
            );
            return;
        }
        clipboard.read_value_async(
            gtk::gdk::FileList::static_type(),
            glib::Priority::DEFAULT,
            gtk::gio::Cancellable::NONE,
            glib::clone!(
                #[weak]
                widget,
                move |res| match res {
                    Ok(value) => match value.get::<gtk::gdk::FileList>() {
                        Ok(list) => load_and_send_files(&widget, list.files(), "paste", None, true),
                        Err(err) => log::warn!("clipboard file list decode failed: {err}"),
                    },
                    Err(err) => log::warn!("clipboard file read failed: {err}"),
                }
            ),
        );
    }

    /// Offered by a source that registered its files with the document portal.
    /// The payload is a key; the portal trades it for paths the sandbox can read.
    const PORTAL_FILE_TRANSFER: &str = "application/vnd.portal.filetransfer";

    /// Resolve a portal file transfer and dispatch each file as a paste/drop.
    /// This is what lets a drop attach without any filesystem permission. (#192)
    fn send_portal_files(
        widget: &super::KarereWebView,
        stream: gtk::gio::InputStream,
        kind: &'static str,
        coords: Option<(f64, f64)>,
    ) {
        let widget = widget.downgrade();
        glib::spawn_future_local(async move {
            let files = match retrieve_portal_files(stream).await {
                Ok(files) => files,
                Err(err) => return log::warn!("portal file transfer failed: {err}"),
            };
            let Some(widget) = widget.upgrade() else {
                return;
            };
            let files = files.into_iter().map(gtk::gio::File::for_path).collect();
            load_and_send_files(&widget, files, kind, coords, true);
        });
    }

    async fn retrieve_portal_files(
        stream: gtk::gio::InputStream,
    ) -> Result<Vec<String>, glib::Error> {
        let out = gtk::gio::MemoryOutputStream::new_resizable();
        out.splice_future(
            &stream,
            gtk::gio::OutputStreamSpliceFlags::CLOSE_SOURCE
                | gtk::gio::OutputStreamSpliceFlags::CLOSE_TARGET,
            glib::Priority::DEFAULT,
        )
        .await?;
        let key = String::from_utf8_lossy(&out.steal_as_bytes())
            .trim_matches(|c: char| c == '\0' || c.is_whitespace())
            .to_owned();

        let bus = gtk::gio::bus_get_future(gtk::gio::BusType::Session).await?;
        let options = glib::VariantDict::new(None).end();
        let reply = bus
            .call_future(
                Some("org.freedesktop.portal.Documents"),
                "/org/freedesktop/portal/documents",
                "org.freedesktop.portal.FileTransfer",
                "RetrieveFiles",
                Some(&glib::Variant::tuple_from_iter([key.to_variant(), options])),
                Some(glib::VariantTy::new("(as)").expect("valid variant type")),
                gtk::gio::DBusCallFlags::NONE,
                -1,
            )
            .await?;
        Ok(reply
            .get::<(Vec<String>,)>()
            .map(|(files,)| files)
            .unwrap_or_default())
    }

    /// Raw bytes one batched paste/drop message may carry; base64 makes that
    /// ~64 MB on the wire, the same ceiling a single file always had.
    const PASTE_BATCH_BYTES: usize = 48 * 1024 * 1024;

    /// Split files of `sizes` bytes, in order, into consecutive batches of at
    /// most `budget` bytes. A file larger than the budget gets a batch alone
    /// (and is then refused by `make_blob`).
    pub(super) fn batch_by_size(sizes: &[usize], budget: usize) -> Vec<std::ops::Range<usize>> {
        let mut batches = Vec::new();
        let (mut start, mut total) = (0, 0usize);
        for (i, &size) in sizes.iter().enumerate() {
            if i > start && total.saturating_add(size) > budget {
                batches.push(start..i);
                start = i;
                total = 0;
            }
            total = total.saturating_add(size);
        }
        if start < sizes.len() {
            batches.push(start..sizes.len());
        }
        batches
    }

    /// Load every file, then dispatch them together so the page builds one
    /// `DataTransfer` holding all of them: one paste/drop, one attach preview,
    /// instead of one per file. (J4)
    fn load_and_send_files(
        widget: &super::KarereWebView,
        files: Vec<gtk::gio::File>,
        kind: &'static str,
        coords: Option<(f64, f64)>,
        offer_chooser: bool,
    ) {
        if files.is_empty() {
            return;
        }
        let weak = widget.downgrade();
        glib::spawn_future_local(async move {
            let mut loaded = Vec::new();
            let mut unreadable = None;
            for file in files {
                let name = file.basename().map(|p| p.to_string_lossy().into_owned());
                match file.load_contents_future().await {
                    Ok((bytes, _etag)) => {
                        let mime = guess_mime(name.as_deref(), &bytes);
                        loaded.push((name, mime, bytes));
                    }
                    Err(err) => {
                        log::warn!("read dropped/pasted file failed: {err}");
                        unreadable.get_or_insert(file);
                    }
                }
            }
            let Some(widget) = weak.upgrade() else {
                return;
            };
            let sizes: Vec<usize> = loaded.iter().map(|(_, _, bytes)| bytes.len()).collect();
            let mut too_large = false;
            for batch in batch_by_size(&sizes, PASTE_BATCH_BYTES) {
                let files: Vec<crate::ipc::PasteFile> = loaded[batch]
                    .iter()
                    .filter_map(|(name, mime, bytes)| match crate::paste::make_blob(bytes) {
                        Ok(payload) => Some(crate::ipc::PasteFile {
                            mime: mime.clone(),
                            name: name.clone(),
                            payload,
                        }),
                        Err(err) => {
                            log::warn!("paste: {err}");
                            too_large = true;
                            None
                        }
                    })
                    .collect();
                if files.is_empty() {
                    continue;
                }
                let (x, y) = coords.map_or((None, None), |(x, y)| (Some(x), Some(y)));
                send_browser_message(
                    &widget,
                    crate::ipc::BrowserMessage::DispatchPasteFiles {
                        kind: kind.to_string(),
                        files,
                        x,
                        y,
                    },
                );
            }
            if too_large {
                toast_paste_too_large(&widget);
            }
            if offer_chooser && let Some(file) = unreadable {
                choose_unreadable_file(&widget, &file, kind, coords);
            }
        });
    }

    /// A drop, or a paste of a file copied in a file manager, hands over a plain
    /// path with no portal involved, and the sandbox cannot read most of those.
    /// Only the file chooser portal can grant access, so say why nothing was
    /// attached and offer the chooser, opened in the file's folder. (#192)
    fn choose_unreadable_file(
        widget: &super::KarereWebView,
        file: &gtk::gio::File,
        kind: &'static str,
        coords: Option<(f64, f64)>,
    ) {
        let Some(window) = widget.root().and_downcast::<crate::window::KarereWindow>() else {
            return;
        };
        if widget.imp().attach_chooser_open.replace(true) {
            return;
        }
        let toast = libadwaita::Toast::new(&gettextrs::gettext(
            "Karere cannot read files from this folder",
        ));
        toast.set_button_label(Some(&gettextrs::gettext("Choose File…")));
        toast.set_timeout(8);

        let chosen = std::rc::Rc::new(std::cell::Cell::new(false));
        toast.connect_button_clicked(glib::clone!(
            #[weak]
            widget,
            #[weak]
            window,
            #[strong]
            file,
            #[strong]
            chosen,
            move |_| {
                chosen.set(true);
                let dialog = gtk::FileDialog::builder()
                    .title(gettextrs::gettext("Select the File to Attach"))
                    .accept_label(gettextrs::gettext("Attach"))
                    .modal(true)
                    .build();
                dialog.set_initial_folder(file.parent().as_ref());
                dialog.open_multiple(
                    Some(&window),
                    gtk::gio::Cancellable::NONE,
                    glib::clone!(
                        #[weak]
                        widget,
                        move |res| {
                            widget.imp().attach_chooser_open.set(false);
                            // An error here is the user dismissing the chooser.
                            let Ok(files) = res else { return };
                            let files = files.iter::<gtk::gio::File>().flatten().collect();
                            load_and_send_files(&widget, files, kind, coords, false);
                        }
                    ),
                );
            }
        ));
        toast.connect_dismissed(glib::clone!(
            #[weak]
            widget,
            move |_| {
                if !chosen.get() {
                    widget.imp().attach_chooser_open.set(false);
                }
            }
        ));
        window.imp().toast_overlay.add_toast(toast);
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

    /// The level-0 (unshifted) keyval of hardware `keycode` in keyboard
    /// `layout`, so Shift+/ still maps to the '/' key's VK.
    fn unshifted_keyval(
        widget: &super::KarereWebView,
        keycode: u32,
        layout: u32,
    ) -> Option<gtk::gdk::Key> {
        widget
            .display()
            .map_keycode(keycode)?
            .into_iter()
            .find(|(k, _)| k.group() == layout as i32 && k.level() == 0)
            .map(|(_, key)| key)
    }

    /// Windows virtual-key code Chromium expects in `windows_key_code`: named
    /// keys and letters/digits from the keyval; punctuation from the unshifted
    /// keyval of the same key, then the keyval itself; last, the key's
    /// US-layout position (hardware keycode). 0 when nothing fits; text still
    /// arrives through the IM commit / CHAR path either way. (I2)
    pub(super) fn gdk_key_to_vk(
        keyval: gtk::gdk::Key,
        unshifted: Option<gtk::gdk::Key>,
        keycode: u32,
    ) -> i32 {
        let alnum = |k: gtk::gdk::Key| {
            k.to_unicode()
                .filter(|c| c.is_ascii_alphanumeric())
                .and_then(|_| ascii_vk(k))
        };
        named_vk(keyval)
            .or_else(|| alnum(keyval))
            .or_else(|| unshifted.and_then(ascii_vk))
            .or_else(|| ascii_vk(keyval))
            .or_else(|| keycode_vk(keycode))
            .unwrap_or(0)
    }

    /// Keys whose VK does not depend on the layout.
    fn named_vk(keyval: gtk::gdk::Key) -> Option<i32> {
        use gtk::gdk::Key;
        use gtk::glib::translate::IntoGlib;
        Some(match keyval {
            Key::Break => 0x03, // VK_CANCEL (Ctrl+Pause)
            Key::BackSpace => 0x08,
            Key::Tab | Key::ISO_Left_Tab | Key::KP_Tab => 0x09,
            Key::Clear | Key::KP_Begin => 0x0C,
            Key::Return | Key::KP_Enter | Key::ISO_Enter => 0x0D,
            Key::Shift_L | Key::Shift_R => 0x10,
            Key::Control_L | Key::Control_R => 0x11,
            Key::Alt_L | Key::Alt_R | Key::Meta_L | Key::Meta_R => 0x12, // VK_MENU
            Key::Pause => 0x13,
            Key::Caps_Lock => 0x14,
            Key::Escape => 0x1B,
            Key::space | Key::KP_Space => 0x20,
            Key::Page_Up | Key::KP_Page_Up => 0x21,
            Key::Page_Down | Key::KP_Page_Down => 0x22,
            Key::End | Key::KP_End => 0x23,
            Key::Home | Key::KP_Home => 0x24,
            Key::Left | Key::KP_Left => 0x25,
            Key::Up | Key::KP_Up => 0x26,
            Key::Right | Key::KP_Right => 0x27,
            Key::Down | Key::KP_Down => 0x28,
            Key::Select => 0x29,
            Key::Execute => 0x2B,
            Key::Print | Key::Sys_Req => 0x2C, // VK_SNAPSHOT
            Key::Insert | Key::KP_Insert => 0x2D,
            Key::Delete | Key::KP_Delete => 0x2E,
            Key::Help => 0x2F,
            Key::Super_L => 0x5B,
            Key::Super_R => 0x5C,
            Key::Menu => 0x5D, // VK_APPS
            Key::KP_0 => 0x60,
            Key::KP_1 => 0x61,
            Key::KP_2 => 0x62,
            Key::KP_3 => 0x63,
            Key::KP_4 => 0x64,
            Key::KP_5 => 0x65,
            Key::KP_6 => 0x66,
            Key::KP_7 => 0x67,
            Key::KP_8 => 0x68,
            Key::KP_9 => 0x69,
            Key::KP_Multiply => 0x6A,
            Key::KP_Add => 0x6B,
            Key::KP_Separator => 0x6C,
            Key::KP_Subtract => 0x6D,
            Key::KP_Decimal => 0x6E,
            Key::KP_Divide => 0x6F,
            Key::KP_Equal => 0xBB,
            Key::Num_Lock => 0x90,
            Key::Scroll_Lock => 0x91,
            Key::ISO_Level3_Shift => 0xE1, // VKEY_ALTGR
            k if (Key::F1.into_glib()..=Key::F24.into_glib()).contains(&k.into_glib()) => {
                0x70 + (k.into_glib() - Key::F1.into_glib()) as i32
            }
            _ => return None,
        })
    }

    /// Letters and digits map to their ASCII uppercase value (VK_A..VK_Z,
    /// VK_0..VK_9). Punctuation must NOT use its ASCII value: '.' is 0x2E =
    /// VK_DELETE, and Chromium then runs the Delete command on the key-down
    /// and drops the character. It gets the `VK_OEM_*` code of its US-layout
    /// key instead, shifted symbols included ('?' is the '/' key, VK_OEM_2).
    fn ascii_vk(keyval: gtk::gdk::Key) -> Option<i32> {
        let c = keyval.to_unicode()?;
        if c.is_ascii_alphanumeric() {
            return Some(c.to_ascii_uppercase() as i32);
        }
        Some(match c {
            ';' | ':' => 0xBA,  // VK_OEM_1
            '=' | '+' => 0xBB,  // VK_OEM_PLUS
            ',' | '<' => 0xBC,  // VK_OEM_COMMA
            '-' | '_' => 0xBD,  // VK_OEM_MINUS
            '.' | '>' => 0xBE,  // VK_OEM_PERIOD
            '/' | '?' => 0xBF,  // VK_OEM_2
            '`' | '~' => 0xC0,  // VK_OEM_3
            '[' | '{' => 0xDB,  // VK_OEM_4
            '\\' | '|' => 0xDC, // VK_OEM_5
            ']' | '}' => 0xDD,  // VK_OEM_6
            '\'' | '"' => 0xDE, // VK_OEM_7
            _ => return None,
        })
    }

    /// VK of the key at this position on a US layout (XKB keycode = evdev + 8).
    /// Last resort for keyvals with no ASCII meaning: non-Latin layouts, dead
    /// keys, shifted digits whose unshifted keyval is not known.
    fn keycode_vk(keycode: u32) -> Option<i32> {
        const ROW_QWERTY: &[u8] = b"QWERTYUIOP";
        const ROW_ASDF: &[u8] = b"ASDFGHJKL";
        const ROW_ZXCV: &[u8] = b"ZXCVBNM";
        let evdev = keycode.checked_sub(8)?;
        Some(match evdev {
            2..=10 => 0x31 + (evdev - 2) as i32, // 1..9
            11 => 0x30,                          // 0
            12 => 0xBD,                          // minus
            13 => 0xBB,                          // equal
            16..=25 => ROW_QWERTY[(evdev - 16) as usize] as i32,
            26 => 0xDB, // left brace
            27 => 0xDD, // right brace
            30..=38 => ROW_ASDF[(evdev - 30) as usize] as i32,
            39 => 0xBA, // semicolon
            40 => 0xDE, // apostrophe
            41 => 0xC0, // grave
            43 => 0xDC, // backslash
            44..=50 => ROW_ZXCV[(evdev - 44) as usize] as i32,
            51 => 0xBC, // comma
            52 => 0xBE, // dot
            53 => 0xBF, // slash
            86 => 0xE2, // VK_OEM_102 (ISO key left of Z)
            _ => return None,
        })
    }

    pub(super) const VS: &str = r#"#version 300 es
precision highp float;
layout(location=0) in vec2 a_pos;
layout(location=1) in vec2 a_uv;
out vec2 v_uv;
// Sampled sub-rect of the texture: xy = offset, zw = size (fractions).
// (0,0,1,1) for CPU frames; a DMA-BUF's visible rect inside its coded size.
uniform vec4 u_uv;
void main() {
    v_uv = u_uv.xy + a_uv * u_uv.zw;
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
        assert!(super::imp::NO_GL.load(AtomicOrdering::Relaxed));
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

    #[test]
    fn layers_are_placed_top_left_without_stretching() {
        // Current-size frame fills the framebuffer exactly.
        assert_eq!(
            super::gl_viewport_for((100, 50), (100, 50), (0, 0, 100, 50)),
            (0, 0, 100, 50)
        );
        // Stale smaller frame mid-resize: own size, top-left (GL y is bottom-up).
        assert_eq!(
            super::gl_viewport_for((100, 50), (100, 50), (0, 0, 80, 40)),
            (0, 10, 80, 40)
        );
        // Popup at view (10, 5).
        assert_eq!(
            super::gl_viewport_for((100, 50), (100, 50), (10, 5, 20, 10)),
            (10, 35, 20, 10)
        );
        // A framebuffer at another scale than the view still maps a full
        // frame onto all of it.
        assert_eq!(
            super::gl_viewport_for((150, 75), (200, 100), (0, 0, 200, 100)),
            (0, 0, 150, 75)
        );
    }

    #[test]
    fn accelerated_frames_sample_only_their_visible_rect() {
        assert_eq!(super::texture_uv((0, 0, 4, 2), (4, 2)), [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(super::texture_uv((0, 0, 2, 1), (4, 2)), [0.0, 0.0, 0.5, 0.5]);
        assert!(super::imp::VS.contains("u_uv"));
    }

    #[test]
    fn background_follows_the_theme_foreground() {
        // Light text = dark theme.
        assert_eq!(
            super::page_background_for_foreground(0.95, 0.95, 0.95),
            super::PAGE_BG_DARK
        );
        assert_eq!(
            super::page_background_for_foreground(0.1, 0.1, 0.1),
            super::PAGE_BG_LIGHT
        );
    }
}

#[cfg(test)]
mod input_tests {
    use super::imp::{
        ExplicitCopyTrigger, IM_FOCUS_OUT_DEBOUNCE_TICKS, ImFocusAction, ImeAction, ImeCommit,
        ImeState, MouseButtonTracker, MouseDispatch, MouseEventSink, MouseInput,
        RendererMessageSink, batch_by_size, dispatch_explicit_copy, dispatch_mouse_with,
        enter_char, gdk_key_to_vk, im_focus_step, ime_utf16_extent, page_coords,
        physical_mouse_coordinates, printable_char, request_live_selection_with, send_click_with,
        send_move_with, should_forward_mouse, zoom_scroll_steps,
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
        // The copy bridge runs under node; build environments without it (the
        // GNOME SDK) skip instead of failing.
        if Command::new("node").arg("--version").output().is_err() {
            eprintln!("skipping: `node` not found on PATH");
            return;
        }
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

    /// Drive the debounce over a tick sequence, returning every action it takes.
    fn run_im_focus(start_focused: bool, requests: &[Option<bool>]) -> Vec<ImFocusAction> {
        let mut focused = start_focused;
        let mut pending = 0u8;
        let mut actions = Vec::new();
        for req in requests {
            let action = im_focus_step(*req, focused, &mut pending);
            match action {
                ImFocusAction::FocusIn => focused = true,
                ImFocusAction::FocusOut => focused = false,
                ImFocusAction::Idle => {}
            }
            actions.push(action);
        }
        actions
    }

    /// The #180 regression: a composer re-render emits editable-focus false then
    /// true. Acting on that false unfocused the IM, and the next keystroke raced
    /// an async ibus `focus_in` — `filter_keypress` consumed it, no `commit` came,
    /// and the character vanished. The pair must be a no-op.
    #[test]
    fn composer_rerender_never_unfocuses_the_im() {
        let actions = run_im_focus(
            true,
            &[Some(false), None, Some(true), None, None, None, None],
        );
        assert!(
            actions.iter().all(|a| *a == ImFocusAction::Idle),
            "re-render pair disturbed IM focus: {actions:?}"
        );
    }

    /// A real blur — clicking out of the composer, no follow-up focus — must
    /// still reach the IM so an on-screen keyboard hides.
    #[test]
    fn genuine_blur_still_focuses_out() {
        let mut requests = vec![Some(false)];
        requests.extend(std::iter::repeat_n(
            None,
            usize::from(IM_FOCUS_OUT_DEBOUNCE_TICKS),
        ));
        let actions = run_im_focus(true, &requests);
        assert_eq!(
            actions
                .iter()
                .filter(|a| **a == ImFocusAction::FocusOut)
                .count(),
            1,
            "expected exactly one focus-out: {actions:?}"
        );
        assert_eq!(*actions.last().unwrap(), ImFocusAction::FocusOut);
    }

    /// Focusing a text field when the IM is out must focus in at once — the
    /// debounce only ever delays going *out*.
    #[test]
    fn editable_focus_focuses_in_immediately() {
        assert_eq!(
            run_im_focus(false, &[Some(true)]),
            vec![ImFocusAction::FocusIn]
        );
        // Already focused: nothing to do.
        assert_eq!(run_im_focus(true, &[Some(true)]), vec![ImFocusAction::Idle]);
    }

    /// An unfocused IM receiving another false must not arm a countdown that
    /// later fires a focus-out against an already-unfocused context.
    #[test]
    fn redundant_focus_out_is_inert() {
        let mut requests = vec![Some(false)];
        requests.extend(std::iter::repeat_n(
            None,
            usize::from(IM_FOCUS_OUT_DEBOUNCE_TICKS) + 2,
        ));
        let actions = run_im_focus(false, &requests);
        assert!(
            actions.iter().all(|a| *a == ImFocusAction::Idle),
            "focus-out fired on an unfocused IM: {actions:?}"
        );
    }

    /// A key the IM declines must still insert text: RAWKEYDOWN alone inserts
    /// nothing, which is the dropped-character path in #180.
    #[test]
    fn declined_printable_keys_synthesize_a_char() {
        use gtk::gdk::{Key, ModifierType};
        let none = ModifierType::empty();
        assert_eq!(printable_char(Key::a, none), Some(u16::from(b'a')));
        assert_eq!(printable_char(Key::A, none), Some(u16::from(b'A')));
        assert_eq!(printable_char(Key::_1, none), Some(u16::from(b'1')));
        assert_eq!(printable_char(Key::space, none), Some(0x20));
        assert_eq!(printable_char(Key::eacute, none), Some(0xe9));
        assert_eq!(
            printable_char(Key::a, ModifierType::SHIFT_MASK),
            Some(u16::from(b'a'))
        );
    }

    /// Shortcuts and control keys keep their raw-key-only handling — a
    /// synthesized CHAR there would type text the user never asked for.
    #[test]
    fn non_text_keys_synthesize_nothing() {
        use gtk::gdk::{Key, ModifierType};
        let none = ModifierType::empty();
        assert_eq!(printable_char(Key::a, ModifierType::CONTROL_MASK), None);
        assert_eq!(printable_char(Key::a, ModifierType::ALT_MASK), None);
        assert_eq!(printable_char(Key::a, ModifierType::SUPER_MASK), None);
        assert_eq!(printable_char(Key::Return, none), None);
        assert_eq!(printable_char(Key::Tab, none), None);
        assert_eq!(printable_char(Key::BackSpace, none), None);
        assert_eq!(printable_char(Key::Escape, none), None);
        assert_eq!(printable_char(Key::Delete, none), None);
        assert_eq!(printable_char(Key::Left, none), None);
        assert_eq!(printable_char(Key::F5, none), None);
        assert_eq!(printable_char(Key::Shift_L, none), None);
        // Enter is not "printable"; its CHAR comes from `enter_char` instead.
        assert_eq!(printable_char(Key::KP_Enter, none), None);
    }

    /// Enter (plain or Shift) sends the `\r` CHAR Blink needs for <textarea>
    /// newlines and <input> submit; nothing else does, and shortcuts don't.
    #[test]
    fn only_plain_enter_synthesizes_a_carriage_return() {
        let none = ModifierType::empty();
        assert_eq!(enter_char(Key::Return, none), Some(0x0D));
        assert_eq!(enter_char(Key::KP_Enter, none), Some(0x0D));
        assert_eq!(enter_char(Key::Return, ModifierType::SHIFT_MASK), Some(0x0D));
        assert_eq!(enter_char(Key::Return, ModifierType::CONTROL_MASK), None);
        assert_eq!(enter_char(Key::Return, ModifierType::ALT_MASK), None);
        for key in [Key::Tab, Key::BackSpace, Key::Escape, Key::a, Key::space] {
            assert_eq!(enter_char(key, none), None, "{key:?}");
        }
    }

    /// Keyval → Windows VK. Punctuation uses VK_OEM_* (never its ASCII value:
    /// '.' = 0x2E = VK_DELETE ran the Delete command and dropped the char).
    #[test]
    fn keyvals_map_to_windows_virtual_keys() {
        let cases: &[(Key, i32)] = &[
            (Key::BackSpace, 0x08),
            (Key::Tab, 0x09),
            (Key::ISO_Left_Tab, 0x09),
            (Key::Return, 0x0D),
            (Key::KP_Enter, 0x0D),
            (Key::Shift_L, 0x10),
            (Key::Shift_R, 0x10),
            (Key::Control_L, 0x11),
            (Key::Control_R, 0x11),
            (Key::Alt_L, 0x12),
            (Key::Alt_R, 0x12),
            (Key::Pause, 0x13),
            (Key::Caps_Lock, 0x14),
            (Key::Escape, 0x1B),
            (Key::space, 0x20),
            (Key::Page_Up, 0x21),
            (Key::KP_Page_Down, 0x22),
            (Key::KP_End, 0x23),
            (Key::KP_Home, 0x24),
            (Key::Left, 0x25),
            (Key::KP_Up, 0x26),
            (Key::KP_Right, 0x27),
            (Key::Down, 0x28),
            (Key::Print, 0x2C),
            (Key::KP_Insert, 0x2D),
            (Key::Delete, 0x2E),
            (Key::KP_Delete, 0x2E),
            (Key::Super_L, 0x5B),
            (Key::Super_R, 0x5C),
            (Key::Menu, 0x5D),
            (Key::KP_0, 0x60),
            (Key::KP_9, 0x69),
            (Key::KP_Multiply, 0x6A),
            (Key::KP_Add, 0x6B),
            (Key::KP_Subtract, 0x6D),
            (Key::KP_Decimal, 0x6E),
            (Key::KP_Divide, 0x6F),
            (Key::F1, 0x70),
            (Key::F5, 0x74),
            (Key::F12, 0x7B),
            (Key::F24, 0x87),
            (Key::Num_Lock, 0x90),
            (Key::Scroll_Lock, 0x91),
            (Key::a, 0x41),
            (Key::Z, 0x5A),
            (Key::_0, 0x30),
            (Key::_7, 0x37),
            (Key::semicolon, 0xBA),
            (Key::equal, 0xBB),
            (Key::comma, 0xBC),
            (Key::minus, 0xBD),
            (Key::period, 0xBE),
            (Key::slash, 0xBF),
            (Key::grave, 0xC0),
            (Key::bracketleft, 0xDB),
            (Key::backslash, 0xDC),
            (Key::bracketright, 0xDD),
            (Key::apostrophe, 0xDE),
            // Shifted symbols of a US layout keep their key's code.
            (Key::question, 0xBF),
            (Key::greater, 0xBE),
            (Key::colon, 0xBA),
            (Key::quotedbl, 0xDE),
        ];
        for &(key, vk) in cases {
            assert_eq!(gdk_key_to_vk(key, None, 0), vk, "{key:?}");
        }
        // No punctuation may land on a command VK (Delete, Insert, arrows...).
        for key in [Key::period, Key::comma, Key::slash, Key::minus, Key::apostrophe] {
            assert!(gdk_key_to_vk(key, None, 0) >= 0xBA, "{key:?}");
        }
    }

    /// Shift+/ and Shift+1 keep the unshifted key's VK; a key with no ASCII
    /// meaning (non-Latin layout, dead key) falls back to its US position.
    #[test]
    fn shifted_and_non_latin_keys_use_unshifted_keyval_then_position() {
        // XKB keycodes (evdev + 8) on a US layout.
        const KC_1: u32 = 10;
        const KC_A: u32 = 38;
        const KC_SLASH: u32 = 61;
        const KC_APOSTROPHE: u32 = 48;
        assert_eq!(gdk_key_to_vk(Key::question, Some(Key::slash), KC_SLASH), 0xBF);
        assert_eq!(gdk_key_to_vk(Key::exclam, Some(Key::_1), KC_1), 0x31);
        // Unshifted keyval unknown: the position still says '1'.
        assert_eq!(gdk_key_to_vk(Key::exclam, None, KC_1), 0x31);
        // Cyrillic 'ф' sits on the US 'A' key.
        assert_eq!(
            gdk_key_to_vk(Key::Cyrillic_ef, Some(Key::Cyrillic_ef), KC_A),
            0x41
        );
        assert_eq!(gdk_key_to_vk(Key::dead_acute, None, KC_APOSTROPHE), 0xDE);
        // A letter's own keyval wins over its position (AZERTY 'q' on US 'A').
        assert_eq!(gdk_key_to_vk(Key::q, Some(Key::q), KC_A), 0x51);
        // Unknown key, unknown position.
        assert_eq!(gdk_key_to_vk(Key::VoidSymbol, None, 0), 0);
    }

    /// Plain typing: no preedit, raw key-down first (#180), commit as CHARs.
    #[test]
    fn ime_plain_typing_keeps_the_raw_first_order_and_chars() {
        let mut ime = ImeState::default();
        for _ in 0..3 {
            ime.key_press_begin();
            assert!(!ime.filter_first());
            assert_eq!(ime.commit(), ImeCommit::Chars);
        }
    }

    /// Dead key ´ then e: GtkIMContextSimple shows a "´" preedit, then on 'e'
    /// ends it and commits "é". The 'e' goes to the IM first (no raw key-down
    /// for the page to act on) and the é lands once, via ime_commit_text. The
    /// next key is ordinary typing again.
    #[test]
    fn ime_dead_key_commits_once_in_either_signal_order() {
        // preedit-end before commit (GtkIMContextSimple).
        let mut ime = ImeState::default();
        ime.key_press_begin();
        assert!(!ime.filter_first(), "the dead key itself goes raw-first");
        ime.preedit_start();
        assert_eq!(ime.preedit_changed(false), ImeAction::SetComposition);
        ime.key_press_begin();
        assert!(ime.filter_first(), "'e' must reach the IM first");
        assert_eq!(ime.preedit_changed(true), ImeAction::Cancel);
        assert_eq!(ime.preedit_end(), ImeAction::Idle);
        assert_eq!(ime.commit(), ImeCommit::Composition);
        ime.key_press_begin();
        assert!(!ime.filter_first());
        assert_eq!(ime.commit(), ImeCommit::Chars);

        // commit before preedit-end (ibus-style).
        let mut ime = ImeState::default();
        ime.key_press_begin();
        assert_eq!(ime.preedit_changed(false), ImeAction::SetComposition);
        ime.key_press_begin();
        assert_eq!(ime.commit(), ImeCommit::Composition);
        assert_eq!(ime.preedit_end(), ImeAction::Idle, "nothing left to cancel");
        ime.key_press_begin();
        assert!(!ime.filter_first());
    }

    /// CJK: while composing, Enter/Backspace/arrows go to the IM first; an
    /// async commit after the key handler returned still replaces the
    /// composition; Escape (preedit emptied, no commit) cancels it.
    #[test]
    fn ime_cjk_composition_routes_keys_to_the_im() {
        let mut ime = ImeState::default();
        ime.key_press_begin();
        ime.preedit_start();
        assert_eq!(ime.preedit_changed(false), ImeAction::SetComposition);
        for _ in 0..4 {
            ime.key_press_begin();
            assert!(ime.filter_first());
            assert_eq!(ime.preedit_changed(false), ImeAction::SetComposition);
        }
        // Enter picks the candidate: preedit ends, then the async commit lands.
        ime.key_press_begin();
        assert!(ime.filter_first());
        assert_eq!(ime.preedit_end(), ImeAction::Cancel);
        assert_eq!(ime.commit(), ImeCommit::Composition);

        // Escape: composition discarded, then typing is plain again.
        let mut ime = ImeState::default();
        assert_eq!(ime.preedit_changed(false), ImeAction::SetComposition);
        assert_eq!(ime.preedit_end(), ImeAction::Cancel);
        ime.key_press_begin();
        assert!(!ime.filter_first());
        assert_eq!(ime.commit(), ImeCommit::Chars);
    }

    #[test]
    fn ime_composition_extent_counts_utf16_units() {
        assert_eq!(ime_utf16_extent("", 0), (0, 0));
        assert_eq!(ime_utf16_extent("nihao", 2), (5, 2));
        assert_eq!(ime_utf16_extent("你好", 1), (2, 1));
        // An astral character is two UTF-16 units.
        assert_eq!(ime_utf16_extent("a🙂b", 2), (4, 3));
        assert_eq!(ime_utf16_extent("ab", 9), (2, 2));
        assert_eq!(ime_utf16_extent("ab", -1), (2, 0));
    }

    /// One zoom step per wheel notch; hi-res and touchpad deltas accumulate;
    /// reversing drops the partial step. Up (dy < 0) zooms in.
    #[test]
    fn ctrl_scroll_zoom_steps() {
        let mut accum = 0.0;
        assert_eq!(zoom_scroll_steps(&mut accum, -1.0, 1.0), 1);
        assert_eq!(zoom_scroll_steps(&mut accum, 1.0, 1.0), -1);
        assert_eq!(zoom_scroll_steps(&mut accum, -3.0, 1.0), 3);
        let mut accum = 0.0;
        let steps: i32 = (0..8)
            .map(|_| zoom_scroll_steps(&mut accum, -0.125, 1.0))
            .sum();
        assert_eq!(steps, 1);
        let mut accum = 0.0;
        assert_eq!(zoom_scroll_steps(&mut accum, -30.0, 40.0), 0);
        assert_eq!(zoom_scroll_steps(&mut accum, 5.0, 40.0), 0);
        assert_eq!(zoom_scroll_steps(&mut accum, -30.0, 40.0), 0, "reversal reset");
        assert_eq!(zoom_scroll_steps(&mut accum, -15.0, 40.0), 1);
        assert_eq!(zoom_scroll_steps(&mut accum, 0.0, 40.0), 0);
    }

    /// Widget logical px → page CSS px: CEF's page zoom includes the display
    /// scale (#158), so at 2x with 100 % user zoom they coincide.
    #[test]
    fn page_coordinates_follow_the_page_zoom() {
        assert_eq!(page_coords(100.0, 50.0, 1.0, 1.0), (100.0, 50.0));
        assert_eq!(page_coords(100.0, 50.0, 2.0, 2.0), (100.0, 50.0));
        assert_eq!(page_coords(150.0, 60.0, 1.0, 1.5), (100.0, 40.0));
        assert_eq!(page_coords(150.0, 60.0, 2.0, 3.0), (100.0, 40.0));
        assert_eq!(page_coords(10.0, 20.0, 1.0, 0.0), (10.0, 20.0));
    }

    #[test]
    fn file_batches_respect_the_message_budget() {
        assert!(batch_by_size(&[], 10).is_empty());
        assert_eq!(batch_by_size(&[3, 3, 3], 10), vec![0..3]);
        assert_eq!(batch_by_size(&[6, 6, 6], 10), vec![0..1, 1..2, 2..3]);
        assert_eq!(batch_by_size(&[4, 6, 1, 9], 10), vec![0..2, 2..4]);
        // Oversized file: alone in its batch (make_blob then refuses it).
        assert_eq!(batch_by_size(&[2, 50, 2], 10), vec![0..1, 1..2, 2..3]);
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
            crate::web_view::imp::MouseDispatch::Move {
                x: 110.0,
                y: 110.0,
                modifiers: 0,
            },
            &active,
            2,
            &observed,
        );
        assert_eq!(
            observed.borrow()[0],
            Observed::Move {
                browser: 7,
                x: 220,
                y: 220,
                modifiers: 0
            }
        );
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
    use super::{ZOOM_MAX, cef_to_linear, linear_to_cef};

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

#[cfg(test)]
mod page_script_tests {
    use super::{autocorrect_language_script, user_css_script};

    #[test]
    fn autocorrect_gate_follows_first_language() {
        let s = |v: &[&str]| autocorrect_language_script(&v.iter().map(|x| x.to_string()).collect::<Vec<_>>());
        assert!(s(&["en-US"]).contains("SetEnabled(true)"));
        assert!(s(&["en_GB", "es"]).contains("SetEnabled(true)"));
        assert!(s(&["es-ES", "en-US"]).contains("SetEnabled(false)"));
        assert!(s(&["pt-BR"]).contains("SetEnabled(false)"));
        assert!(s(&[]).contains("SetEnabled(null)"));
    }

    #[test]
    fn user_css_is_a_single_json_literal() {
        let css = "a::after{content:\"x'\\\"}\n</style><script>alert(1)</script>\u{2028}";
        let js = user_css_script(css);
        // Exactly one literal assigned; raw newline never reaches the source.
        let lit = serde_json::to_string(css).unwrap();
        assert!(js.contains(&format!("e.textContent={lit};")));
        assert!(!lit.contains('\n'));
        let back: String = serde_json::from_str(&lit).unwrap();
        assert_eq!(back, css);
        assert!(js.contains("karere-user-css"));
    }
}
