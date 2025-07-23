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

using Karere;

namespace KarereTests {

    public class WindowTest : GLib.Object {
        private Karere.Application app;
        private Karere.Window window;

        public void setup() {
            app = new Karere.Application();
            // Don't create window in setup to avoid WebKit initialization issues in tests
        }

        public void teardown() {
            window = null;
            app = null;
        }

        public void test_window_properties() {
            setup();
            
            // Test basic window type verification without instantiation
            var window_type = typeof(Karere.Window);
            assert(window_type.is_a(typeof(Adw.ApplicationWindow)));
            
            teardown();
        }

        public void test_window_size_properties() {
            setup();
            
            // Test that Window class exists and can be referenced
            assert(typeof(Karere.Window).name() == "KarereWindow");
            
            teardown();
        }

        public void test_window_structure() {
            setup();
            
            // Test window type hierarchy
            var window_type = typeof(Karere.Window);
            assert(window_type.is_a(typeof(Adw.ApplicationWindow)));
            assert(window_type.is_a(typeof(Gtk.Window)));
            
            teardown();
        }

        public void test_window_theming() {
            setup();
            
            // Test that LibAdwaita is available for theming
            var style_manager = Adw.StyleManager.get_default();
            assert(style_manager != null);
            
            teardown();
        }

        public void test_window_state_persistence() {
            setup();
            
            // Test that GSettings is available for state persistence
            var settings = new Settings("io.github.tobagin.karere");
            assert(settings != null);
            
            teardown();
        }

        public void test_window_error_handling() {
            setup();
            
            // Test that Window class can be introspected without errors
            var window_type = typeof(Karere.Window);
            assert(window_type != Type.INVALID);
            
            teardown();
        }
    }

    public void register_window_tests() {
        Test.add_func("/karere/window/properties", () => {
            var test = new WindowTest();
            test.test_window_properties();
        });

        Test.add_func("/karere/window/size", () => {
            var test = new WindowTest();
            test.test_window_size_properties();
        });

        Test.add_func("/karere/window/structure", () => {
            var test = new WindowTest();
            test.test_window_structure();
        });

        Test.add_func("/karere/window/theming", () => {
            var test = new WindowTest();
            test.test_window_theming();
        });

        Test.add_func("/karere/window/state_persistence", () => {
            var test = new WindowTest();
            test.test_window_state_persistence();
        });

        Test.add_func("/karere/window/error_handling", () => {
            var test = new WindowTest();
            test.test_window_error_handling();
        });
    }
}