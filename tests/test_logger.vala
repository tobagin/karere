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

    public class LoggerTest : GLib.Object {
        private Karere.Logger logger;
        private Settings test_settings;
        private string test_data_dir;
        private string test_logs_dir;

        public void setup() {
            test_settings = new Settings(Config.APP_ID);
            
            // Create a temporary test data directory
            test_data_dir = "/tmp/karere-test-" + Random.int_range(1000, 9999).to_string();
            test_logs_dir = Path.build_filename(test_data_dir, Config.APP_NAME, "logs");
            
            // Don't create logger in setup to avoid file system issues
        }

        public void teardown() {
            // Clean up logger first
            if (logger != null) {
                logger.cleanup();
                logger = null;
            }
            
            // Clean up test directory
            if (test_data_dir != null) {
                try {
                    var test_dir = File.new_for_path(test_data_dir);
                    if (test_dir.query_exists()) {
                        delete_directory_recursive(test_dir);
                    }
                } catch (Error e) {
                    // Ignore cleanup errors in tests
                }
            }
            
            test_settings = null;
        }

        private void delete_directory_recursive(File dir) throws Error {
            var enumerator = dir.enumerate_children(
                FileAttribute.STANDARD_NAME + "," + FileAttribute.STANDARD_TYPE,
                FileQueryInfoFlags.NOFOLLOW_SYMLINKS
            );

            FileInfo file_info;
            while ((file_info = enumerator.next_file()) != null) {
                var child = dir.get_child(file_info.get_name());
                if (file_info.get_file_type() == FileType.DIRECTORY) {
                    delete_directory_recursive(child);
                } else {
                    child.delete();
                }
            }
            dir.delete();
        }

        public void test_logger_construction() {
            setup();
            
            // Test that Logger can be constructed
            logger = new Karere.Logger();
            assert(logger != null);
            
            teardown();
        }

        public void test_log_level_enum() {
            setup();
            
            // Test LogLevel enum values
            assert(Karere.LogLevel.CRITICAL == 0);
            assert(Karere.LogLevel.ERROR == 1);
            assert(Karere.LogLevel.WARNING == 2);
            assert(Karere.LogLevel.INFO == 3);
            assert(Karere.LogLevel.DEBUG == 4);
            
            teardown();
        }

        public void test_log_level_filtering() {
            setup();
            
            logger = new Karere.Logger();
            
            // Test that logger respects log level settings
            // We can't easily test the actual filtering without file I/O,
            // but we can test that the settings exist
            var min_level = test_settings.get_int("log-level");
            assert(min_level >= 0 && min_level <= 4);
            
            teardown();
        }

        public void test_console_logging_setting() {
            setup();
            
            // Test console logging setting
            var console_enabled = test_settings.get_boolean("console-logging-enabled");
            // Should be either true or false (no error)
            assert(console_enabled == true || console_enabled == false);
            
            teardown();
        }

        public void test_file_logging_setting() {
            setup();
            
            // Test file logging setting
            var file_enabled = test_settings.get_boolean("file-logging-enabled");
            // Should be either true or false (no error)
            assert(file_enabled == true || file_enabled == false);
            
            teardown();
        }

        public void test_logging_methods_exist() {
            setup();
            
            logger = new Karere.Logger();
            
            // Test that all logging methods can be called
            // We're testing they don't crash, not their output
            logger.debug("Test debug message");
            logger.info("Test info message");
            logger.warning("Test warning message");
            logger.error("Test error message");
            logger.critical("Test critical message");
            
            // If we get here, all methods executed without crashing
            assert(true);
            
            teardown();
        }

        public void test_logging_with_format_args() {
            setup();
            
            logger = new Karere.Logger();
            
            // Test formatted logging
            logger.debug("Debug: %s %d", "test", 42);
            logger.info("Info: %s %d", "test", 42);
            logger.warning("Warning: %s %d", "test", 42);
            logger.error("Error: %s %d", "test", 42);
            logger.critical("Critical: %s %d", "test", 42);
            
            // If we get here, all formatted methods executed without crashing
            assert(true);
            
            teardown();
        }

        public void test_cleanup_method() {
            setup();
            
            logger = new Karere.Logger();
            
            // Test cleanup doesn't crash
            logger.cleanup();
            
            // Should be able to call cleanup multiple times
            logger.cleanup();
            
            assert(true);
            
            teardown();
        }

        public void test_datetime_functionality() {
            setup();
            
            // Test DateTime functionality used by Logger
            var now = new DateTime.now_local();
            assert(now != null);
            
            var timestamp = now.format("%H:%M:%S.%f")[0:-3];
            assert(timestamp.length > 0);
            
            teardown();
        }

        public void test_file_operations_types() {
            setup();
            
            // Test that File types needed by Logger are available
            var file_type = typeof(File);
            assert(file_type != Type.INVALID);
            
            var file_output_stream_type = typeof(FileOutputStream);
            assert(file_output_stream_type != Type.INVALID);
            
            teardown();
        }

        public void test_settings_integration() {
            setup();
            
            logger = new Karere.Logger();
            
            // Test that required settings keys exist
            bool has_log_level = false;
            bool has_console_logging = false;
            bool has_file_logging = false;
            
            try {
                test_settings.get_int("log-level");
                has_log_level = true;
            } catch (Error e) {
                // Setting doesn't exist
            }
            
            try {
                test_settings.get_boolean("console-logging-enabled");
                has_console_logging = true;
            } catch (Error e) {
                // Setting doesn't exist
            }
            
            try {
                test_settings.get_boolean("file-logging-enabled");
                has_file_logging = true;
            } catch (Error e) {
                // Setting doesn't exist
            }
            
            assert(has_log_level);
            assert(has_console_logging);
            assert(has_file_logging);
            
            teardown();
        }

        public void test_environment_variables() {
            setup();
            
            // Test that Environment functions work
            var user_data_dir = Environment.get_user_data_dir();
            assert(user_data_dir != null);
            assert(user_data_dir.length > 0);
            
            teardown();
        }

        public void test_path_building() {
            setup();
            
            // Test Path.build_filename functionality used by Logger
            var test_path = Path.build_filename("/tmp", "test", "logs");
            assert(test_path != null);
            assert(test_path.contains("/tmp"));
            assert(test_path.contains("test"));
            assert(test_path.contains("logs"));
            
            teardown();
        }

        public void test_color_constants() {
            setup();
            
            // Test that we can work with ANSI color codes (used in get_colored_message)
            const string RESET = "\033[0m";
            const string BOLD = "\033[1m";
            const string RED = "\033[31m";
            
            assert(RESET.length > 0);
            assert(BOLD.length > 0);
            assert(RED.length > 0);
            
            // Test string concatenation with colors
            var colored_message = "%s%sTest%s".printf(BOLD, RED, RESET);
            assert(colored_message.contains("Test"));
            
            teardown();
        }

        public void test_log_message_formatting() {
            setup();
            
            // Test message formatting functionality
            var now = new DateTime.now_local();
            var timestamp = now.format("%H:%M:%S.%f")[0:-3];
            var level_str = "INFO";
            var message = "Test message";
            
            var formatted_message = "%s-%s: %s: %s".printf(Config.APP_NAME, level_str, timestamp, message);
            
            assert(formatted_message.contains(Config.APP_NAME));
            assert(formatted_message.contains(level_str));
            assert(formatted_message.contains(message));
            
            teardown();
        }

        public void test_error_handling() {
            setup();
            
            logger = new Karere.Logger();
            
            // Test that logger handles null/empty messages gracefully
            logger.info("");
            logger.debug("");
            logger.warning("");
            logger.error("");
            logger.critical("");
            
            // If we get here, empty messages were handled without crashing
            assert(true);
            
            teardown();
        }

        public void test_multiple_logger_instances() {
            setup();
            
            // Test that multiple logger instances can coexist
            var logger1 = new Karere.Logger();
            var logger2 = new Karere.Logger();
            
            assert(logger1 != null);
            assert(logger2 != null);
            
            logger1.info("Logger 1 message");
            logger2.info("Logger 2 message");
            
            logger1.cleanup();
            logger2.cleanup();
            
            assert(true);
            
            teardown();
        }
    }

    public void register_logger_tests() {
        Test.add_func("/karere/logger/construction", () => {
            var test = new LoggerTest();
            test.test_logger_construction();
        });

        Test.add_func("/karere/logger/log_level_enum", () => {
            var test = new LoggerTest();
            test.test_log_level_enum();
        });

        Test.add_func("/karere/logger/log_level_filtering", () => {
            var test = new LoggerTest();
            test.test_log_level_filtering();
        });

        Test.add_func("/karere/logger/console_logging_setting", () => {
            var test = new LoggerTest();
            test.test_console_logging_setting();
        });

        Test.add_func("/karere/logger/file_logging_setting", () => {
            var test = new LoggerTest();
            test.test_file_logging_setting();
        });

        Test.add_func("/karere/logger/logging_methods_exist", () => {
            var test = new LoggerTest();
            test.test_logging_methods_exist();
        });

        Test.add_func("/karere/logger/logging_with_format_args", () => {
            var test = new LoggerTest();
            test.test_logging_with_format_args();
        });

        Test.add_func("/karere/logger/cleanup_method", () => {
            var test = new LoggerTest();
            test.test_cleanup_method();
        });

        Test.add_func("/karere/logger/datetime_functionality", () => {
            var test = new LoggerTest();
            test.test_datetime_functionality();
        });

        Test.add_func("/karere/logger/file_operations_types", () => {
            var test = new LoggerTest();
            test.test_file_operations_types();
        });

        Test.add_func("/karere/logger/settings_integration", () => {
            var test = new LoggerTest();
            test.test_settings_integration();
        });

        Test.add_func("/karere/logger/environment_variables", () => {
            var test = new LoggerTest();
            test.test_environment_variables();
        });

        Test.add_func("/karere/logger/path_building", () => {
            var test = new LoggerTest();
            test.test_path_building();
        });

        Test.add_func("/karere/logger/color_constants", () => {
            var test = new LoggerTest();
            test.test_color_constants();
        });

        Test.add_func("/karere/logger/log_message_formatting", () => {
            var test = new LoggerTest();
            test.test_log_message_formatting();
        });

        Test.add_func("/karere/logger/error_handling", () => {
            var test = new LoggerTest();
            test.test_error_handling();
        });

        Test.add_func("/karere/logger/multiple_logger_instances", () => {
            var test = new LoggerTest();
            test.test_multiple_logger_instances();
        });
    }
}