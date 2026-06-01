//! Locale helpers for the Chromium spellchecker.
//!
//! Ported from karere v3's `spellcheck.rs`, adapted to gtk-cef-shell: the
//! Hunspell dictionary-directory scanning is dropped (Chromium auto-downloads
//! its own `.bdic` files), `parse_locale` returns owned strings, `region_name`
//! is public, `short_code` yields the lowercase language part, and a
//! `KNOWN_LANGUAGES` table of Chromium-supported spellcheck locales is added.
//!
//! BCP-47 note: Chromium's `--spell-check-languages` switch and the underlying
//! `.bdic` filenames use hyphenated codes (`en-US`, `pt-BR`). POSIX `LANG`
//! environment values use underscores and may carry an encoding/modifier
//! suffix (`en_GB.UTF-8`, `pt_BR@euro`). `parse_locale` normalises both.

/// Splits a locale code like `en_GB.UTF-8`, `pt-BR`, or `en` into its language
/// and optional region parts, stripping any `.encoding` / `@modifier` suffix.
///
/// Returns `None` for an empty input. The region (when present) is upper-cased
/// and the language lower-cased so callers get canonical BCP-47 casing.
///
/// `parse_locale("en_GB.UTF-8")` → `Some(("en", Some("GB")))`.
pub fn parse_locale(code: &str) -> Option<(String, Option<String>)> {
    // Drop POSIX encoding (".UTF-8") and modifier ("@euro") suffixes.
    let base = code
        .split(['.', '@'])
        .next()
        .unwrap_or("")
        .trim();
    if base.is_empty() {
        return None;
    }
    match base.find(['_', '-']) {
        Some(idx) => {
            let lang = base[..idx].to_lowercase();
            let region = base[idx + 1..].trim();
            if lang.is_empty() {
                return None;
            }
            let region = if region.is_empty() {
                None
            } else {
                Some(region.to_uppercase())
            };
            Some((lang, region))
        }
        None => Some((base.to_lowercase(), None)),
    }
}

/// Resolve the effective spellcheck language list: the explicit user selection
/// if non-empty, else a single auto-detected code from the user's preferred
/// locales (mapped to the closest Chromium-supported variant) when `auto_detect`
/// is on, else empty. Shared by the headerbar dropdown init, the CEF
/// command-line wiring, and the load handler so all agree.
pub fn resolve_languages(explicit: &[String], auto_detect: bool) -> Vec<String> {
    let explicit: Vec<String> = explicit
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect();
    if !explicit.is_empty() {
        return explicit;
    }
    if !auto_detect {
        return Vec::new();
    }
    glib::language_names()
        .into_iter()
        .map(|s| s.to_string())
        .filter(|s| s != "C" && !s.is_empty())
        .find_map(|lang| best_supported_code(&lang))
        .into_iter()
        .collect()
}

/// Map an arbitrary locale code to the closest Chromium-supported spellcheck
/// code in `KNOWN_LANGUAGES`, or `None` if the language isn't supported at all.
///
/// Resolution order: exact `lang-REGION` → bare `lang` → a sensible default
/// region for languages that only ship regional dictionaries → any variant with
/// the same language. This lets auto-detect handle locales like `en_IE` or
/// `en_GH` (no `.bdic`) by falling back to British/US English.
pub fn best_supported_code(code: &str) -> Option<String> {
    let (lang, region) = parse_locale(code)?;
    let has = |c: &str| KNOWN_LANGUAGES.iter().any(|(k, _)| *k == c);

    if let Some(r) = &region {
        let exact = format!("{lang}-{r}");
        if has(&exact) {
            return Some(exact);
        }
    }
    if has(&lang) {
        return Some(lang);
    }
    // Languages whose `.bdic` set is region-only: pick the closest default for
    // regions Chromium doesn't ship. English defaults to en-GB — Ireland (IE),
    // Ghana, New Zealand, South Africa, India, etc. all use British-aligned
    // English; the directly-supported en-AU/CA/US already match exactly.
    let default = match lang.as_str() {
        "en" => Some("en-GB"),
        "pt" => Some("pt-PT"),
        "es" => Some("es-ES"),
        _ => None,
    };
    if let Some(d) = default
        && has(d)
    {
        return Some(d.to_string());
    }
    KNOWN_LANGUAGES
        .iter()
        .map(|(k, _)| *k)
        .find(|k| short_code(k) == lang)
        .map(str::to_string)
}

/// The lowercase language part of a locale code: `en-GB` → `en`, `pt_BR` → `pt`.
/// Falls back to the trimmed/lower-cased input when no region is present.
pub fn short_code(code: &str) -> String {
    parse_locale(code)
        .map(|(lang, _)| lang)
        .unwrap_or_else(|| code.trim().to_lowercase())
}

/// User-facing display name for a locale code.
/// `en-GB` → `English (United Kingdom)`, `pt-BR` → `Portuguese (Brazil)`.
/// Falls back to the raw code when the language is unknown.
pub fn display_name(code: &str) -> String {
    let Some((lang, region)) = parse_locale(code) else {
        return code.to_string();
    };
    let lang_label = lang_name(&lang).unwrap_or(code).to_string();
    match region.as_deref().and_then(region_name) {
        Some(r) => format!("{lang_label} ({r})"),
        None => lang_label,
    }
}

/// English display name for an ISO 3166-1 region (or the `419` UN M.49 code for
/// Latin America). Input is case-insensitive. Returns `None` if unknown.
pub fn region_name(code: &str) -> Option<&'static str> {
    Some(match code.to_uppercase().as_str() {
        "419" => "Latin America",
        "AR" => "Argentina",
        "AT" => "Austria",
        "AU" => "Australia",
        "BR" => "Brazil",
        "CA" => "Canada",
        "CH" => "Switzerland",
        "CL" => "Chile",
        "CN" => "China",
        "CO" => "Colombia",
        "DE" => "Germany",
        "ES" => "Spain",
        "FR" => "France",
        "GB" => "United Kingdom",
        "HK" => "Hong Kong",
        "IE" => "Ireland",
        "IN" => "India",
        "IT" => "Italy",
        "MX" => "Mexico",
        "NL" => "Netherlands",
        "NZ" => "New Zealand",
        "PT" => "Portugal",
        "TW" => "Taiwan",
        "US" => "United States",
        "ZA" => "South Africa",
        _ => return None,
    })
}

/// English display name for an ISO 639 language code. Covers the languages
/// Chromium ships `.bdic` dictionaries for; returns `None` otherwise.
fn lang_name(code: &str) -> Option<&'static str> {
    Some(match code {
        "af" => "Afrikaans",
        "bg" => "Bulgarian",
        "ca" => "Catalan",
        "cs" => "Czech",
        "cy" => "Welsh",
        "da" => "Danish",
        "de" => "German",
        "el" => "Greek",
        "en" => "English",
        "es" => "Spanish",
        "et" => "Estonian",
        "fa" => "Persian",
        "fo" => "Faroese",
        "fr" => "French",
        "he" => "Hebrew",
        "hi" => "Hindi",
        "hr" => "Croatian",
        "hu" => "Hungarian",
        "id" => "Indonesian",
        "it" => "Italian",
        "ko" => "Korean",
        "lt" => "Lithuanian",
        "lv" => "Latvian",
        "nb" => "Norwegian Bokmål",
        "nl" => "Dutch",
        "pl" => "Polish",
        "pt" => "Portuguese",
        "ro" => "Romanian",
        "ru" => "Russian",
        "sh" => "Serbo-Croatian",
        "sk" => "Slovak",
        "sl" => "Slovenian",
        "sq" => "Albanian",
        "sr" => "Serbian",
        "sv" => "Swedish",
        "ta" => "Tamil",
        "tg" => "Tajik",
        "tr" => "Turkish",
        "uk" => "Ukrainian",
        "vi" => "Vietnamese",
        _ => return None,
    })
}

/// Chromium-supported spellcheck locales as `(BCP-47 code, friendly name)`.
///
/// This mirrors the dictionary set Chromium can download as `.bdic` files. A
/// code absent here may still work if Chromium adds it; conversely Chromium
/// silently falls back when a chosen `.bdic` is unavailable. The friendly
/// names match `display_name(code)`.
pub static KNOWN_LANGUAGES: &[(&str, &str)] = &[
    ("af", "Afrikaans"),
    ("bg", "Bulgarian"),
    ("ca", "Catalan"),
    ("cs", "Czech"),
    ("cy", "Welsh"),
    ("da", "Danish"),
    ("de", "German"),
    ("de-DE", "German (Germany)"),
    ("el", "Greek"),
    ("en-AU", "English (Australia)"),
    ("en-CA", "English (Canada)"),
    ("en-GB", "English (United Kingdom)"),
    ("en-US", "English (United States)"),
    ("es", "Spanish"),
    ("es-419", "Spanish (Latin America)"),
    ("es-AR", "Spanish (Argentina)"),
    ("es-ES", "Spanish (Spain)"),
    ("es-MX", "Spanish (Mexico)"),
    ("es-US", "Spanish (United States)"),
    ("et", "Estonian"),
    ("fa", "Persian"),
    ("fo", "Faroese"),
    ("fr", "French"),
    ("he", "Hebrew"),
    ("hi", "Hindi"),
    ("hr", "Croatian"),
    ("hu", "Hungarian"),
    ("id", "Indonesian"),
    ("it", "Italian"),
    ("ko", "Korean"),
    ("lt", "Lithuanian"),
    ("lv", "Latvian"),
    ("nb", "Norwegian Bokmål"),
    ("nl", "Dutch"),
    ("pl", "Polish"),
    ("pt-BR", "Portuguese (Brazil)"),
    ("pt-PT", "Portuguese (Portugal)"),
    ("ro", "Romanian"),
    ("ru", "Russian"),
    ("sh", "Serbo-Croatian"),
    ("sk", "Slovak"),
    ("sl", "Slovenian"),
    ("sq", "Albanian"),
    ("sr", "Serbian"),
    ("sv", "Swedish"),
    ("ta", "Tamil"),
    ("tg", "Tajik"),
    ("tr", "Turkish"),
    ("uk", "Ukrainian"),
    ("vi", "Vietnamese"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_locale_splits_language_and_region() {
        assert_eq!(
            parse_locale("en_GB.UTF-8"),
            Some(("en".to_string(), Some("GB".to_string())))
        );
        assert_eq!(
            parse_locale("pt-BR"),
            Some(("pt".to_string(), Some("BR".to_string())))
        );
        assert_eq!(parse_locale("en"), Some(("en".to_string(), None)));
        assert_eq!(parse_locale("C"), Some(("c".to_string(), None)));
        assert_eq!(parse_locale(""), None);
    }

    #[test]
    fn short_code_strips_region() {
        assert_eq!(short_code("en-GB"), "en");
        assert_eq!(short_code("pt_BR"), "pt");
        assert_eq!(short_code("en"), "en");
    }

    #[test]
    fn display_name_renders_friendly_form() {
        assert_eq!(display_name("en-GB"), "English (United Kingdom)");
        assert_eq!(display_name("en-US"), "English (United States)");
        assert_eq!(display_name("pt-BR"), "Portuguese (Brazil)");
        assert_eq!(display_name("es-419"), "Spanish (Latin America)");
    }

    #[test]
    fn best_supported_maps_to_chromium_set() {
        // Exact matches pass through.
        assert_eq!(best_supported_code("pt_BR"), Some("pt-BR".to_string()));
        assert_eq!(best_supported_code("en_US.UTF-8"), Some("en-US".to_string()));
        // Unsupported English regions fall back to British English.
        assert_eq!(best_supported_code("en_IE"), Some("en-GB".to_string()));
        assert_eq!(best_supported_code("en_GH"), Some("en-GB".to_string()));
        // Bare language present in the set.
        assert_eq!(best_supported_code("de"), Some("de".to_string()));
        // Unknown language → None.
        assert_eq!(best_supported_code("zz"), None);
    }

    #[test]
    fn known_languages_friendly_names_match_display_name() {
        for (code, friendly) in KNOWN_LANGUAGES {
            assert_eq!(&display_name(code), friendly, "mismatch for {code}");
        }
    }
}
