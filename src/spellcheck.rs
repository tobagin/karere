//! Locale helpers for the Chromium spellchecker. Normalises POSIX `LANG`
//! (`en_GB.UTF-8`) to BCP-47 (`en-GB`); Chromium auto-downloads `.bdic` dicts.

/// `parse_locale("en_GB.UTF-8")` → `Some(("en", Some("GB")))`; `None` if empty.
pub fn parse_locale(code: &str) -> Option<(String, Option<String>)> {
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

/// Effective language list: explicit selection if non-empty, else one
/// auto-detected supported code when `auto_detect`, else empty.
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

/// Closest code in `KNOWN_LANGUAGES`, or `None` if unsupported. Order: exact
/// `lang-REGION` → bare `lang` → default region → any same-language variant.
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
    // Default unshipped regions to the closest variant (en_IE → en-GB).
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

/// Lower-cased language part: `en-GB` → `en`.
pub fn short_code(code: &str) -> String {
    parse_locale(code)
        .map(|(lang, _)| lang)
        .unwrap_or_else(|| code.trim().to_lowercase())
}

/// `en-GB` → `English (United Kingdom)`; raw code if unknown.
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

/// English name for an ISO 3166-1 region (or `419` = Latin America).
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

/// English name for an ISO 639 language code Chromium ships a `.bdic` for.
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
        assert_eq!(best_supported_code("pt_BR"), Some("pt-BR".to_string()));
        assert_eq!(best_supported_code("en_US.UTF-8"), Some("en-US".to_string()));
        assert_eq!(best_supported_code("en_IE"), Some("en-GB".to_string()));
        assert_eq!(best_supported_code("en_GH"), Some("en-GB".to_string()));
        assert_eq!(best_supported_code("de"), Some("de".to_string()));
        assert_eq!(best_supported_code("zz"), None);
    }

    #[test]
    fn known_languages_friendly_names_match_display_name() {
        for (code, friendly) in KNOWN_LANGUAGES {
            assert_eq!(&display_name(code), friendly, "mismatch for {code}");
        }
    }
}
