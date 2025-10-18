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
     * Manages WebKit configuration and behavior for WhatsApp Web integration
     *
     * This class handles WebKit settings, user agent configuration, and other
     * WebKit-specific functionality needed for optimal WhatsApp Web experience.
     */
    public class WebKitManager : GLib.Object {
        private SettingsManager settings_manager;

        public WebKitManager() {
            settings_manager = SettingsManager.get_instance();

            debug("WebKitManager initialized");
        }
        
        /**
         * Configure WebKit settings for optimal WhatsApp Web experience
         *
         * @param web_view The WebKit.WebView to configure
         */
        public void configure_web_view(WebKit.WebView web_view) {
            debug("Configuring WebKit WebView for WhatsApp Web");
            
            var web_settings = web_view.get_settings();
            
            // Enable JavaScript (required for WhatsApp Web)
            web_settings.enable_javascript = true;
            
            // Enable WebGL for better performance
            web_settings.enable_webgl = true;
            
            // Enable media stream for voice messages and calls
            web_settings.enable_media_stream = true;
            
            // Note: enable_notifications property is not available in this WebKitGTK version
            // Notifications will be handled through JavaScript injection
            
            // Enable developer tools if enabled in settings
            web_settings.enable_developer_extras = settings_manager.get_boolean_with_fallback("developer-tools-enabled", false);
            
            // Note: enable_plugins property is not available in this WebKitGTK version
            // Plugins are disabled by default in modern WebKitGTK
            
            // Enable hardware acceleration
            web_settings.hardware_acceleration_policy = WebKit.HardwareAccelerationPolicy.ALWAYS;
            
            // Enable media with auto-loading
            web_settings.enable_media = true;
            web_settings.media_playback_requires_user_gesture = false;
            web_settings.media_playback_allows_inline = true;

            // Improve font and emoji rendering quality
            web_settings.enable_smooth_scrolling = true;
            web_settings.allow_file_access_from_file_urls = false;
            web_settings.allow_universal_access_from_file_urls = false;

            // Note: Canvas acceleration properties are deprecated in this WebKit version
            // Modern WebKitGTK handles hardware acceleration automatically

            // Additional rendering improvements
            web_settings.set_property("enable-site-specific-quirks", true);
            web_settings.set_property("enable-page-cache", false);
            web_settings.set_property("enable-offline-web-application-cache", false);
            web_settings.set_property("enable-html5-database", true);
            web_settings.set_property("enable-html5-local-storage", true);
            // Note: XSS auditor and hyperlink auditing are deprecated in modern WebKit

            // Advanced font and rendering settings
            web_settings.set_property("enable-back-forward-navigation-gestures", true);
            web_settings.set_property("enable-mock-capture-devices", false);
            web_settings.set_property("enable-spatial-navigation", false);
            web_settings.set_property("enable-tabs-to-links", true);
            web_settings.set_property("enable-caret-browsing", false);

            // Note: Font family properties are deprecated in this WebKit version
            // Font settings are handled by the system and CSS

            // Set minimum font size for better readability
            web_settings.minimum_font_size = 9;
            web_settings.default_font_size = 16;
            web_settings.default_monospace_font_size = 13;
            
            // Set user agent - Use more explicit user agent setting
            var user_agent = get_user_agent();
            string final_user_agent;
            if (user_agent != null && user_agent.strip() != "") {
                final_user_agent = user_agent;
                info("Using custom user agent: %s", user_agent);
            } else {
                // Use default user agent optimized for WhatsApp Web
                final_user_agent = get_default_user_agent();
                info("Using default Linux user agent: %s", final_user_agent);
            }

            // Set user agent with explicit property setting
            web_settings.set_property("user-agent", final_user_agent);

            // Verify it was set correctly
            info("Final user agent set on WebView: %s", web_settings.user_agent);

            // Also try setting it via JavaScript injection as a fallback
            debug("Will inject user agent override after page load");
            
            // Configure zoom level
            var zoom_level = settings_manager.get_double_with_fallback("webkit-zoom-level", 1.0);
            web_view.zoom_level = zoom_level;
            debug("Set zoom level to: %f", zoom_level);

            // Note: enable_spell_checking property is not available in this WebKitGTK version
            // Spell checking will use system defaults
            if (settings_manager.get_boolean_with_fallback("spell-checking-enabled", false)) {

                if (settings_manager.get_boolean_with_fallback("spell-checking-auto-detect", true)) {
                    // Note: spell_checking_languages property is not available in this WebKitGTK version
                    // Spell checking will use system defaults
                    debug("Using system defaults for spell checking");
                } else {
                    // Note: spell_checking_languages property is not available in this WebKitGTK version
                    // Spell checking will use system defaults
                    debug("Using system defaults for spell checking");
                }
            } else {
                // Spell checking disabled in settings
            }

            info("WebKit WebView configured successfully");
        }
        
        /**
         * Get the configured user agent string
         *
         * @return The user agent string to use, or null to use default
         */
        public string? get_user_agent() {
            var custom_user_agent = settings_manager.get_string_with_fallback("webkit-user-agent", "");
            if (custom_user_agent != null && custom_user_agent.strip() != "") {
                return custom_user_agent;
            }
            return null;
        }
        
        /**
         * Get the default user agent optimized for WhatsApp Web
         *
         * @return Default user agent string
         */
        public string get_default_user_agent() {
            // Use a user agent that ensures WhatsApp Web works properly
            // Based on Safari on Linux for a more native feel
            return "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15 Karere/%s".printf(Config.VERSION);
        }

        
        /**
         * Update WebView settings when preferences change
         *
         * @param web_view The WebKit.WebView to update
         */
        public void update_settings(WebKit.WebView web_view) {
            debug("Updating WebKit settings");

            var web_settings = web_view.get_settings();

            // Note: enable_notifications property is not available in this WebKitGTK version
            // Notifications handled through JavaScript injection

            // Update developer tools
            web_settings.enable_developer_extras = settings_manager.get_boolean_with_fallback("developer-tools-enabled", false);

            // Update zoom level
            var zoom_level = settings_manager.get_double_with_fallback("webkit-zoom-level", 1.0);
            web_view.zoom_level = zoom_level;

            // Note: enable_spell_checking property is not available in this WebKitGTK version
            // Spell checking uses system defaults

            if (settings_manager.get_boolean_with_fallback("spell-checking-enabled", false)) {
                // Note: spell_checking_languages property is not available in this WebKitGTK version
                // Spell checking will use system defaults
                debug("Spell checking enabled with system defaults");
            }

            // Update user agent if changed
            var user_agent = get_user_agent();
            if (user_agent != null && user_agent.strip() != "") {
                web_settings.user_agent = user_agent;
            } else {
                web_settings.user_agent = get_default_user_agent();
            }

            debug("WebKit settings updated");
        }
        
        /**
         * Configure WebKit context for security and performance
         *
         * @param context The WebKit.WebContext to configure
         */
        public void configure_web_context(WebKit.WebContext context) {
            debug("Configuring WebKit context");

            // Set cache model based on user preference
            var cache_mode = settings_manager.get_string_with_fallback("webkit-cache-mode", "enabled");
            switch (cache_mode) {
                case "disabled":
                    context.set_cache_model(WebKit.CacheModel.DOCUMENT_VIEWER);
                    debug("WebKit cache disabled");
                    break;
                case "web-browser":
                default:
                    context.set_cache_model(WebKit.CacheModel.WEB_BROWSER);
                    debug("WebKit cache enabled (web browser mode)");
                    break;
            }

            // Configure security settings
            var security_manager = context.get_security_manager();

            // Allow HTTPS content only (WhatsApp Web uses HTTPS)
            security_manager.register_uri_scheme_as_secure("https");
            security_manager.register_uri_scheme_as_cors_enabled("https");

            // Note: Cookie storage configuration moved to per-WebView setup
            // The WebKit 6.0 API no longer exposes website data manager from context

            debug("WebKit context configured");
        }


        /**
         * Inject user agent override into the page with current user agent
         */
        public void inject_user_agent_override(WebKit.WebView web_view) {
            // Check if user has custom desktop user agent
            var custom_user_agent = get_user_agent();
            var current_user_agent = custom_user_agent ?? get_default_user_agent();
            inject_user_agent_override_with_agent(web_view, current_user_agent);
        }

        /**
         * Inject user agent override into the page
         */
        private void inject_user_agent_override_with_agent(WebKit.WebView web_view, string user_agent) {
            var javascript_code = """
                // Override navigator.userAgent
                Object.defineProperty(navigator, 'userAgent', {
                    get: function() {
                        return '%s';
                    },
                    configurable: false,
                    enumerable: true
                });

                // Update platform to match user agent
                var platform = '%s';
                Object.defineProperty(navigator, 'platform', {
                    get: function() {
                        return platform;
                    },
                    configurable: false,
                    enumerable: true
                });

                console.log('User agent overridden to:', navigator.userAgent);
            """.printf(user_agent, "Linux x86_64");

            web_view.evaluate_javascript.begin(javascript_code, -1, null, null, null, (obj, res) => {
                try {
                    web_view.evaluate_javascript.end(res);
                    debug("User agent JavaScript override injected successfully");
                } catch (Error e) {
                    warning("Failed to inject user agent override: %s", e.message);
                }
            });
        }

    }
}