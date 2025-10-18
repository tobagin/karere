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
     * WebViewManager handles WebView lifecycle, navigation, and policy decisions.
     *
     * This manager creates and configures the WebView, manages navigation policies,
     * handles external links, and provides developer tools access.
     */
    public class WebViewManager : Object {

        // Signals
        public signal void load_started();
        public signal void load_finished();
        public signal void load_failed(string uri, string error_message);
        public signal void external_link_clicked(string uri);

        // Properties
        public WebKit.WebView web_view { get; private set; }

        private Settings? settings;
        private WebKitManager webkit_manager;

        /**
         * Create a new WebViewManager
         *
         * @param settings GSettings instance (can be null)
         * @param webkit_manager WebKitManager for configuration
         */
        public WebViewManager(Settings? settings, WebKitManager webkit_manager) {
            this.settings = settings;
            this.webkit_manager = webkit_manager;

            // Create WebView programmatically
            web_view = new WebKit.WebView();
            web_view.vexpand = true;
            web_view.hexpand = true;
        }

        /**
         * Setup and configure the WebView
         *
         * @param container The container to add the WebView to
         */
        public void setup(Gtk.Box container) {
            // Configure cookie storage
            configure_cookie_storage();

            // Use WebKitManager to configure WebView settings
            webkit_manager.configure_web_view(web_view);

            // Add WebView to container
            container.append(web_view);

            // Connect WebView signals
            web_view.load_changed.connect(on_load_changed);
            web_view.load_failed.connect(on_load_failed);
            web_view.decide_policy.connect(on_navigation_policy_decision);
            web_view.create.connect(on_create_new_web_view);

            // Load WhatsApp Web
            web_view.load_uri("https://web.whatsapp.com");

            debug("WebView configured and loaded");
        }

        /**
         * Configure cookie storage
         */
        private void configure_cookie_storage() {
            // Note: WebKit 6.0 API change - cookie manager access has changed
            // Cookie storage will be handled automatically by WebKit
            info("Cookie storage will be handled by WebKit defaults");
        }

        /**
         * Reload the WebView
         *
         * @param force If true, bypass cache
         */
        public void reload(bool force = false) {
            if (force) {
                web_view.reload_bypass_cache();
                debug("WebView force reloaded (bypassing cache)");
            } else {
                web_view.reload();
                debug("WebView reloaded");
            }
        }

        /**
         * Open developer tools
         */
        public void open_developer_tools() {
            if (settings == null || !settings.get_boolean("developer-tools-enabled")) {
                warning("Developer tools are disabled in preferences");
                return;
            }

            var inspector = web_view.get_inspector();
            inspector.show();
            debug("Developer tools opened");
        }

        /**
         * Check if developer tools are open
         */
        public bool is_developer_tools_open() {
            if (settings == null || !settings.get_boolean("developer-tools-enabled")) {
                return false;
            }

            var inspector = web_view.get_inspector();
            return inspector.is_attached();
        }

        /**
         * Close developer tools
         */
        public void close_developer_tools() {
            var inspector = web_view.get_inspector();
            if (inspector.is_attached()) {
                inspector.close();
                debug("Developer tools closed");
            }
        }

        /**
         * Update developer tools setting
         */
        public void update_developer_tools_setting(bool enabled) {
            var webkit_settings = web_view.get_settings();
            webkit_settings.enable_developer_extras = enabled;

            // Close developer tools if they're being disabled
            if (!enabled) {
                close_developer_tools();
            }

            debug("Developer tools %s", enabled ? "enabled" : "disabled");
        }

        /**
         * Inject JavaScript into the WebView
         */
        public void inject_javascript(string script) {
            web_view.evaluate_javascript.begin(script, -1, null, null, null, (obj, res) => {
                try {
                    web_view.evaluate_javascript.end(res);
                } catch (Error e) {
                    critical("Failed to execute JavaScript: %s", e.message);
                }
            });
        }

        /**
         * Handle load changed events
         */
        private void on_load_changed(WebKit.LoadEvent load_event) {
            switch (load_event) {
                case WebKit.LoadEvent.STARTED:
                    debug("Load started");
                    load_started();
                    break;

                case WebKit.LoadEvent.REDIRECTED:
                    debug("Load redirected");
                    break;

                case WebKit.LoadEvent.COMMITTED:
                    debug("Load committed");
                    break;

                case WebKit.LoadEvent.FINISHED:
                    debug("Load finished");
                    webkit_manager.inject_user_agent_override(web_view);
                    load_finished();
                    break;
            }
        }

        /**
         * Handle load failed events
         */
        private bool on_load_failed(WebKit.LoadEvent load_event, string failing_uri, Error error) {
            critical("Load failed for %s: %s", failing_uri, error.message);
            load_failed(failing_uri, error.message);
            return false;
        }

        /**
         * Handle navigation policy decisions
         */
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
                            external_link_clicked(uri);
                        } catch (Error e) {
                            critical("Failed to open external link: %s", e.message);
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
                    external_link_clicked(uri);
                } catch (Error e) {
                    critical("Failed to open external link in new window: %s", e.message);
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
    }
}
