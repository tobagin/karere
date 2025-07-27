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

using Soup;
using Json;

namespace Karere {

    public class CrashReporter : GLib.Object {
        private Settings? settings = null;
        private Logger? logger = null;
        private string crash_reports_dir;
        private bool handlers_installed = false;
        private bool initialization_deferred = true;
        
        // Signal handler storage
        private static CrashReporter? instance = null;
        
        public CrashReporter() {
            // Note: Settings and Logger initialization is deferred until GTK is initialized
            
            // Set up crash reports directory
            var data_dir = Environment.get_user_data_dir();
            crash_reports_dir = GLib.Path.build_filename(data_dir, Config.APP_NAME, "crash-reports");
            
            // Store instance for signal handlers
            instance = this;
        }
        
        public void initialize_settings_and_logger(Logger app_logger) {
            if (initialization_deferred) {
                try {
                    settings = new Settings(Config.APP_ID);
                    logger = app_logger; // Use the application's logger instead of creating a new one
                    initialization_deferred = false;
                    
                    // Ensure directory exists now that logger is available
                    try {
                        var dir = File.new_for_path(crash_reports_dir);
                        if (!dir.query_exists()) {
                            dir.make_directory_with_parents();
                        }
                    } catch (Error e) {
                        if (logger != null) {
                            logger.error("Failed to create crash reports directory: %s", e.message);
                        }
                    }
                } catch (Error e) {
                    // Settings not available, continue without crash reporting
                    initialization_deferred = true;
                }
            }
        }

        public void initialize() {
            if (handlers_installed) {
                return;
            }
            
            logger.info("Initializing crash reporter");
            
            // Install signal handlers for common crash signals
            install_signal_handlers();
            
            handlers_installed = true;
            logger.debug("Crash reporter signal handlers installed");
        }

        private void install_signal_handlers() {
            // Install handlers for fatal signals
            Posix.signal(Posix.Signal.SEGV, handle_crash_signal);  // Segmentation fault
            Posix.signal(Posix.Signal.ABRT, handle_crash_signal);  // Abort signal
            Posix.signal(Posix.Signal.FPE, handle_crash_signal);   // Floating point exception
            Posix.signal(Posix.Signal.ILL, handle_crash_signal);   // Illegal instruction
            Posix.signal(Posix.Signal.BUS, handle_crash_signal);   // Bus error
            Posix.signal(Posix.Signal.TERM, handle_crash_signal);  // Termination signal
        }

        private static void handle_crash_signal(int signal_num) {
            if (instance == null) {
                return;
            }
            
            // Generate crash report
            instance.generate_crash_report(signal_num);
            
            // Restore default handler and re-raise signal
            Posix.signal(signal_num, null);
            Posix.raise(signal_num);
        }

        private void generate_crash_report(int signal_num) {
            try {
                var now = new DateTime.now_local();
                var timestamp = now.format("%Y-%m-%d_%H-%M-%S");
                var crash_file = GLib.Path.build_filename(crash_reports_dir, "crash_%s.txt".printf(timestamp));
                
                var report = new StringBuilder();
                
                // Header information
                report.append_printf("Karere Crash Report\n");
                report.append_printf("==================\n\n");
                report.append_printf("Crash Time: %s\n", now.to_string());
                report.append_printf("Application: %s %s\n", Config.APP_NAME, Config.VERSION);
                report.append_printf("Signal: %d (%s)\n", signal_num, get_signal_name(signal_num));
                report.append_printf("Process ID: %d\n", (int)Posix.getpid());
                
                // System information (if enabled)
                var include_system_info = true; // Default to true if settings not available
                if (settings != null) {
                    include_system_info = settings.get_boolean("crash-include-system-info");
                }
                
                if (include_system_info) {
                    report.append("\nSystem Information:\n");
                    report.append_printf("OS: %s\n", Environment.get_os_info("PRETTY_NAME") ?? "Unknown");
                    report.append_printf("Architecture: %s\n", get_architecture());
                    report.append_printf("User: %s\n", Environment.get_user_name());
                    report.append_printf("Home: %s\n", Environment.get_home_dir());
                }
                
                // Stack trace
                report.append("\nStack Trace:\n");
                var backtrace = get_stack_trace();
                report.append(backtrace);
                
                // Write crash report to file
                FileUtils.set_contents(crash_file, report.str);
                
                // Update crash count
                if (settings != null) {
                    var crash_count = settings.get_int("crash-reports-count");
                    settings.set_int("crash-reports-count", crash_count + 1);
                }
                
                if (logger != null) {
                    logger.critical("Crash detected and report saved: %s", crash_file);
                }
                
                // Show crash dialog if enabled
                var crash_reporter_enabled = true; // Default to true if settings not available
                if (settings != null) {
                    crash_reporter_enabled = settings.get_boolean("crash-reporter-enabled");
                }
                
                if (crash_reporter_enabled) {
                    show_crash_dialog(crash_file);
                }
                
            } catch (Error e) {
                // Last resort - at least log to stderr
                stderr.printf("CRITICAL: Failed to generate crash report: %s\n", e.message);
            }
        }

        private string get_signal_name(int signal_num) {
            switch (signal_num) {
                case Posix.Signal.SEGV:
                    return "SIGSEGV (Segmentation fault)";
                case Posix.Signal.ABRT:
                    return "SIGABRT (Abort)";
                case Posix.Signal.FPE:
                    return "SIGFPE (Floating point exception)";
                case Posix.Signal.ILL:
                    return "SIGILL (Illegal instruction)";
                case Posix.Signal.BUS:
                    return "SIGBUS (Bus error)";
                case Posix.Signal.TERM:
                    return "SIGTERM (Termination)";
                default:
                    return "Unknown signal";
            }
        }

        private string get_architecture() {
            string arch = "Unknown";
            try {
                string uname_output;
                Process.spawn_command_line_sync("uname -m", out uname_output);
                arch = uname_output.strip();
            } catch (Error e) {
                // Fallback - continue with "Unknown"
            }
            return arch;
        }

        private string get_stack_trace() {
            var trace = new StringBuilder();
            
            try {
                // Try to get stack trace using gdb or other tools
                string backtrace_output;
                string backtrace_error;
                int exit_status;
                
                // Try using gdb to get a backtrace of current process
                var pid = (int)Posix.getpid();
                var gdb_command = "gdb --batch --quiet -ex \"thread apply all bt\" -ex \"quit\" --pid=%d".printf(pid);
                
                if (Process.spawn_command_line_sync(gdb_command, out backtrace_output, out backtrace_error, out exit_status)) {
                    if (exit_status == 0 && backtrace_output.length > 0) {
                        trace.append("Stack trace (via gdb):\n");
                        trace.append(backtrace_output);
                    } else {
                        trace.append("Failed to get detailed stack trace via gdb\n");
                        trace.append_printf("Error: %s\n", backtrace_error);
                        trace.append(get_basic_trace());
                    }
                } else {
                    trace.append("GDB not available for stack trace\n");
                    trace.append(get_basic_trace());
                }
                
            } catch (Error e) {
                trace.append_printf("Failed to generate stack trace: %s\n", e.message);
                trace.append(get_basic_trace());
            }
            
            return trace.str;
        }

        private string get_basic_trace() {
            var trace = new StringBuilder();
            trace.append("\nBasic trace information:\n");
            trace.append_printf("Process ID: %d\n", (int)Posix.getpid());
            trace.append_printf("Parent Process ID: %d\n", (int)Posix.getppid());
            trace.append_printf("User ID: %d\n", (int)Posix.getuid());
            trace.append_printf("Group ID: %d\n", (int)Posix.getgid());
            
            // Try to get some process information
            try {
                string proc_status;
                if (FileUtils.get_contents("/proc/self/status", out proc_status)) {
                    trace.append("\nProcess status:\n");
                    // Only include relevant lines
                    var lines = proc_status.split("\n");
                    foreach (var line in lines) {
                        if (line.has_prefix("Name:") || 
                            line.has_prefix("State:") || 
                            line.has_prefix("VmSize:") || 
                            line.has_prefix("VmRSS:") ||
                            line.has_prefix("Threads:")) {
                            trace.append_printf("%s\n", line);
                        }
                    }
                }
            } catch (Error e) {
                trace.append_printf("Could not read process info: %s\n", e.message);
            }
            
            return trace.str;
        }

        private void show_crash_dialog(string crash_file) {
            logger.debug("Showing crash dialog for: %s", crash_file);
            
            // Create crash report dialog
            var dialog = new Adw.AlertDialog(
                "Application Crashed",
                "Karere has encountered an unexpected error and needs to close. A crash report has been generated to help improve the application."
            );
            
            // Add responses
            dialog.add_response("close", "Close");
            dialog.add_response("details", "View Details");
            dialog.add_response("submit", "Submit Report");
            
            dialog.set_response_appearance("submit", Adw.ResponseAppearance.SUGGESTED);
            dialog.set_default_response("close");
            dialog.set_close_response("close");
            
            dialog.response.connect((response) => {
                switch (response) {
                    case "details":
                        show_crash_details(crash_file);
                        break;
                    case "submit":
                        handle_crash_submission(crash_file);
                        break;
                    case "close":
                    default:
                        // Just close - crash report is already saved
                        break;
                }
            });
            
            // Try to present dialog - this might fail if the application is too damaged
            try {
                // Note: In a real crash scenario, the main window might be destroyed
                // We'll need to create a temporary window or handle this differently
                dialog.present(null);
            } catch (Error e) {
                logger.error("Failed to show crash dialog: %s", e.message);
                // Fallback - at least log that a crash report was created
                stderr.printf("CRASH: Report saved to %s\n", crash_file);
            }
        }

        private void show_crash_details(string crash_file) {
            try {
                // Read crash report content
                string crash_content;
                if (FileUtils.get_contents(crash_file, out crash_content)) {
                    
                    // Create details dialog
                    var details_dialog = new Adw.AlertDialog(
                        "Crash Report Details",
                        "Below are the technical details of the crash. This information can help developers identify and fix the issue."
                    );
                    
                    // Add the crash report content (truncated if too long)
                    var truncated_content = crash_content;
                    if (crash_content.length > 2000) {
                        truncated_content = crash_content.substring(0, 2000) + "\n\n... (truncated for display)";
                    }
                    
                    details_dialog.set_body(truncated_content);
                    
                    details_dialog.add_response("close", "Close");
                    details_dialog.add_response("copy", "Copy to Clipboard");
                    details_dialog.add_response("open", "Open File");
                    
                    details_dialog.set_default_response("close");
                    details_dialog.set_close_response("close");
                    
                    details_dialog.response.connect((response) => {
                        switch (response) {
                            case "copy":
                                copy_to_clipboard(crash_content);
                                break;
                            case "open":
                                open_crash_file(crash_file);
                                break;
                        }
                    });
                    
                    details_dialog.present(null);
                    
                } else {
                    logger.error("Could not read crash file: %s", crash_file);
                }
            } catch (Error e) {
                logger.error("Failed to show crash details: %s", e.message);
            }
        }

        private void handle_crash_submission(string crash_file) {
            // Check if user has consented to crash reporting
            var crash_reporter_enabled = false; // Default to false if settings not available
            if (settings != null) {
                crash_reporter_enabled = settings.get_boolean("crash-reporter-enabled");
            }
            
            if (!crash_reporter_enabled) {
                show_consent_dialog(crash_file);
                return;
            }
            
            // Show submission dialog
            var submit_dialog = new Adw.AlertDialog(
                "Submit Crash Report",
                "This will send the crash report to help improve Karere. The report contains technical information about the crash but no personal data."
            );
            
            submit_dialog.add_response("cancel", "Cancel");
            submit_dialog.add_response("submit", "Submit Report");
            submit_dialog.set_response_appearance("submit", Adw.ResponseAppearance.SUGGESTED);
            submit_dialog.set_default_response("submit");
            submit_dialog.set_close_response("cancel");
            
            submit_dialog.response.connect((response) => {
                if (response == "submit") {
                    submit_crash_report.begin(crash_file);
                }
            });
            
            submit_dialog.present(null);
        }

        private void show_consent_dialog(string crash_file) {
            var consent_dialog = new Adw.AlertDialog(
                "Enable Crash Reporting?",
                "To submit this crash report and help improve Karere, you need to enable crash reporting. This will allow automatic submission of future crash reports."
            );
            
            consent_dialog.add_response("cancel", "Cancel");
            consent_dialog.add_response("enable", "Enable & Submit");
            consent_dialog.set_response_appearance("enable", Adw.ResponseAppearance.SUGGESTED);
            consent_dialog.set_default_response("enable");
            consent_dialog.set_close_response("cancel");
            
            consent_dialog.response.connect((response) => {
                if (response == "enable") {
                    if (settings != null) {
                        settings.set_boolean("crash-reporter-enabled", true);
                        submit_crash_report.begin(crash_file);
                    } else if (logger != null) {
                        logger.warning("Cannot enable crash reporting: settings not available");
                    }
                }
            });
            
            consent_dialog.present(null);
        }

        private async void submit_crash_report(string crash_file) {
            logger.info("Submitting crash report: %s", crash_file);
            
            try {
                // Read crash report content
                string crash_content;
                if (!FileUtils.get_contents(crash_file, out crash_content)) {
                    throw new IOError.FAILED("Could not read crash report file");
                }
                
                // Create HTTP session
                var session = new Soup.Session();
                
                var timeout = 30; // Default timeout
                var endpoint_url = ""; // Default empty endpoint
                
                if (settings != null) {
                    timeout = settings.get_int("crash-report-timeout");
                    endpoint_url = settings.get_string("crash-report-endpoint") ?? "";
                }
                
                session.timeout = timeout;
                session.user_agent = "%s/%s".printf(Config.APP_NAME, Config.VERSION);
                
                // Check endpoint URL
                if (endpoint_url.strip() == "") {
                    throw new IOError.INVALID_ARGUMENT("Crash report endpoint not configured");
                }
                
                // Prepare JSON payload
                var json_builder = new Json.Builder();
                json_builder.begin_object();
                
                // Basic crash information
                json_builder.set_member_name("application");
                json_builder.add_string_value(Config.APP_NAME);
                
                json_builder.set_member_name("version");
                json_builder.add_string_value(Config.VERSION);
                
                json_builder.set_member_name("timestamp");
                json_builder.add_string_value(new DateTime.now_utc().to_string());
                
                json_builder.set_member_name("report_content");
                json_builder.add_string_value(crash_content);
                
                // Optional: Include logs if enabled
                var include_logs = false; // Default to false if settings not available
                if (settings != null) {
                    include_logs = settings.get_boolean("crash-include-logs");
                }
                
                if (include_logs) {
                    var logs_content = get_recent_logs();
                    if (logs_content != null) {
                        json_builder.set_member_name("logs");
                        json_builder.add_string_value(logs_content);
                    }
                }
                
                json_builder.end_object();
                
                // Convert to JSON string
                var json_generator = new Json.Generator();
                json_generator.set_root(json_builder.get_root());
                var json_data = json_generator.to_data(null);
                
                // Create HTTP request
                var message = new Soup.Message("POST", endpoint_url);
                message.set_request_body_from_bytes("application/json", new Bytes(json_data.data));
                
                // Add headers
                message.request_headers.append("Content-Type", "application/json");
                message.request_headers.append("Accept", "application/json");
                message.request_headers.append("X-Crash-Reporter", "Karere");
                
                logger.debug("Sending crash report to: %s", endpoint_url);
                
                // Send request
                yield session.send_and_read_async(message, Priority.DEFAULT, null);
                
                // Check response
                if (message.status_code >= 200 && message.status_code < 300) {
                    logger.info("Crash report submitted successfully (HTTP %u)", message.status_code);
                    
                    // Mark this crash as submitted
                    try {
                        var submitted_file = crash_file + ".submitted";
                        var submission_info = new StringBuilder();
                        submission_info.append_printf("Submitted: %s\n", new DateTime.now_local().to_string());
                        submission_info.append_printf("Endpoint: %s\n", endpoint_url);
                        submission_info.append_printf("Status: %u\n", message.status_code);
                        FileUtils.set_contents(submitted_file, submission_info.str);
                    } catch (Error e) {
                        logger.warning("Could not mark crash as submitted: %s", e.message);
                    }
                    
                    // Show success dialog
                    show_submission_result_dialog(true, null);
                    
                } else {
                    throw new IOError.FAILED("HTTP %u: %s".printf(message.status_code, message.reason_phrase));
                }
                
            } catch (Error e) {
                logger.error("Failed to submit crash report: %s", e.message);
                show_submission_result_dialog(false, e.message);
            }
        }

        private string? get_recent_logs() {
            try {
                // Get logs directory
                var data_dir = Environment.get_user_data_dir();
                var logs_dir = GLib.Path.build_filename(data_dir, Config.APP_NAME, "logs");
                var logs_directory = File.new_for_path(logs_dir);
                
                if (!logs_directory.query_exists()) {
                    return null;
                }
                
                // Find the most recent log file
                var enumerator = logs_directory.enumerate_children(
                    FileAttribute.STANDARD_NAME + "," + FileAttribute.TIME_MODIFIED,
                    FileQueryInfoFlags.NONE
                );
                
                File? latest_log_file = null;
                int64 latest_time = 0;
                
                FileInfo info;
                while ((info = enumerator.next_file()) != null) {
                    var filename = info.get_name();
                    if (filename.has_suffix(".log")) {
                        var modification_time = info.get_modification_date_time().to_unix();
                        if (modification_time > latest_time) {
                            latest_time = modification_time;
                            latest_log_file = logs_directory.get_child(filename);
                        }
                    }
                }
                
                if (latest_log_file == null) {
                    return null;
                }
                
                // Read the last 50 lines of the log file
                string log_content;
                if (!FileUtils.get_contents(latest_log_file.get_path(), out log_content)) {
                    return null;
                }
                
                var lines = log_content.split("\n");
                if (lines.length <= 50) {
                    return log_content;
                }
                
                // Return last 50 lines
                var recent_lines = new StringBuilder();
                recent_lines.append("... (showing last 50 lines)\n");
                for (int i = lines.length - 50; i < lines.length; i++) {
                    recent_lines.append_printf("%s\n", lines[i]);
                }
                
                return recent_lines.str;
                
            } catch (Error e) {
                logger.debug("Could not read recent logs: %s", e.message);
                return null;
            }
        }

        private void show_submission_result_dialog(bool success, string? error_message) {
            Adw.AlertDialog result_dialog;
            
            if (success) {
                result_dialog = new Adw.AlertDialog(
                    // TRANSLATORS: Dialog title when crash report is successfully submitted
                    _("Crash Report Submitted"),
                    // TRANSLATORS: Success message for crash report submission
                    _("Thank you for helping improve Karere. The crash report has been submitted successfully and will help us fix this issue.")
                );
            } else {
                // TRANSLATORS: Error message when crash report submission fails
                var message = _("Failed to submit crash report");
                if (error_message != null) {
                    message += ": " + error_message;
                }
                // TRANSLATORS: Additional info about local crash report storage
                message += "\n\n" + _("The crash report has been saved locally and you can try submitting it again later.");
                
                result_dialog = new Adw.AlertDialog(
                    // TRANSLATORS: Dialog title when crash report submission fails
                    _("Submission Failed"),
                    message
                );
            }
            
            // TRANSLATORS: Button to close the crash report result dialog
            result_dialog.add_response("close", _("Close"));
            result_dialog.set_default_response("close");
            result_dialog.set_close_response("close");
            
            result_dialog.present(null);
        }

        private void copy_to_clipboard(string content) {
            try {
                var clipboard = Gdk.Display.get_default().get_clipboard();
                clipboard.set_text(content);
                logger.debug("Crash report copied to clipboard");
            } catch (Error e) {
                logger.warning("Failed to copy to clipboard: %s", e.message);
            }
        }

        private void open_crash_file(string crash_file) {
            try {
                AppInfo.launch_default_for_uri("file://" + crash_file, null);
                logger.debug("Opened crash file: %s", crash_file);
            } catch (Error e) {
                logger.warning("Failed to open crash file: %s", e.message);
            }
        }

        public void cleanup() {
            if (!handlers_installed) {
                return;
            }
            
            logger.debug("Cleaning up crash reporter");
            
            // Restore default signal handlers
            Posix.signal(Posix.Signal.SEGV, null);
            Posix.signal(Posix.Signal.ABRT, null);
            Posix.signal(Posix.Signal.FPE, null);
            Posix.signal(Posix.Signal.ILL, null);
            Posix.signal(Posix.Signal.BUS, null);
            Posix.signal(Posix.Signal.TERM, null);
            
            handlers_installed = false;
            instance = null;
        }
    }
}