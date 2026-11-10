use cef::{self, Browser, ImplBrowser};
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::imp::{KarereWebView, resolved_browser};

thread_local! {
    /// One app `GSettings` for the hot-path readers below (allocation, load
    /// end, mute pushes) instead of constructing a new object per call. Reads
    /// still go to the backend, so values are always current. Main thread only
    /// (`gio::Settings` is not `Send`).
    static APP_SETTINGS: gtk::gio::Settings = gtk::gio::Settings::new(crate::application::APP_ID);
}

pub(super) fn with_app_settings<R>(f: impl FnOnce(&gtk::gio::Settings) -> R) -> R {
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
const EMBED_MOBILE: &str = include_str!("../../data/js-deferred/mobile_responsive.js");

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
pub(super) fn mobile_layout_mismatch(browser_id: i32, width_logical_px: i32) -> bool {
    let wanted = should_use_mobile_layout(width_logical_px);
    MOBILE_LOADED.with(|m| m.borrow().get(&browser_id).is_some_and(|&loaded| loaded != wanted))
}

pub(super) fn record_mobile_layout(browser_id: i32, mobile: bool) {
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

impl KarereWebView {
    /// Debounced mobile-layout re-check (D5): ~400 ms after the last call,
    /// reload the foreground page if it was loaded with the other layout.
    /// Every call restarts the wait, so a window dragged across the
    /// threshold reloads at most once, after it settles.
    pub(super) fn schedule_mobile_layout_check(&self) {
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
