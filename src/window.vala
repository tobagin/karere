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

namespace Karere {

    [GtkTemplate (ui = "/io/github/tobagin/karere/window.ui")]
    public class Window : Adw.ApplicationWindow {
        
        
        [GtkChild]
        private unowned Adw.HeaderBar header_bar;
        
        [GtkChild]
        private unowned Gtk.Button back_button;
        
        [GtkChild]
        private unowned Gtk.Button forward_button;
        
        [GtkChild]
        private unowned Gtk.Button reload_button;
        
        [GtkChild]
        private unowned Gtk.MenuButton menu_button;
        
        [GtkChild]
        private unowned Adw.ToastOverlay toast_overlay;
        
        [GtkChild]
        private unowned WebKit.WebView web_view;
        
        private Settings settings;
        private WebKitManager webkit_manager;
        private NotificationManager notification_manager;
        private Logger logger;

        public Window(Gtk.Application app) {
            Object(application: app);
            
            logger = new Logger();
            settings = new Settings(Config.APP_ID);
            
            setup_window_properties();
            setup_actions();
            setup_webkit();
            setup_notifications();
            restore_window_state();
            
            logger.info("Window created and initialized");
        }

        private void setup_window_properties() {
            // Set window properties
            set_title(Config.APP_NAME);
            set_icon_name(Config.APP_ID);
            
            // Connect window state change signals
            notify["maximized"].connect(save_window_state);
            notify["default-width"].connect(save_window_state);
            notify["default-height"].connect(save_window_state);
            
            // Set up drag and drop for files
            setup_drag_and_drop();
            
            logger.debug("Window properties configured");
        }

        private void setup_actions() {
            // Zoom actions
            var zoom_in_action = new SimpleAction("zoom-in", null);
            zoom_in_action.activate.connect(() => {
                var zoom = web_view.zoom_level;
                web_view.zoom_level = (zoom * 1.2).clamp(0.5, 5.0);
                logger.debug("Zoomed in to %f", web_view.zoom_level);
            });
            add_action(zoom_in_action);

            var zoom_out_action = new SimpleAction("zoom-out", null);
            zoom_out_action.activate.connect(() => {
                var zoom = web_view.zoom_level;
                web_view.zoom_level = (zoom / 1.2).clamp(0.5, 5.0);
                logger.debug("Zoomed out to %f", web_view.zoom_level);
            });
            add_action(zoom_out_action);

            var zoom_reset_action = new SimpleAction("zoom-reset", null);
            zoom_reset_action.activate.connect(() => {
                web_view.zoom_level = 1.0;
                logger.debug("Zoom reset to 1.0");
            });
            add_action(zoom_reset_action);

            // Find action
            var find_action = new SimpleAction("find", null);
            find_action.activate.connect(() => {
                var find_controller = web_view.get_find_controller();
                // TODO: Show find bar
                show_info_toast("Find functionality will be implemented in a future version");
                logger.debug("Find action activated");
            });
            add_action(find_action);

            // Developer tools action
            var dev_tools_action = new SimpleAction("dev-tools", null);
            dev_tools_action.activate.connect(() => {
                var inspector = web_view.get_inspector();
                inspector.show();
                logger.debug("Developer tools opened");
            });
            add_action(dev_tools_action);

            logger.debug("Window actions configured");
        }

        private void setup_webkit() {
            webkit_manager = new WebKitManager();
            
            // Configure WebView
            web_view.load_uri("https://web.whatsapp.com");
            
            // Connect WebView signals
            web_view.load_changed.connect(on_load_changed);
            web_view.load_failed.connect(on_load_failed);
            
            logger.debug("WebKit configured");
        }

        private void setup_notifications() {
            notification_manager = new NotificationManager();
            
            // TODO: Set up notification handling in future version
            logger.debug("Notifications configured (placeholder)");
        }

        private void setup_drag_and_drop() {
            // Set up drag and drop for file sharing
            var drop_target = new Gtk.DropTarget(typeof(File), Gdk.DragAction.COPY);
            drop_target.drop.connect(on_file_dropped);
            web_view.add_controller(drop_target);
            
            logger.debug("Drag and drop configured");
        }

        private void restore_window_state() {
            // Restore window size
            var width = settings.get_int("window-width");
            var height = settings.get_int("window-height");
            set_default_size(width, height);
            
            // Restore maximized state
            if (settings.get_boolean("window-maximized")) {
                maximize();
            }
            
            logger.debug("Window state restored: %dx%d, maximized: %s", 
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
            
            logger.debug("Window state saved: %dx%d, maximized: %s", 
                        width, height, maximized.to_string());
        }

        [GtkCallback]
        private void on_back_clicked() {
            if (web_view.can_go_back()) {
                web_view.go_back();
                logger.debug("Navigated back");
            }
        }

        [GtkCallback]
        private void on_forward_clicked() {
            if (web_view.can_go_forward()) {
                web_view.go_forward();
                logger.debug("Navigated forward");
            }
        }

        [GtkCallback]
        private void on_reload_clicked() {
            web_view.reload();
            logger.debug("Page reloaded");
        }

        private void on_load_changed(WebKit.LoadEvent load_event) {
            switch (load_event) {
                case WebKit.LoadEvent.STARTED:
                    logger.debug("Load started");
                    update_navigation_buttons();
                    break;
                    
                case WebKit.LoadEvent.COMMITTED:
                    logger.debug("Load committed");
                    break;
                    
                case WebKit.LoadEvent.FINISHED:
                    logger.debug("Load finished");
                    update_navigation_buttons();
                    inject_notification_script();
                    break;
            }
        }

        private bool on_load_failed(WebKit.LoadEvent load_event, string failing_uri, Error error) {
            logger.error("Load failed for %s: %s", failing_uri, error.message);
            
            show_error_toast("Failed to load WhatsApp Web. Please check your internet connection.");
            return false;
        }


        private void update_navigation_buttons() {
            back_button.sensitive = web_view.can_go_back();
            forward_button.sensitive = web_view.can_go_forward();
        }

        private void inject_notification_script() {
            // TODO: Implement notification script injection in future version
            logger.debug("Notification script injection (placeholder)");
        }


        private bool on_file_dropped(Gtk.DropTarget target, Value value, double x, double y) {
            if (value.holds(typeof(File))) {
                var file = (File) value.get_object();
                logger.debug("File dropped: %s", file.get_path());
                
                // TODO: Implement file sharing with WhatsApp
                show_info_toast("File sharing will be implemented in a future version");
                return true;
            }
            return false;
        }

        public void show_error_toast(string message) {
            var toast = new Adw.Toast(message);
            toast.timeout = 5;
            toast_overlay.add_toast(toast);
            
            logger.debug("Error toast shown: %s", message);
        }

        public void show_info_toast(string message) {
            var toast = new Adw.Toast(message);
            toast.timeout = 3;
            toast_overlay.add_toast(toast);
            
            logger.debug("Info toast shown: %s", message);
        }

        public void show_success_toast(string message) {
            var toast = new Adw.Toast(message);
            toast.timeout = 2;
            toast_overlay.add_toast(toast);
            
            logger.debug("Success toast shown: %s", message);
        }

        public override bool close_request() {
            logger.info("Window close requested");
            save_window_state();
            
            // Allow the window to close
            return false;
        }

        public override void dispose() {
            logger.info("Window being disposed");
            
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