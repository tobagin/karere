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

    public class Application : Adw.Application {
        private Window? main_window = null;
        private Logger logger;
        private CrashReporter crash_reporter;
        private NotificationManager notification_manager;
        private AccessibilityManager accessibility_manager;
        private KeyboardShortcuts keyboard_shortcuts;
        private Settings? settings = null;

        public Application() {
            Object(
                application_id: Config.APP_ID,
                flags: ApplicationFlags.HANDLES_COMMAND_LINE
            );
            
            // Set the application to quit when the last window is closed
            set_option_context_description(_("A modern, native GTK4/LibAdwaita wrapper for WhatsApp Web"));
            register_session = true;
            
            logger = new Logger();
            crash_reporter = new CrashReporter();
            notification_manager = new NotificationManager(this, logger);
            accessibility_manager = new AccessibilityManager(logger);
            keyboard_shortcuts = new KeyboardShortcuts(this, logger);
            
            // Note: Settings initialization is deferred to startup() method
            // to ensure GTK is properly initialized first
        }

        public override void startup() {
            base.startup();
            
            logger.info("Application starting up");
            
            // Initialize Settings now that GTK is initialized
            settings = new Settings(Config.APP_ID);
            
            // Initialize logger settings now that GTK is initialized
            logger.initialize_settings();
            
            // Initialize crash reporter settings and logger now that GTK is initialized
            crash_reporter.initialize_settings_and_logger(logger);
            
            // Initialize notification manager settings now that GTK is initialized
            notification_manager.initialize_settings();
            
            // Initialize accessibility manager settings now that GTK is initialized
            accessibility_manager.initialize_settings();
            
            // Initialize keyboard shortcuts settings now that GTK is initialized
            keyboard_shortcuts.initialize_settings();
            
            // Update log handlers with user preferences now that settings are available
            setup_log_handlers_with_preferences();
            
            // Initialize crash reporting
            crash_reporter.initialize();
            
            // Set up application actions
            setup_actions();
            
            // Set up keyboard shortcuts
            setup_keyboard_shortcuts();
            
            // Set up theme handling
            setup_theme_handling();
            
            logger.info("Application startup complete");
        }

        public override void activate() {
            base.activate();
            
            logger.info("Application activated");
            
            // Create window if it doesn't exist
            if (main_window == null) {
                logger.info("Creating new main window");
                main_window = new Window(this);
                
                // Set up accessibility and keyboard shortcuts for the new window
                accessibility_manager.set_main_window(main_window);
                keyboard_shortcuts.set_window_reference(main_window, accessibility_manager);
                
                main_window.present();
                logger.info("Main window created and presented with accessibility support");
                
                // Check if we should show What's New dialog
                check_and_show_whats_new();
            } else {
                // Show the existing window (it might be hidden)
                main_window.set_visible(true);
                main_window.present();
                
                // Reapply focus indicators setting when window is shown
                main_window.update_focus_indicators();
                
                // Reset background notification state when window is shown
                notification_manager.on_window_focus_changed(true);
                
                logger.info("Main window shown and presented");
            }
        }

        public override int command_line(ApplicationCommandLine command_line) {
            logger.info("Processing command line arguments");
            
            var options = command_line.get_options_dict();
            
            // Handle version option
            if (options.contains("version")) {
                // TRANSLATORS: %s is the version number
                command_line.print(_("Karere %s\n"), Config.VERSION);
                return 0;
            }
            
            // Handle help option
            if (options.contains("help")) {
                command_line.print(_("Usage: karere [OPTIONS]\n"));
                command_line.print(_("  --version    Show version information\n"));
                command_line.print(_("  --help       Show this help message\n"));
                return 0;
            }
            
            // Hold the application and activate it
            hold();
            activate();
            
            return 0;
        }

        public override void shutdown() {
            logger.info("Application shutting down");
            
            // Save application state
            save_application_state();
            
            // Clean up resources
            cleanup_resources();
            
            base.shutdown();
            
            logger.info("Application shutdown complete");
        }

        private void setup_actions() {
            // Preferences action
            var preferences_action = new SimpleAction("preferences", null);
            preferences_action.activate.connect(on_preferences_activate);
            add_action(preferences_action);
            
            // About action
            var about_action = new SimpleAction("about", null);
            about_action.activate.connect(on_about_activate);
            add_action(about_action);
            
            // Quit action
            var quit_action = new SimpleAction("quit", null);
            quit_action.activate.connect(() => {
                logger.info("Quit action activated");
                quit();
            });
            add_action(quit_action);
            
            // Notification click action
            var notification_action = new SimpleAction("notification-clicked", null);
            notification_action.activate.connect(() => {
                logger.info("Notification clicked - presenting window");
                activate();
            });
            add_action(notification_action);
            
            logger.debug("Application actions set up");
        }

        private void setup_keyboard_shortcuts() {
            // Note: Keyboard shortcuts are now managed by KeyboardShortcuts class
            // This method is kept for compatibility but the actual setup is done
            // when the window is created and keyboard_shortcuts.set_window_reference() is called
            
            logger.debug("Keyboard shortcuts will be configured when window is created");
        }
        
        private void setup_log_handlers_with_preferences() {
            if (settings == null) {
                logger.warning("Settings not initialized, cannot update log handlers");
                return;
            }
            
            try {
                // Update environment variables based on user preferences
                if (settings.get_boolean("console-logging-enabled")) {
                    Environment.set_variable("G_MESSAGES_DEBUG", "all", true);
                    Environment.set_variable("SOUP_DEBUG", "1", true);
                } else {
                    Environment.set_variable("G_MESSAGES_DEBUG", "", true);
                    Environment.set_variable("SOUP_DEBUG", "0", true);
                }
                
                // Custom log handler that respects user preferences
                LogFunc system_log_handler = (log_domain, log_level, message) => {
                    // Always allow our application logs through
                    if (log_domain != null && log_domain == "Karere") {
                        Log.default_handler(log_domain, log_level, message);
                        return;
                    }
                    
                    // Check if console logging is enabled
                    if (settings != null && !settings.get_boolean("console-logging-enabled")) {
                        // Console logging disabled - suppress all system debug messages and info messages
                        if ((log_level & LogLevelFlags.LEVEL_DEBUG) != 0 || 
                            (log_level & LogLevelFlags.LEVEL_INFO) != 0) {
                            return; // Suppress debug and info messages
                        }
                    }
                    
                    // For all other cases, use default handler
                    Log.default_handler(log_domain, log_level, message);
                };
                
                // Install our custom log handler for all domains
                Log.set_handler(null, LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, system_log_handler);
                
                // Also install handlers for specific domains that use their own logging
                Log.set_handler("GLib", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, system_log_handler);
                Log.set_handler("GLib-GIO", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, system_log_handler);
                Log.set_handler("Gdk", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, system_log_handler);
                Log.set_handler("Gtk", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, system_log_handler);
                Log.set_handler("GdkPixbuf", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, system_log_handler);
                Log.set_handler("Pango", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, system_log_handler);
                Log.set_handler("Cairo", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, system_log_handler);
                Log.set_handler("libsoup", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, system_log_handler);
                Log.set_handler("GVFS", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, system_log_handler);
                Log.set_handler("WebKit", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, system_log_handler);
                Log.set_handler("Adwaita", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, system_log_handler);
                Log.set_handler("MESA-INTEL", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, system_log_handler);
                
                logger.debug("Log handlers updated with user preferences");
                
            } catch (Error e) {
                logger.error("Failed to update log handlers with preferences: %s", e.message);
            }
        }
        
        private void setup_theme_handling() {
            // Apply initial theme
            apply_theme();
            
            // Listen for theme changes
            if (settings != null) {
                settings.changed["theme-preference"].connect(() => {
                    apply_theme();
                    logger.debug("Theme preference changed");
                });
            }
            
            logger.debug("Theme handling configured");
        }
        
        private void apply_theme() {
            var style_manager = Adw.StyleManager.get_default();
            
            if (settings == null) {
                // Use system default if settings are not available yet
                style_manager.color_scheme = Adw.ColorScheme.DEFAULT;
                logger.debug("Applied system theme (settings not available)");
                return;
            }
            
            var theme_preference = settings.get_int("theme-preference");
            
            switch (theme_preference) {
                case 0: // System
                    style_manager.color_scheme = Adw.ColorScheme.DEFAULT;
                    logger.debug("Applied system theme");
                    break;
                case 1: // Light
                    style_manager.color_scheme = Adw.ColorScheme.FORCE_LIGHT;
                    logger.debug("Applied light theme");
                    break;
                case 2: // Dark
                    style_manager.color_scheme = Adw.ColorScheme.FORCE_DARK;
                    logger.debug("Applied dark theme");
                    break;
                default:
                    style_manager.color_scheme = Adw.ColorScheme.DEFAULT;
                    logger.warning("Unknown theme preference: %d, using system default", theme_preference);
                    break;
            }
        }

        private void on_preferences_activate() {
            logger.debug("Preferences action activated");
            
            if (main_window != null && !main_window.in_destruction()) {
                var preferences = new Preferences();
                preferences.present(main_window);
            }
        }

        private void on_about_activate() {
            KarereAboutDialog.show(main_window);
        }

        private void check_and_show_whats_new() {
            // Check if this is a new version and show release notes automatically
            if (should_show_release_notes()) {
                // Small delay to ensure main window is fully presented
                Timeout.add(500, () => {
                    if (main_window != null && !main_window.in_destruction()) {
                        logger.info("Showing automatic release notes for new version");
                        KarereAboutDialog.show_with_release_notes(main_window);
                    }
                    return false;
                });
            }
        }

        private bool should_show_release_notes() {
            if (settings == null) {
                return false;
            }
            
            try {
                string last_version = settings.get_string("last-version-shown");
                string current_version = Config.VERSION;

                // Show if this is the first run (empty last version) or version has changed
                if (last_version == "" || last_version != current_version) {
                    settings.set_string("last-version-shown", current_version);
                    logger.info("New version detected: %s (was: %s)", current_version, last_version == "" ? "first run" : last_version);
                    return true;
                }
            } catch (Error e) {
                logger.warning("Failed to check last version shown: %s", e.message);
            }
            
            return false;
        }

        private bool should_show_version_alert() {
            if (settings == null) {
                return false;
            }
            
            try {
                string last_version = settings.get_string("last-shown-version");
                if (last_version != Config.VERSION) {
                    return true;
                }
            } catch (Error e) {
                logger.warning("Failed to check last shown version: %s", e.message);
            }
            
            return false;
        }

        private void show_version_alert() {
            if (main_window == null || main_window.in_destruction()) {
                return;
            }
            
            // Get release notes from AppData
            string release_notes = KarereAboutDialog.get_current_release_notes();
            if (release_notes == "") {
                release_notes = "New version available with improvements and bug fixes.";
            }
            
            var alert = new Adw.AlertDialog(
                "What's New in Karere %s".printf(Config.VERSION),
                release_notes
            );
            
            alert.add_response("ok", "Got it");
            alert.set_response_appearance("ok", Adw.ResponseAppearance.SUGGESTED);
            alert.set_default_response("ok");
            
            alert.response.connect(() => {
                // Mark this version as shown
                if (settings != null) {
                    try {
                        settings.set_string("last-shown-version", Config.VERSION);
                        logger.debug("Marked version %s as shown", Config.VERSION);
                    } catch (Error e) {
                        logger.warning("Failed to save last shown version: %s", e.message);
                    }
                }
            });
            
            alert.present(main_window);
        }




        private void save_application_state() {
            try {
                if (main_window != null && settings != null) {
                    // Save window state
                    int width, height;
                    main_window.get_default_size(out width, out height);
                    settings.set_int("window-width", width);
                    settings.set_int("window-height", height);
                    settings.set_boolean("window-maximized", main_window.maximized);
                    
                    logger.debug("Application state saved");
                } else if (settings == null) {
                    logger.warning("Cannot save application state: settings not initialized");
                }
            } catch (Error e) {
                logger.error("Failed to save application state: %s", e.message);
            }
        }

        private void cleanup_resources() {
            try {
                // Clean up accessibility manager resources
                accessibility_manager.cleanup();
                
                // Clean up keyboard shortcuts resources
                keyboard_shortcuts.cleanup();
                
                // Clean up crash reporter resources
                crash_reporter.cleanup();
                
                // Clean up logger resources
                logger.cleanup();
                
                logger.debug("Resources cleaned up successfully");
            } catch (Error e) {
                // Log error but don't prevent shutdown
                logger.error("Error during resource cleanup: %s", e.message);
            }
        }

        public override bool dbus_register(DBusConnection connection, string object_path) throws Error {
            base.dbus_register(connection, object_path);
            
            logger.debug("Application registered on D-Bus");
            return true;
        }

        public override void dbus_unregister(DBusConnection connection, string object_path) {
            base.dbus_unregister(connection, object_path);
            
            logger.debug("Application unregistered from D-Bus");
        }

        public void window_destroyed() {
            logger.info("Window destroyed, clearing reference");
            main_window = null;
            
            // Application continues running in background
            // User can reopen via launcher or quick settings
            logger.info("Window destroyed, application continues in background");
        }

        public NotificationManager get_notification_manager() {
            return notification_manager;
        }

        public KeyboardShortcuts get_keyboard_shortcuts() {
            return keyboard_shortcuts;
        }
    }
}