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

    public enum LogLevel {
        CRITICAL = 0,
        ERROR = 1,
        WARNING = 2,
        INFO = 3,
        DEBUG = 4
    }

    public class Logger : GLib.Object {
        private Settings? settings = null;
        private File? log_file;
        private FileOutputStream? log_stream;
        private bool initialization_deferred = true;
        
        public Logger() {
            // Note: Settings initialization is deferred until GTK is initialized
            // setup_file_logging() will be called when settings are available
        }

        public void initialize_settings() {
            if (settings == null) {
                try {
                    settings = new Settings(Config.APP_ID);
                    initialization_deferred = false;
                    setup_file_logging();
                } catch (Error e) {
                    // Settings not available, continue without file logging
                    initialization_deferred = true;
                }
            }
        }
        
        private void setup_file_logging() {
            if (settings == null || !settings.get_boolean("file-logging-enabled")) {
                return;
            }

            try {
                // Create logs directory in user data dir
                var data_dir = Environment.get_user_data_dir();
                var logs_dir = File.new_for_path(Path.build_filename(data_dir, Config.APP_NAME, "logs"));
                if (!logs_dir.query_exists()) {
                    logs_dir.make_directory_with_parents();
                }

                // Create log file with timestamp
                var now = new DateTime.now_local();
                var filename = "karere-%s.log".printf(now.format("%Y-%m-%d"));
                log_file = logs_dir.get_child(filename);
                
                // Append to existing file or create new one
                log_stream = log_file.append_to(FileCreateFlags.NONE);
            } catch (Error e) {
                // Fallback - file logging will be disabled
                stderr.printf("Failed to setup file logging: %s\n", e.message);
            }
        }

        private bool should_log(LogLevel level) {
            if (settings == null) {
                // Default to INFO level if settings not available
                return level <= LogLevel.INFO;
            }
            var min_level = (LogLevel) settings.get_int("log-level");
            return level <= min_level;
        }

        private void write_log(LogLevel level, string message) {
            // Check if we should log this level at all
            if (!should_log(level)) {
                return;
            }

            // Check if either console or file logging is enabled
            var console_enabled = true;  // Default to enabled
            var file_enabled = false;    // Default to disabled
            
            if (settings != null) {
                console_enabled = settings.get_boolean("console-logging-enabled");
                file_enabled = settings.get_boolean("file-logging-enabled");
            }
            
            if (!console_enabled && !file_enabled) {
                return; // Nothing to do if both are disabled
            }

            var now = new DateTime.now_local();
            var timestamp = now.format("%H:%M:%S.%f")[0:-3]; // Remove microseconds, keep milliseconds
            var level_str = get_level_string(level);
            var formatted_message = "%s-%s: %s: %s".printf(Config.APP_NAME, level_str, timestamp, message);

            // Console logging with colors (only if enabled)
            if (console_enabled) {
                var colored_message = get_colored_message(level, formatted_message);
                if (level <= LogLevel.ERROR) { // CRITICAL and ERROR go to stderr
                    stderr.printf("%s\n", colored_message);
                } else { // WARNING, INFO, DEBUG go to stdout
                    stdout.printf("%s\n", colored_message);
                }
                stdout.flush();
                stderr.flush();
            }

            // File logging (only if enabled)
            if (file_enabled && log_stream != null) {
                try {
                    var file_message = "%s\n".printf(formatted_message);
                    log_stream.write(file_message.data);
                    log_stream.flush();
                } catch (Error e) {
                    stderr.printf("Failed to write to log file: %s\n", e.message);
                }
            }
        }

        private string get_level_string(LogLevel level) {
            switch (level) {
                case LogLevel.CRITICAL:
                    return "CRITICAL";
                case LogLevel.ERROR:
                    return "ERROR";
                case LogLevel.WARNING:
                    return "WARNING";
                case LogLevel.INFO:
                    return "INFO";
                case LogLevel.DEBUG:
                    return "DEBUG";
                default:
                    return "UNKNOWN";
            }
        }

        private string get_colored_message(LogLevel level, string message) {
            // ANSI color codes
            const string RESET = "\033[0m";
            const string BOLD = "\033[1m";
            const string RED = "\033[31m";
            const string YELLOW = "\033[33m";
            const string ORANGE = "\033[38;5;208m"; // 256-color orange
            const string MAGENTA = "\033[35m";
            const string CYAN = "\033[36m";

            switch (level) {
                case LogLevel.CRITICAL:
                    return "%s%s%s%s".printf(BOLD, MAGENTA, message, RESET);
                case LogLevel.ERROR:
                    return "%s%s%s%s".printf(BOLD, RED, message, RESET);
                case LogLevel.WARNING:
                    return "%s%s%s".printf(ORANGE, message, RESET);
                case LogLevel.INFO:
                    return "%s%s%s".printf(YELLOW, message, RESET);
                case LogLevel.DEBUG:
                    return "%s%s%s".printf(CYAN, message, RESET);
                default:
                    return message;
            }
        }

        public void debug(string format, ...) {
            var args = va_list();
            var message = format.vprintf(args);
            write_log(LogLevel.DEBUG, message);
        }

        public void info(string format, ...) {
            var args = va_list();
            var message = format.vprintf(args);
            write_log(LogLevel.INFO, message);
        }

        public void warning(string format, ...) {
            var args = va_list();
            var message = format.vprintf(args);
            write_log(LogLevel.WARNING, message);
        }

        public void error(string format, ...) {
            var args = va_list();
            var message = format.vprintf(args);
            write_log(LogLevel.ERROR, message);
        }

        public void critical(string format, ...) {
            var args = va_list();
            var message = format.vprintf(args);
            write_log(LogLevel.CRITICAL, message);
        }

        public void cleanup() {
            if (log_stream != null) {
                try {
                    log_stream.close();
                } catch (Error e) {
                    stderr.printf("Failed to close log file: %s\n", e.message);
                }
                log_stream = null;
            }
        }
    }
}