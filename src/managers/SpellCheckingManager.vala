/*
 * Copyright (C) 2025 Thiago Fernandes
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 */

namespace Karere {

    /**
     * Manages spell checking functionality including dictionary discovery and WebKit integration
     *
     * This class handles:
     * - Discovery of available hunspell dictionaries from multiple paths
     * - Validation of language codes against available dictionaries
     * - Configuration of WebKitGTK spell checking
     * - Locale to dictionary matching with intelligent fallback
     */
    public class SpellCheckingManager : GLib.Object {
        private SettingsManager settings_manager;
        private GLib.HashTable<string, string> available_dictionaries;
        private WebKit.WebContext? web_context = null;

        public SpellCheckingManager() {
            settings_manager = SettingsManager.get_instance();
            available_dictionaries = new GLib.HashTable<string, string>(str_hash, str_equal);

            // Scan for available dictionaries
            scan_dictionary_paths();

            debug("SpellCheckingManager initialized with %d dictionaries", get_dictionary_count());
        }

        /**
         * Scan multiple filesystem paths for hunspell dictionaries
         */
        private void scan_dictionary_paths() {
            string[] search_paths = {
                "/app/share/hunspell",           // Flatpak bundled dictionaries
                "/usr/share/hunspell",            // GNOME runtime/system dictionaries
                "/run/host/usr/share/hunspell",   // Host system dictionaries (if accessible)
                Environment.get_variable("WEBKIT_SPELL_CHECKER_DIR")  // Custom override
            };

            foreach (var path in search_paths) {
                if (path == null) {
                    continue;
                }
                scan_directory_for_dictionaries(path);
            }
        }

        /**
         * Scan a single directory for hunspell dictionary files
         *
         * @param directory_path Path to scan for .dic and .aff files
         */
        private void scan_directory_for_dictionaries(string directory_path) {
            if (!FileUtils.test(directory_path, FileTest.IS_DIR)) {
                return;
            }

            try {
                var dir = Dir.open(directory_path, 0);
                string? name = null;

                while ((name = dir.read_name()) != null) {
                    if (name.has_suffix(".dic")) {
                        // Extract language code from filename (e.g., "en_US.dic" -> "en_US")
                        var lang_code = name.substring(0, name.length - 4);

                        if (validate_dictionary(lang_code, directory_path)) {
                            // Only add if not already in dictionary (priority order)
                            if (!available_dictionaries.contains(lang_code)) {
                                available_dictionaries.insert(lang_code, directory_path);
                                debug("Found dictionary: %s at %s", lang_code, directory_path);
                            }
                        }
                    }
                }
            } catch (FileError e) {
                debug("Error scanning directory %s: %s", directory_path, e.message);
            }
        }

        /**
         * Validate that both .dic and .aff files exist for a language
         *
         * @param lang_code Language code (e.g., "en_US")
         * @param directory_path Directory containing the dictionary files
         * @return true if both files exist and are valid
         */
        private bool validate_dictionary(string lang_code, string directory_path) {
            var dic_path = Path.build_filename(directory_path, lang_code + ".dic");
            var aff_path = Path.build_filename(directory_path, lang_code + ".aff");

            // Check if both files exist
            var dic_exists = FileUtils.test(dic_path, FileTest.EXISTS);
            var aff_exists = FileUtils.test(aff_path, FileTest.EXISTS);

            if (!dic_exists || !aff_exists) {
                if (dic_exists && !aff_exists) {
                    // Handle symlinked dictionaries - try to resolve the link
                    if (FileUtils.test(dic_path, FileTest.IS_SYMLINK)) {
                        try {
                            var link_target = FileUtils.read_link(dic_path);
                            // If it's a relative symlink, resolve it relative to directory
                            if (!Path.is_absolute(link_target)) {
                                link_target = Path.build_filename(directory_path, link_target);
                            }

                            // Extract the base name and check for .aff
                            var target_base = Path.get_basename(link_target);
                            if (target_base.has_suffix(".dic")) {
                                var target_lang = target_base.substring(0, target_base.length - 4);
                                var target_aff = Path.build_filename(Path.get_dirname(link_target), target_lang + ".aff");
                                if (FileUtils.test(target_aff, FileTest.EXISTS)) {
                                    return true;
                                }
                            }
                        } catch (FileError e) {
                            debug("Error resolving symlink for %s: %s", lang_code, e.message);
                        }
                    }
                }
                return false;
            }

            return true;
        }

        /**
         * Match a system locale to an available dictionary
         *
         * @param locale Locale string (e.g., "en_US.UTF-8", "pt_BR", "en")
         * @return Matched language code or null if no match found
         */
        public string? match_locale_to_dictionary(string? locale) {
            if (locale == null || locale.strip() == "") {
                return null;
            }

            // Parse locale string - handle formats like "en_US.UTF-8", "en_US", "en"
            string normalized_locale = locale;

            // Remove encoding suffix (e.g., ".UTF-8")
            var parts = normalized_locale.split(".");
            if (parts.length > 1) {
                normalized_locale = parts[0];
            }

            // Remove @variant suffix (e.g., "@euro")
            parts = normalized_locale.split("@");
            if (parts.length > 1) {
                normalized_locale = parts[0];
            }

            // 1. Try exact match
            if (is_language_available(normalized_locale)) {
                debug("Exact match found for locale %s: %s", locale, normalized_locale);
                return normalized_locale;
            }

            // 2. Try other variants of the same language (e.g., en_US -> en_GB, en_AU)
            if (normalized_locale.contains("_")) {
                var lang_parts = normalized_locale.split("_");
                var base_lang = lang_parts[0];

                // Look for any variant of the base language
                var available = get_available_languages();
                foreach (var lang in available) {
                    if (lang.has_prefix(base_lang + "_")) {
                        debug("Fallback variant found for locale %s: %s", locale, lang);
                        return lang;
                    }
                }

                // 3. Try base language without country code
                if (is_language_available(base_lang)) {
                    debug("Fallback to base language found for locale %s: %s", locale, base_lang);
                    return base_lang;
                }
            }

            // No match found
            debug("No dictionary match found for locale: %s", locale);
            return null;
        }

        /**
         * Get count of available dictionaries
         *
         * @return Number of available dictionaries
         */
        public int get_dictionary_count() {
            return (int)available_dictionaries.size();
        }

        /**
         * Get list of available language codes
         *
         * @return Sorted array of language codes
         */
        public string[] get_available_languages() {
            var languages = new GLib.GenericArray<string>();
            var keys = available_dictionaries.get_keys();
            foreach (var lang in keys) {
                languages.add(lang);
            }
            // Sort the languages
            languages.sort((a, b) => strcmp(a, b));

            // Convert to string array
            string[] result = new string[languages.length];
            for (int i = 0; i < languages.length; i++) {
                result[i] = languages[i];
            }
            return result;
        }

        /**
         * Check if a language is available
         *
         * @param language Language code to check
         * @return true if dictionary is available for this language
         */
        public bool is_language_available(string language) {
            return available_dictionaries.contains(language);
        }

        /**
         * Get status message for spell checking
         *
         * @return Human-readable status message
         */
        public string get_status_message() {
            var enabled = settings_manager.get_boolean_with_fallback("spell-checking-enabled", false);
            if (!enabled) {
                return "Spell checking disabled";
            }

            var dict_count = get_dictionary_count();
            if (dict_count == 0) {
                return "Spell checking unavailable - no dictionaries found";
            }

            var languages = get_validated_languages();
            if (languages.length == 0) {
                return "Spell checking enabled but no languages configured";
            }

            return "Spell checking active with %d languages (%s)".printf(languages.length, string.joinv(", ", languages));
        }

        /**
         * Configure WebKit spell checking
         *
         * @param context WebKit context to configure
         */
        public void configure_webkit(WebKit.WebContext context) {
            this.web_context = context;

            var enabled = settings_manager.get_boolean_with_fallback("spell-checking-enabled", false);
            info("Configuring WebKit spell checking: enabled=%s", enabled.to_string());

            if (!enabled) {
                context.set_spell_checking_enabled(false);
                context.set_spell_checking_languages({});
                return;
            }

            var languages = get_validated_languages();
            if (languages.length == 0) {
                warning("Spell checking enabled but no dictionaries available");
                context.set_spell_checking_enabled(false);
                return;
            }

            context.set_spell_checking_enabled(true);
            context.set_spell_checking_languages(languages);
            info("Spell checking enabled with languages: %s", string.joinv(", ", languages));
        }

        /**
         * Update spell checking settings
         */
        public void update_spell_checking() {
            if (web_context == null) {
                warning("Cannot update spell checking: WebContext not initialized");
                return;
            }

            configure_webkit(web_context);
        }

        /**
         * Get validated list of spell checking languages based on settings
         *
         * @return Array of validated language codes
         */
        private string[] get_validated_languages() {
            var auto_detect = settings_manager.get_boolean_with_fallback("spell-checking-auto-detect", true);

            if (auto_detect) {
                // Auto-detect from system locale
                var locale = Intl.setlocale(LocaleCategory.MESSAGES, null);
                var matched = match_locale_to_dictionary(locale);
                if (matched != null) {
                    return {matched};
                }
                warning("Auto-detect enabled but no matching dictionary for locale: %s", locale ?? "null");
                return {};
            }

            // Use user-specified languages
            var user_languages = settings_manager.get_strv_with_fallback("spell-checking-languages", {});
            var validated = new GLib.GenericArray<string>();

            foreach (var lang in user_languages) {
                if (is_language_available(lang)) {
                    validated.add(lang);
                } else {
                    warning("Language '%s' not available, skipping", lang);
                }
            }

            // Convert to string array
            string[] result = new string[validated.length];
            for (int i = 0; i < validated.length; i++) {
                result[i] = validated[i];
            }
            return result;
        }

        /**
         * Setup settings change listeners
         */
        public void setup_settings_listeners() {
            var settings = settings_manager.get_settings();
            if (settings == null) {
                warning("Cannot setup settings listeners: settings not initialized");
                return;
            }

            settings.changed["spell-checking-enabled"].connect(() => {
                update_spell_checking();
            });

            settings.changed["spell-checking-auto-detect"].connect(() => {
                update_spell_checking();
            });

            settings.changed["spell-checking-languages"].connect(() => {
                update_spell_checking();
            });
        }
    }
}
