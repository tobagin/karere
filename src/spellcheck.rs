use gettextrs::gettext;
use std::collections::HashSet;

/// Scans system directories for available Hunspell dictionaries.
/// Returns a list of language codes (e.g., "en_US", "en_GB").
pub fn get_available_dictionaries() -> Vec<String> {
    let mut available_dicts = HashSet::new();
    let search_paths = [
        "/app/share/hunspell",
        "/usr/share/hunspell",
        "/usr/share/myspell",
        "/usr/share/myspell/dicts",
    ];

    for path_str in search_paths {
        let path = std::path::Path::new(path_str);
        if path.exists() && path.is_dir()
            && let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    if let Ok(name) = entry.file_name().into_string()
                        && name.ends_with(".dic") {
                            // "en_US.dic" -> "en_US"
                            let lang = &name[..name.len() - 4];
                            available_dicts.insert(lang.to_string());
                        }
                }
            }
    }

    let mut sorted_dicts: Vec<String> = available_dicts.into_iter().collect();
    sorted_dicts.sort();
    sorted_dicts
}

/// Splits a locale code like "en_GB" or "pt-BR" into (lang, Some(region)).
/// Falls back to (whole, None) if no separator is present.
fn parse_locale(code: &str) -> (&str, Option<&str>) {
    if let Some(idx) = code.find(['_', '-']) {
        (&code[..idx], Some(&code[idx + 1..]))
    } else {
        (code, None)
    }
}

/// Maps an ISO 639-1 / 639-3 language code to its English display name.
/// Covers languages commonly shipped as Hunspell dictionaries.
fn lang_name(code: &str) -> Option<&'static str> {
    Some(match code {
        "af" => "Afrikaans",
        "ak" => "Akan",
        "am" => "Amharic",
        "an" => "Aragonese",
        "ar" => "Arabic",
        "as" => "Assamese",
        "ast" => "Asturian",
        "az" => "Azerbaijani",
        "be" => "Belarusian",
        "bg" => "Bulgarian",
        "bn" => "Bengali",
        "bo" => "Tibetan",
        "br" => "Breton",
        "bs" => "Bosnian",
        "ca" => "Catalan",
        "cs" => "Czech",
        "cy" => "Welsh",
        "da" => "Danish",
        "de" => "German",
        "dz" => "Dzongkha",
        "el" => "Greek",
        "en" => "English",
        "eo" => "Esperanto",
        "es" => "Spanish",
        "et" => "Estonian",
        "eu" => "Basque",
        "fa" => "Persian",
        "fi" => "Finnish",
        "fo" => "Faroese",
        "fr" => "French",
        "fur" => "Friulian",
        "fy" => "Frisian",
        "ga" => "Irish",
        "gd" => "Scottish Gaelic",
        "gl" => "Galician",
        "gu" => "Gujarati",
        "gv" => "Manx",
        "haw" => "Hawaiian",
        "he" => "Hebrew",
        "hi" => "Hindi",
        "hr" => "Croatian",
        "hsb" => "Upper Sorbian",
        "ht" => "Haitian Creole",
        "hu" => "Hungarian",
        "hy" => "Armenian",
        "ia" => "Interlingua",
        "id" => "Indonesian",
        "is" => "Icelandic",
        "it" => "Italian",
        "ja" => "Japanese",
        "ka" => "Georgian",
        "kk" => "Kazakh",
        "km" => "Khmer",
        "kmr" => "Kurmanji Kurdish",
        "kn" => "Kannada",
        "ko" => "Korean",
        "ku" => "Kurdish",
        "ky" => "Kyrgyz",
        "la" => "Latin",
        "lb" => "Luxembourgish",
        "ln" => "Lingala",
        "lo" => "Lao",
        "lt" => "Lithuanian",
        "lv" => "Latvian",
        "mg" => "Malagasy",
        "mi" => "Maori",
        "mk" => "Macedonian",
        "ml" => "Malayalam",
        "mn" => "Mongolian",
        "mr" => "Marathi",
        "ms" => "Malay",
        "mt" => "Maltese",
        "my" => "Burmese",
        "nb" => "Norwegian Bokmål",
        "nds" => "Low German",
        "ne" => "Nepali",
        "nl" => "Dutch",
        "nn" => "Norwegian Nynorsk",
        "no" => "Norwegian",
        "nr" => "Southern Ndebele",
        "nso" => "Northern Sotho",
        "ny" => "Chichewa",
        "oc" => "Occitan",
        "om" => "Oromo",
        "or" => "Odia",
        "pa" => "Punjabi",
        "pl" => "Polish",
        "ps" => "Pashto",
        "pt" => "Portuguese",
        "qu" => "Quechua",
        "rm" => "Romansh",
        "ro" => "Romanian",
        "ru" => "Russian",
        "rw" => "Kinyarwanda",
        "sa" => "Sanskrit",
        "se" => "Northern Sami",
        "shs" => "Shuswap",
        "si" => "Sinhala",
        "sk" => "Slovak",
        "sl" => "Slovenian",
        "sma" => "Southern Sami",
        "smj" => "Lule Sami",
        "sq" => "Albanian",
        "sr" => "Serbian",
        "ss" => "Swati",
        "st" => "Southern Sotho",
        "sv" => "Swedish",
        "sw" => "Swahili",
        "ta" => "Tamil",
        "te" => "Telugu",
        "tg" => "Tajik",
        "th" => "Thai",
        "ti" => "Tigrinya",
        "tk" => "Turkmen",
        "tl" => "Tagalog",
        "tn" => "Tswana",
        "tpi" => "Tok Pisin",
        "tr" => "Turkish",
        "ts" => "Tsonga",
        "uk" => "Ukrainian",
        "ur" => "Urdu",
        "uz" => "Uzbek",
        "ve" => "Venda",
        "vi" => "Vietnamese",
        "wa" => "Walloon",
        "xh" => "Xhosa",
        "yi" => "Yiddish",
        "zh" => "Chinese",
        "zu" => "Zulu",
        _ => return None,
    })
}

/// Maps an ISO 3166-1 alpha-2 region code to its English display name.
/// Covers regions commonly paired with Hunspell language codes.
fn region_name(code: &str) -> Option<&'static str> {
    Some(match code.to_uppercase().as_str() {
        "AE" => "UAE",
        "AR" => "Argentina",
        "AT" => "Austria",
        "AU" => "Australia",
        "BA" => "Bosnia and Herzegovina",
        "BD" => "Bangladesh",
        "BE" => "Belgium",
        "BG" => "Bulgaria",
        "BH" => "Bahrain",
        "BO" => "Bolivia",
        "BR" => "Brazil",
        "BY" => "Belarus",
        "CA" => "Canada",
        "CH" => "Switzerland",
        "CL" => "Chile",
        "CN" => "China",
        "CO" => "Colombia",
        "CR" => "Costa Rica",
        "CU" => "Cuba",
        "CY" => "Cyprus",
        "CZ" => "Czechia",
        "DE" => "Germany",
        "DK" => "Denmark",
        "DO" => "Dominican Republic",
        "DZ" => "Algeria",
        "EC" => "Ecuador",
        "EE" => "Estonia",
        "EG" => "Egypt",
        "ES" => "Spain",
        "ET" => "Ethiopia",
        "FI" => "Finland",
        "FR" => "France",
        "GB" => "UK",
        "GE" => "Georgia",
        "GH" => "Ghana",
        "GR" => "Greece",
        "GT" => "Guatemala",
        "HK" => "Hong Kong",
        "HN" => "Honduras",
        "HR" => "Croatia",
        "HT" => "Haiti",
        "HU" => "Hungary",
        "ID" => "Indonesia",
        "IE" => "Ireland",
        "IL" => "Israel",
        "IN" => "India",
        "IQ" => "Iraq",
        "IR" => "Iran",
        "IS" => "Iceland",
        "IT" => "Italy",
        "JM" => "Jamaica",
        "JO" => "Jordan",
        "JP" => "Japan",
        "KE" => "Kenya",
        "KH" => "Cambodia",
        "KR" => "South Korea",
        "KW" => "Kuwait",
        "KZ" => "Kazakhstan",
        "LA" => "Laos",
        "LB" => "Lebanon",
        "LK" => "Sri Lanka",
        "LT" => "Lithuania",
        "LU" => "Luxembourg",
        "LV" => "Latvia",
        "LY" => "Libya",
        "MA" => "Morocco",
        "MD" => "Moldova",
        "ME" => "Montenegro",
        "MK" => "North Macedonia",
        "MN" => "Mongolia",
        "MT" => "Malta",
        "MX" => "Mexico",
        "MY" => "Malaysia",
        "NG" => "Nigeria",
        "NI" => "Nicaragua",
        "NL" => "Netherlands",
        "NO" => "Norway",
        "NP" => "Nepal",
        "NZ" => "New Zealand",
        "OM" => "Oman",
        "PA" => "Panama",
        "PE" => "Peru",
        "PH" => "Philippines",
        "PK" => "Pakistan",
        "PL" => "Poland",
        "PR" => "Puerto Rico",
        "PT" => "Portugal",
        "PY" => "Paraguay",
        "QA" => "Qatar",
        "RO" => "Romania",
        "RS" => "Serbia",
        "RU" => "Russia",
        "RW" => "Rwanda",
        "SA" => "Saudi Arabia",
        "SD" => "Sudan",
        "SE" => "Sweden",
        "SG" => "Singapore",
        "SI" => "Slovenia",
        "SK" => "Slovakia",
        "SV" => "El Salvador",
        "SY" => "Syria",
        "TH" => "Thailand",
        "TJ" => "Tajikistan",
        "TM" => "Turkmenistan",
        "TN" => "Tunisia",
        "TR" => "Türkiye",
        "TW" => "Taiwan",
        "TZ" => "Tanzania",
        "UA" => "Ukraine",
        "UG" => "Uganda",
        "US" => "US",
        "UY" => "Uruguay",
        "UZ" => "Uzbekistan",
        "VE" => "Venezuela",
        "VN" => "Vietnam",
        "YE" => "Yemen",
        "ZA" => "South Africa",
        "ZM" => "Zambia",
        "ZW" => "Zimbabwe",
        _ => return None,
    })
}

/// Returns a user-facing display name for a locale code.
/// "en_GB" → "English (UK)", "pt_BR" → "Portuguese (Brazil)".
/// Falls back to the raw code when the language or region is unknown.
pub fn display_name(code: &str) -> String {
    let (lang, region) = parse_locale(code);
    let lang = lang_name(lang).map(gettext).unwrap_or_else(|| code.to_string());
    match region.and_then(region_name) {
        Some(r) => format!("{} ({})", lang, gettext(r)),
        None => lang,
    }
}

/// Returns a short label for a locale: the uppercase 2-letter (or 3-letter for
/// 639-3) language code. "en_GB" → "EN", "pt_BR" → "PT", "ast" → "AST".
pub fn short_code(code: &str) -> String {
    let (lang, _) = parse_locale(code);
    lang.to_uppercase()
}
