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

#if DEVELOPMENT
    [GtkTemplate (ui = "/io/github/tobagin/karere/Devel/window.ui")]
#else
    [GtkTemplate (ui = "/io/github/tobagin/karere/window.ui")]
#endif
    public class Window : Adw.ApplicationWindow {
        
        
        [GtkChild]
        private unowned Adw.HeaderBar header_bar;
        
        
        [GtkChild]
        private unowned Gtk.MenuButton menu_button;
        
        [GtkChild]
        private unowned Adw.ToastOverlay toast_overlay;
        
        [GtkChild]
        private unowned Gtk.Box web_container;
        
        [GtkChild]
        private unowned Gtk.Box zoom_controls_box;
        
        private Settings settings;
        private WebKitManager webkit_manager;
        private WebViewManager webview_manager;
        private NotificationManager notification_manager;
        private WindowStateManager window_state_manager;
        private ClipboardManager clipboard_manager;
        private WebKitNotificationBridge notification_bridge;
        private DownloadManager download_manager;
        private SpellCheckingManager spell_checking_manager;
        private Gdk.Clipboard clipboard;

        public Window(Gtk.Application app) {
            Object(application: app);

            // Initialize settings with error handling
            try {
                settings = new Settings(Config.APP_ID);
            } catch (Error e) {
                critical("Failed to initialize settings: %s", e.message);
                warning("Continuing without settings - using defaults");
                settings = null;
            }

            setup_window_properties();
            setup_actions();
            setup_webkit();
            setup_notifications();
            setup_downloads();
            setup_settings_listeners();
            setup_accessibility_features();

            // Initialize clipboard manager
            clipboard = this.get_clipboard();
            clipboard_manager = new ClipboardManager(clipboard, webview_manager.web_view);
            clipboard_manager.setup_paste_detection(webview_manager.web_view);

            // Connect clipboard manager signals for toast notifications
            clipboard_manager.paste_succeeded.connect((image_type) => {
                show_success_toast(_("Image pasted to WhatsApp"));
            });
            clipboard_manager.paste_failed.connect((error_message) => {
                show_error_toast(error_message);
            });

            // Initialize focus indicators state
            if (settings != null) {
                update_focus_indicators();
                update_zoom_controls_visibility(); // Initialize zoom controls visibility
            }

            // Initialize window state manager
            window_state_manager = new WindowStateManager(settings, this);
            window_state_manager.restore_state();
            window_state_manager.start_tracking();

            info("Window created and initialized");
        }

        private void setup_window_properties() {
            // Set window properties
            set_title(Config.APP_NAME);
            set_icon_name(Config.APP_ID);

            debug("Window properties configured");
        }

        private void setup_actions() {
            // Developer tools action (keep for debugging if needed)
            var dev_tools_action = new SimpleAction("dev-tools", null);
            dev_tools_action.activate.connect(() => {
                webview_manager.open_developer_tools();
            });
            add_action(dev_tools_action);

            // Open downloaded file action
            var open_file_action = new SimpleAction("open-downloaded-file", VariantType.STRING);
            open_file_action.activate.connect((parameter) => {
                if (parameter != null) {
                    var file_path = parameter.get_string();
                    debug("Opening downloaded file: %s", file_path);
                    download_manager.open_file(file_path);
                }
            });
            add_action(open_file_action);

            debug("Window actions configured");
        }

        private void setup_webkit() {
            webkit_manager = new WebKitManager();

            // Initialize WebView manager
            webview_manager = new WebViewManager(settings, webkit_manager);
            webview_manager.setup(web_container);

            // Initialize spell checking manager
            spell_checking_manager = new SpellCheckingManager();
            var web_context = webview_manager.web_view.get_context();
            spell_checking_manager.configure_webkit(web_context);
            spell_checking_manager.setup_settings_listeners();

            // Connect to WebViewManager signals
            webview_manager.load_failed.connect((uri, error_message) => {
                // TRANSLATORS: Error message when WhatsApp Web fails to load
                show_error_toast(_("Failed to load WhatsApp Web. Please check your internet connection."));
            });

            webview_manager.external_link_clicked.connect((uri) => {
                debug("External link opened: %s", uri);
            });

            debug("WebKit configured");
        }

        private void setup_notifications() {
            // Get the notification manager from the application
            var app = get_application() as Karere.Application;
            if (app != null) {
                notification_manager = app.get_notification_manager();

                // Initialize WebKit notification bridge
                notification_bridge = new WebKitNotificationBridge(settings, notification_manager, this);
                notification_bridge.setup(webview_manager.web_view);

                debug("Notifications configured");
            } else {
                critical("Could not get application reference for notifications");
            }
        }

        private void setup_downloads() {
            // Get the download manager from the application
            var app = get_application() as Karere.Application;
            if (app != null) {
                download_manager = app.get_download_manager();

                // Set initial download directory in WebViewManager
                var download_dir = download_manager.get_download_directory();
                webview_manager.set_download_directory(download_dir);

                // Update WebViewManager when download directory changes
                if (settings != null) {
                    settings.changed["custom-download-directory"].connect(() => {
                        var new_dir = download_manager.get_download_directory();
                        webview_manager.set_download_directory(new_dir);
                        debug("Download directory updated to: %s", new_dir);
                    });
                }

                // Connect to WebViewManager download signal
                webview_manager.download_detected.connect((uri, filename) => {
                    debug("Download detected in Window: %s -> %s", uri, filename);
                    on_download_detected(uri, filename);
                });

                // Connect to DownloadManager signals for toasts
                download_manager.error_opening_file.connect((error_msg) => {
                    show_error_toast(error_msg);
                });

                download_manager.directory_fallback.connect((reason) => {
                    show_info_toast(_("Download directory unavailable, using default"));
                });

                debug("Downloads configured");
            } else {
                critical("Could not get application reference for downloads");
            }
        }

        /**
         * Handle download detection
         */
        private void on_download_detected(string uri, string filename) {
            debug("Handling download: %s", filename);

            // Check if notifications are enabled
            if (settings == null || !settings.get_boolean("download-notifications-enabled")) {
                return;
            }

            // Get download directory
            var download_dir = download_manager.get_download_directory();
            var file_path = Path.build_filename(download_dir, filename);

            // Show toast notification with "Open" button
            var message = _("Downloaded: %s").printf(filename);
            var toast = new Adw.Toast(message);
            toast.timeout = 5;
            toast.button_label = _("Open");

            // Connect toast button to open file action
            toast.action_name = "win.open-downloaded-file";
            toast.action_target = new Variant.string(file_path);

            toast_overlay.add_toast(toast);
            debug("Download toast shown for: %s", filename);
        }

        private void setup_settings_listeners() {
            if (settings == null) {
                warning("Cannot setup settings listeners: settings is null");
                return;
            }

            // Listen for settings changes (spell checking listeners are now in SpellCheckingManager)
            settings.changed["developer-tools-enabled"].connect(() => {
                update_developer_tools();
            });
            settings.changed["focus-indicators-enabled"].connect(() => {
                update_focus_indicators();
            });
            settings.changed["webview-zoom-controls-enabled"].connect(() => {
                update_zoom_controls_visibility();
            });
            settings.changed["webview-zoom-enabled"].connect(() => {
                update_zoom_controls_visibility();
            });
        }


        /**
         * Setup accessibility features for the window
         */
        private void setup_accessibility_features() {
            // Set up ARIA roles and labels
            setup_aria_roles();
            
            // Set up skip links for keyboard navigation
            setup_skip_links();
            
            // Set up focus management
            setup_focus_management();
            
            debug("Accessibility features configured");
        }

        /**
         * Setup ARIA roles and labels for major UI elements
         */
        private void setup_aria_roles() {
            // Set main content role
            web_container.update_property(Gtk.AccessibleProperty.LABEL, _("WhatsApp Web Content"));
            web_container.update_property(Gtk.AccessibleProperty.DESCRIPTION, 
                _("Main WhatsApp Web interface - use Tab to navigate, Enter to activate"));
            
            // Set header bar role and label
            header_bar.update_property(Gtk.AccessibleProperty.LABEL, _("Application Header"));
            header_bar.update_property(Gtk.AccessibleProperty.DESCRIPTION, 
                _("Contains application title and main menu"));
            
            // Set menu button label
            menu_button.update_property(Gtk.AccessibleProperty.LABEL, _("Main Menu"));
            menu_button.update_property(Gtk.AccessibleProperty.DESCRIPTION, 
                _("Access preferences, help, and other application options"));
            
            // Set toast overlay role
            toast_overlay.update_property(Gtk.AccessibleProperty.LABEL, _("Notification Area"));
        }

        /**
         * Setup skip links for keyboard navigation
         */
        private void setup_skip_links() {
            // Skip links functionality temporarily disabled due to LibAdwaita API limitations
            // AdwToastOverlay doesn't support add_overlay method
            debug("Skip links not implemented - AdwToastOverlay API limitation");
        }

        /**
         * Setup enhanced focus management
         */
        private void setup_focus_management() {
            // Ensure proper focus chain
            var focus_chain = new List<Gtk.Widget>();
            focus_chain.append(menu_button);
            focus_chain.append(webview_manager.web_view);

            // Set up focus event controller using the correct Widget method
            var focus_controller = new Gtk.EventControllerFocus();
            focus_controller.enter.connect(on_window_focus_in);
            focus_controller.leave.connect(on_window_focus_out);
            // Add to the web view instead of the window since the method signature is incompatible
            webview_manager.web_view.add_controller(focus_controller);

            debug("Focus management configured");
        }

        /**
         * Handle window focus in events
         */
        private void on_window_focus_in() {
            debug("Window gained focus");
        }

        /**
         * Handle window focus out events
         */
        private void on_window_focus_out() {
            debug("Window lost focus");
        }


        private void update_developer_tools() {
            if (settings == null || webview_manager == null) {
                warning("Cannot update developer tools: settings or webview_manager is null");
                return;
            }

            var dev_tools_enabled = settings.get_boolean("developer-tools-enabled");
            webview_manager.update_developer_tools_setting(dev_tools_enabled);
        }

        /**
         * Update WebView zoom level
         *
         * @param zoom_level The new zoom level to apply
         */
        public void update_webkit_zoom(double zoom_level) {
            if (webview_manager == null || webview_manager.web_view == null) {
                warning("Cannot update WebView zoom: webview_manager.web_view is null");
                return;
            }

            webview_manager.web_view.zoom_level = zoom_level;
            info("WebView zoom level updated to: %f", zoom_level);
        }

        /**
         * Update all WebKit settings through WebKitManager
         */
        public void update_webkit_settings() {
            if (webkit_manager != null && webview_manager.web_view != null) {
                webkit_manager.update_settings(webview_manager.web_view);
                debug("WebKit settings updated through WebKitManager");
            } else {
                warning("Cannot update WebKit settings: webkit_manager or webview_manager.web_view is null");
            }
        }

        /**
         * Increase WebView zoom level
         */
        public void webkit_zoom_in() {
            if (settings == null || webview_manager.web_view == null) return;
            if (!settings.get_boolean("webview-zoom-enabled")) return;
            
            var current_zoom = settings.get_double("webkit-zoom-level");
            var zoom_step = settings.get_double("webkit-zoom-step");
            var new_zoom = Math.fmin(current_zoom + zoom_step, 3.0);
            
            settings.set_double("webkit-zoom-level", new_zoom);
            webview_manager.web_view.zoom_level = new_zoom;
            debug("WebView zoomed in to %f", new_zoom);
        }

        /**
         * Decrease WebView zoom level
         */
        public void webkit_zoom_out() {
            if (settings == null || webview_manager.web_view == null) return;
            if (!settings.get_boolean("webview-zoom-enabled")) return;
            
            var current_zoom = settings.get_double("webkit-zoom-level");
            var zoom_step = settings.get_double("webkit-zoom-step");
            var new_zoom = Math.fmax(current_zoom - zoom_step, 0.5);
            
            settings.set_double("webkit-zoom-level", new_zoom);
            webview_manager.web_view.zoom_level = new_zoom;
            debug("WebView zoomed out to %f", new_zoom);
        }

        /**
         * Reset WebView zoom level to default
         */
        public void webkit_zoom_reset() {
            if (settings == null || webview_manager.web_view == null) return;
            if (!settings.get_boolean("webview-zoom-enabled")) return;
            
            settings.set_double("webkit-zoom-level", 1.0);
            webview_manager.web_view.zoom_level = 1.0;
            debug("WebView zoom reset to default");
        }

        /**
         * Update focus indicators visibility based on settings
         */
        public void update_focus_indicators() {
            if (settings == null) {
                warning("Cannot update focus indicators: settings is null");
                return;
            }
            
            var focus_indicators_enabled = settings.get_boolean("focus-indicators-enabled");
            debug("Focus indicators setting value: %s", focus_indicators_enabled.to_string());
            
            // Apply the CSS class to the main window using modern GTK 4.10+ approach
            if (focus_indicators_enabled) {
                // Add the focus indicators CSS class to window
                add_css_class("karere-focus-indicators");
                info("Focus indicators enabled - CSS class added");
            } else {
                // Remove the focus indicators CSS class from window
                remove_css_class("karere-focus-indicators");
                info("Focus indicators disabled - CSS class removed");
            }
        }

        /**
         * Update zoom controls visibility based on webview-zoom-controls-enabled setting
         */
        private void update_zoom_controls_visibility() {
            if (settings == null) {
                warning("Cannot update zoom controls visibility: settings is null");
                return;
            }
            
            var zoom_controls_enabled = settings.get_boolean("webview-zoom-controls-enabled");
            var zoom_enabled = settings.get_boolean("webview-zoom-enabled");
            
            // Only show controls if both zoom is enabled AND controls are enabled
            zoom_controls_box.visible = zoom_enabled && zoom_controls_enabled;
            
            info("Zoom controls %s", (zoom_enabled && zoom_controls_enabled) ? "shown" : "hidden");
        }

        // Removed: setup_drag_and_drop() method
        // Native WebKit drag and drop now handles file drops directly to WhatsApp Web

        // Removed: on_file_dropped() method
        // Files are now handled natively by WebKit and WhatsApp Web

        // Removed: share_file_with_whatsapp() method
        // File sharing now handled natively by WebKit and WhatsApp Web

        // Removed: show_file_sharing_fallback_dialog() method
        // No longer needed as files are handled natively by WebKit

        // Removed: copy_file_path_to_clipboard() method
        // No longer needed as files are handled natively by WebKit

        // Removed: open_file_location() method
        // No longer needed as files are handled natively by WebKit

        public void show_error_toast(string message) {
            var toast = new Adw.Toast(message);
            toast.timeout = 5;
            toast_overlay.add_toast(toast);
            
            debug("Error toast shown: %s", message);
        }

        public void show_info_toast(string message) {
            var toast = new Adw.Toast(message);
            toast.timeout = 3;
            toast_overlay.add_toast(toast);
            
            debug("Info toast shown: %s", message);
        }

        public void show_success_toast(string message) {
            var toast = new Adw.Toast(message);
            toast.timeout = 2;
            toast_overlay.add_toast(toast);
            
            debug("Success toast shown: %s", message);
        }
        
        public void show_toast(Adw.Toast toast) {
            toast_overlay.add_toast(toast);
            debug("Toast shown: %s", toast.title);
        }

        public void open_developer_tools() {
            webview_manager.open_developer_tools();
        }

        public bool is_developer_tools_open() {
            return webview_manager.is_developer_tools_open();
        }

        public void close_developer_tools() {
            webview_manager.close_developer_tools();
        }

        /**
         * Reload the WebView content
         */
        public void reload_webview(bool force_reload = false) {
            webview_manager.reload(force_reload);
            show_info_toast(force_reload ? _("Force reloading...") : _("Reloading..."));
        }

        /**
         * Show accessibility status in a toast
         */
        public void show_accessibility_status() {
            var app = get_application() as Karere.Application;
            if (app != null) {
                // This would need to be implemented to get the accessibility manager reference
                // For now, show a simple status message
                show_info_toast(_("Accessibility features active"));
            }
        }

        public override bool close_request() {
            info("Window close requested");

            // Save window state before closing
            if (window_state_manager != null) {
                window_state_manager.save_state();
            }

            // Hide the window instead of closing it to keep app running in background
            set_visible(false);

            // Trigger background notification when window is actually hidden
            if (notification_manager != null) {
                notification_manager.on_window_focus_changed(false);
            }

            // Prevent the window from being destroyed
            return true;
        }

        public override void dispose() {
            info("Window being disposed");
            
            // Notify the application that this window is being destroyed
            var app = get_application() as Karere.Application;
            if (app != null) {
                app.window_destroyed();
            }
            
            // Clean up resources
            if (webkit_manager != null) {
                webkit_manager = null;
            }
            
            if (notification_manager != null) {
                notification_manager = null;
            }
            
            base.dispose();
        }

    }
}