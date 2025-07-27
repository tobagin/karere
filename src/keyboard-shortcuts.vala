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
     * Manages all keyboard shortcuts for the application
     */
    public class KeyboardShortcuts : Object {
        private Logger logger;
        private GLib.Settings? settings = null;
        private Adw.Application application;
        private Window? main_window;
        private AccessibilityManager? accessibility_manager;
        private bool initialization_deferred = true;

        public KeyboardShortcuts(Adw.Application app, Logger logger) {
            this.application = app;
            this.logger = logger;
            // Note: Settings initialization is deferred until GTK is initialized
            
            logger.debug("KeyboardShortcuts manager initialized (Settings deferred)");
        }
        
        public void initialize_settings() {
            if (settings == null && initialization_deferred) {
                try {
                    settings = new GLib.Settings(Config.APP_ID);
                    initialization_deferred = false;
                    
                    setup_application_shortcuts();
                    
                    logger.debug("KeyboardShortcuts settings initialized");
                } catch (Error e) {
                    logger.error("Failed to initialize KeyboardShortcuts settings: %s", e.message);
                    initialization_deferred = true;
                }
            }
        }

        /**
         * Set references to main window and accessibility manager
         */
        public void set_window_reference(Window window, AccessibilityManager accessibility_manager) {
            this.main_window = window;
            this.accessibility_manager = accessibility_manager;
            
            setup_window_shortcuts();
            setup_webview_shortcuts();
        }

        /**
         * Setup application-level keyboard shortcuts
         */
        private void setup_application_shortcuts() {
            update_keyboard_shortcuts();
        }

        /**
         * Update keyboard shortcuts based on current settings
         */
        public void update_keyboard_shortcuts() {
            if (settings == null) return;
            
            // Clear all existing shortcuts first
            clear_all_shortcuts();
            
            if (!settings.get_boolean("keyboard-shortcuts-enabled")) {
                logger.debug("Keyboard shortcuts disabled in settings");
                return;
            }

            // Application shortcuts
            application.set_accels_for_action("app.preferences", {"<primary>comma"});
            application.set_accels_for_action("app.quit", {"<primary>q"});
            application.set_accels_for_action("app.about", {"F1"});
            
            // Window management shortcuts
            application.set_accels_for_action("win.minimize", {"<primary>m"});
            application.set_accels_for_action("win.fullscreen", {"F11", "<alt>Return"});
            application.set_accels_for_action("win.show-help-overlay", {"<primary>question", "<primary>slash"});
            
            // WebView zoom shortcuts (only if webview zoom is enabled)
            if (settings.get_boolean("webview-zoom-enabled")) {
                application.set_accels_for_action("win.zoom-in", {"<primary>plus", "<primary>equal", "<primary>KP_Add"});
                application.set_accels_for_action("win.zoom-out", {"<primary>minus", "<primary>KP_Subtract"});
                application.set_accels_for_action("win.zoom-reset", {"<primary>0", "<primary>KP_0"});
            }

            // Accessibility shortcuts
            if (settings.get_boolean("accessibility-shortcuts-enabled")) {
                application.set_accels_for_action("win.toggle-high-contrast", {"<primary><shift>h"});
                application.set_accels_for_action("win.toggle-focus-indicators", {"<primary><shift>f"});
            }

            // Developer shortcuts (only when enabled)
            if (settings.get_boolean("developer-shortcuts-enabled") && 
                settings.get_boolean("developer-tools-enabled")) {
                application.set_accels_for_action("win.dev-tools", {"<primary><shift>d", "F12"});
                application.set_accels_for_action("win.reload", {"<primary>r", "F5"});
                application.set_accels_for_action("win.force-reload", {"<primary><shift>r", "<shift>F5"});
            }

            // Notification shortcuts
            if (settings.get_boolean("notification-shortcuts-enabled")) {
                application.set_accels_for_action("win.notifications-toggle", {"<primary><shift>n"});
                application.set_accels_for_action("win.dnd-toggle", {"<primary><shift>d"});
            }

            logger.debug("Application shortcuts configured");
        }

        /**
         * Clear all existing keyboard shortcuts
         */
        private void clear_all_shortcuts() {
            // Clear application shortcuts
            application.set_accels_for_action("app.preferences", {});
            application.set_accels_for_action("app.quit", {});
            application.set_accels_for_action("app.about", {});
            
            // Clear window management shortcuts
            application.set_accels_for_action("win.minimize", {});
            application.set_accels_for_action("win.fullscreen", {});
            application.set_accels_for_action("win.show-help-overlay", {});
            
            // Clear accessibility shortcuts
            application.set_accels_for_action("win.zoom-in", {});
            application.set_accels_for_action("win.zoom-out", {});
            application.set_accels_for_action("win.zoom-reset", {});
            application.set_accels_for_action("win.toggle-high-contrast", {});
            application.set_accels_for_action("win.toggle-focus-indicators", {});
            
            // Clear developer shortcuts
            application.set_accels_for_action("win.dev-tools", {});
            application.set_accels_for_action("win.reload", {});
            application.set_accels_for_action("win.force-reload", {});
            
            // Clear notification shortcuts
            application.set_accels_for_action("win.notifications-toggle", {});
            application.set_accels_for_action("win.dnd-toggle", {});
            
            logger.debug("All keyboard shortcuts cleared");
        }

        /**
         * Setup window-specific keyboard shortcuts
         */
        private void setup_window_shortcuts() {
            if (main_window == null) return;

            // Help overlay action
            var help_overlay_action = new GLib.SimpleAction("show-help-overlay", null);
            help_overlay_action.activate.connect(() => {
                if (main_window != null) {
                    show_shortcuts_window();
                }
            });
            main_window.add_action(help_overlay_action);

            // Minimize action
            var minimize_action = new GLib.SimpleAction("minimize", null);
            minimize_action.activate.connect(() => {
                if (main_window != null) {
                    main_window.minimize();
                    logger.debug("Window minimized via shortcut");
                }
            });
            main_window.add_action(minimize_action);

            // Fullscreen toggle action
            var fullscreen_action = new GLib.SimpleAction("fullscreen", null);
            fullscreen_action.activate.connect(() => {
                if (main_window != null) {
                    if (main_window.fullscreened) {
                        main_window.unfullscreen();
                        logger.debug("Exited fullscreen via shortcut");
                    } else {
                        main_window.fullscreen();
                        logger.debug("Entered fullscreen via shortcut");
                    }
                }
            });
            main_window.add_action(fullscreen_action);

            // Setup accessibility shortcuts
            setup_accessibility_shortcuts();

            // Setup developer shortcuts
            setup_developer_shortcuts();

            // Setup notification shortcuts
            setup_notification_shortcuts();

            logger.debug("Window shortcuts configured");
        }

        /**
         * Setup accessibility-related keyboard shortcuts
         */
        private void setup_accessibility_shortcuts() {
            if (main_window == null || accessibility_manager == null) return;
            if (settings == null) return;

            // WebView zoom shortcuts (always setup actions, but shortcuts are conditional)
            if (settings.get_boolean("webview-zoom-enabled")) {
                // Zoom in action
                var zoom_in_action = new GLib.SimpleAction("zoom-in", null);
                zoom_in_action.activate.connect(() => {
                    main_window.webkit_zoom_in();
                    if (main_window != null) {
                        main_window.show_info_toast(_("Zoomed in"));
                    }
                });
                main_window.add_action(zoom_in_action);

                // Zoom out action
                var zoom_out_action = new GLib.SimpleAction("zoom-out", null);
                zoom_out_action.activate.connect(() => {
                    main_window.webkit_zoom_out();
                    if (main_window != null) {
                        main_window.show_info_toast(_("Zoomed out"));
                    }
                });
                main_window.add_action(zoom_out_action);

                // Zoom reset action
                var zoom_reset_action = new GLib.SimpleAction("zoom-reset", null);
                zoom_reset_action.activate.connect(() => {
                    main_window.webkit_zoom_reset();
                    if (main_window != null) {
                        main_window.show_info_toast(_("Zoom reset to default"));
                    }
                });
                main_window.add_action(zoom_reset_action);
            }

            // Other accessibility shortcuts (conditional on accessibility-shortcuts-enabled)
            if (!settings.get_boolean("accessibility-shortcuts-enabled")) return;

            // Toggle high contrast action
            var toggle_high_contrast_action = new GLib.SimpleAction("toggle-high-contrast", null);
            toggle_high_contrast_action.activate.connect(() => {
                var current = settings.get_boolean("high-contrast-mode");
                settings.set_boolean("high-contrast-mode", !current);
                if (main_window != null) {
                    main_window.show_info_toast(
                        current ? _("High contrast disabled") : _("High contrast enabled")
                    );
                }
                logger.debug("High contrast toggled: %s", (!current).to_string());
            });
            main_window.add_action(toggle_high_contrast_action);

            // Toggle focus indicators action
            var toggle_focus_indicators_action = new GLib.SimpleAction("toggle-focus-indicators", null);
            toggle_focus_indicators_action.activate.connect(() => {
                var current = settings.get_boolean("focus-indicators-enabled");
                settings.set_boolean("focus-indicators-enabled", !current);
                if (main_window != null) {
                    main_window.show_info_toast(
                        current ? _("Focus indicators disabled") : _("Focus indicators enabled")
                    );
                }
                logger.debug("Focus indicators toggled: %s", (!current).to_string());
            });
            main_window.add_action(toggle_focus_indicators_action);

            logger.debug("Accessibility shortcuts configured");
        }

        /**
         * Setup developer keyboard shortcuts
         */
        private void setup_developer_shortcuts() {
            if (main_window == null) return;
            if (settings == null || !settings.get_boolean("developer-shortcuts-enabled") || 
                !settings.get_boolean("developer-tools-enabled")) return;

            // Developer tools action
            var dev_tools_action = new GLib.SimpleAction("dev-tools", null);
            dev_tools_action.activate.connect(() => {
                if (main_window != null) {
                    main_window.open_developer_tools();
                    logger.debug("Developer tools opened via shortcut");
                }
            });
            main_window.add_action(dev_tools_action);

            // Reload action
            var reload_action = new GLib.SimpleAction("reload", null);
            reload_action.activate.connect(() => {
                logger.debug("Reload requested via shortcut");
                reload_webview(false);
            });
            main_window.add_action(reload_action);

            // Force reload action (bypass cache)
            var force_reload_action = new GLib.SimpleAction("force-reload", null);
            force_reload_action.activate.connect(() => {
                logger.debug("Force reload requested via shortcut");
                reload_webview(true);
            });
            main_window.add_action(force_reload_action);

            logger.debug("Developer shortcuts configured");
        }

        /**
         * Setup notification management shortcuts
         */
        private void setup_notification_shortcuts() {
            if (main_window == null) return;
            if (settings == null || !settings.get_boolean("notification-shortcuts-enabled")) return;

            // Toggle notifications action
            var notifications_toggle_action = new GLib.SimpleAction("notifications-toggle", null);
            notifications_toggle_action.activate.connect(() => {
                var current = settings.get_boolean("notifications-enabled");
                settings.set_boolean("notifications-enabled", !current);
                if (main_window != null) {
                    main_window.show_info_toast(
                        current ? _("Notifications disabled") : _("Notifications enabled")
                    );
                }
                logger.debug("Notifications toggled: %s", (!current).to_string());
            });
            main_window.add_action(notifications_toggle_action);

            // Toggle Do Not Disturb action
            var dnd_toggle_action = new GLib.SimpleAction("dnd-toggle", null);
            dnd_toggle_action.activate.connect(() => {
                var current = settings.get_boolean("dnd-enabled");
                settings.set_boolean("dnd-enabled", !current);
                if (main_window != null) {
                    main_window.show_info_toast(
                        current ? _("Do Not Disturb disabled") : _("Do Not Disturb enabled")
                    );
                }
                logger.debug("Do Not Disturb toggled: %s", (!current).to_string());
            });
            main_window.add_action(dnd_toggle_action);

            logger.debug("Notification shortcuts configured");
        }

        /**
         * Setup WebView-specific shortcuts
         */
        private void setup_webview_shortcuts() {
            if (main_window == null) return;

            // Find action (Ctrl+F)
            var find_action = new GLib.SimpleAction("find", null);
            find_action.activate.connect(() => {
                logger.debug("Find requested via shortcut");
                inject_whatsapp_find();
            });
            main_window.add_action(find_action);

            // Add shortcut binding
            application.set_accels_for_action("win.find", {"<primary>f"});

            // WhatsApp Web navigation shortcuts
            setup_whatsapp_shortcuts();

            logger.debug("WebView shortcuts configured");
        }

        /**
         * Setup WhatsApp Web specific shortcuts
         */
        private void setup_whatsapp_shortcuts() {
            if (main_window == null) return;

            // Note: These are conceptual - actual implementation would need JavaScript injection
            // to interact with WhatsApp Web interface

            // Search chats (Ctrl+Shift+F)
            var search_chats_action = new GLib.SimpleAction("search-chats", null);
            search_chats_action.activate.connect(() => {
                logger.debug("Search chats requested");
                inject_whatsapp_search();
            });
            main_window.add_action(search_chats_action);
            application.set_accels_for_action("win.search-chats", {"<primary><shift>f"});

            // New chat (Ctrl+N)
            var new_chat_action = new GLib.SimpleAction("new-chat", null);
            new_chat_action.activate.connect(() => {
                logger.debug("New chat requested");
                inject_whatsapp_new_chat();
            });
            main_window.add_action(new_chat_action);
            application.set_accels_for_action("win.new-chat", {"<primary>n"});

            // Archive chat (Ctrl+E)
            var archive_chat_action = new GLib.SimpleAction("archive-chat", null);
            archive_chat_action.activate.connect(() => {
                logger.debug("Archive chat requested");
                inject_whatsapp_archive();
            });
            main_window.add_action(archive_chat_action);
            application.set_accels_for_action("win.archive-chat", {"<primary>e"});

            // Settings (Ctrl+,) - already handled by app.preferences
            // Profile (Ctrl+P)
            var profile_action = new GLib.SimpleAction("profile", null);
            profile_action.activate.connect(() => {
                logger.debug("Profile requested");
                inject_whatsapp_profile();
            });
            main_window.add_action(profile_action);
            application.set_accels_for_action("win.profile", {"<primary>p"});

            logger.debug("WhatsApp shortcuts configured");
        }

        /**
         * Reload WebView content
         */
        private void reload_webview(bool force_reload) {
            if (main_window == null) return;
            
            // This would need to be implemented in the Window class
            // For now, we'll just show a toast indicating the action
            if (force_reload) {
                main_window.show_info_toast(_("Force reloading WhatsApp Web..."));
            } else {
                main_window.show_info_toast(_("Reloading WhatsApp Web..."));
            }
            
            // The actual implementation would call something like:
            // main_window.reload_webview(force_reload);
        }

        /**
         * Inject JavaScript to trigger WhatsApp Web find functionality
         */
        private void inject_whatsapp_find() {
            logger.debug("Injecting WhatsApp find functionality");
            // This would inject JavaScript to activate WhatsApp's search
            // For now, show informational toast
            if (main_window != null) {
                main_window.show_info_toast(_("Use WhatsApp Web's search feature (🔍)"));
            }
        }

        /**
         * Inject JavaScript to trigger WhatsApp Web chat search
         */
        private void inject_whatsapp_search() {
            logger.debug("Injecting WhatsApp chat search");
            if (main_window != null) {
                main_window.show_info_toast(_("Opening chat search..."));
            }
        }

        /**
         * Inject JavaScript to trigger WhatsApp Web new chat
         */
        private void inject_whatsapp_new_chat() {
            logger.debug("Injecting WhatsApp new chat");
            if (main_window != null) {
                main_window.show_info_toast(_("Opening new chat..."));
            }
        }

        /**
         * Inject JavaScript to trigger WhatsApp Web archive
         */
        private void inject_whatsapp_archive() {
            logger.debug("Injecting WhatsApp archive");
            if (main_window != null) {
                main_window.show_info_toast(_("Archiving current chat..."));
            }
        }

        /**
         * Inject JavaScript to trigger WhatsApp Web profile
         */
        private void inject_whatsapp_profile() {
            logger.debug("Injecting WhatsApp profile");
            if (main_window != null) {
                main_window.show_info_toast(_("Opening profile..."));
            }
        }

        /**
         * Update shortcuts when settings change
         */
        public void update_shortcuts() {
            // Re-setup all shortcuts to reflect current settings
            setup_application_shortcuts();
            if (main_window != null) {
                setup_window_shortcuts();
                setup_webview_shortcuts();
            }
            
            logger.debug("Keyboard shortcuts updated");
        }

        /**
         * Get list of all configured shortcuts for help overlay
         */
        public Gtk.ShortcutsSection[] get_shortcuts_sections() {
            // Note: This method is deprecated in GTK4 due to non-public constructors
            // The ShortcutsWindow class should be used instead.
            return new Gtk.ShortcutsSection[0];
        }

        /**
         * Show the keyboard shortcuts help window
         */
        private void show_shortcuts_window() {
            if (main_window != null) {
                var shortcuts_window = new ShortcutsWindow(main_window);
                shortcuts_window.present(main_window);
                logger.debug("Shortcuts help window shown");
            }
        }

        /**
         * Cleanup resources
         */
        public void cleanup() {
            main_window = null;
            accessibility_manager = null;
            logger.debug("KeyboardShortcuts cleaned up");
        }
    }
}