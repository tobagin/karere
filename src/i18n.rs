//! UI-language override plumbing.
//!
//! Karere ships a gettext `.po`/`.mo` catalog per locale in [`ui_locales`]. The
//! user can pin one via the `app-language` GSetting; [`override_locale`] reads
//! it (guarded so a missing schema never aborts early startup), `main` exports
//! it as `LANGUAGE` before `textdomain`, and `cef_runtime` maps it onto the
//! nearest Chromium `.pak` UI locale via [`cef_locale_for`] so Chromium's own
//! in-page UI (context menus, error pages) follows the same choice.

use gtk::gio;
use gtk::gio::prelude::SettingsExt;

use crate::application::APP_ID;

/// gettext locale name for a BCP-47 dictionary code (`en-AU` → `en_AU`,
/// `es-419` → `es_419`).
fn gettext_locale(code: &str) -> String {
    match code.split_once('-') {
        Some((lang, region)) => format!("{lang}_{}", region.to_uppercase()),
        None => code.to_string(),
    }
}

/// UI-translation locales with no Chromium spellcheck dictionary: the four
/// v3-inherited catalogs plus the Chromium UI-only languages (`.pak`-backed,
/// big-population languages Chromium localises its own UI into but ships no
/// Hunspell dict for). gettext-named, with a display label.
const UI_ONLY_LOCALES: &[(&str, &str)] = &[
    ("ar", "Arabic"),
    ("ga", "Irish"),
    ("kk", "Kazakh"),
    ("it_IT", "Italian (Italy)"),
    ("am", "Amharic"),
    ("bn", "Bengali"),
    ("fi", "Finnish"),
    ("fil", "Filipino"),
    ("gu", "Gujarati"),
    ("ja", "Japanese"),
    ("kn", "Kannada"),
    ("ml", "Malayalam"),
    ("mr", "Marathi"),
    ("ms", "Malay"),
    ("sw", "Swahili"),
    ("te", "Telugu"),
    ("th", "Thai"),
    ("ur", "Urdu"),
    ("zh_CN", "Chinese (Simplified)"),
    ("zh_TW", "Chinese (Traditional)"),
];

/// Every locale Karere ships a UI catalog for: the spellcheck dictionary set
/// (gettext-named) plus [`UI_ONLY_LOCALES`]. Returned as `(gettext code, label)`
/// sorted by label. This is the source of truth for `po/LINGUAS`.
pub fn ui_locales() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = crate::spellcheck::KNOWN_LANGUAGES
        .iter()
        .map(|(code, name)| (gettext_locale(code), (*name).to_string()))
        .collect();
    v.extend(
        UI_ONLY_LOCALES
            .iter()
            .map(|(code, name)| ((*code).to_string(), (*name).to_string())),
    );
    v.sort_by(|a, b| a.1.cmp(&b.1));
    v
}

/// The `app-language` override, or `None` when unset / following the system
/// locale. Schema-guarded: returns `None` rather than aborting if the GSchema
/// is not installed (can happen very early in `main` on uninstalled dev runs).
pub fn override_locale() -> Option<String> {
    let source = gio::SettingsSchemaSource::default()?;
    source.lookup(APP_ID, true)?;
    let value = gio::Settings::new(APP_ID).string("app-language");
    (!value.is_empty()).then(|| value.to_string())
}

/// Chromium UI `.pak` locales shipped with this CEF build (gender variants and
/// the `_FEMININE/_MASCULINE/_NEUTER` duplicates omitted).
const CEF_UI_LOCALES: &[&str] = &[
    "af", "am", "ar", "bg", "bn", "ca", "cs", "da", "de", "el", "en-GB", "en-US", "es", "es-419",
    "et", "fa", "fi", "fil", "fr", "gu", "he", "hi", "hr", "hu", "id", "it", "ja", "kn", "ko",
    "lt", "lv", "ml", "mr", "ms", "nb", "nl", "pl", "pt-BR", "pt-PT", "ro", "ru", "sk", "sl", "sr",
    "sv", "sw", "ta", "te", "th", "tr", "uk", "ur", "vi", "zh-CN", "zh-TW",
];

/// Nearest Chromium UI `.pak` locale for a gettext code: exact (`pt_BR`→
/// `pt-BR`) first, then base language (`de_DE`→`de`), then the English regional
/// fallbacks Chromium uses. `None` when Chromium ships no matching UI catalog
/// (e.g. `cy`, `gl`, `hy`) — Chromium then falls back to its own default while
/// Karere's GTK UI still shows the translation.
pub fn cef_locale_for(gettext_code: &str) -> Option<String> {
    let bcp = gettext_code.replace('_', "-");
    if CEF_UI_LOCALES.contains(&bcp.as_str()) {
        return Some(bcp);
    }
    let base = bcp.split('-').next().unwrap_or(&bcp);
    if CEF_UI_LOCALES.contains(&base) {
        return Some(base.to_string());
    }
    match base {
        "en" => Some("en-US".to_string()),
        "pt" => Some("pt-BR".to_string()),
        _ => None,
    }
}

/// `accept_language_list` for a gettext code: drives the `Accept-Language`
/// header AND `navigator.languages`, so it must be a plain comma list with NO
/// `q=` weights (Chromium adds those to the header itself; q-values here would
/// leak into `navigator.languages`). Region variant, then base, then English:
/// `pt_BR` → `pt-BR,pt,en`.
pub fn accept_language_for(gettext_code: &str) -> String {
    let bcp = gettext_code.replace('_', "-");
    let base = bcp.split('-').next().unwrap_or(&bcp).to_string();
    let mut langs = vec![bcp.clone()];
    if base != bcp {
        langs.push(base.clone());
    }
    if base != "en" {
        langs.push("en".to_string());
    }
    langs.join(",")
}

/// WhatsApp Web locale (its `WALangUserPref`) for a gettext code: BCP-47 hyphen
/// form (`pt_BR` → `pt-BR`). WhatsApp Web otherwise follows the linked phone's
/// language; setting this overrides it to match the chosen UI language.
pub fn whatsapp_locale_for(gettext_code: &str) -> String {
    gettext_code.replace('_', "-")
}
