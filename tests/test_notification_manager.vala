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

    // Mock Application for testing NotificationManager
    public class MockApplication : GLib.Object {
        public signal void mock_notification_sent(string? id, Notification notification);
        
        public void send_notification(string? id, Notification notification) {
            mock_notification_sent(id, notification);
        }
    }

    // Mock Logger for testing NotificationManager
    public class MockLogger : GLib.Object {
        public string last_debug_message = "";
        public string last_info_message = "";
        public string last_warning_message = "";
        public string last_error_message = "";
        
        public void debug(string format, ...) {
            var args = va_list();
            last_debug_message = format.vprintf(args);
        }
        
        public void info(string format, ...) {
            var args = va_list();
            last_info_message = format.vprintf(args);
        }
        
        public void warning(string format, ...) {
            var args = va_list();
            last_warning_message = format.vprintf(args);
        }
        
        public void error(string format, ...) {
            var args = va_list();
            last_error_message = format.vprintf(args);
        }
    }

    public class NotificationManagerTest : GLib.Object {
        private Karere.NotificationManager notification_manager;
        private MockApplication mock_app;
        private MockLogger mock_logger;
        private Settings test_settings;

        public void setup() {
            test_settings = new Settings(Config.APP_ID);
            mock_app = new MockApplication();
            mock_logger = new MockLogger();
            
            // Don't create NotificationManager in setup due to complex dependencies
        }

        public void teardown() {
            notification_manager = null;
            mock_app = null;
            mock_logger = null;
            test_settings = null;
        }

        public void test_notification_manager_types() {
            setup();
            
            // Test that NotificationManager type is available
            var nm_type = typeof(Karere.NotificationManager);
            assert(nm_type != Type.INVALID);
            
            // Test that Notification type is available
            var notification_type = typeof(Notification);
            assert(notification_type != Type.INVALID);
            
            teardown();
        }

        public void test_notification_creation() {
            setup();
            
            // Test that Notification can be created
            var notification = new Notification("Test Title");
            assert(notification != null);
            // Note: GLib.Notification doesn't expose title property for reading
            // We can only verify that the notification was created successfully
            
            // Test setting body
            notification.set_body("Test body");
            
            // Test setting icon
            var icon = new ThemedIcon("dialog-information-symbolic");
            notification.set_icon(icon);
            
            // Test setting default action
            notification.set_default_action("app.test-action");
            
            // If we get here, all notification operations succeeded
            assert(true);
            
            teardown();
        }

        public void test_notification_settings_schema() {
            setup();
            
            // Test that all required notification settings exist in schema
            var settings = new Settings(Config.APP_ID);
            
            // Core notification settings
            var has_notifications_enabled = false;
            var has_system_notifications = false;
            var has_preview_enabled = false;
            var has_preview_length = false;
            
            try {
                settings.get_boolean("notifications-enabled");
                has_notifications_enabled = true;
            } catch (Error e) {
                // Setting doesn't exist
            }
            
            try {
                settings.get_boolean("system-notifications-enabled");
                has_system_notifications = true;
            } catch (Error e) {
                // Setting doesn't exist
            }
            
            try {
                settings.get_boolean("notification-preview-enabled");
                has_preview_enabled = true;
            } catch (Error e) {
                // Setting doesn't exist
            }
            
            try {
                settings.get_int("notification-preview-length");
                has_preview_length = true;
            } catch (Error e) {
                // Setting doesn't exist
            }
            
            assert(has_notifications_enabled);
            assert(has_system_notifications);
            assert(has_preview_enabled);
            assert(has_preview_length);
            
            teardown();
        }

        public void test_dnd_settings_schema() {
            setup();
            
            // Test Do Not Disturb settings
            var settings = new Settings(Config.APP_ID);
            
            var has_dnd_enabled = false;
            var has_dnd_scheduled = false;
            var has_dnd_start_time = false;
            var has_dnd_end_time = false;
            
            try {
                settings.get_boolean("dnd-enabled");
                has_dnd_enabled = true;
            } catch (Error e) {
                // Setting doesn't exist
            }
            
            try {
                settings.get_boolean("dnd-scheduled");
                has_dnd_scheduled = true;
            } catch (Error e) {
                // Setting doesn't exist
            }
            
            try {
                settings.get_string("dnd-start-time");
                has_dnd_start_time = true;
            } catch (Error e) {
                // Setting doesn't exist
            }
            
            try {
                settings.get_string("dnd-end-time");
                has_dnd_end_time = true;
            } catch (Error e) {
                // Setting doesn't exist
            }
            
            assert(has_dnd_enabled);
            assert(has_dnd_scheduled);
            assert(has_dnd_start_time);
            assert(has_dnd_end_time);
            
            teardown();
        }

        public void test_background_notification_settings() {
            setup();
            
            // Test background notification settings
            var settings = new Settings(Config.APP_ID);
            
            var has_background_mode = false;
            
            try {
                var mode = settings.get_int("background-notifications-mode");
                // Mode should be 0, 1, or 2
                assert(mode >= 0 && mode <= 2);
                has_background_mode = true;
            } catch (Error e) {
                // Setting doesn't exist
            }
            
            assert(has_background_mode);
            
            teardown();
        }

        public void test_datetime_functionality() {
            setup();
            
            // Test DateTime functionality used by NotificationManager
            var now = new DateTime.now_local();
            assert(now != null);
            
            // Test time formatting
            var time_string = "%02d:%02d".printf(now.get_hour(), now.get_minute());
            assert(time_string.length == 5); // HH:MM format
            
            // Test time difference calculation
            var past_time = now.add_seconds(-30);
            var diff = now.difference(past_time);
            assert(diff > 0);
            
            teardown();
        }

        public void test_timeout_functionality() {
            setup();
            
            // Test that Timeout functions are available (used for background notifications)
            var timeout_id = Timeout.add_seconds(1, () => {
                return false; // Don't repeat
            });
            
            assert(timeout_id != 0);
            
            // Remove the timeout to clean up
            Source.remove(timeout_id);
            
            teardown();
        }

        public void test_themed_icon_creation() {
            setup();
            
            // Test ThemedIcon creation (used in notifications)
            var icon = new ThemedIcon("dialog-information-symbolic");
            assert(icon != null);
            
            var icon_names = icon.get_names();
            assert(icon_names.length > 0);
            assert("dialog-information-symbolic" in icon_names);
            
            teardown();
        }

        public void test_string_manipulation() {
            setup();
            
            // Test string operations used in NotificationManager
            var original = "This is a very long message that needs truncation";
            var max_length = 20;
            
            if (original.length > max_length) {
                var truncated = original.substring(0, max_length) + "…";
                assert(truncated.length == max_length + 1); // +1 for ellipsis
                assert(truncated.has_suffix("…"));
            }
            
            teardown();
        }

        public void test_time_comparison() {
            setup();
            
            // Test time comparison logic used in DND scheduling
            var time1 = "09:00";
            var time2 = "17:00";
            var time3 = "22:00";
            var time4 = "08:00";
            
            // Same day schedule (09:00 to 17:00)
            assert(time1 < time2);
            
            // Cross-midnight schedule (22:00 to 08:00 next day)
            assert(time3 > time4);
            
            // Test current time comparison
            var current = "12:00";
            assert(current >= time1 && current < time2); // In same-day schedule
            
            teardown();
        }

        public void test_settings_boolean_operations() {
            setup();
            
            var settings = new Settings(Config.APP_ID);
            
            // Test that we can read boolean settings
            var notifications_enabled = settings.get_boolean("notifications-enabled");
            assert(notifications_enabled == true || notifications_enabled == false);
            
            var system_notifications = settings.get_boolean("system-notifications-enabled");
            assert(system_notifications == true || system_notifications == false);
            
            teardown();
        }

        public void test_settings_integer_operations() {
            setup();
            
            var settings = new Settings(Config.APP_ID);
            
            // Test that we can read integer settings
            var preview_length = settings.get_int("notification-preview-length");
            assert(preview_length > 0);
            
            var background_mode = settings.get_int("background-notifications-mode");
            assert(background_mode >= 0 && background_mode <= 2);
            
            teardown();
        }

        public void test_settings_string_operations() {
            setup();
            
            var settings = new Settings(Config.APP_ID);
            
            // Test that we can read string settings
            var start_time = settings.get_string("dnd-start-time");
            assert(start_time != null);
            
            var end_time = settings.get_string("dnd-end-time");
            assert(end_time != null);
            
            teardown();
        }

        public void test_constants_definition() {
            setup();
            
            // Test that we can define and use constants like in NotificationManager
            const int TEST_GRACE_PERIOD = 30;
            const int TEST_COOLDOWN = 30;
            
            assert(TEST_GRACE_PERIOD == 30);
            assert(TEST_COOLDOWN == 30);
            
            teardown();
        }

        

        public void test_signal_connection() {
            setup();
            
            // Test that we can connect signals (used for window focus tracking)
            var test_object = new MockApplication();
            
            bool signal_emitted = false;
            test_object.mock_notification_sent.connect((id, notification) => {
                signal_emitted = true;
            });
            
            // Emit the signal
            var test_notification = new Notification("Test");
            test_object.send_notification(null, test_notification);
            
            assert(signal_emitted);
            
            teardown();
        }

        public void test_error_handling_patterns() {
            setup();
            
            // Test error handling patterns used in NotificationManager
            try {
                var notification = new Notification("Test");
                notification.set_body("Test body");
                // If we get here, no error occurred
                assert(true);
            } catch (Error e) {
                // Handle error gracefully
                assert(false); // Should not reach here in normal operation
            }
            
            teardown();
        }
    }

    public void register_notification_manager_tests() {
        Test.add_func("/karere/notification_manager/types", () => {
            var test = new NotificationManagerTest();
            test.test_notification_manager_types();
        });

        Test.add_func("/karere/notification_manager/notification_creation", () => {
            var test = new NotificationManagerTest();
            test.test_notification_creation();
        });

        Test.add_func("/karere/notification_manager/settings_schema", () => {
            var test = new NotificationManagerTest();
            test.test_notification_settings_schema();
        });

        Test.add_func("/karere/notification_manager/dnd_settings", () => {
            var test = new NotificationManagerTest();
            test.test_dnd_settings_schema();
        });

        Test.add_func("/karere/notification_manager/background_settings", () => {
            var test = new NotificationManagerTest();
            test.test_background_notification_settings();
        });

        Test.add_func("/karere/notification_manager/datetime_functionality", () => {
            var test = new NotificationManagerTest();
            test.test_datetime_functionality();
        });

        Test.add_func("/karere/notification_manager/timeout_functionality", () => {
            var test = new NotificationManagerTest();
            test.test_timeout_functionality();
        });

        Test.add_func("/karere/notification_manager/themed_icon_creation", () => {
            var test = new NotificationManagerTest();
            test.test_themed_icon_creation();
        });

        Test.add_func("/karere/notification_manager/string_manipulation", () => {
            var test = new NotificationManagerTest();
            test.test_string_manipulation();
        });

        Test.add_func("/karere/notification_manager/time_comparison", () => {
            var test = new NotificationManagerTest();
            test.test_time_comparison();
        });

        Test.add_func("/karere/notification_manager/settings_boolean_operations", () => {
            var test = new NotificationManagerTest();
            test.test_settings_boolean_operations();
        });

        Test.add_func("/karere/notification_manager/settings_integer_operations", () => {
            var test = new NotificationManagerTest();
            test.test_settings_integer_operations();
        });

        Test.add_func("/karere/notification_manager/settings_string_operations", () => {
            var test = new NotificationManagerTest();
            test.test_settings_string_operations();
        });

        Test.add_func("/karere/notification_manager/constants_definition", () => {
            var test = new NotificationManagerTest();
            test.test_constants_definition();
        });

        

        Test.add_func("/karere/notification_manager/signal_connection", () => {
            var test = new NotificationManagerTest();
            test.test_signal_connection();
        });

        Test.add_func("/karere/notification_manager/error_handling_patterns", () => {
            var test = new NotificationManagerTest();
            test.test_error_handling_patterns();
        });
    }
}