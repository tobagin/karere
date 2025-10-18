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

    public interface SettingsInitializable : GLib.Object {
        public abstract void on_settings_initialized();
    }

    /**
     * Centralized settings manager with lifecycle management
     */
    public class SettingsManager : GLib.Object {
        private static SettingsManager? _instance = null;
        private Settings? _settings = null;
        private List<weak SettingsInitializable> _listeners;
        private bool _is_initialized = false;

        private SettingsManager() {
            _listeners = new List<weak SettingsInitializable>();
        }

        public static SettingsManager get_instance() {
            if (_instance == null) {
                _instance = new SettingsManager();
            }
            return _instance;
        }

        /**
         * Initialize settings - should be called once GTK is ready
         */
        public bool initialize() {
            if (_is_initialized) {
                return true;
            }

            return Utils.ErrorHandling.try_execute(() => {
                _settings = new Settings(Config.APP_ID);
                _is_initialized = true;

                info("Settings initialized successfully");

                // Notify all registered listeners
                notify_listeners();
            }, new Utils.ErrorHandler(), "settings initialization");
        }

        /**
         * Get the settings instance (may be null if not initialized)
         */
        public Settings? get_settings() {
            return _settings;
        }

        /**
         * Check if settings are initialized
         */
        public bool is_initialized() {
            return _is_initialized;
        }

        /**
         * Register a listener to be notified when settings are initialized
         */
        public void register_listener(SettingsInitializable listener) {
            _listeners.append(listener);

            // If already initialized, notify immediately
            if (_is_initialized) {
                listener.on_settings_initialized();
            }
        }

        /**
         * Unregister a listener
         */
        public void unregister_listener(SettingsInitializable listener) {
            _listeners.remove(listener);
        }

        /**
         * Get a setting value with fallback
         */
        public Variant? get_value_with_fallback(string key, Variant fallback) {
            if (_settings == null) {
                debug("Settings not initialized, using fallback for key: %s", key);
                return fallback;
            }

            try {
                return _settings.get_value(key);
            } catch (Error e) {
                warning("Failed to get setting '%s': %s, using fallback", key, e.message);
                return fallback;
            }
        }

        /**
         * Get boolean setting with fallback
         */
        public bool get_boolean_with_fallback(string key, bool fallback = false) {
            if (_settings == null) {
                return fallback;
            }

            try {
                return _settings.get_boolean(key);
            } catch (Error e) {
                warning("Failed to get boolean setting '%s': %s", key, e.message);
                return fallback;
            }
        }

        /**
         * Get string setting with fallback
         */
        public string get_string_with_fallback(string key, string fallback = "") {
            if (_settings == null) {
                return fallback;
            }

            try {
                return _settings.get_string(key);
            } catch (Error e) {
                warning("Failed to get string setting '%s': %s", key, e.message);
                return fallback;
            }
        }

        /**
         * Get int setting with fallback
         */
        public int get_int_with_fallback(string key, int fallback = 0) {
            if (_settings == null) {
                return fallback;
            }

            try {
                return _settings.get_int(key);
            } catch (Error e) {
                warning("Failed to get int setting '%s': %s", key, e.message);
                return fallback;
            }
        }

        /**
         * Get double setting with fallback
         */
        public double get_double_with_fallback(string key, double fallback = 0.0) {
            if (_settings == null) {
                return fallback;
            }

            try {
                return _settings.get_double(key);
            } catch (Error e) {
                warning("Failed to get double setting '%s': %s", key, e.message);
                return fallback;
            }
        }

        /**
         * Get string array setting with fallback
         */
        public string[] get_strv_with_fallback(string key, string[] fallback = {}) {
            if (_settings == null) {
                return fallback;
            }

            try {
                return _settings.get_strv(key);
            } catch (Error e) {
                warning("Failed to get string array setting '%s': %s", key, e.message);
                return fallback;
            }
        }

        private void notify_listeners() {
            foreach (weak SettingsInitializable listener in _listeners) {
                if (listener != null) {
                    listener.on_settings_initialized();
                }
            }
        }

        /**
         * Cleanup
         */
        public void cleanup() {
            _listeners = new List<weak SettingsInitializable>();
            _settings = null;
            _is_initialized = false;
        }
    }
}