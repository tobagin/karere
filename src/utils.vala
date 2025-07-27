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

namespace Karere.Utils {

    public errordomain ApplicationError {
        INITIALIZATION_FAILED,
        WINDOW_CREATION_FAILED,
        WEBKIT_INITIALIZATION_FAILED,
        SETTINGS_ERROR,
        RESOURCE_ERROR
    }

    public class ErrorHandler : GLib.Object {
        private Logger logger;
        private weak Gtk.Window? parent_window;

        public ErrorHandler(Logger logger, Gtk.Window? parent = null) {
            this.logger = logger;
            this.parent_window = parent;
        }

        public void handle_error(Error error, string context = "") {
            logger.error("Error in %s: %s", context.length > 0 ? context : "application", error.message);
            
            // Show user-friendly error dialog
            show_error_dialog(error, context);
        }

        public void handle_critical_error(Error error, string context = "") {
            logger.critical("Critical error in %s: %s", context.length > 0 ? context : "application", error.message);
            
            // Show critical error dialog with option to restart
            show_critical_error_dialog(error, context);
        }

        private void show_error_dialog(Error error, string context) {
            var dialog = new Adw.AlertDialog(
                _("Error"),
                get_user_friendly_message(error)
            );
            
            dialog.add_response("ok", _("OK"));
            dialog.set_default_response("ok");
            
            if (parent_window != null) {
                dialog.present(parent_window);
            }
        }

        private void show_critical_error_dialog(Error error, string context) {
            var dialog = new Adw.AlertDialog(
                _("Critical Error"),
                get_user_friendly_message(error) + "\n\n" + _("The application may need to be restarted.")
            );
            
            dialog.add_response("restart", _("Restart"));
            dialog.add_response("continue", _("Continue"));
            dialog.set_default_response("restart");
            
            dialog.response.connect((response) => {
                if (response == "restart") {
                    restart_application();
                }
            });
            
            if (parent_window != null) {
                dialog.present(parent_window);
            }
        }

        private string get_user_friendly_message(Error error) {
            if (error is ApplicationError) {
                switch (((ApplicationError) error).code) {
                    case ApplicationError.INITIALIZATION_FAILED:
                        return _("Failed to initialize the application. Please try restarting.");
                    
                    case ApplicationError.WINDOW_CREATION_FAILED:
                        return _("Failed to create the main window. Please check your system configuration.");
                    
                    case ApplicationError.WEBKIT_INITIALIZATION_FAILED:
                        return _("Failed to initialize the web engine. Please ensure WebKitGTK is properly installed.");
                    
                    case ApplicationError.SETTINGS_ERROR:
                        return _("Failed to access application settings. Your preferences may not be saved.");
                    
                    case ApplicationError.RESOURCE_ERROR:
                        return _("Failed to load application resources. Please reinstall the application.");
                    
                    default:
                        return _("An unexpected error occurred.");
                }
            }
            
            return _("An unexpected error occurred: %s").printf(error.message);
        }

        private void restart_application() {
            logger.info("Restarting application due to critical error");
            
            try {
                var app_path = Environment.get_current_dir() + "/" + Config.APP_NAME.down();
                Process.spawn_async(
                    null,
                    {app_path},
                    null,
                    SpawnFlags.SEARCH_PATH,
                    null,
                    null
                );
                
                logger.info("Successfully spawned new instance, exiting current instance");
            } catch (Error e) {
                logger.error("Failed to restart application: %s", e.message);
            }
            
            // Exit current instance after try-catch block
            Process.exit(1);
        }
    }

    public class ResourceManager : GLib.Object {
        private Logger logger;

        public ResourceManager(Logger logger) {
            this.logger = logger;
        }

        public bool ensure_user_directories() {
            try {
                var config_dir = Path.build_filename(Environment.get_user_config_dir(), Config.APP_NAME);
                var data_dir = Path.build_filename(Environment.get_user_data_dir(), Config.APP_NAME);
                var cache_dir = Path.build_filename(Environment.get_user_cache_dir(), Config.APP_NAME);
                
                DirUtils.create_with_parents(config_dir, 0755);
                DirUtils.create_with_parents(data_dir, 0755);
                DirUtils.create_with_parents(cache_dir, 0755);
                
                logger.debug("User directories ensured: config=%s, data=%s, cache=%s", config_dir, data_dir, cache_dir);
                return true;
            } catch (Error e) {
                logger.error("Failed to create user directories: %s", e.message);
                return false;
            }
        }

        public string get_user_config_dir() {
            return Path.build_filename(Environment.get_user_config_dir(), Config.APP_NAME);
        }

        public string get_user_data_dir() {
            return Path.build_filename(Environment.get_user_data_dir(), Config.APP_NAME);
        }

        public string get_user_cache_dir() {
            return Path.build_filename(Environment.get_user_cache_dir(), Config.APP_NAME);
        }
    }

    public void setup_signal_handlers(Logger logger) {
        // Handle SIGINT (Ctrl+C)
        Unix.signal_add(2, () => { // SIGINT
            logger.info("Received SIGINT, shutting down gracefully");
            var app = GLib.Application.get_default();
            if (app != null) {
                app.quit();
            }
            return Source.REMOVE;
        });

        // Handle SIGTERM
        Unix.signal_add(15, () => { // SIGTERM
            logger.info("Received SIGTERM, shutting down gracefully");
            var app = GLib.Application.get_default();
            if (app != null) {
                app.quit();
            }
            return Source.REMOVE;
        });
    }

    /**
     * Format a file size in bytes to a human-readable string
     *
     * @param size The size in bytes
     * @return A formatted string like "1.2 MB", "345 KB", etc.
     */
    public string format_size(int64 size) {
        const string[] units = {"B", "KB", "MB", "GB", "TB"};
        double size_d = (double) size;
        int unit_index = 0;

        while (size_d >= 1024.0 && unit_index < units.length - 1) {
            size_d /= 1024.0;
            unit_index++;
        }

        if (unit_index == 0) {
            return "%.0f %s".printf(size_d, units[unit_index]);
        } else {
            return "%.1f %s".printf(size_d, units[unit_index]);
        }
    }
}