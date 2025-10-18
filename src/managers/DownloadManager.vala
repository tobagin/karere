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
     * DownloadManager handles download detection, directory management, and file opening.
     *
     * This manager detects downloads via WebKit policy decisions, resolves the target
     * download directory (custom or default), and provides functionality to open downloaded
     * files with the system default application via Flatpak portals.
     */
    public class DownloadManager : Object {

        // Signals
        public signal void download_detected(string uri, string suggested_filename);
        public signal void download_completed(string filename, string file_path);
        public signal void download_failed(string filename, string error_message);
        public signal void directory_fallback(string reason);
        public signal void error_opening_file(string error_message);

        // Properties
        private Settings? settings;
        private string? cached_download_directory = null;

        /**
         * Create a new DownloadManager
         *
         * @param settings GSettings instance (can be null)
         */
        public DownloadManager(Settings? settings) {
            this.settings = settings;

            // Listen for settings changes
            if (settings != null) {
                settings.changed["custom-download-directory"].connect(() => {
                    // Clear cache when setting changes
                    cached_download_directory = null;
                    debug("Custom download directory setting changed, cache cleared");
                });
            }

            debug("DownloadManager initialized");
        }

        /**
         * Get the download directory to use
         *
         * Returns the custom directory if set and accessible, otherwise falls back to
         * xdg-download or ~/Downloads as ultimate fallback.
         *
         * @return The download directory path
         */
        public string get_download_directory() {
            // Return cached value if available
            if (cached_download_directory != null) {
                return cached_download_directory;
            }

            string? result_dir = null;
            string? fallback_reason = null;

            // Check custom directory first
            if (settings != null) {
                var custom_dir = settings.get_string("custom-download-directory");
                if (custom_dir != null && custom_dir != "") {
                    // Check if custom directory exists and is accessible
                    if (FileUtils.test(custom_dir, FileTest.IS_DIR)) {
                        // Test if writable by checking access
                        var test_file = Path.build_filename(custom_dir, ".karere-write-test");
                        try {
                            FileUtils.set_contents(test_file, "test");
                            FileUtils.remove(test_file);
                            result_dir = custom_dir;
                            debug("Using custom download directory: %s", custom_dir);
                        } catch (Error e) {
                            fallback_reason = "Custom directory not writable: " + e.message;
                            warning("%s", fallback_reason);
                        }
                    } else {
                        fallback_reason = "Custom directory does not exist or is not accessible";
                        warning("%s: %s", fallback_reason, custom_dir);
                    }
                }
            }

            // Fall back to xdg-download
            if (result_dir == null) {
                var download_dir = Environment.get_user_special_dir(UserDirectory.DOWNLOAD);
                if (download_dir != null && FileUtils.test(download_dir, FileTest.IS_DIR)) {
                    result_dir = download_dir;
                    if (fallback_reason != null) {
                        debug("Falling back to xdg-download: %s", download_dir);
                        directory_fallback(fallback_reason);
                    } else {
                        debug("Using default download directory: %s", download_dir);
                    }
                }
            }

            // Ultimate fallback to ~/Downloads
            if (result_dir == null) {
                result_dir = Path.build_filename(Environment.get_home_dir(), "Downloads");

                // Create ~/Downloads if it doesn't exist
                if (!FileUtils.test(result_dir, FileTest.IS_DIR)) {
                    try {
                        DirUtils.create_with_parents(result_dir, 0755);
                        info("Created fallback download directory: %s", result_dir);
                    } catch (Error e) {
                        critical("Failed to create download directory: %s", e.message);
                    }
                }

                if (fallback_reason != null) {
                    debug("Using ultimate fallback directory: %s", result_dir);
                }
            }

            // Cache the result
            cached_download_directory = result_dir;
            return result_dir;
        }

        /**
         * Open a file with the system default application
         *
         * Uses AppInfo.launch_default_for_uri_async() which automatically uses
         * Flatpak portals when running in a sandboxed environment.
         *
         * @param file_path The absolute path to the file to open
         */
        public void open_file(string file_path) {
            debug("Opening file: %s", file_path);

            // Check if file exists
            if (!FileUtils.test(file_path, FileTest.EXISTS)) {
                var error_msg = "File does not exist: " + file_path;
                warning("%s", error_msg);
                error_opening_file(error_msg);
                return;
            }

            // Construct file:// URI
            var file = File.new_for_path(file_path);
            var uri = file.get_uri();

            // Open with default application using AppInfo (uses portal automatically)
            AppInfo.launch_default_for_uri_async.begin(uri, null, null, (obj, res) => {
                try {
                    AppInfo.launch_default_for_uri_async.end(res);
                    info("File opened successfully: %s", file_path);
                } catch (Error e) {
                    var error_msg = "Failed to open file: " + e.message;
                    critical("%s", error_msg);
                    error_opening_file(error_msg);
                }
            });
        }

        /**
         * Handle download detection from WebViewManager
         *
         * @param uri The URI being downloaded
         * @param suggested_filename The suggested filename for the download
         */
        public void on_download_detected(string uri, string suggested_filename) {
            debug("Download detected: %s -> %s", uri, suggested_filename);
            // Stub - will be expanded in Task 2.2
        }
    }
}
