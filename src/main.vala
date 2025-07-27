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

int main(string[] args) {
    // Initialize internationalization
    Intl.setlocale(LocaleCategory.ALL, "");
    Intl.bindtextdomain(Config.GETTEXT_PACKAGE, Config.LOCALEDIR);
    Intl.bind_textdomain_codeset(Config.GETTEXT_PACKAGE, "UTF-8");
    Intl.textdomain(Config.GETTEXT_PACKAGE);
    
    // Set up early environment variable suppression before any libraries initialize
    Environment.set_variable("G_MESSAGES_DEBUG", "", true);
    Environment.set_variable("SOUP_DEBUG", "0", true);
    
    // Set up default system debug message filtering before GTK initialization
    // Note: We can't check user preferences yet since that requires GTK/GSettings initialization
    // The application will apply user preferences after GTK initialization in startup()
    
    // Custom log handler that initially suppresses debug/info messages
    // This will be updated by the application after GTK initialization
    LogFunc initial_log_handler = (log_domain, log_level, message) => {
        // Always allow our application logs through
        if (log_domain != null && log_domain == "Karere") {
            Log.default_handler(log_domain, log_level, message);
            return;
        }
        
        // Initially suppress debug and info messages (default behavior)
        // The application will update this after GTK initialization
        if ((log_level & LogLevelFlags.LEVEL_DEBUG) != 0 || 
            (log_level & LogLevelFlags.LEVEL_INFO) != 0) {
            return; // Suppress debug and info messages
        }
        
        // For all other cases, use default handler
        Log.default_handler(log_domain, log_level, message);
    };
    
    // Install our initial log handler for all domains
    Log.set_handler(null, LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, initial_log_handler);
    
    // Also install handlers for specific domains that use their own logging
    Log.set_handler("GLib", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, initial_log_handler);
    Log.set_handler("GLib-GIO", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, initial_log_handler);
    Log.set_handler("Gdk", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, initial_log_handler);
    Log.set_handler("Gtk", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, initial_log_handler);
    Log.set_handler("GdkPixbuf", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, initial_log_handler);
    Log.set_handler("Pango", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, initial_log_handler);
    Log.set_handler("Cairo", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, initial_log_handler);
    Log.set_handler("libsoup", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, initial_log_handler);
    Log.set_handler("GVFS", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, initial_log_handler);
    Log.set_handler("WebKit", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, initial_log_handler);
    Log.set_handler("Adwaita", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, initial_log_handler);
    Log.set_handler("MESA-INTEL", LogLevelFlags.LEVEL_MASK | LogLevelFlags.FLAG_FATAL | LogLevelFlags.FLAG_RECURSION, initial_log_handler);
    
    // Initialize internationalization
    Intl.bindtextdomain(Config.GETTEXT_PACKAGE, Config.LOCALEDIR);
    Intl.bind_textdomain_codeset(Config.GETTEXT_PACKAGE, "UTF-8");
    Intl.textdomain(Config.GETTEXT_PACKAGE);

    // Set application name for window manager
    Environment.set_application_name(Config.APP_NAME);
    Environment.set_prgname(Config.APP_NAME);

    var logger = new Karere.Logger();
    logger.info("Starting %s version %s", Config.APP_NAME, Config.VERSION);

    try {
        // Set up signal handlers for graceful shutdown
        Karere.Utils.setup_signal_handlers(logger);

        // Ensure user directories exist
        var resource_manager = new Karere.Utils.ResourceManager(logger);
        if (!resource_manager.ensure_user_directories()) {
            logger.error("Failed to create user directories, continuing anyway");
        }

        // Create and run application
        var app = new Karere.Application();
        
        // Add command line options
        app.add_main_option(
            "version",
            'v',
            OptionFlags.NONE,
            OptionArg.NONE,
            _("Show version information"),
            null
        );
        
        app.add_main_option(
            "help",
            'h',
            OptionFlags.NONE,
            OptionArg.NONE,
            _("Show help information"),
            null
        );

        // Run the application
        var exit_code = app.run(args);
        
        logger.info("Application exiting with code %d", exit_code);
        return exit_code;
        
    } catch (Error e) {
        logger.critical("Fatal error during application startup: %s", e.message);
        
        // Show critical error to user if possible
        var error_handler = new Karere.Utils.ErrorHandler(logger);
        error_handler.handle_critical_error(e, "application startup");
        
        return 1;
    }
}