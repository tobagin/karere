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
        private Settings settings;
        private Logger logger;
        
        public WebKitManager() {
            settings = new Settings(Config.APP_ID);
            logger = new Logger();
            
            logger.debug("WebKitManager initialized");
        }
        
        /**
         * Configure WebKit settings for optimal WhatsApp Web experience
         *
         * @param web_view The WebKit.WebView to configure
         */
        public void configure_web_view(WebKit.WebView web_view) {
            logger.debug("Configuring WebKit WebView for WhatsApp Web");
            
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
            web_settings.enable_developer_extras = settings.get_boolean("developer-tools-enabled");
            
            // Note: enable_plugins property is not available in this WebKitGTK version
            // Plugins are disabled by default in modern WebKitGTK
            
            // Enable hardware acceleration
            web_settings.hardware_acceleration_policy = WebKit.HardwareAccelerationPolicy.ALWAYS;
            
            // Set user agent - Use more explicit user agent setting
            var user_agent = get_user_agent();
            string final_user_agent;
            if (user_agent != null && user_agent.strip() != "") {
                final_user_agent = user_agent;
                logger.info("Using custom user agent: %s", user_agent);
            } else {
                // Use default user agent optimized for WhatsApp Web
                final_user_agent = get_default_user_agent();
                logger.info("Using default Linux user agent: %s", final_user_agent);
            }
            
            // Set user agent with explicit property setting
            web_settings.set_property("user-agent", final_user_agent);
            
            // Verify it was set correctly
            logger.info("Final user agent set on WebView: %s", web_settings.user_agent);
            
            // Also try setting it via JavaScript injection as a fallback
            logger.debug("Will inject user agent override after page load");
            
            // Configure zoom level
            var zoom_level = settings.get_double("webkit-zoom-level");
            web_view.zoom_level = zoom_level;
            logger.debug("Set zoom level to: %f", zoom_level);
            
            // Note: enable_spell_checking property is not available in this WebKitGTK version
            // Spell checking will use system defaults
            if (settings.get_boolean("spell-checking-enabled")) {
                
                if (settings.get_boolean("spell-checking-auto-detect")) {
                    // Note: spell_checking_languages property is not available in this WebKitGTK version
                    // Spell checking will use system defaults
                    logger.debug("Using system defaults for spell checking");
                } else {
                    // Note: spell_checking_languages property is not available in this WebKitGTK version
                    // Spell checking will use system defaults
                    logger.debug("Using system defaults for spell checking");
                }
            } else {
                // Spell checking disabled in settings
            }
            
            logger.info("WebKit WebView configured successfully");
        }
        
        /**
         * Get the configured user agent string
         *
         * @return The user agent string to use, or null to use default
         */
        public string? get_user_agent() {
            var custom_user_agent = settings.get_string("webkit-user-agent");
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
            logger.debug("Updating WebKit settings");
            
            var web_settings = web_view.get_settings();
            
            // Note: enable_notifications property is not available in this WebKitGTK version
            // Notifications handled through JavaScript injection
            
            // Update developer tools
            web_settings.enable_developer_extras = settings.get_boolean("developer-tools-enabled");
            
            // Update zoom level
            var zoom_level = settings.get_double("webkit-zoom-level");
            web_view.zoom_level = zoom_level;
            
            // Note: enable_spell_checking property is not available in this WebKitGTK version
            // Spell checking uses system defaults
            
            if (settings.get_boolean("spell-checking-enabled")) {
                // Note: spell_checking_languages property is not available in this WebKitGTK version
                // Spell checking will use system defaults
                logger.debug("Spell checking enabled with system defaults");
            }
            
            // Update user agent if changed
            var user_agent = get_user_agent();
            if (user_agent != null && user_agent.strip() != "") {
                web_settings.user_agent = user_agent;
            } else {
                web_settings.user_agent = get_default_user_agent();
            }
            
            logger.debug("WebKit settings updated");
        }
        
        /**
         * Configure WebKit context for security and performance
         *
         * @param context The WebKit.WebContext to configure
         */
        public void configure_web_context(WebKit.WebContext context) {
            logger.debug("Configuring WebKit context");
            
            // Set cache model for better performance
            context.set_cache_model(WebKit.CacheModel.WEB_BROWSER);
            
            // Configure security settings
            var security_manager = context.get_security_manager();
            
            // Allow HTTPS content only (WhatsApp Web uses HTTPS)
            security_manager.register_uri_scheme_as_secure("https");
            security_manager.register_uri_scheme_as_cors_enabled("https");
            
            // Note: Cookie storage configuration moved to per-WebView setup
            // The WebKit 6.0 API no longer exposes website data manager from context
            
            logger.debug("WebKit context configured");
        }
    }
}