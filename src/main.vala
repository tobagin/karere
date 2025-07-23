/*
 * Copyright (C) 2025 Karere Contributors
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