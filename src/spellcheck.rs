use std::collections::HashSet;
use std::iter::FromIterator;

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
        if path.exists() && path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    if let Ok(name) = entry.file_name().into_string() {
                        if name.ends_with(".dic") {
                            // "en_US.dic" -> "en_US"
                            let lang = &name[..name.len() - 4];
                            available_dicts.insert(lang.to_string());
                        }
                    }
                }
            }
        }
    }
    
    let mut sorted_dicts: Vec<String> = Vec::from_iter(available_dicts);
    sorted_dicts.sort();
    sorted_dicts
}



/// Matches a system locale (e.g. "en_GB.UTF-8") to an available dictionary.
pub fn match_locale_to_dictionary(locale: &str, available_dicts: &[String]) -> Option<String> {
    let dict_set: HashSet<&String> = HashSet::from_iter(available_dicts);
    
    // Strip encoding (e.g. en_GB.UTF-8 -> en_GB)
    let normalized_locale = locale.split('.').next().unwrap_or(locale).to_string();

    // 1. Exact match
    if dict_set.contains(&normalized_locale) {
        return Some(normalized_locale);
    }

    // 2. Variant match (e.g. en_US -> en_GB)
    if let Some((base, _)) = normalized_locale.split_once('_') {
        for dict in available_dicts {
            if dict.starts_with(base) {
                return Some(dict.clone());
            }
        }
    }
    
    // 3. Fallback to base match if exact normalized is found in dicts (unlikely for "en_US" vs "en_US.dic" logic but safe)
    if dict_set.contains(&normalized_locale) {
        return Some(normalized_locale);
    }

    None
}
