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

    public class NotificationManager : GLib.Object {
        private SettingsManager settings_manager;
        private Application app;
        private bool initialization_deferred = true;

        // Background notification tracking
        private DateTime? window_background_start_time = null;
        private DateTime? last_background_notification_time = null;
        private bool session_background_notification_shown = false;
        private uint background_notification_timer_id = 0;

        // Constants from Python version
        private const int BACKGROUND_NOTIFICATION_GRACE_PERIOD_SECONDS = 1;
        private const int BACKGROUND_NOTIFICATION_COOLDOWN_SECONDS = 30;

        public NotificationManager(Application app) {
            this.app = app;
            settings_manager = SettingsManager.get_instance();
            // Note: Settings initialization is deferred until GTK is initialized

            debug("NotificationManager initialized");
        }

        public void initialize_settings() {
            // SettingsManager is initialized by Application, just mark as ready
            initialization_deferred = false;
            debug("NotificationManager settings initialized");
        }
        
        public void send_notification(string title, string body, Icon icon) {
            // Check if notifications are enabled
            if (!should_show_notification()) {
                debug("Notification blocked by settings: %s", title);
                return;
            }

            try {
                // Create GNotification
                var notification = new Notification(title);

                // Set body with preview handling
                string notification_body = body;
                var preview_enabled = settings_manager.get_boolean_with_fallback("notification-preview-enabled", true);
                var max_length = settings_manager.get_int_with_fallback("notification-preview-length", 100);

                if (preview_enabled) {
                    if (body.length > max_length) {
                        notification_body = body.substring(0, max_length) + "…";
                    }
                } else {
                    // TRANSLATORS: Default notification text when preview is disabled
                    notification_body = _("New message");
                }

                notification.set_body(notification_body);

                // Set icon
                if (icon != null) {
                    notification.set_icon(icon);
                }

                // Add default action to focus window
                notification.set_default_action("app.notification-clicked");

                // Send notification
                app.send_notification(null, notification);

                info("Notification sent: %s", title);

            } catch (Error e) {
                critical("Failed to send notification: %s", e.message);
            }
        }
        
        private bool should_show_notification() {
            if (!settings_manager.is_initialized()) {
                // Default to showing notifications if settings not available
                return true;
            }

            // Check if notifications are globally enabled
            if (!settings_manager.get_boolean_with_fallback("notifications-enabled", true)) {
                return false;
            }

            // Check if system notifications are enabled
            if (!settings_manager.get_boolean_with_fallback("system-notifications-enabled", true)) {
                return false;
            }

            return true;
        }
        
        public void on_window_focus_changed(bool is_active) {
            if (is_active) {
                // Window gained focus (came to foreground)
                if (window_background_start_time != null) {
                    var background_duration = get_window_background_duration();
                    debug("Window returned to foreground after %d seconds", (int)background_duration);
                    window_background_start_time = null;
                }

                // Cancel any pending background notification
                if (background_notification_timer_id != 0) {
                    Source.remove(background_notification_timer_id);
                    background_notification_timer_id = 0;
                    debug("Cancelled pending background notification");
                }
            } else {
                // Window lost focus (went to background)
                window_background_start_time = new DateTime.now_local();
                debug("Window went to background at %s", window_background_start_time.to_string());

                // Schedule background notification after grace period
                schedule_background_notification();
            }
        }
        
        private void schedule_background_notification() {
            // Cancel any existing timer
            if (background_notification_timer_id != 0) {
                Source.remove(background_notification_timer_id);
            }
            
            // Check if we should schedule a notification
            if (!should_show_background_notification_basic_check()) {
                debug("Background notification not scheduled due to basic check failure");
                return;
            }

            // Schedule the notification after grace period
            background_notification_timer_id = Timeout.add_seconds(BACKGROUND_NOTIFICATION_GRACE_PERIOD_SECONDS, () => {
                background_notification_timer_id = 0;

                // Check again if we should still send the notification
                if (should_show_background_notification()) {
                    send_background_notification();
                }

                return false; // Don't repeat
            });

            debug("Background notification scheduled in %d seconds", BACKGROUND_NOTIFICATION_GRACE_PERIOD_SECONDS);
        }
        
        private double get_window_background_duration() {
            if (window_background_start_time == null) {
                return 0.0;
            }
            
            var now = new DateTime.now_local();
            var duration = now.difference(window_background_start_time) / 1000000.0; // Convert to seconds
            return duration;
        }
        
        private bool should_show_background_notification_basic_check() {
            var mode = settings_manager.get_int_with_fallback("background-notifications-mode", 0);
            
            // Check mode-specific conditions
            switch (mode) {
                case 2: // Never
                    debug("Background notifications disabled by user preference");
                    return false;

                case 1: // First time only
                    if (session_background_notification_shown) {
                        debug("Background notification already shown this session");
                        return false;
                    }
                    break;

                case 0: // Always
                    // Check cooldown period
                    if (last_background_notification_time != null) {
                        var now = new DateTime.now_local();
                        var time_since_last = now.difference(last_background_notification_time) / 1000000.0;
                        if (time_since_last < BACKGROUND_NOTIFICATION_COOLDOWN_SECONDS) {
                            debug("Background notification cooldown active (%.1f seconds remaining)",
                                        BACKGROUND_NOTIFICATION_COOLDOWN_SECONDS - time_since_last);
                            return false;
                        }
                    }
                    break;

                default:
                    warning("Unknown background notification mode: %d", mode);
                    return false;
            }

            // Check if general notifications are enabled
            if (!should_show_notification()) {
                debug("Background notification blocked by general notification settings");
                return false;
            }
            
            return true;
        }
        
        private bool should_show_background_notification() {
            var mode = settings_manager.get_int_with_fallback("background-notifications-mode", 0);
            
            debug("Background notification check: mode=%d, session_shown=%s", mode, session_background_notification_shown.to_string());

            // Check mode first
            switch (mode) {
                case 2: // Never
                    debug("Background notifications disabled (Never)");
                    return false;
                case 1: // First time only
                    if (session_background_notification_shown) {
                        debug("Background notification already shown this session (First time only)");
                        return false;
                    }
                    break;
                case 0: // Always
                    break;
                default:
                    warning("Unknown background notification mode: %d", mode);
                    return false;
            }

            // First do basic checks
            if (!should_show_background_notification_basic_check()) {
                return false;
            }

            // Check if window is still in background (grace period is handled by timer)
            if (window_background_start_time == null) {
                debug("Window is not in background anymore");
                return false;
            }

            debug("Background notification approved");
            return true;
        }
        
        private void send_background_notification() {
            var mode = settings_manager.get_int_with_fallback("background-notifications-mode", 0);
            
            string message;
            switch (mode) {
                case 0: // Always
                    // TRANSLATORS: Background notification when app continues running in background
                    message = _("Karere is running in the background. Notifications are active.");
                    break;
                case 1: // First time only
                    // TRANSLATORS: First-time background notification to inform user of background operation
                    message = _("Karere is now running in the background. You'll receive notifications for new messages.");
                    break;
                default:
                    warning("Attempting to send background notification with mode: %d", mode);
                    return;
            }

            // TRANSLATORS: Application name in background notifications
            send_notification(_("Karere"), message, new ThemedIcon("dialog-information-symbolic"));

            // Update tracking variables
            last_background_notification_time = new DateTime.now_local();
            session_background_notification_shown = true;

            info("Background notification sent (mode=%d)", mode);
        }
    }
}