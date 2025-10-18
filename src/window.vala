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
        
        private WebKit.WebView web_view;

        private Settings settings;
        private WebKitManager webkit_manager;
        private NotificationManager notification_manager;
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
            setup_settings_listeners();
            setup_accessibility_features();
            setup_clipboard_paste();

            // Initialize focus indicators state
            if (settings != null) {
                update_focus_indicators();
                update_zoom_controls_visibility(); // Initialize zoom controls visibility
            }

            restore_window_state();

            info("Window created and initialized");
        }

        private void setup_window_properties() {
            // Set window properties
            set_title(Config.APP_NAME);
            set_icon_name(Config.APP_ID);

            // Connect window state change signals
            notify["maximized"].connect(save_window_state);
            notify["default-width"].connect(save_window_state);
            notify["default-height"].connect(save_window_state);


            debug("Window properties configured");
        }

        private void setup_actions() {
            // Developer tools action (keep for debugging if needed)
            var dev_tools_action = new SimpleAction("dev-tools", null);
            dev_tools_action.activate.connect(() => {
                var inspector = web_view.get_inspector();
                inspector.show();
                debug("Developer tools opened");
            });
            add_action(dev_tools_action);

            debug("Window actions configured");
        }

        private void setup_webkit() {
            webkit_manager = new WebKitManager();
            
            // Create WebView programmatically
            web_view = new WebKit.WebView();
            web_view.vexpand = true;
            web_view.hexpand = true;
            
            // Configure cookie storage after WebView creation
            configure_cookie_storage();
            
            // Use WebKitManager to configure WebView settings properly
            webkit_manager.configure_web_view(web_view);
            
            // Configure spell checking
            var webkit_settings = web_view.get_settings();
            setup_spell_checking(webkit_settings);
            
            // Add WebView to container
            web_container.append(web_view);
            
            // Note: Removed custom drag and drop handling to allow native WebKit behavior
            // This lets WhatsApp Web handle file drops directly, just like copy-paste
            
            // Configure WebView
            web_view.load_uri("https://web.whatsapp.com");


            // Connect WebView signals
            web_view.load_changed.connect(on_load_changed);
            web_view.load_failed.connect(on_load_failed);
            web_view.decide_policy.connect(on_navigation_policy_decision);
            web_view.create.connect(on_create_new_web_view);

            // Connect notification permission signal
            web_view.permission_request.connect(on_permission_request);

            // Set up WebKit notification permissions persistence
            setup_webkit_permission_persistence();

            debug("WebKit configured");
        }

        private void setup_notifications() {
            // Get the notification manager from the application
            var app = get_application() as Karere.Application;
            if (app != null) {
                notification_manager = app.get_notification_manager();
                debug("Notifications configured");
            } else {
                critical("Could not get application reference for notifications");
            }
        }

        private void configure_cookie_storage() {
            // Note: WebKit 6.0 API change - cookie manager access has changed
            // var website_data_manager = network_session.get_website_data_manager();
            // var cookie_manager = website_data_manager.get_cookie_manager();

            // Note: Cookie storage setup disabled due to WebKit 6.0 API changes
            // Cookie storage will be handled automatically by WebKit
            info("Cookie storage will be handled by WebKit defaults");
        }

        private void setup_webkit_permission_persistence() {
            // The key insight: WebKit persists permissions automatically when using the default
            // WebContext with persistent storage. Our native dialog approach should work
            // because the permission decision gets stored in WebKit's internal database.
            info("WebKit permission persistence relies on native permission handling");
        }


        private void setup_spell_checking(WebKit.Settings webkit_settings) {
            if (settings == null) {
                warning("Cannot setup spell checking: settings is null");
                return;
            }

            var spell_enabled = settings.get_boolean("spell-checking-enabled");
            var web_context = web_view.get_context();

            info("Setting up spell checking: enabled=%s", spell_enabled.to_string());

            // Enable/disable spell checking
            web_context.set_spell_checking_enabled(spell_enabled);

            if (spell_enabled) {
                // Get spell checking languages
                string[] spell_languages = get_spell_checking_languages();

                // Set the languages
                web_context.set_spell_checking_languages(spell_languages);
                info("Spell checking languages set: %s", string.joinv(", ", spell_languages));
            } else {
                info("Spell checking disabled");
            }
        }

        private string[] get_spell_checking_languages() {
            if (settings == null) {
                warning("Cannot get spell checking languages: settings is null, using fallback");
                return {"en_US"};
            }

            var auto_detect = settings.get_boolean("spell-checking-auto-detect");
            var languages = settings.get_strv("spell-checking-languages");

            if (auto_detect || languages.length == 0) {
                // Auto-detect from system locale
                var locale = Intl.setlocale(LocaleCategory.MESSAGES, null);
                if (locale != null) {
                    // Extract language code (e.g., "en_US.UTF-8" -> "en_US")
                    var parts = locale.split(".");
                    var lang_code = parts[0];
                    info("Auto-detected spell checking language: %s", lang_code);
                    return {lang_code};
                } else {
                    info("Using fallback spell checking language: en_US");
                    return {"en_US"};
                }
            } else {
                info("Using user-specified spell checking languages: %s", string.joinv(", ", languages));
                return languages;
            }
        }

        private void setup_settings_listeners() {
            if (settings == null) {
                warning("Cannot setup settings listeners: settings is null");
                return;
            }
            
            // Listen for settings changes
            settings.changed["spell-checking-enabled"].connect(() => {
                update_spell_checking();
            });
            settings.changed["spell-checking-auto-detect"].connect(() => {
                update_spell_checking();
            });
            settings.changed["spell-checking-languages"].connect(() => {
                update_spell_checking();
            });
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
            focus_chain.append(web_view);
            
            // Set up focus event controller using the correct Widget method
            var focus_controller = new Gtk.EventControllerFocus();
            focus_controller.enter.connect(on_window_focus_in);
            focus_controller.leave.connect(on_window_focus_out);
            // Add to the web view instead of the window since the method signature is incompatible
            web_view.add_controller(focus_controller);
            
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

        /**
         * Setup clipboard image paste functionality
         */
        private void setup_clipboard_paste() {
            // Get the display clipboard
            clipboard = this.get_clipboard();

            // Set up key event controller to detect Ctrl+V
            var key_controller = new Gtk.EventControllerKey();
            key_controller.key_pressed.connect(on_key_pressed);

            // Add the controller to the web view so it can intercept paste events
            web_view.add_controller(key_controller);

            debug("Clipboard paste functionality configured");
        }

        /**
         * Handle key press events to detect Ctrl+V for image paste
         */
        private bool on_key_pressed(uint keyval, uint keycode, Gdk.ModifierType state) {
            // Check for Ctrl+V (paste)
            if (keyval == Gdk.Key.v && (state & Gdk.ModifierType.CONTROL_MASK) != 0) {
                debug("Ctrl+V detected, checking clipboard for image");
                handle_paste_event();
                return true; // Consume the event to prevent default paste
            }

            return false; // Let other key events pass through
        }

        /**
         * Handle paste events by checking clipboard for images
         */
        private void handle_paste_event() {
            if (clipboard == null) {
                warning("Clipboard not available");
                return;
            }

            // Check if clipboard contains an image
            var formats = clipboard.get_formats();

            if (formats.contain_gtype(typeof(Gdk.Texture))) {
                // Clipboard contains an image texture
                clipboard.read_texture_async.begin(null, (obj, res) => {
                    try {
                        var texture = clipboard.read_texture_async.end(res);
                        if (texture != null) {
                            process_clipboard_image(texture);
                        }
                    } catch (Error e) {
                        critical("Failed to read texture from clipboard: %s", e.message);
                    }
                });
            } else if (formats.contain_mime_type("image/png") ||
                       formats.contain_mime_type("image/jpeg") ||
                       formats.contain_mime_type("image/gif")) {
                // Clipboard contains image data
                string[] mime_types = {"image/png", "image/jpeg", "image/gif"};
                clipboard.read_async.begin(mime_types, GLib.Priority.DEFAULT, null, (obj, res) => {
                    try {
                        string mime_type;
                        var input_stream = clipboard.read_async.end(res, out mime_type);
                        if (input_stream != null) {
                            process_clipboard_image_stream(input_stream, mime_type);
                        }
                    } catch (Error e) {
                        critical("Failed to read image stream from clipboard: %s", e.message);
                    }
                });
            } else {
                debug("No image found in clipboard");
                // Let the default paste behavior handle text or other content
                inject_default_paste();
            }
        }

        /**
         * Process a clipboard image texture
         */
        private void process_clipboard_image(Gdk.Texture texture) {
            info("Processing clipboard image texture: %dx%d", texture.get_width(), texture.get_height());

            try {
                // Convert texture to PNG bytes
                var bytes = texture.save_to_png_bytes();
                inject_image_into_whatsapp(bytes, "image/png");
            } catch (Error e) {
                critical("Failed to convert texture to PNG: %s", e.message);
                show_error_toast(_("Failed to process clipboard image"));
            }
        }

        /**
         * Process a clipboard image stream
         */
        private void process_clipboard_image_stream(GLib.InputStream input_stream, string mime_type) {
            info("Processing clipboard image stream with MIME type: %s", mime_type);

            try {
                // Read the entire stream into memory
                var output_stream = new MemoryOutputStream(null, GLib.realloc, GLib.free);
                output_stream.splice(input_stream, OutputStreamSpliceFlags.CLOSE_SOURCE | OutputStreamSpliceFlags.CLOSE_TARGET);

                var data = output_stream.steal_data();
                var bytes = new Bytes(data);

                inject_image_into_whatsapp(bytes, mime_type);
            } catch (Error e) {
                critical("Failed to process clipboard image stream: %s", e.message);
                show_error_toast(_("Failed to process clipboard image"));
            }
        }

        /**
         * Inject image data into WhatsApp Web
         */
        private void inject_image_into_whatsapp(Bytes image_bytes, string mime_type) {
            info("Injecting %s image (%zu bytes) into WhatsApp Web", mime_type, image_bytes.get_size());

            // Convert bytes to base64 data URL
            var base64_data = Base64.encode(image_bytes.get_data());
            var data_url = "data:%s;base64,%s".printf(mime_type, base64_data);

            // JavaScript to inject the image into WhatsApp Web
            var javascript = """
                (function() {
                    try {
                        // Find the message input area
                        const messageBox = document.querySelector('[contenteditable="true"][data-tab="10"]') ||
                                          document.querySelector('[contenteditable="true"]') ||
                                          document.querySelector('div[contenteditable="true"]');

                        if (!messageBox) {
                            console.log('WhatsApp message box not found');
                            return;
                        }

                        // Focus the message box
                        messageBox.focus();

                        // Create a File object from the data URL
                        fetch('%s')
                            .then(res => res.blob())
                            .then(blob => {
                                const file = new File([blob], 'clipboard_image.png', { type: '%s' });

                                // Create a DataTransfer object to simulate drag and drop
                                const dt = new DataTransfer();
                                dt.items.add(file);

                                // Create and dispatch a paste event
                                const pasteEvent = new ClipboardEvent('paste', {
                                    clipboardData: dt,
                                    bubbles: true,
                                    cancelable: true
                                });

                                // Dispatch to the message box
                                messageBox.dispatchEvent(pasteEvent);

                                console.log('Image paste event dispatched to WhatsApp');
                            })
                            .catch(err => {
                                console.error('Failed to create file from data URL:', err);

                                // Fallback: try to trigger file input click
                                const fileInput = document.querySelector('input[type="file"][accept*="image"]');
                                if (fileInput) {
                                    // Create a new file input with our image
                                    const newFileInput = document.createElement('input');
                                    newFileInput.type = 'file';
                                    newFileInput.accept = 'image/*';
                                    newFileInput.style.display = 'none';
                                    document.body.appendChild(newFileInput);

                                    // We can't programmatically set files on input elements for security reasons
                                    // So this approach won't work either
                                    console.log('Cannot programmatically set file input - security restriction');
                                    document.body.removeChild(newFileInput);
                                }
                            });
                    } catch (error) {
                        console.error('Error injecting image into WhatsApp:', error);
                    }
                })();
            """.printf(data_url, mime_type);

            // Execute the JavaScript
            web_view.evaluate_javascript.begin(javascript, -1, null, null, null, (obj, res) => {
                try {
                    web_view.evaluate_javascript.end(res);
                    info("Image injection JavaScript executed successfully");
                    show_success_toast(_("Image pasted to WhatsApp"));
                } catch (Error e) {
                    critical("Failed to execute image injection JavaScript: %s", e.message);
                    show_error_toast(_("Failed to paste image to WhatsApp"));
                }
            });
        }

        /**
         * Inject default paste behavior (for non-image content)
         */
        private void inject_default_paste() {
            debug("Executing default paste behavior");

            var javascript = """
                (function() {
                    // Find the message input area
                    const messageBox = document.querySelector('[contenteditable="true"][data-tab="10"]') ||
                                      document.querySelector('[contenteditable="true"]') ||
                                      document.querySelector('div[contenteditable="true"]');

                    if (messageBox) {
                        // Focus the message box
                        messageBox.focus();

                        // Execute paste command
                        document.execCommand('paste');
                        console.log('Default paste executed');
                    } else {
                        console.log('WhatsApp message box not found for default paste');
                    }
                })();
            """;

            web_view.evaluate_javascript.begin(javascript, -1, null, null, null, (obj, res) => {
                try {
                    web_view.evaluate_javascript.end(res);
                    debug("Default paste JavaScript executed");
                } catch (Error e) {
                    warning("Failed to execute default paste JavaScript: %s", e.message);
                }
            });
        }

        private void update_spell_checking() {
            if (settings == null || web_view == null) {
                warning("Cannot update spell checking: settings or web_view is null");
                return;
            }
            
            var spell_enabled = settings.get_boolean("spell-checking-enabled");
            var web_context = web_view.get_context();
            
            web_context.set_spell_checking_enabled(spell_enabled);
            
            if (spell_enabled) {
                string[] spell_languages = get_spell_checking_languages();
                web_context.set_spell_checking_languages(spell_languages);
                info("Spell checking updated - languages: %s", string.joinv(", ", spell_languages));
            } else {
                web_context.set_spell_checking_languages({});
                info("Spell checking disabled");
            }
        }


        private void update_developer_tools() {
            if (settings == null || web_view == null) {
                warning("Cannot update developer tools: settings or web_view is null");
                return;
            }
            
            var dev_tools_enabled = settings.get_boolean("developer-tools-enabled");
            var webkit_settings = web_view.get_settings();
            webkit_settings.enable_developer_extras = dev_tools_enabled;
            
            // Close developer tools if they're being disabled
            if (!dev_tools_enabled) {
                close_developer_tools();
            }
            
            debug("Developer tools %s", dev_tools_enabled ? "enabled" : "disabled");
        }

        /**
         * Update WebView zoom level
         *
         * @param zoom_level The new zoom level to apply
         */
        public void update_webkit_zoom(double zoom_level) {
            if (web_view == null) {
                warning("Cannot update WebView zoom: web_view is null");
                return;
            }
            
            web_view.zoom_level = zoom_level;
            info("WebView zoom level updated to: %f", zoom_level);
        }

        /**
         * Update all WebKit settings through WebKitManager
         */
        public void update_webkit_settings() {
            if (webkit_manager != null && web_view != null) {
                webkit_manager.update_settings(web_view);
                debug("WebKit settings updated through WebKitManager");
            } else {
                warning("Cannot update WebKit settings: webkit_manager or web_view is null");
            }
        }

        /**
         * Increase WebView zoom level
         */
        public void webkit_zoom_in() {
            if (settings == null || web_view == null) return;
            if (!settings.get_boolean("webview-zoom-enabled")) return;
            
            var current_zoom = settings.get_double("webkit-zoom-level");
            var zoom_step = settings.get_double("webkit-zoom-step");
            var new_zoom = Math.fmin(current_zoom + zoom_step, 3.0);
            
            settings.set_double("webkit-zoom-level", new_zoom);
            web_view.zoom_level = new_zoom;
            debug("WebView zoomed in to %f", new_zoom);
        }

        /**
         * Decrease WebView zoom level
         */
        public void webkit_zoom_out() {
            if (settings == null || web_view == null) return;
            if (!settings.get_boolean("webview-zoom-enabled")) return;
            
            var current_zoom = settings.get_double("webkit-zoom-level");
            var zoom_step = settings.get_double("webkit-zoom-step");
            var new_zoom = Math.fmax(current_zoom - zoom_step, 0.5);
            
            settings.set_double("webkit-zoom-level", new_zoom);
            web_view.zoom_level = new_zoom;
            debug("WebView zoomed out to %f", new_zoom);
        }

        /**
         * Reset WebView zoom level to default
         */
        public void webkit_zoom_reset() {
            if (settings == null || web_view == null) return;
            if (!settings.get_boolean("webview-zoom-enabled")) return;
            
            settings.set_double("webkit-zoom-level", 1.0);
            web_view.zoom_level = 1.0;
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

        private void restore_window_state() {
            if (settings == null) {
                warning("Cannot restore window state: settings is null, using defaults");
                set_default_size(1200, 800);
                return;
            }
            
            // Restore window size
            var width = settings.get_int("window-width");
            var height = settings.get_int("window-height");
            set_default_size(width, height);
            
            // Restore maximized state
            if (settings.get_boolean("window-maximized")) {
                maximize();
            }
            
            debug("Window state restored: %dx%d, maximized: %s", 
                        width, height, settings.get_boolean("window-maximized").to_string());
        }

        private void save_window_state() {
            if (settings == null) return;
            
            // Save window size
            int width, height;
            get_default_size(out width, out height);
            settings.set_int("window-width", width);
            settings.set_int("window-height", height);
            
            // Save maximized state
            settings.set_boolean("window-maximized", maximized);
            
            debug("Window state saved: %dx%d, maximized: %s", 
                        width, height, maximized.to_string());
        }




        private void on_load_changed(WebKit.LoadEvent load_event) {
            switch (load_event) {
                case WebKit.LoadEvent.STARTED:
                    debug("Load started");
                    break;

                case WebKit.LoadEvent.REDIRECTED:
                    debug("Load redirected");
                    break;

                case WebKit.LoadEvent.COMMITTED:
                    debug("Load committed");
                    break;

                case WebKit.LoadEvent.FINISHED:
                    debug("Load finished");
                    setup_webkit_notifications();
                    webkit_manager.inject_user_agent_override(web_view);
                    break;
            }
        }

        private bool on_load_failed(WebKit.LoadEvent load_event, string failing_uri, Error error) {
            critical("Load failed for %s: %s", failing_uri, error.message);

            // TRANSLATORS: Error message when WhatsApp Web fails to load
            show_error_toast(_("Failed to load WhatsApp Web. Please check your internet connection."));
            return false;
        }

        private bool on_navigation_policy_decision(WebKit.PolicyDecision decision, WebKit.PolicyDecisionType decision_type) {
            // Handle both navigation actions and new window requests
            if (decision_type == WebKit.PolicyDecisionType.NAVIGATION_ACTION ||
                decision_type == WebKit.PolicyDecisionType.NEW_WINDOW_ACTION) {

                var navigation_decision = decision as WebKit.NavigationPolicyDecision;
                if (navigation_decision != null) {
                    var navigation_action = navigation_decision.get_navigation_action();
                    var request = navigation_action.get_request();
                    var uri = request.get_uri();

                    debug("Navigation policy decision (type: %s) for URI: %s",
                               decision_type.to_string(), uri);

                    // Allow internal WhatsApp Web navigation
                    if (is_whatsapp_internal_uri(uri)) {
                        debug("Allowing internal WhatsApp navigation: %s", uri);
                        decision.use();
                        return true;
                    }

                    // For external links, open in system browser
                    if (is_external_link(uri)) {
                        info("Opening external link in system browser: %s", uri);

                        try {
                            // Try to open with portal first (for Flatpak compatibility)
                            open_uri_external(uri);
                        } catch (Error e) {
                            critical("Failed to open external link: %s", e.message);
                            // TRANSLATORS: Error message when external link fails to open
                            show_error_toast(_("Failed to open link in external browser"));
                        }

                        // Ignore the navigation in the WebView
                        decision.ignore();
                        return true;
                    }

                    // Default: allow navigation
                    decision.use();
                    return true;
                }
            }

            // For other decision types, use default behavior
            decision.use();
            return false;
        }

        /**
         * Handle requests to create new web views (new windows/tabs)
         */
        private Gtk.Widget on_create_new_web_view(WebKit.NavigationAction navigation_action) {
            var request = navigation_action.get_request();
            var uri = request.get_uri();

            info("New web view creation requested for URI: %s", uri);

            // For external links, open in system browser instead of creating new web view
            if (is_external_link(uri)) {
                info("Intercepting new window for external link: %s", uri);

                try {
                    open_uri_external(uri);
                } catch (Error e) {
                    critical("Failed to open external link in new window: %s", e.message);
                    show_error_toast(_("Failed to open link in external browser"));
                }

                // Return null to prevent new window creation
                return null;
            }

            // For internal links, allow default behavior (shouldn't normally happen)
            debug("Allowing new window creation for internal URI: %s", uri);
            return null;
        }

        /**
         * Check if URI is internal to WhatsApp Web
         */
        private bool is_whatsapp_internal_uri(string uri) {
            // Allow WhatsApp Web domains and related services
            return uri.has_prefix("https://web.whatsapp.com/") ||
                   uri.has_prefix("https://whatsapp.com/") ||
                   uri.has_prefix("https://www.whatsapp.com/") ||
                   uri.has_prefix("https://static.whatsapp.net/") ||
                   uri.has_prefix("https://mmg.whatsapp.net/") ||
                   uri.has_prefix("wss://web.whatsapp.com/") ||
                   uri.has_prefix("blob:https://web.whatsapp.com/") ||
                   // Allow data URIs for inline content
                   uri.has_prefix("data:") ||
                   // Allow about: URIs for internal pages
                   uri.has_prefix("about:");
        }

        /**
         * Check if URI is an external link that should open in browser
         */
        private bool is_external_link(string uri) {
            // Consider HTTP/HTTPS links that are not WhatsApp internal as external
            return (uri.has_prefix("http://") || uri.has_prefix("https://")) &&
                   !is_whatsapp_internal_uri(uri);
        }

        /**
         * Open URI externally using Flatpak portal system
         */
        private void open_uri_external(string uri) throws Error {
            // Always use the portal system since this is a Flatpak-only application
            debug("Opening URI via Flatpak portal: %s", uri);
            open_uri_with_portal(uri);
        }

        /**
         * Open URI using the Flatpak portal system
         */
        private void open_uri_with_portal(string uri) throws Error {
            // Use the proper portal API through GLib's AppInfo.launch_default_for_uri_async
            // This automatically uses the portal when running in Flatpak
            AppInfo.launch_default_for_uri_async.begin(uri, null, null, (obj, res) => {
                try {
                    AppInfo.launch_default_for_uri_async.end(res);
                    debug("URI opened successfully via portal: %s", uri);
                } catch (Error e) {
                    critical("Portal URI opening failed: %s", e.message);
                }
            });
        }



        private void setup_webkit_notifications() {
            // Set up notification permission handling 
            debug("WebKit notifications setup complete");
        }


        private bool on_permission_request(WebKit.PermissionRequest request) {
            if (request is WebKit.NotificationPermissionRequest) {
                info("WhatsApp requesting notification permission");
                
                // Check if we have a saved permission decision
                if (settings != null) {
                    bool permission_asked = settings.get_boolean("web-notification-permission-asked");
                    bool permission_granted = settings.get_boolean("web-notification-permission-granted");
                    
                    if (permission_asked) {
                        // We have a previous decision, use it
                        if (permission_granted) {
                            info("Using saved permission: granted");
                            request.allow();
                            setup_notification_handler();
                        } else {
                            info("Using saved permission: denied");
                            request.deny();
                        }
                        return true;
                    }
                }
                
                // No previous decision, show native dialog
                show_notification_permission_dialog(request);
                return true;
            }
            
            return false;
        }
        
        private void show_notification_permission_dialog(WebKit.PermissionRequest request) {
            var dialog = new Adw.AlertDialog(
                // TRANSLATORS: Title for notification permission dialog
                _("WhatsApp Web Notification Permission"),
                // TRANSLATORS: Body text for notification permission dialog
                _("WhatsApp Web wants to show desktop notifications for new messages. Would you like to allow notifications?")
            );
            
            // TRANSLATORS: Button text to deny notifications
            dialog.add_response("deny", _("Deny"));
            // TRANSLATORS: Button text to allow notifications
            dialog.add_response("allow", _("Allow"));
            
            dialog.set_response_appearance("allow", Adw.ResponseAppearance.SUGGESTED);
            dialog.set_default_response("allow");
            dialog.set_close_response("deny");
            
            dialog.response.connect((response) => {
                bool granted = (response == "allow");
                
                // Save the user's decision
                if (settings != null) {
                    settings.set_boolean("web-notification-permission-asked", true);
                    settings.set_boolean("web-notification-permission-granted", granted);
                }
                
                // Handle the WebKit permission request
                if (granted) {
                    info("User granted notification permission");
                    request.allow();
                    setup_notification_handler();
                } else {
                    info("User denied notification permission");
                    request.deny();
                }
            });
            
            dialog.present(this);
        }
        
        private void setup_notification_handler() {
            // Set up notification handler on the WebView
            web_view.show_notification.connect(on_webkit_notification);
            info("Notification handler connected");
        }

        private bool on_webkit_notification(WebKit.WebView webview, WebKit.Notification webkit_notification) {
            info("WebKit notification received: %s", webkit_notification.get_title());
            
            if (notification_manager != null) {
                var title = webkit_notification.get_title();
                var body = webkit_notification.get_body() ?? "";
                
                // Send native notification
                notification_manager.send_notification(title, body, new ThemedIcon("dialog-information-symbolic"));
                
                // Handle notification click
                webkit_notification.clicked.connect(() => {
                    debug("Notification clicked, focusing window");
                    present();
                    webkit_notification.clicked();
                });
                
                webkit_notification.closed.connect(() => {
                    debug("Notification closed");
                });
                
                // Close the WebKit notification since we're showing our own
                webkit_notification.close();
            }
            
            return true; // We handled it
        }


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
            if (settings == null || !settings.get_boolean("developer-tools-enabled")) {
                // TRANSLATORS: Error message when developer tools are disabled
                show_error_toast(_("Developer tools are disabled in preferences"));
                return;
            }
            
            var inspector = web_view.get_inspector();
            inspector.show();
            debug("Developer tools opened");
        }

        public bool is_developer_tools_open() {
            if (settings == null || !settings.get_boolean("developer-tools-enabled")) {
                return false;
            }
            
            var inspector = web_view.get_inspector();
            return inspector.is_attached();
        }

        public void close_developer_tools() {
            if (web_view == null) {
                return;
            }
            
            var inspector = web_view.get_inspector();
            if (inspector.is_attached()) {
                inspector.close();
                debug("Developer tools closed");
            }
        }

        /**
         * Reload the WebView content
         */
        public void reload_webview(bool force_reload = false) {
            if (web_view != null) {
                if (force_reload) {
                    web_view.reload_bypass_cache();
                    debug("WebView force reloaded (bypassing cache)");
                } else {
                    web_view.reload();
                    debug("WebView reloaded");
                }
                show_info_toast(force_reload ? _("Force reloading...") : _("Reloading..."));
            }
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
            save_window_state();
            
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