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
     * WebKitNotificationBridge handles WebKit notification permissions and bridging.
     *
     * This manager bridges WebKit notification permission requests and events
     * to native desktop notifications through the NotificationManager.
     */
    public class WebKitNotificationBridge : Object {

        private Settings? settings;
        private NotificationManager notification_manager;
        private Adw.ApplicationWindow parent_window;
        private WebKit.WebView? web_view;

        /**
         * Create a new WebKitNotificationBridge
         *
         * @param settings GSettings instance for permission persistence (can be null)
         * @param notification_manager The notification manager for sending native notifications
         * @param parent_window The parent window for presenting dialogs
         */
        public WebKitNotificationBridge(
            Settings? settings,
            NotificationManager notification_manager,
            Adw.ApplicationWindow parent_window
        ) {
            this.settings = settings;
            this.notification_manager = notification_manager;
            this.parent_window = parent_window;
        }

        /**
         * Setup the bridge with a WebView
         *
         * @param web_view The WebView to monitor for permission requests
         */
        public void setup(WebKit.WebView web_view) {
            this.web_view = web_view;

            // Connect to permission request signal
            web_view.permission_request.connect(on_permission_request);

            // Inject notification permission state early so WhatsApp Web can detect it
            inject_notification_permission_state(web_view);

            debug("WebKit notification bridge configured");
        }

        /**
         * Inject notification permission state into the page
         *
         * This ensures that when WhatsApp Web checks Notification.permission,
         * it gets the correct value based on our saved GSettings.
         *
         * NOTE: This overrides the permission property but WebKit still sends
         * notifications via the show_notification signal which we handle separately.
         */
        private void inject_notification_permission_state(WebKit.WebView web_view) {
            if (settings == null) {
                return;
            }

            bool permission_asked = settings.get_boolean("web-notification-permission-asked");
            bool permission_granted = settings.get_boolean("web-notification-permission-granted");

            if (!permission_asked || !permission_granted) {
                // No permission granted yet, don't inject anything
                return;
            }

            // Inject a script that makes Notification.permission report "granted"
            // This needs to run at document start, before WhatsApp Web's scripts
            var user_content_manager = web_view.get_user_content_manager();

            var script = new WebKit.UserScript(
                """
                // Override Notification.permission to report granted state
                // This ensures WhatsApp Web sees the permission as granted
                Object.defineProperty(Notification, 'permission', {
                    get: function() {
                        return 'granted';
                    },
                    configurable: false
                });
                console.log('[Karere] Notification permission injected as granted');
                """,
                WebKit.UserContentInjectedFrames.TOP_FRAME,
                WebKit.UserScriptInjectionTime.START,
                null,
                null
            );

            user_content_manager.add_script(script);
            info("Injected notification permission state: granted");

            // Also set up the notification handler so we can receive notifications
            setup_notification_handler();
        }

        /**
         * Handle WebKit permission requests
         */
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

        /**
         * Show native permission dialog for notification permission
         */
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

            dialog.present(parent_window);
        }

        /**
         * Setup notification handler on the WebView
         */
        private void setup_notification_handler() {
            if (web_view == null) {
                warning("Cannot setup notification handler: web_view is null");
                return;
            }

            // Set up notification handler on the WebView
            web_view.show_notification.connect(on_webkit_notification);
            info("Notification handler connected");
        }

        /**
         * Handle WebKit notifications and bridge them to native notifications
         */
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
                    parent_window.present();
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
    }
}
