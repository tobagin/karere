/*
 * Copyright (C) 2025 Karere Contributors
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

        public Application() {
            Object(
                application_id: Config.APP_ID,
                flags: ApplicationFlags.HANDLES_COMMAND_LINE
            );
            
            logger = new Logger();
            crash_reporter = new CrashReporter();
        }

        public override void startup() {
            base.startup();
            
            logger.info("Application starting up");
            
            // Initialize crash reporting
            crash_reporter.initialize();
            
            // Set up application actions
            setup_actions();
            
            // Set up keyboard shortcuts
            setup_keyboard_shortcuts();
            
            logger.info("Application startup complete");
        }

        public override void activate() {
            base.activate();
            
            logger.info("Application activated");
            
            if (main_window == null) {
                main_window = new Window(this);
                main_window.present();
                logger.info("Main window created and presented");
            } else {
                main_window.present();
                logger.info("Main window presented");
            }
        }

        public override int command_line(ApplicationCommandLine command_line) {
            logger.info("Processing command line arguments");
            
            var options = command_line.get_options_dict();
            
            // Handle version option
            if (options.contains("version")) {
                command_line.print("Karere %s\n", Config.VERSION);
                return 0;
            }
            
            // Handle help option
            if (options.contains("help")) {
                command_line.print("Usage: karere [OPTIONS]\n");
                command_line.print("  --version    Show version information\n");
                command_line.print("  --help       Show this help message\n");
                return 0;
            }
            
            // Activate the application
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
            // New window action
            var new_window_action = new SimpleAction("new-window", null);
            new_window_action.activate.connect(() => {
                logger.info("New window action activated");
                var new_window = new Window(this);
                new_window.present();
            });
            add_action(new_window_action);
            
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
            
            logger.debug("Application actions set up");
        }

        private void setup_keyboard_shortcuts() {
            // Application shortcuts
            set_accels_for_action("app.new-window", {"<primary>n"});
            set_accels_for_action("app.preferences", {"<primary>comma"});
            set_accels_for_action("app.quit", {"<primary>q"});
            
            // Window shortcuts
            set_accels_for_action("win.show-help-overlay", {"<primary>question"});
            set_accels_for_action("win.zoom-in", {"<primary>plus", "<primary>equal"});
            set_accels_for_action("win.zoom-out", {"<primary>minus"});
            set_accels_for_action("win.zoom-reset", {"<primary>0"});
            set_accels_for_action("win.find", {"<primary>f"});
            
            logger.debug("Keyboard shortcuts configured");
        }

        private void on_preferences_activate() {
            logger.debug("Preferences action activated");
            
            if (main_window != null) {
                var preferences = new Preferences();
                preferences.present(main_window);
            }
        }

        private void on_about_activate() {
            logger.debug("About action activated");
            
            var about = new Adw.AboutDialog() {
                application_name = Config.APP_NAME,
                application_icon = Config.APP_ID,
                version = Config.VERSION,
                developer_name = "Karere Contributors",
                license_type = Gtk.License.GPL_3_0,
                website = "https://github.com/tobagin/karere-vala",
                issue_url = "https://github.com/tobagin/karere-vala/issues",
                copyright = "© 2025 Karere Contributors"
            };
            
            about.add_acknowledgement_section(
                "Special Thanks",
                {
                    "The GNOME Project",
                    "The WebKitGTK Team",
                    "WhatsApp Inc."
                }
            );
            
            if (main_window != null) {
                about.present(main_window);
            }
        }

        private void save_application_state() {
            try {
                if (main_window != null) {
                    var settings = new Settings(Config.APP_ID);
                    
                    // Save window state
                    int width, height;
                    main_window.get_default_size(out width, out height);
                    settings.set_int("window-width", width);
                    settings.set_int("window-height", height);
                    settings.set_boolean("window-maximized", main_window.maximized);
                    
                    logger.debug("Application state saved");
                }
            } catch (Error e) {
                logger.error("Failed to save application state: %s", e.message);
            }
        }

        private void cleanup_resources() {
            try {
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
    }
}