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
        private unowned Adw.SwitchRow notification_preview_row;
        [GtkChild]
        private unowned Adw.SpinRow notification_preview_length_row;
        [GtkChild]
        private unowned Adw.SpinRow background_frequency_row;
        
        // Do Not Disturb page widgets
        [GtkChild]
        private unowned Adw.SwitchRow dnd_enabled_row;
        [GtkChild]
        private unowned Adw.SwitchRow dnd_scheduled_row;
        [GtkChild]
        private unowned Adw.ActionRow dnd_start_time_row;
        [GtkChild]
        private unowned Adw.ActionRow dnd_end_time_row;
        [GtkChild]
        private unowned Gtk.Entry dnd_start_time_entry;
        [GtkChild]
        private unowned Gtk.Entry dnd_end_time_entry;
        [GtkChild]
        private unowned Adw.PreferencesGroup dnd_notifications_group;
        [GtkChild]
        private unowned Adw.SwitchRow dnd_background_row;
        [GtkChild]
        private unowned Adw.ActionRow dnd_contact_messages_row;
        [GtkChild]
        private unowned Gtk.CheckButton dnd_contact_messages_check;
        [GtkChild]
        private unowned Adw.ActionRow dnd_group_messages_row;
        [GtkChild]
        private unowned Gtk.CheckButton dnd_group_messages_check;
        [GtkChild]
        private unowned Adw.ActionRow dnd_mentions_row;
        [GtkChild]
        private unowned Gtk.CheckButton dnd_mentions_check;
        [GtkChild]
        private unowned Adw.ActionRow dnd_voice_calls_row;
        [GtkChild]
        private unowned Gtk.CheckButton dnd_voice_calls_check;
        [GtkChild]
        private unowned Adw.ActionRow dnd_video_calls_row;
        [GtkChild]
        private unowned Gtk.CheckButton dnd_video_calls_check;
        [GtkChild]
        private unowned Adw.ActionRow dnd_media_messages_row;
        [GtkChild]
        private unowned Gtk.CheckButton dnd_media_messages_check;
        [GtkChild]
        private unowned Adw.ActionRow dnd_status_updates_row;
        [GtkChild]
        private unowned Gtk.CheckButton dnd_status_updates_check;
        
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

        
        private Settings settings;

        public PreferencesDialog() {
            settings = new Settings(Config.APP_ID);
            
            setup_general_settings();
            setup_accessibility_settings();
            setup_notification_settings();
            setup_dnd_settings();
            setup_spell_checking_settings();

            // Initialize UI visibility
            on_system_notifications_changed();
            on_notifications_changed();
            on_dnd_changed();
            on_dnd_scheduled_changed();
            on_dnd_background_changed();
            on_spell_checking_changed();
            on_developer_tools_changed();
            on_webview_zoom_changed();

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
            
            // Preview settings
            settings.bind("notification-preview-enabled", notification_preview_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("notification-preview-length", notification_preview_length_row, "value", SettingsBindFlags.DEFAULT);
            
            // Background frequency
            settings.bind("background-notification-frequency", background_frequency_row, "value", SettingsBindFlags.DEFAULT);
        }
        
        private void setup_dnd_settings() {
            // DND main settings
            settings.bind("dnd-enabled", dnd_enabled_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("dnd-scheduled", dnd_scheduled_row, "active", SettingsBindFlags.DEFAULT);
            
            // DND time settings
            settings.bind("dnd-start-time", dnd_start_time_entry, "text", SettingsBindFlags.DEFAULT);
            settings.bind("dnd-end-time", dnd_end_time_entry, "text", SettingsBindFlags.DEFAULT);
            
            // DND background notifications master toggle
            settings.bind("dnd-background-notifications", dnd_background_row, "active", SettingsBindFlags.DEFAULT);
            
            // DND notification type checkboxes
            settings.bind("dnd-contact-messages", dnd_contact_messages_check, "active", SettingsBindFlags.DEFAULT);
            settings.bind("dnd-group-messages", dnd_group_messages_check, "active", SettingsBindFlags.DEFAULT);
            settings.bind("dnd-mentions", dnd_mentions_check, "active", SettingsBindFlags.DEFAULT);
            settings.bind("dnd-voice-calls", dnd_voice_calls_check, "active", SettingsBindFlags.DEFAULT);
            settings.bind("dnd-video-calls", dnd_video_calls_check, "active", SettingsBindFlags.DEFAULT);
            settings.bind("dnd-media-messages", dnd_media_messages_check, "active", SettingsBindFlags.DEFAULT);
            settings.bind("dnd-status-updates", dnd_status_updates_check, "active", SettingsBindFlags.DEFAULT);
        }
        
        private void setup_spell_checking_settings() {
            // Spell checking settings
            settings.bind("spell-checking-enabled", spell_enabled_row, "active", SettingsBindFlags.DEFAULT);
            settings.bind("spell-checking-auto-detect", spell_auto_detect_row, "active", SettingsBindFlags.DEFAULT);

            // Update current languages display
            update_current_languages_display();
        }
        
        private void connect_signals() {
            // Spell checking language buttons
            add_language_button.clicked.connect(on_add_language_clicked);

            // Listen for settings changes that affect UI
            settings.changed["system-notifications-enabled"].connect(on_system_notifications_changed);
            settings.changed["notifications-enabled"].connect(on_notifications_changed);
            settings.changed["background-notifications-mode"].connect(on_notifications_changed);
            settings.changed["dnd-enabled"].connect(on_dnd_changed);
            settings.changed["dnd-scheduled"].connect(on_dnd_scheduled_changed);
            settings.changed["dnd-background-notifications"].connect(on_dnd_background_changed);
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
        
        private void on_dnd_changed() {
            // Update DND Notifications group visibility based on either DND mode being enabled
            var manual_enabled = settings.get_boolean("dnd-enabled");
            var scheduled_enabled = settings.get_boolean("dnd-scheduled");
            var any_dnd_enabled = manual_enabled || scheduled_enabled;
            
            // Show DND Notifications group if either DND mode is active
            dnd_notifications_group.visible = any_dnd_enabled;
            
            // Also update scheduled DND time controls visibility
            on_dnd_scheduled_changed();
        }
        
        private void on_dnd_scheduled_changed() {
            // Update UI visibility for scheduled DND time controls
            var scheduled = settings.get_boolean("dnd-scheduled");
            
            // Show time controls only when scheduled DND is enabled
            dnd_start_time_row.visible = scheduled;
            dnd_end_time_row.visible = scheduled;
            
            // Keep entry sensitivity for when controls are visible
            dnd_start_time_entry.sensitive = scheduled;
            dnd_end_time_entry.sensitive = scheduled;
            
            // Update DND Notifications group visibility (called from on_dnd_changed too)
            var manual_enabled = settings.get_boolean("dnd-enabled");
            var any_dnd_enabled = manual_enabled || scheduled;
            dnd_notifications_group.visible = any_dnd_enabled;
        }
        
        private void on_dnd_background_changed() {
            // Update visibility of individual notification type checkboxes based on background notifications toggle
            var background_enabled = settings.get_boolean("dnd-background-notifications");
            dnd_contact_messages_row.visible = background_enabled;
            dnd_group_messages_row.visible = background_enabled;
            dnd_mentions_row.visible = background_enabled;
            dnd_voice_calls_row.visible = background_enabled;
            dnd_video_calls_row.visible = background_enabled;
            dnd_media_messages_row.visible = background_enabled;
            dnd_status_updates_row.visible = background_enabled;
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