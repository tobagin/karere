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
     * Simple dependency injection container for managing application dependencies
     */
    public class DependencyContainer : GLib.Object {
        private static DependencyContainer? _instance = null;
        private HashTable<string, GLib.Object> _services;

        private DependencyContainer() {
            _services = new HashTable<string, GLib.Object>(str_hash, str_equal);
        }

        public static DependencyContainer get_instance() {
            if (_instance == null) {
                _instance = new DependencyContainer();
            }
            return _instance;
        }

        /**
         * Register a singleton service
         */
        public void register_singleton(string key, GLib.Object service) {
            _services.set(key, service);
        }

        /**
         * Resolve a service by key
         */
        public GLib.Object? resolve(string key) {
            return _services.get(key);
        }

        /**
         * Check if a service is registered
         */
        public bool is_registered(string key) {
            return _services.contains(key);
        }

        /**
         * Clear all services
         */
        public void clear() {
            _services.remove_all();
        }
    }
}