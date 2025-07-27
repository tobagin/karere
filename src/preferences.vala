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

    [GtkTemplate (ui = "/io/github/tobagin/karere/preferences.ui")]
    public class Preferences : Adw.PreferencesDialog {
        
        // General page widgets
        [GtkChild]
        private unowned Adw.ComboRow theme_row;
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
        private unowned Adw.SwitchRow notification_preview_row;
        [GtkChild]
        private unowned Adw.SpinRow notification_preview_length_row;
        [GtkChild]
        private unowned Adw.SpinRow background_frequency_row;
        
        // Do Not Disturb page widgets
        [GtkChild]
        private unowned Adw.SwitchRow dnd_enabled_row;
        [GtkChild]
        private unowned Adw.SwitchRow dnd_background_row;
        [GtkChild]
        private unowned Adw.SwitchRow dnd_scheduled_row;
        [GtkChild]
        private unowned Gtk.Entry dnd_start_time_entry;
        [GtkChild]
        private unowned Gtk.Entry dnd_end_time_entry;
        
        // Spell checking page widgets
        [GtkChild]
        private unowned Adw.SwitchRow spell_enabled_row;
        [GtkChild]
        private unowned Adw.SwitchRow spell_auto_detect_row;
        [GtkChild]
        private unowned Adw.PreferencesGroup spell_languages_group;
        [GtkChild]
        private unowned Gtk.Label current_languages_label;
        [GtkChild]
        private unowned Adw.ActionRow add_language_row;
        [GtkChild]
        private unowned Gtk.Button add_language_button;
        
        // Crash reporting page widgets
        [GtkChild]
        private unowned Adw.SwitchRow crash_reporter_row;
        [GtkChild]
        private unowned Adw.SwitchRow crash_system_info_row;
        [GtkChild]
        private unowned Adw.SwitchRow crash_logs_row;
        [GtkChild]
        private unowned Gtk.Label crash_count_label;
        [GtkChild]
        private unowned Gtk.Button crash_clear_button;
        
        // Logging page widgets
        [GtkChild]
        private unowned Adw.ComboRow log_level_row;
        [GtkChild]
        private unowned Adw.SwitchRow console_logging_row;
        [GtkChild]
        private unowned Adw.SwitchRow file_logging_row;
        [GtkChild]
        private unowned Adw.SpinRow log_file_max_size_row;
        [GtkChild]
        private unowned Adw.SpinRow log_file_retention_row;
        [GtkChild]
        private unowned Gtk.Button log_location_button;
        [GtkChild]
        private unowned Gtk.Button log_view_button;
        [GtkChild]
        private unowned Gtk.Button log_clear_button;
        
        private Settings settings;
        
        public Preferences() {
            settings = new Settings(Config.APP_ID);
            
            setup_general_settings();
            setup_accessibility_settings();
            setup_notification_settings();
            setup_dnd_settings();
            setup_spell_checking_settings();
            
            // Initialize UI visibility
            on_notifications_changed();
            on_spell_checking_changed();
            on_developer_tools_changed();
            on_webview_zoom_changed();
            setup_crash_reporting_settings();
            setup_logging_settings();
            
            
            connect_signals();
        }
        
        private void setup_general_settings() {
            // Theme setting
            settings.bind("theme-preference", theme_row, "selected", SettingsBindFlags.DEFAULT);
            
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
            
            // Preview settings
            settings.bind("notification-preview-enabled", notification_preview_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("notification-preview-length", notification_preview_length_row, "value", SettingsBindFlags.DEFAULT);
            
            // Background frequency
            settings.bind("background-notification-frequency", background_frequency_row, "value", SettingsBindFlags.DEFAULT);
        }
        
        private void setup_dnd_settings() {
            // DND main settings
            settings.bind("dnd-enabled", dnd_enabled_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("dnd-background-notifications", dnd_background_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("dnd-scheduled", dnd_scheduled_row, "active", SettingsBindFlags.DEFAULT);
            
            // DND time settings
            settings.bind("dnd-start-time", dnd_start_time_entry, "text", SettingsBindFlags.DEFAULT);
            settings.bind("dnd-end-time", dnd_end_time_entry, "text", SettingsBindFlags.DEFAULT);
        }
        
        private void setup_spell_checking_settings() {
            // Spell checking settings
            settings.bind("spell-checking-enabled", spell_enabled_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("spell-checking-auto-detect", spell_auto_detect_row, "active", SettingsBindFlags.DEFAULT);
            
            // Update current languages display
            update_current_languages_display();
        }
        
        private void setup_crash_reporting_settings() {
            // Crash reporting settings
            settings.bind("crash-reporter-enabled", crash_reporter_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("crash-include-system-info", crash_system_info_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("crash-include-logs", crash_logs_row, "active", SettingsBindFlags.DEFAULT);
            
            // Update crash count display
            update_crash_count();
        }
        
        private void setup_logging_settings() {
            // Log level setting
            settings.bind("log-level", log_level_row, "selected", SettingsBindFlags.DEFAULT);
            
            // Logging toggles
            settings.bind("console-logging-enabled", console_logging_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("file-logging-enabled", file_logging_row, "active", SettingsBindFlags.DEFAULT);
            
            // Log file settings
            settings.bind("log-file-max-size", log_file_max_size_row, "value", SettingsBindFlags.DEFAULT);
            settings.bind("log-file-retention", log_file_retention_row, "value", SettingsBindFlags.DEFAULT);
        }
        
        private void connect_signals() {
            // Spell checking language buttons
            add_language_button.clicked.connect(on_add_language_clicked);
            
            // Crash clear button
            crash_clear_button.clicked.connect(on_crash_clear_clicked);
            
            // Logging buttons
            log_location_button.clicked.connect(on_log_location_clicked);
            log_view_button.clicked.connect(on_log_view_clicked);
            log_clear_button.clicked.connect(on_log_clear_clicked);
            
            // Listen for settings changes that affect UI
            settings.changed["notifications-enabled"].connect(on_notifications_changed);
            settings.changed["background-notifications-mode"].connect(on_notifications_changed);
            settings.changed["dnd-scheduled"].connect(on_dnd_scheduled_changed);
            settings.changed["spell-checking-enabled"].connect(on_spell_checking_changed);
            settings.changed["spell-checking-auto-detect"].connect(on_spell_checking_changed);
            settings.changed["spell-checking-languages"].connect(on_spell_checking_changed);
            settings.changed["developer-tools-enabled"].connect(on_developer_tools_changed);
            settings.changed["file-logging-enabled"].connect(on_file_logging_changed);
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
        
        
        private void on_add_language_clicked() {
            // Create a simple dialog for adding languages
            var dialog = new Adw.AlertDialog(_("Add Language"), _("Enter a language code (e.g., en_US, fr_FR, de_DE):"));
            
            var entry = new Gtk.Entry();
            // TRANSLATORS: Placeholder text for language code entry (example format)
            entry.placeholder_text = _("en_US");
            entry.margin_top = 12;
            entry.margin_bottom = 12;
            entry.margin_start = 12;
            entry.margin_end = 12;
            
            dialog.set_extra_child(entry);
            dialog.add_response("cancel", _("Cancel"));
            dialog.add_response("add", _("Add"));
            dialog.set_response_appearance("add", Adw.ResponseAppearance.SUGGESTED);
            dialog.set_default_response("add");
            dialog.set_close_response("cancel");
            
            dialog.response.connect((response) => {
                if (response == "add") {
                    var language = entry.get_text().strip();
                    if (language.length > 0) {
                        add_spell_checking_language(language);
                    }
                }
            });
            
            dialog.present(this);
        }
        
        private void add_spell_checking_language(string language) {
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
            show_toast(_("Language '%s' added successfully").printf(language));
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
                var locale = Intl.setlocale(LocaleCategory.MESSAGES, null);
                if (locale != null) {
                    var parts = locale.split(".");
                    var lang_code = parts[0];
                    current_languages_label.set_text(_("Auto: %s").printf(lang_code));
                } else {
                    current_languages_label.set_text(_("Auto: en_US"));
                }
            } else {
                // Show user-specified languages
                current_languages_label.set_text(string.joinv(", ", languages));
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
        
        private void on_crash_clear_clicked() {
            settings.set_int("crash-reports-count", 0);
            update_crash_count();
            
            // TRANSLATORS: Confirmation message when crash statistics are cleared
            show_toast(_("Crash statistics cleared"));
        }
        
        private void update_crash_count() {
            var count = settings.get_int("crash-reports-count");
            crash_count_label.set_text(count.to_string());
        }
        
        private void on_notifications_changed() {
            // Update UI sensitivity based on notifications enabled
            var enabled = settings.get_boolean("notifications-enabled");
            background_notifications_row.sensitive = enabled;
            system_notifications_row.sensitive = enabled;
            notification_sound_row.sensitive = enabled;
            notification_preview_row.sensitive = enabled;
            notification_preview_length_row.sensitive = enabled && settings.get_boolean("notification-preview-enabled");
            background_frequency_row.sensitive = enabled && settings.get_int("background-notifications-mode") == 0; // Always
        }
        
        private void on_dnd_scheduled_changed() {
            // Update UI sensitivity for scheduled DND
            var scheduled = settings.get_boolean("dnd-scheduled");
            dnd_start_time_entry.sensitive = scheduled;
            dnd_end_time_entry.sensitive = scheduled;
        }
        
        private void on_spell_checking_changed() {
            // Update UI sensitivity for spell checking
            var enabled = settings.get_boolean("spell-checking-enabled");
            var auto_detect = settings.get_boolean("spell-checking-auto-detect");
            
            spell_auto_detect_row.visible = enabled;
            spell_languages_group.visible = enabled;
            
            // Hide add language button when auto-detect is enabled
            add_language_row.visible = enabled && !auto_detect;
            
            // Update current languages display
            update_current_languages_display();
        }
        
        private void on_developer_tools_changed() {
            // Update UI visibility for developer tools
            var enabled = settings.get_boolean("developer-tools-enabled");
            open_dev_tools_row.visible = enabled;
        }
        
        private void on_log_location_clicked() {
            // Open log directory in file manager
            try {
                var log_dir = Environment.get_user_data_dir() + "/" + Config.APP_NAME + "/logs";
                AppInfo.launch_default_for_uri("file://" + log_dir, null);
            } catch (Error e) {
                // TRANSLATORS: Error message when log directory cannot be opened. %s is the error details
                show_toast(_("Could not open log directory: %s").printf(e.message));
            }
        }
        
        private void on_log_view_clicked() {
            // Open current log file in default text editor
            try {
                var log_dir = Environment.get_user_data_dir() + "/" + Config.APP_NAME + "/logs";
                var now = new DateTime.now_local();
                var today_filename = "karere-%s.log".printf(now.format("%Y-%m-%d"));
                var log_file = log_dir + "/" + today_filename;
                
                // Check if today's log file exists, if not try yesterday's
                var file = File.new_for_path(log_file);
                if (!file.query_exists()) {
                    var yesterday = now.add_days(-1);
                    var yesterday_filename = "karere-%s.log".printf(yesterday.format("%Y-%m-%d"));
                    log_file = log_dir + "/" + yesterday_filename;
                    file = File.new_for_path(log_file);
                    
                    if (!file.query_exists()) {
                        // TRANSLATORS: Error message when no recent log files are found
                        show_toast(_("No recent log files found"));
                        return;
                    }
                }
                
                AppInfo.launch_default_for_uri("file://" + log_file, null);
            } catch (Error e) {
                // TRANSLATORS: Error message when log file cannot be opened. %s is the error details
                show_toast(_("Could not open log file: %s").printf(e.message));
            }
        }
        
        private void on_log_clear_clicked() {
            // Show confirmation dialog before clearing logs
            var dialog = new Adw.AlertDialog(
                "Clear All Log Files?",
                "This will permanently delete all application log files. This action cannot be undone."
            );
            
            dialog.add_response("cancel", "Cancel");
            dialog.add_response("clear", "Clear All Logs");
            dialog.set_response_appearance("clear", Adw.ResponseAppearance.DESTRUCTIVE);
            dialog.set_default_response("cancel");
            dialog.set_close_response("cancel");
            
            dialog.response.connect((response) => {
                if (response == "clear") {
                    clear_all_log_files();
                }
            });
            
            dialog.present(this);
        }
        
        private void clear_all_log_files() {
            // Clear all log files
            try {
                var log_dir = Environment.get_user_data_dir() + "/" + Config.APP_NAME + "/logs";
                var dir = File.new_for_path(log_dir);
                if (dir.query_exists()) {
                    var enumerator = dir.enumerate_children("*", FileQueryInfoFlags.NONE);
                    FileInfo info;
                    while ((info = enumerator.next_file()) != null) {
                        var child = dir.resolve_relative_path(info.get_name());
                        if (info.get_name().has_suffix(".log")) {
                            child.delete();
                        }
                    }
                }
                // TRANSLATORS: Confirmation message when all log files are cleared
                show_toast(_("All log files cleared"));
            } catch (Error e) {
                // TRANSLATORS: Error message when log files cannot be cleared. %s is the error details
                show_toast(_("Could not clear log files: %s").printf(e.message));
            }
        }
        
        private void on_file_logging_changed() {
            // Update UI sensitivity for file logging settings
            var enabled = settings.get_boolean("file-logging-enabled");
            log_file_max_size_row.sensitive = enabled;
            log_file_retention_row.sensitive = enabled;
            log_view_button.sensitive = enabled;
            log_clear_button.sensitive = enabled;
        }
        
        private void on_webview_zoom_changed() {
            // Update UI visibility for webview zoom settings
            var enabled = settings.get_boolean("webview-zoom-enabled");
            webkit_zoom_level_row.visible = enabled;
            webview_zoom_controls_row.visible = enabled;
        }
        
        private void on_shortcuts_help_clicked() {
            // Show keyboard shortcuts help window
            var shortcuts_window = new ShortcutsWindow(this.get_root() as Gtk.Window);
            shortcuts_window.present(this.get_root() as Gtk.Window);
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