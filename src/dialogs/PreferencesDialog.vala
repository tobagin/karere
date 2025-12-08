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

#if DEVELOPMENT
    [GtkTemplate (ui = "/io/github/tobagin/karere/Devel/preferences.ui")]
#else
    [GtkTemplate (ui = "/io/github/tobagin/karere/preferences.ui")]
#endif
    public class PreferencesDialog : Adw.PreferencesDialog {
        
        // General page widgets
        [GtkChild]
        private unowned Adw.SwitchRow start_background_row;
        [GtkChild]
        private unowned Adw.ComboRow theme_row;
        [GtkChild]
        private unowned Adw.ActionRow download_directory_row;
        [GtkChild]
        private unowned Gtk.Button choose_directory_button;
        [GtkChild]
        private unowned Gtk.Button reset_directory_button;
        [GtkChild]
        private unowned Adw.SwitchRow developer_tools_row;
        [GtkChild]
        private unowned Adw.ActionRow open_dev_tools_row;
        [GtkChild]
        private unowned Gtk.Button dev_tools_button;
        
        // Accessibility page widgets
        [GtkChild]
        private unowned Adw.SwitchRow keyboard_shortcuts_row;
        [GtkChild]
        private unowned Adw.SwitchRow focus_indicators_row;
        [GtkChild]
        private unowned Gtk.Button shortcuts_help_button;
        [GtkChild]
        private unowned Adw.SwitchRow high_contrast_row;
        [GtkChild]
        private unowned Adw.SwitchRow reduced_motion_row;
        [GtkChild]
        private unowned Adw.SwitchRow webview_zoom_enabled_row;
        [GtkChild]
        private unowned Adw.SpinRow webkit_zoom_level_row;
        [GtkChild]
        private unowned Adw.SpinRow webkit_zoom_step_row;
        [GtkChild]
        private unowned Adw.SwitchRow webview_zoom_controls_row;
        [GtkChild]
        private unowned Adw.SwitchRow screen_reader_row;
        [GtkChild]
        private unowned Gtk.Label screen_reader_status_label;
        [GtkChild]
        private unowned Adw.SwitchRow accessibility_shortcuts_row;
        [GtkChild]
        private unowned Adw.SwitchRow developer_shortcuts_row;
        [GtkChild]
        private unowned Adw.SwitchRow notification_shortcuts_row;
        
        // Notifications page widgets
        [GtkChild]
        private unowned Adw.SwitchRow notifications_enabled_row;
        [GtkChild]
        private unowned Adw.ComboRow background_notifications_row;
        [GtkChild]
        private unowned Adw.SwitchRow system_notifications_row;
        [GtkChild]
        private unowned Adw.SwitchRow notification_sound_row;
        [GtkChild]
        private unowned Adw.SwitchRow download_notifications_row;
        [GtkChild]
        private unowned Adw.SwitchRow notification_preview_row;
        [GtkChild]
        private unowned Adw.SpinRow notification_preview_length_row;
        [GtkChild]
        private unowned Adw.SpinRow background_frequency_row;

        // Spell checking page widgets
        [GtkChild]
        private unowned Adw.ActionRow dictionary_status_row;
        [GtkChild]
        private unowned Gtk.Label dictionary_count_label;
        [GtkChild]
        private unowned Gtk.Image dictionary_status_icon;
        [GtkChild]
        private unowned Adw.SwitchRow spell_enabled_row;
        [GtkChild]
        private unowned Adw.SwitchRow spell_auto_detect_row;
        [GtkChild]
        private unowned Adw.PreferencesGroup spell_languages_group;
        [GtkChild]
        private unowned Adw.ActionRow current_languages_row;
        [GtkChild]
        private unowned Gtk.Label current_languages_label;
        [GtkChild]
        private unowned Adw.ComboRow language_selection_row;
        [GtkChild]
        private unowned Adw.ActionRow no_dictionaries_row;

        private Settings settings;
        private SpellCheckingManager? spell_checking_manager;
        private Gtk.StringList? available_languages_model;

        public PreferencesDialog() {
            settings = new Settings(Config.APP_ID);

            setup_general_settings();
            setup_accessibility_settings();
            setup_notification_settings();
            setup_spell_checking_settings();

            on_system_notifications_changed();
            on_notifications_changed();
            on_spell_checking_changed();
            on_developer_tools_changed();
            on_webview_zoom_changed();

            connect_signals();
        }
        
        private void setup_general_settings() {
            // Startup setting
            settings.bind("start-in-background", start_background_row, "active", SettingsBindFlags.DEFAULT);

            // Theme setting
            settings.bind("theme-preference", theme_row, "selected", SettingsBindFlags.DEFAULT);

            // Download directory settings
            update_download_directory_label();
            update_reset_button_visibility();
            choose_directory_button.clicked.connect(on_choose_directory_clicked);
            reset_directory_button.clicked.connect(on_reset_directory_clicked);

            // General settings
            settings.bind("developer-tools-enabled", developer_tools_row, "active", SettingsBindFlags.DEFAULT);

            // Connect developer tools button
            dev_tools_button.clicked.connect(on_open_dev_tools_clicked);
        }
        
        private void setup_accessibility_settings() {
            // Keyboard navigation settings
            settings.bind("keyboard-shortcuts-enabled", keyboard_shortcuts_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("focus-indicators-enabled", focus_indicators_row, "active", SettingsBindFlags.DEFAULT);
            shortcuts_help_button.clicked.connect(on_shortcuts_help_clicked);
            
            // Visual accessibility settings
            settings.bind("high-contrast-mode", high_contrast_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("reduced-motion-enabled", reduced_motion_row, "active", SettingsBindFlags.DEFAULT);
            
            // WebView zoom settings
            settings.bind("webview-zoom-enabled", webview_zoom_enabled_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("webkit-zoom-level", webkit_zoom_level_row, "value", SettingsBindFlags.DEFAULT);
            settings.bind("webkit-zoom-step", webkit_zoom_step_row, "value", SettingsBindFlags.DEFAULT);
            settings.bind("webview-zoom-controls-enabled", webview_zoom_controls_row, "active", SettingsBindFlags.DEFAULT);
            
            // Screen reader settings
            settings.bind("screen-reader-optimized", screen_reader_row, "active", SettingsBindFlags.DEFAULT);
            update_screen_reader_status();
            
            // Accessibility shortcuts settings
            settings.bind("accessibility-shortcuts-enabled", accessibility_shortcuts_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("developer-shortcuts-enabled", developer_shortcuts_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("notification-shortcuts-enabled", notification_shortcuts_row, "active", SettingsBindFlags.DEFAULT);
            
        }
        
        private void setup_notification_settings() {
            // Main notification settings
            settings.bind("notifications-enabled", notifications_enabled_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("background-notifications-mode", background_notifications_row, "selected", SettingsBindFlags.DEFAULT);
            settings.bind("system-notifications-enabled", system_notifications_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("notification-sound-enabled", notification_sound_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("download-notifications-enabled", download_notifications_row, "active", SettingsBindFlags.DEFAULT);

            // Preview settings
            settings.bind("notification-preview-enabled", notification_preview_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("notification-preview-length", notification_preview_length_row, "value", SettingsBindFlags.DEFAULT);

            // Background frequency
            settings.bind("background-notification-frequency", background_frequency_row, "value", SettingsBindFlags.DEFAULT);
        }

        private void setup_spell_checking_settings() {
            // Initialize spell checking manager
            spell_checking_manager = new SpellCheckingManager();

            // Spell checking settings
            settings.bind("spell-checking-enabled", spell_enabled_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("spell-checking-auto-detect", spell_auto_detect_row, "active", SettingsBindFlags.DEFAULT);

            // Update dictionary status display
            update_dictionary_status();

            // Setup language selection dropdown
            setup_language_selection();

            // Update current languages display
            update_current_languages_display();
        }

        private void update_dictionary_status() {
            if (spell_checking_manager == null) {
                return;
            }

            var dict_count = spell_checking_manager.get_dictionary_count();

            // Update dictionary count label
            if (dict_count == 0) {
                dictionary_count_label.set_text(_("No dictionaries"));
                dictionary_status_icon.visible = false;
                no_dictionaries_row.visible = true;
                language_selection_row.visible = false;
            } else if (dict_count == 1) {
                dictionary_count_label.set_text(_("1 dictionary"));
                dictionary_status_icon.visible = true;
                no_dictionaries_row.visible = false;
                language_selection_row.visible = true;
            } else {
                dictionary_count_label.set_text(_("%d dictionaries").printf(dict_count));
                dictionary_status_icon.visible = true;
                no_dictionaries_row.visible = false;
                language_selection_row.visible = true;
            }
        }

        private void setup_language_selection() {
            if (spell_checking_manager == null) {
                return;
            }

            // Create string list model for available languages
            available_languages_model = new Gtk.StringList(null);

            // Get available languages from spell checking manager
            var available_languages = spell_checking_manager.get_available_languages();

            // Add language display names to the model
            foreach (var lang_code in available_languages) {
                var display_name = get_language_display_name(lang_code);
                available_languages_model.append(display_name);
            }

            // Set model to combo row
            language_selection_row.model = available_languages_model;

            // Connect selection changed signal
            language_selection_row.notify["selected"].connect(() => {
                on_language_selected();
            });
        }

        private void on_language_selected() {
            if (spell_checking_manager == null || available_languages_model == null) {
                return;
            }

            var selected_index = language_selection_row.selected;
            if (selected_index == Gtk.INVALID_LIST_POSITION) {
                return;
            }

            // Get the selected display name
            var display_name = available_languages_model.get_string(selected_index);

            // Find the corresponding language code
            var available_languages = spell_checking_manager.get_available_languages();
            string? lang_code = null;

            for (int i = 0; i < available_languages.length; i++) {
                if (get_language_display_name(available_languages[i]) == display_name) {
                    lang_code = available_languages[i];
                    break;
                }
            }

            if (lang_code != null) {
                add_spell_checking_language(lang_code);
                // Reset selection
                language_selection_row.selected = Gtk.INVALID_LIST_POSITION;
            }
        }

        private string get_language_display_name(string lang_code) {
            // Map language codes to user-friendly names
            // This is a basic implementation - could be expanded with full locale database
            var name_map = new GLib.HashTable<string, string>(str_hash, str_equal);

            // Common languages
            name_map.insert("en_US", _("English (United States)"));
            name_map.insert("en_GB", _("English (United Kingdom)"));
            name_map.insert("es_ES", _("Spanish (Spain)"));
            name_map.insert("es_MX", _("Spanish (Mexico)"));
            name_map.insert("pt_BR", _("Portuguese (Brazil)"));
            name_map.insert("pt_PT", _("Portuguese (Portugal)"));
            name_map.insert("fr_FR", _("French (France)"));
            name_map.insert("fr", _("French"));
            name_map.insert("de_DE_frami", _("German (Germany)"));
            name_map.insert("de", _("German"));
            name_map.insert("it_IT", _("Italian (Italy)"));
            name_map.insert("ru_RU", _("Russian (Russia)"));
            name_map.insert("ar", _("Arabic"));
            name_map.insert("id_ID", _("Indonesian (Indonesia)"));
            name_map.insert("id", _("Indonesian"));
            name_map.insert("nl_NL", _("Dutch (Netherlands)"));
            name_map.insert("pl_PL", _("Polish (Poland)"));
            name_map.insert("cs_CZ", _("Czech (Czechia)"));
            name_map.insert("sv_SE", _("Swedish (Sweden)"));
            name_map.insert("fi_FI", _("Finnish (Finland)"));
            name_map.insert("no", _("Norwegian"));
            name_map.insert("da_DK", _("Danish (Denmark)"));
            name_map.insert("el_GR", _("Greek (Greece)"));
            name_map.insert("he_IL", _("Hebrew (Israel)"));
            name_map.insert("hi_IN", _("Hindi (India)"));
            name_map.insert("th_TH", _("Thai (Thailand)"));
            name_map.insert("vi", _("Vietnamese"));
            name_map.insert("zh_CN", _("Chinese (Simplified)"));
            name_map.insert("zh_TW", _("Chinese (Traditional)"));
            name_map.insert("ja_JP", _("Japanese (Japan)"));
            name_map.insert("ko_KR", _("Korean (South Korea)"));
            name_map.insert("tr_TR", _("Turkish (Turkey)"));
            name_map.insert("uk_UA", _("Ukrainian (Ukraine)"));
            name_map.insert("bg_BG", _("Bulgarian (Bulgaria)"));
            name_map.insert("ro", _("Romanian"));
            name_map.insert("hr_HR", _("Croatian (Croatia)"));
            name_map.insert("sk_SK", _("Slovak (Slovakia)"));
            name_map.insert("sl_SI", _("Slovenian (Slovenia)"));
            name_map.insert("sr", _("Serbian"));
            name_map.insert("ca", _("Catalan"));
            name_map.insert("eu", _("Basque"));
            name_map.insert("gl", _("Galician"));

            // Try to get friendly name
            var friendly_name = name_map.lookup(lang_code);
            if (friendly_name != null) {
                return "%s (%s)".printf(friendly_name, lang_code);
            }

            // Fallback: just return the code
            return lang_code;
        }

        private void update_download_directory_label() {
            var custom_dir = settings.get_string("custom-download-directory");
            if (custom_dir == "" || custom_dir == null) {
                download_directory_row.subtitle = _("Default (Downloads)");
            } else {
                download_directory_row.subtitle = custom_dir;
            }
        }

        private void update_reset_button_visibility() {
            var custom_dir = settings.get_string("custom-download-directory");
            reset_directory_button.sensitive = (custom_dir != "" && custom_dir != null);
        }

        private async void on_choose_directory_clicked() {
            var dialog = new Gtk.FileDialog();
            dialog.title = _("Select Download Directory");
            dialog.modal = true;

            // Get the root window for the dialog parent
            var root = this.get_root() as Gtk.Window;

            try {
                var file = yield dialog.select_folder(root, null);
                var path = file.get_path();

                if (path != null) {
                    settings.set_string("custom-download-directory", path);
                    update_download_directory_label();
                    update_reset_button_visibility();
                    debug("Custom download directory set to: %s", path);
                }
            } catch (Error e) {
                if (!(e is Gtk.DialogError.DISMISSED)) {
                    warning("Failed to select directory: %s", e.message);
                }
            }
        }

        private void on_reset_directory_clicked() {
            settings.set_string("custom-download-directory", "");
            update_download_directory_label();
            update_reset_button_visibility();
            debug("Download directory reset to default");
        }

        private void connect_signals() {
            // Listen for settings changes that affect UI
            settings.changed["system-notifications-enabled"].connect(on_system_notifications_changed);
            settings.changed["notifications-enabled"].connect(on_notifications_changed);
            settings.changed["background-notifications-mode"].connect(on_notifications_changed);
            settings.changed["spell-checking-enabled"].connect(on_spell_checking_changed);
            settings.changed["spell-checking-auto-detect"].connect(on_spell_checking_changed);
            settings.changed["spell-checking-languages"].connect(on_spell_checking_changed);
            settings.changed["developer-tools-enabled"].connect(on_developer_tools_changed);
            settings.changed["webview-zoom-enabled"].connect(on_webview_zoom_changed);
            
            // Accessibility settings listeners
            settings.changed["screen-reader-optimized"].connect(() => {
                update_screen_reader_status();
            });
            settings.changed["keyboard-shortcuts-enabled"].connect(() => {
                // Update shortcuts when this setting changes
                var main_window = get_root() as Karere.Window;
                if (main_window != null) {
                    var app = main_window.get_application() as Karere.Application;
                    if (app != null) {
                        var keyboard_shortcuts = app.get_keyboard_shortcuts();
                        if (keyboard_shortcuts != null) {
                            keyboard_shortcuts.update_keyboard_shortcuts();
                        }
                    }
                }
            });
            settings.changed["webkit-zoom-level"].connect(() => {
                // Update WebView zoom level when setting changes
                update_webkit_zoom();
            });
            settings.changed["webkit-zoom-step"].connect(() => {
                // WebView zoom step setting changed
                // This affects future zoom operations but doesn't need immediate action
            });
            settings.changed["webview-zoom-enabled"].connect(() => {
                // Update keyboard shortcuts when webview zoom setting changes
                var main_window = get_root() as Karere.Window;
                if (main_window != null) {
                    var app = main_window.get_application() as Karere.Application;
                    if (app != null) {
                        var keyboard_shortcuts = app.get_keyboard_shortcuts();
                        if (keyboard_shortcuts != null) {
                            keyboard_shortcuts.update_keyboard_shortcuts();
                        }
                    }
                }
            });
        }
        
        private void on_open_dev_tools_clicked() {
            // Get the main window and open developer tools
            var main_window = get_root() as Karere.Window;
            if (main_window != null) {
                main_window.open_developer_tools();
                // Close the preferences dialog
                close();
            }
        }


        private void add_spell_checking_language(string language) {
            // Validate language is available
            if (spell_checking_manager != null && !spell_checking_manager.is_language_available(language)) {
                show_toast(_("Dictionary for '%s' is not available").printf(language));
                return;
            }

            var current_languages = settings.get_strv("spell-checking-languages");

            // Check if language already exists
            foreach (var lang in current_languages) {
                if (lang == language) {
                    show_toast(_("Language '%s' is already added").printf(language));
                    return;
                }
            }

            // Add the new language
            string[] new_languages = new string[current_languages.length + 1];
            for (int i = 0; i < current_languages.length; i++) {
                new_languages[i] = current_languages[i];
            }
            new_languages[current_languages.length] = language;

            settings.set_strv("spell-checking-languages", new_languages);

            // Get friendly name for toast
            var friendly_name = get_language_display_name(language);
            show_toast(_("Added: %s").printf(friendly_name));
        }
        
        private void update_current_languages_display() {
            var spell_enabled = settings.get_boolean("spell-checking-enabled");
            if (!spell_enabled) {
                current_languages_label.set_text(_("Disabled"));
                return;
            }

            var auto_detect = settings.get_boolean("spell-checking-auto-detect");
            var languages = settings.get_strv("spell-checking-languages");

            if (auto_detect || languages.length == 0) {
                // Show auto-detected language
                if (spell_checking_manager != null) {
                    var locale = Intl.setlocale(LocaleCategory.MESSAGES, null);
                    var matched = spell_checking_manager.match_locale_to_dictionary(locale);
                    if (matched != null) {
                        var friendly_name = get_language_display_name(matched);
                        current_languages_label.set_text(_("Auto: %s").printf(friendly_name));
                    } else {
                        current_languages_label.set_text(_("Auto (no match)"));
                    }
                } else {
                    current_languages_label.set_text(_("Auto"));
                }
            } else {
                // Show user-specified languages with friendly names
                var friendly_names = new GLib.GenericArray<string>();
                foreach (var lang in languages) {
                    friendly_names.add(get_language_display_name(lang));
                }

                if (friendly_names.length > 0) {
                    // Convert to array for joinv
                    string[] names_array = new string[friendly_names.length];
                    for (int i = 0; i < friendly_names.length; i++) {
                        names_array[i] = friendly_names[i];
                    }
                    current_languages_label.set_text(string.joinv(", ", names_array));
                } else {
                    current_languages_label.set_text(_("None"));
                }
            }
        }
        
        private void show_toast(string message) {
            // Get the main window and use its toast overlay
            var main_window = get_root() as Karere.Window;
            if (main_window != null) {
                var toast = new Adw.Toast(message);
                // Access the toast overlay through the main window
                // Since we can't directly access private members, we'll use a public method
                main_window.show_toast(toast);
            } else {
                // Fallback to debug print if we can't access the main window
                print("Toast: %s\n", message);
            }
        }
        
        private void on_system_notifications_changed() {
            // Update UI sensitivity based on system notifications enabled - master toggle
            var system_enabled = settings.get_boolean("system-notifications-enabled");
            notifications_enabled_row.sensitive = system_enabled;
            background_notifications_row.sensitive = system_enabled;
            notification_sound_row.sensitive = system_enabled;
            notification_preview_row.sensitive = system_enabled;
            notification_preview_length_row.sensitive = system_enabled && settings.get_boolean("notification-preview-enabled");
            background_frequency_row.sensitive = system_enabled && settings.get_int("background-notifications-mode") == 0; // Always
        }

        private void on_notifications_changed() {
            // Update UI sensitivity based on message notifications enabled (secondary toggle)
            var system_enabled = settings.get_boolean("system-notifications-enabled");
            var message_enabled = settings.get_boolean("notifications-enabled");

            // Only update sensitivity if system notifications are enabled
            if (system_enabled) {
                background_notifications_row.sensitive = message_enabled;
                notification_sound_row.sensitive = message_enabled;
                notification_preview_row.sensitive = message_enabled;
                notification_preview_length_row.sensitive = message_enabled && settings.get_boolean("notification-preview-enabled");
                background_frequency_row.sensitive = message_enabled && settings.get_int("background-notifications-mode") == 0; // Always
            }
        }
        
        private void on_spell_checking_changed() {
            // Update UI sensitivity for spell checking
            var enabled = settings.get_boolean("spell-checking-enabled");
            var auto_detect = settings.get_boolean("spell-checking-auto-detect");

            spell_auto_detect_row.visible = enabled;
            spell_languages_group.visible = enabled;

            // Hide language selection when auto-detect is enabled or no dictionaries
            if (spell_checking_manager != null) {
                var dict_count = spell_checking_manager.get_dictionary_count();
                language_selection_row.visible = enabled && !auto_detect && dict_count > 0;
            }

            // Update current languages display
            update_current_languages_display();
        }
        
        private void on_developer_tools_changed() {
            // Update UI visibility for developer tools
            var enabled = settings.get_boolean("developer-tools-enabled");
            open_dev_tools_row.visible = enabled;
        }
        
        private void on_webview_zoom_changed() {
            // Update UI visibility for webview zoom settings
            var enabled = settings.get_boolean("webview-zoom-enabled");
            webkit_zoom_level_row.visible = enabled;
            webkit_zoom_step_row.visible = enabled;
            webview_zoom_controls_row.visible = enabled;
        }
        
        private void on_shortcuts_help_clicked() {
            // Show keyboard shortcuts help dialog
            var shortcuts_dialog = new ShortcutsDialog(this.get_root() as Gtk.Window);
            shortcuts_dialog.present(this.get_root() as Gtk.Window);
        }
        
        private void update_screen_reader_status() {
            // Check if screen reader is detected
            var at_spi_detected = GLib.Environment.get_variable("AT_SPI_IOR") != null;
            var accessibility_enabled = GLib.Environment.get_variable("GNOME_ACCESSIBILITY") == "1";
            
            if (at_spi_detected || accessibility_enabled) {
                screen_reader_status_label.set_text(_("Detected"));
                screen_reader_status_label.remove_css_class("dim-label");
                screen_reader_status_label.add_css_class("success");
            } else {
                screen_reader_status_label.set_text(_("Not detected"));
                screen_reader_status_label.add_css_class("dim-label");
                screen_reader_status_label.remove_css_class("success");
            }
        }
        
        private void update_webkit_zoom() {
            // Update WebView zoom level when setting changes
            var main_window = get_root() as Karere.Window;
            if (main_window != null) {
                var zoom_level = settings.get_double("webkit-zoom-level");
                main_window.update_webkit_zoom(zoom_level);
                // Also update any other webkit settings that might have changed
                main_window.update_webkit_settings();
            }
        }

    }
}