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

namespace Karere.Constants {

    /**
     * Application-wide constants
     */
    public class App {
        public const int DEFAULT_WINDOW_WIDTH = 1200;
        public const int DEFAULT_WINDOW_HEIGHT = 800;
        public const int MIN_WINDOW_WIDTH = 800;
        public const int MIN_WINDOW_HEIGHT = 600;
    }

    /**
     * WebKit and WebView related constants
     */
    public class WebKit {
        public const string WHATSAPP_URL = "https://web.whatsapp.com";
        public const double DEFAULT_ZOOM_LEVEL = 1.0;
        public const double MIN_ZOOM_LEVEL = 0.5;
        public const double MAX_ZOOM_LEVEL = 3.0;
        public const double DEFAULT_ZOOM_STEP = 0.1;
        public const int DEFAULT_FONT_SIZE = 16;
        public const int DEFAULT_MONOSPACE_FONT_SIZE = 13;
        public const int MIN_FONT_SIZE = 9;
    }

    /**
     * Notification system constants
     */
    public class Notifications {
        public const int BACKGROUND_GRACE_PERIOD_SECONDS = 0;
        public const int BACKGROUND_COOLDOWN_SECONDS = 30;
        public const int DEFAULT_PREVIEW_LENGTH = 100;
        public const int DEFAULT_TIMEOUT_SECONDS = 5;
        public const int SUCCESS_TIMEOUT_SECONDS = 2;
        public const int INFO_TIMEOUT_SECONDS = 3;
        public const int ERROR_TIMEOUT_SECONDS = 5;

        // Background notification modes
        public const int MODE_ALWAYS = 0;
        public const int MODE_FIRST_TIME_ONLY = 1;
        public const int MODE_NEVER = 2;
    }

    /**
     * Logging system constants
     */
    public class Logging {
        public const int MAX_LOG_FILE_SIZE_MB = 10;
        public const int DEFAULT_LOG_RETENTION_DAYS = 7;
        public const int DEFAULT_RECENT_LOGS_LINES = 50;
        public const int CRASH_REPORT_TIMEOUT_SECONDS = 30;
        public const int CRASH_REPORT_MAX_CONTENT_LENGTH = 2000;
    }

    /**
     * Accessibility constants
     */
    public class Accessibility {
        public const int FOCUS_RING_TIMEOUT_MS = 200;
        public const string CSS_HIGH_CONTRAST = "karere-high-contrast";
        public const string CSS_REDUCED_MOTION = "karere-reduced-motion";
        public const string CSS_FOCUS_INDICATORS = "karere-focus-indicators";
        public const string CSS_SCREEN_READER = "karere-screen-reader";
        public const string CSS_FOCUS_VISIBLE = "karere-focus-visible";
        public const string CSS_FOCUS_RING = "karere-focus-ring";
    }

    /**
     * Theme constants
     */
    public class Theme {
        public const int SYSTEM_THEME = 0;
        public const int LIGHT_THEME = 1;
        public const int DARK_THEME = 2;
    }

    /**
     * Keyboard shortcuts constants
     */
    public class Shortcuts {
        // Standard shortcuts
        public const string PREFERENCES = "<primary>comma";
        public const string QUIT = "<primary>q";
        public const string ABOUT = "F1";
        public const string MINIMIZE = "<primary>m";
        public const string FULLSCREEN = "F11";
        public const string FULLSCREEN_ALT = "<alt>Return";
        public const string HELP = "<primary>question";
        public const string HELP_ALT = "<primary>slash";

        // Developer shortcuts
        public const string DEV_TOOLS = "<primary><shift>d";
        public const string DEV_TOOLS_ALT = "F12";
        public const string RELOAD = "<primary>r";
        public const string RELOAD_ALT = "F5";
        public const string FORCE_RELOAD = "<primary><shift>r";
        public const string FORCE_RELOAD_ALT = "<shift>F5";

        // Zoom shortcuts
        public const string ZOOM_IN = "<primary>plus";
        public const string ZOOM_IN_ALT = "<primary>equal";
        public const string ZOOM_IN_KEYPAD = "<primary>KP_Add";
        public const string ZOOM_OUT = "<primary>minus";
        public const string ZOOM_OUT_KEYPAD = "<primary>KP_Subtract";
        public const string ZOOM_RESET = "<primary>0";
        public const string ZOOM_RESET_KEYPAD = "<primary>KP_0";

        // Accessibility shortcuts
        public const string TOGGLE_HIGH_CONTRAST = "<primary><shift>h";
        public const string TOGGLE_FOCUS_INDICATORS = "<primary><shift>f";

        // Notification shortcuts
        public const string TOGGLE_NOTIFICATIONS = "<primary><shift>n";
        public const string TOGGLE_DND = "<primary><shift>d";

        // WhatsApp Web shortcuts
        public const string FIND = "<primary>f";
        public const string SEARCH_CHATS = "<primary><shift>f";
        public const string NEW_CHAT = "<primary>n";
        public const string ARCHIVE_CHAT = "<primary>e";
        public const string PROFILE = "<primary>p";
    }

    /**
     * File and directory constants
     */
    public class Paths {
        public const string LOGS_SUBDIR = "logs";
        public const string CRASH_REPORTS_SUBDIR = "crash-reports";
        public const string LOG_FILE_PATTERN = "karere-%s.log";
        public const string CRASH_FILE_PATTERN = "crash_%s.txt";
    }

    /**
     * Network and HTTP constants
     */
    public class Network {
        public const int DEFAULT_HTTP_TIMEOUT = 30;
        public const string USER_AGENT_FORMAT = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15 Karere/%s";
        public const string[] WHATSAPP_DOMAINS = {
            "https://web.whatsapp.com/",
            "https://whatsapp.com/",
            "https://www.whatsapp.com/",
            "https://static.whatsapp.net/",
            "https://mmg.whatsapp.net/",
            "wss://web.whatsapp.com/",
            "blob:https://web.whatsapp.com/"
        };
        public const string[] ALLOWED_URI_SCHEMES = {
            "data:",
            "about:"
        };
    }

    /**
     * Signal constants
     */
    public class Signals {
        public const int SIGINT = 2;
        public const int SIGTERM = 15;
    }

    /**
     * MIME types for clipboard handling
     */
    public class MimeTypes {
        public const string[] IMAGE_TYPES = {
            "image/png",
            "image/jpeg",
            "image/gif"
        };
    }
}