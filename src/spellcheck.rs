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
