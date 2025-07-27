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

using Karere;

namespace KarereTests {

    public class WindowTest : GLib.Object {
        private Karere.Application app;
        private Karere.Window window;
        private Settings test_settings;

        public void setup() {
            app = new Karere.Application();
            test_settings = new Settings(Config.APP_ID);
            // Don't create window in setup to avoid WebKit initialization issues in tests
        }

        public void teardown() {
            window = null;
            app = null;
            test_settings = null;
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
            var settings = new Settings(Config.APP_ID);
            assert(settings != null);
            
            // Test default window size settings exist
            var default_width = settings.get_int("window-width");
            var default_height = settings.get_int("window-height");
            assert(default_width > 0);
            assert(default_height > 0);
            
            teardown();
        }

        public void test_window_settings_schema() {
            setup();
            
            // Test that all required window settings exist in schema
            var settings = new Settings(Config.APP_ID);
            
            // Window size and state settings
            var has_width = false;
            var has_height = false;
            var has_maximized = false;
            
            try {
                settings.get_int("window-width");
                has_width = true;
            } catch (Error e) {
                // Setting doesn't exist
            }
            
            try {
                settings.get_int("window-height");
                has_height = true;
            } catch (Error e) {
                // Setting doesn't exist
            }
            
            try {
                settings.get_boolean("window-maximized");
                has_maximized = true;
            } catch (Error e) {
                // Setting doesn't exist
            }
            
            assert(has_width);
            assert(has_height);
            assert(has_maximized);
            
            teardown();
        }

        public void test_webkit_settings_schema() {
            setup();
            
            // Test that all required WebKit settings exist in schema
            var settings = new Settings(Config.APP_ID);
            
            // Spell checking settings
            var has_spell_enabled = false;
            var has_spell_auto_detect = false;
            var has_spell_languages = false;
            var has_dev_tools = false;
            
            try {
                settings.get_boolean("spell-checking-enabled");
                has_spell_enabled = true;
            } catch (Error e) {
                // Setting doesn't exist
            }
            
            try {
                settings.get_boolean("spell-checking-auto-detect");
                has_spell_auto_detect = true;
            } catch (Error e) {
                // Setting doesn't exist
            }
            
            try {
                settings.get_strv("spell-checking-languages");
                has_spell_languages = true;
            } catch (Error e) {
                // Setting doesn't exist
            }
            
            try {
                settings.get_boolean("developer-tools-enabled");
                has_dev_tools = true;
            } catch (Error e) {
                // Setting doesn't exist
            }
            
            assert(has_spell_enabled);
            assert(has_spell_auto_detect);
            assert(has_spell_languages);
            assert(has_dev_tools);
            
            teardown();
        }

        public void test_window_actions_introspection() {
            setup();
            
            // Test that we can introspect window actions without creating a window
            // This tests the action setup without WebKit dependencies
            
            // Test that SimpleAction type is available
            var action_type = typeof(SimpleAction);
            assert(action_type != Type.INVALID);
            
            // Test that we can create a simple action (like dev-tools action)
            var test_action = new SimpleAction("test-action", null);
            assert(test_action != null);
            assert(test_action.get_name() == "test-action");
            
            teardown();
        }

        public void test_toast_functionality() {
            setup();
            
            // Test that Adw.Toast can be created (used by Window for notifications)
            var toast = new Adw.Toast("Test message");
            assert(toast != null);
            assert(toast.title == "Test message");
            
            // Test timeout property
            toast.timeout = 5;
            assert(toast.timeout == 5);
            
            teardown();
        }

        public void test_file_drop_types() {
            setup();
            
            // Test that DropTarget can be created for file handling
            var drop_target = new Gtk.DropTarget(typeof(File), Gdk.DragAction.COPY);
            assert(drop_target != null);
            
            teardown();
        }

        public void test_webkit_context_availability() {
            setup();
            
            // Test that WebKit context can be accessed (needed for spell checking)
            // This tests the WebKit types are available without creating a web view
            var context_type = typeof(WebKit.WebContext);
            assert(context_type != Type.INVALID);
            
            var settings_type = typeof(WebKit.Settings);
            assert(settings_type != Type.INVALID);
            
            teardown();
        }

        public void test_settings_change_handling() {
            setup();
            
            var settings = new Settings(Config.APP_ID);
            
            // Test that we can connect to settings change signals
            bool signal_connected = false;
            var signal_id = settings.changed["spell-checking-enabled"].connect(() => {
                signal_connected = true;
            });
            
            assert(signal_id != 0); // Signal connection successful
            
            // Disconnect to clean up
            settings.disconnect(signal_id);
            
            teardown();
        }

        public void test_window_error_handling() {
            setup();
            
            // Test that Window class can be introspected without errors
            var window_type = typeof(Karere.Window);
            assert(window_type != Type.INVALID);
            
            teardown();
        }

        public void test_adwaita_components() {
            setup();
            
            // Test that required Adwaita components are available
            var header_bar_type = typeof(Adw.HeaderBar);
            assert(header_bar_type != Type.INVALID);
            
            var toast_overlay_type = typeof(Adw.ToastOverlay);
            assert(toast_overlay_type != Type.INVALID);
            
            var application_window_type = typeof(Adw.ApplicationWindow);
            assert(application_window_type != Type.INVALID);
            
            teardown();
        }

        public void test_gtk_components() {
            setup();
            
            // Test that required GTK components are available
            var menu_button_type = typeof(Gtk.MenuButton);
            assert(menu_button_type != Type.INVALID);
            
            var box_type = typeof(Gtk.Box);
            assert(box_type != Type.INVALID);
            
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

        Test.add_func("/karere/window/settings_schema", () => {
            var test = new WindowTest();
            test.test_window_settings_schema();
        });

        Test.add_func("/karere/window/webkit_settings_schema", () => {
            var test = new WindowTest();
            test.test_webkit_settings_schema();
        });

        Test.add_func("/karere/window/actions_introspection", () => {
            var test = new WindowTest();
            test.test_window_actions_introspection();
        });

        Test.add_func("/karere/window/toast_functionality", () => {
            var test = new WindowTest();
            test.test_toast_functionality();
        });

        Test.add_func("/karere/window/file_drop_types", () => {
            var test = new WindowTest();
            test.test_file_drop_types();
        });

        Test.add_func("/karere/window/webkit_context_availability", () => {
            var test = new WindowTest();
            test.test_webkit_context_availability();
        });

        Test.add_func("/karere/window/settings_change_handling", () => {
            var test = new WindowTest();
            test.test_settings_change_handling();
        });

        Test.add_func("/karere/window/error_handling", () => {
            var test = new WindowTest();
            test.test_window_error_handling();
        });

        Test.add_func("/karere/window/adwaita_components", () => {
            var test = new WindowTest();
            test.test_adwaita_components();
        });

        Test.add_func("/karere/window/gtk_components", () => {
            var test = new WindowTest();
            test.test_gtk_components();
        });
    }
}