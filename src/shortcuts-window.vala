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
     * Keyboard shortcuts help dialog
     */
    public class ShortcutsWindow : Adw.Dialog {
        private GLib.Settings settings;
        private Adw.PreferencesPage content_page;
        private Gtk.ScrolledWindow scrolled_window;

        public ShortcutsWindow(Gtk.Window parent) {
            Object();

            settings = new GLib.Settings(Config.APP_ID);

            setup_dialog();
            setup_settings_listeners();

            debug("ShortcutsWindow created");
        }

        /**
         * Setup the shortcuts dialog
         */
        private void setup_dialog() {
            title = _("Keyboard Shortcuts");
            content_width = 600;
            content_height = 500;
            
            // Create main content
            var content_box = new Gtk.Box(Gtk.Orientation.VERTICAL, 0);
            content_box.add_css_class("dialog-content");
            
            // Create header
            var header = new Adw.HeaderBar();
            header.show_title = true;
            header.title_widget = new Adw.WindowTitle(_("Keyboard Shortcuts"), "");
            content_box.append(header);
            
            // Create scrolled window with preferences
            var scrolled = new Gtk.ScrolledWindow();
            scrolled.vexpand = true;
            scrolled.hscrollbar_policy = Gtk.PolicyType.NEVER;
            scrolled.vscrollbar_policy = Gtk.PolicyType.AUTOMATIC;
            
            content_page = create_shortcuts_content();
            scrolled.child = content_page;
            scrolled_window = scrolled;
            
            content_box.append(scrolled);
            child = content_box;
        }

        /**
         * Create the shortcuts content using AdwPreferencesPage
         */
        private Adw.PreferencesPage create_shortcuts_content() {
            var page = new Adw.PreferencesPage();
            
            // If keyboard shortcuts are completely disabled, show a message
            if (!settings.get_boolean("keyboard-shortcuts-enabled")) {
                var group = new Adw.PreferencesGroup();
                group.title = _("Keyboard Shortcuts Disabled");
                group.description = _("Keyboard shortcuts are currently disabled. Enable them in Preferences → Accessibility.");
                page.add(group);
                return page;
            }
            
            // Add general shortcuts group
            add_general_shortcuts(page);
            
            // Add window management shortcuts group
            add_window_shortcuts(page);
            
            // Add WhatsApp Web shortcuts group
            add_whatsapp_shortcuts(page);
            
            // Add WebView zoom shortcuts (if webview zoom is enabled)
            if (settings.get_boolean("webview-zoom-enabled")) {
                add_webview_zoom_shortcuts(page);
            }
            
            // Add accessibility shortcuts (if enabled)
            if (settings.get_boolean("accessibility-shortcuts-enabled")) {
                add_accessibility_shortcuts(page);
            }
            
            // Add developer shortcuts (if enabled)
            if (settings.get_boolean("developer-shortcuts-enabled") && 
                settings.get_boolean("developer-tools-enabled")) {
                add_developer_shortcuts(page);
            }
            
            // Add notification shortcuts (if enabled)
            if (settings.get_boolean("notification-shortcuts-enabled")) {
                add_notification_shortcuts(page);
            }
            
            return page;
        }

        /**
         * Add general application shortcuts
         */
        private void add_general_shortcuts(Adw.PreferencesPage page) {
            var group = new Adw.PreferencesGroup();
            group.title = _("General");
            group.description = _("Application-wide keyboard shortcuts");
            
            add_shortcut_row(group, _("Show this help"), "Ctrl+?", _("Display keyboard shortcuts"));
            add_shortcut_row(group, _("Preferences"), "Ctrl+,", _("Open application preferences"));
            add_shortcut_row(group, _("About"), "F1", _("Show application information"));
            add_shortcut_row(group, _("Quit application"), "Ctrl+Q", _("Close the application"));
            
            page.add(group);
        }

        /**
         * Add window management shortcuts
         */
        private void add_window_shortcuts(Adw.PreferencesPage page) {
            var group = new Adw.PreferencesGroup();
            group.title = _("Window");
            group.description = _("Window management shortcuts");
            
            add_shortcut_row(group, _("Minimize window"), "Ctrl+M", _("Minimize the main window"));
            add_shortcut_row(group, _("Fullscreen"), "F11 or Alt+Return", _("Toggle fullscreen mode"));
            
            page.add(group);
        }

        /**
         * Add WhatsApp Web shortcuts
         */
        private void add_whatsapp_shortcuts(Adw.PreferencesPage page) {
            var group = new Adw.PreferencesGroup();
            group.title = _("WhatsApp Web");
            group.description = _("Shortcuts for WhatsApp Web functionality");
            
            add_shortcut_row(group, _("Find in chat"), "Ctrl+F", _("Search within current chat"));
            add_shortcut_row(group, _("Search chats"), "Ctrl+Shift+F", _("Search across all chats"));
            add_shortcut_row(group, _("New chat"), "Ctrl+N", _("Start a new chat"));
            add_shortcut_row(group, _("Archive current chat"), "Ctrl+E", _("Archive the current conversation"));
            add_shortcut_row(group, _("Open profile"), "Ctrl+P", _("Open user profile"));
            add_shortcut_row(group, _("Reload WhatsApp Web"), "F5", _("Refresh the WhatsApp Web interface"));
            
            page.add(group);
        }

        /**
         * Add accessibility shortcuts
         */
        private void add_accessibility_shortcuts(Adw.PreferencesPage page) {
            var group = new Adw.PreferencesGroup();
            group.title = _("Accessibility");
            group.description = _("Accessibility shortcuts");
            
            // Only add general accessibility shortcuts (high contrast, focus indicators)
            add_shortcut_row(group, _("Toggle high contrast"), "Ctrl+Shift+H", _("Toggle high contrast mode"));
            add_shortcut_row(group, _("Toggle focus indicators"), "Ctrl+Shift+F", _("Toggle focus indicators visibility"));
            
            page.add(group);
        }

        /**
         * Add developer shortcuts
         */
        private void add_developer_shortcuts(Adw.PreferencesPage page) {
            var group = new Adw.PreferencesGroup();
            group.title = _("Developer");
            group.description = _("Development and debugging shortcuts");
            
            add_shortcut_row(group, _("Developer tools"), "F12 or Ctrl+Shift+D", _("Open web developer tools"));
            add_shortcut_row(group, _("Reload page"), "F5 or Ctrl+R", _("Reload the current page"));
            add_shortcut_row(group, _("Force reload"), "Shift+F5 or Ctrl+Shift+R", _("Reload bypassing cache"));
            
            page.add(group);
        }

        /**
         * Add WebView zoom shortcuts
         */
        private void add_webview_zoom_shortcuts(Adw.PreferencesPage page) {
            var group = new Adw.PreferencesGroup();
            group.title = _("WebView Zoom");
            group.description = _("WebView content zoom shortcuts");
            
            add_shortcut_row(group, _("Zoom in"), "Ctrl++ or Ctrl+Numpad+", _("Increase page zoom level"));
            add_shortcut_row(group, _("Zoom out"), "Ctrl+- or Ctrl+Numpad-", _("Decrease page zoom level"));
            add_shortcut_row(group, _("Reset zoom"), "Ctrl+0 or Ctrl+Numpad0", _("Reset zoom to default level"));
            
            page.add(group);
        }

        /**
         * Add notification management shortcuts
         */
        private void add_notification_shortcuts(Adw.PreferencesPage page) {
            var group = new Adw.PreferencesGroup();
            group.title = _("Notifications");
            group.description = _("Notification management shortcuts");
            
            add_shortcut_row(group, _("Toggle notifications"), "Ctrl+Shift+N", _("Enable or disable notifications"));
            add_shortcut_row(group, _("Toggle Do Not Disturb"), "Ctrl+Shift+D", _("Toggle Do Not Disturb mode"));
            
            page.add(group);
        }

        /**
         * Helper method to add a shortcut row to a group
         */
        private void add_shortcut_row(Adw.PreferencesGroup group, string title, string shortcut, string description) {
            var row = new Adw.ActionRow();
            row.title = title;
            row.subtitle = description;
            
            // Create shortcut label with styling
            var shortcut_label = new Gtk.Label(shortcut);
            shortcut_label.add_css_class("keycap");
            shortcut_label.add_css_class("monospace");
            shortcut_label.halign = Gtk.Align.END;
            shortcut_label.valign = Gtk.Align.CENTER;
            
            row.add_suffix(shortcut_label);
            group.add(row);
        }

        /**
         * Setup settings listeners to refresh shortcuts when settings change
         */
        private void setup_settings_listeners() {
            // Listen for changes to master toggles that affect shortcut visibility
            settings.changed["webview-zoom-enabled"].connect(refresh_shortcuts_content);
            settings.changed["accessibility-shortcuts-enabled"].connect(refresh_shortcuts_content);
            settings.changed["developer-shortcuts-enabled"].connect(refresh_shortcuts_content);
            settings.changed["developer-tools-enabled"].connect(refresh_shortcuts_content);
            settings.changed["notification-shortcuts-enabled"].connect(refresh_shortcuts_content);
            settings.changed["keyboard-shortcuts-enabled"].connect(refresh_shortcuts_content);
        }

        /**
         * Refresh the shortcuts content when settings change
         */
        private void refresh_shortcuts_content() {
            if (scrolled_window != null) {
                // Create new content
                var new_content = create_shortcuts_content();
                
                // Replace the old content
                scrolled_window.child = new_content;
                content_page = new_content;

                debug("Shortcuts content refreshed");
            }
        }
    }
}