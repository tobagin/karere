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
     * WindowStateManager handles window geometry and state persistence.
     *
     * This manager is responsible for saving and restoring window size,
     * position, and maximized state to/from GSettings.
     */
    public class WindowStateManager : Object {

        private Settings? settings;
        private Adw.ApplicationWindow window;

        private uint save_timeout_id = 0;

        /**
         * Create a new WindowStateManager
         *
         * @param settings GSettings instance for persistence (can be null)
         * @param window The application window to manage
         */
        public WindowStateManager(Settings? settings, Adw.ApplicationWindow window) {
            this.settings = settings;
            this.window = window;
        }

        /**
         * Restore window state from GSettings
         */
        public void restore_state() {
            if (settings == null) {
                warning("Cannot restore window state: settings is null, using defaults");
                window.set_default_size(1200, 800);
                return;
            }

            // Restore window size
            var width = settings.get_int("window-width");
            var height = settings.get_int("window-height");
            window.set_default_size(width, height);

            // Restore maximized state
            if (settings.get_boolean("window-maximized")) {
                window.maximize();
            }

            debug("Window state restored: %dx%d, maximized: %s",
                        width, height, settings.get_boolean("window-maximized").to_string());
        }

        /**
         * Start tracking window state changes
         */
        public void start_tracking() {
            // Connect to window property change signals
            window.notify["maximized"].connect(on_state_changed);
            window.notify["default-width"].connect(on_state_changed);
            window.notify["default-height"].connect(on_state_changed);

            debug("Window state tracking started");
        }

        /**
         * Handle state changes by scheduling a save
         */
        private void on_state_changed(Object object, ParamSpec pspec) {
            // Cancel existing timeout if any
            if (save_timeout_id != 0) {
                Source.remove(save_timeout_id);
            }

            // Schedule new save (debounce for 500ms)
            save_timeout_id = Timeout.add(500, () => {
                save_state();
                save_timeout_id = 0;
                return false;
            });
        }

        /**
         * Save current window state to GSettings immediately
         */
        public void save_state() {
            if (settings == null) return;

            // Save window size
            int width, height;
            window.get_default_size(out width, out height);
            
            // Only save valid dimensions
            if (width > 0 && height > 0) {
                settings.set_int("window-width", width);
                settings.set_int("window-height", height);
            }

            // Save maximized state
            settings.set_boolean("window-maximized", window.maximized);

            debug("Window state saved: %dx%d, maximized: %s",
                        width, height, window.maximized.to_string());
        }
        
        ~WindowStateManager() {
            if (save_timeout_id != 0) {
                Source.remove(save_timeout_id);
                // Last ditch effort to save if pending, but avoid blocking destruction seriously
                save_state();
            }
        }
    }
}
