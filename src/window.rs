use gtk::{gio, glib, prelude::*, subclass::prelude::*};
use gettextrs::gettext;
use libadwaita as adw;
use adw::prelude::*;
use adw::subclass::prelude::*;
use base64::prelude::*;
use webkit6::prelude::*;

use crate::accounts::{Account, AccountManager, build_account_row, apply_avatar_texture, create_account_icon_bytes, DEFAULT_ACCOUNT_ID, DEFAULT_COLOR, DEFAULT_EMOJI};

mod imp {
    use super::*;
    use gtk::gio;
    use std::rc::Rc;
    use tokio::sync::OnceCell;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/tobagin/karere/ui/window.ui")]
    pub struct KarereWindow {
        #[template_child]
        pub view_container: TemplateChild<gtk::Box>,
        #[template_child]
        pub zoom_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub dictionary_dropdown: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub account_bottom_sheet: TemplateChild<adw::BottomSheet>,
        #[template_child]
        pub account_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub account_avatar: TemplateChild<adw::Avatar>,
        #[template_child]
        pub accounts_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub add_account_button: TemplateChild<gtk::Button>,

        // Pool of WebViews keyed by account ID — kept alive to avoid reloading
        pub webviews: std::cell::RefCell<std::collections::HashMap<String, webkit6::WebView>>,

        // Force Close Flag
        pub force_close: std::cell::Cell<bool>,

        // Active Notifications Storage (Map ID -> WebKitNotification)
        pub active_notifications: std::cell::RefCell<std::collections::HashMap<String, webkit6::Notification>>,

        // Portal notification IDs for withdrawing on window focus
        pub portal_notification_ids: std::cell::RefCell<Vec<String>>,
        
        // Mobile Layout State (tracks if mobile layout is currently active)
        pub mobile_layout_active: std::cell::Cell<bool>,

        // Flag to prevent reload loops during mobile layout transitions
        pub mobile_layout_transitioning: std::cell::Cell<bool>,

        // Notification Proxy (Reusable)
        pub notification_proxy: Rc<OnceCell<ashpd::desktop::notification::NotificationProxy<'static>>>,

        // Window State Persistence
        pub last_unmaximized_size: std::cell::Cell<(i32, i32)>,

        // Resize Debounce Timer
        pub resize_timer: std::cell::RefCell<Option<glib::SourceId>>,

        // Account Manager
        pub account_manager: Rc<std::cell::RefCell<Option<AccountManager>>>,

        // Flag to prevent multiple concurrent account list updates
        pub updating_accounts: std::cell::Cell<bool>,

        // Currently active account ID (matches a key in webviews)
        pub active_account_id: std::cell::RefCell<Option<String>>,
    }

    /// Helper function to determine if mobile layout should be used
    /// window_width: current window width in pixels (use 0 if unknown)
    fn should_use_mobile_layout(settings: &gio::Settings, window_width: i32) -> bool {
        const MOBILE_WIDTH_THRESHOLD: i32 = 768;

        let mode = settings.string("mobile-layout");
        match mode.as_str() {
            "enabled" => true,
            "disabled" => false,
            "auto" | _ => {
                // Check for mobile desktop environments
                if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
                    let desktop_lower = desktop.to_lowercase();
                    let mobile_desktops = ["phosh", "plasma-mobile", "lomiri"];
                    if mobile_desktops.iter().any(|d| desktop_lower.contains(d)) {
                        return true;
                    }
                }
                // Check window width (0 means unknown, don't trigger)
                if window_width > 0 && window_width < MOBILE_WIDTH_THRESHOLD {
                    return true;
                }
                false
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for KarereWindow {
        const NAME: &'static str = "KarereWindow";
        type Type = super::KarereWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for KarereWindow {
        fn constructed(&self) {
            self.parent_constructed(); // Call parent constructed first
            let obj = self.obj();

            // Debug Font Config
            if let Ok(val) = std::env::var("FONTCONFIG_FILE") {
                println!("DEBUG: FONTCONFIG_FILE={}", val);
            } else {
                println!("DEBUG: FONTCONFIG_FILE is NOT SET");
            }

            // Apply devel class if needed
            // Fallback to env var since obj.application() might be None in constructed
            let app_id = obj.application()
                .and_then(|app| app.application_id())
                .map(|s| s.to_string())
                .or_else(|| std::env::var("FLATPAK_ID").ok())
                .unwrap_or_default();

            if app_id.contains("Dev") || app_id.contains("Devel") {
                 obj.add_css_class("devel");
            }

            // 0. Setup Account Manager (before WebView so we can determine per-account dirs)
            let account_manager = AccountManager::new();
            
            // Ensure a default account always exists (migrates legacy session if present)
            match account_manager.ensure_default_account() {
                Ok(account) => println!("Karere: Active account: '{}' (has_session: {})", account.name, account.has_session),
                Err(e) => eprintln!("Karere: Failed to ensure default account: {}", e),
            }
            
            *self.account_manager.borrow_mut() = Some(account_manager);

            // Determine initial WebView directories based on active account
            let (account_id, data_dir, cache_dir) = {
                let mgr_ref = self.account_manager.borrow();

                let default_data = glib::user_data_dir().join("karere").join("webkit");
                let default_cache = glib::user_cache_dir().join("karere").join("webkit");

                if let Some(mgr) = mgr_ref.as_ref() {
                    mgr.get_active_account()
                        .ok()
                        .flatten()
                        .map(|account| {
                            let id = account.id.clone();
                            (id.clone(), mgr.data_dir_for(&id), mgr.cache_dir_for(&id))
                        })
                        .unwrap_or((DEFAULT_ACCOUNT_ID.to_string(), default_data, default_cache))
                } else {
                    (DEFAULT_ACCOUNT_ID.to_string(), default_data, default_cache)
                }
            };


            // Create an initial WebView with a per-account session
            let web_view = self.setup_webview(&account_id, data_dir, cache_dir, true);

            // One-time: Pre-emptive Camera Permission Request (Fix for "0 Devices" / Catch-22)
            // We must ask for permission *before* WebKit initializes GStreamer, otherwise
            // GStreamer sees 0 devices and WebKit never asks for permission.
            let ctx = glib::MainContext::default();
            ctx.spawn_local(async move {
                let _ = crate::RUNTIME.spawn(async {
                    println!("Karere: Requesting Camera Access at Startup...");
                    if let Ok(proxy) = ashpd::desktop::camera::Camera::new().await {
                        match proxy.request_access().await {
                            Ok(_) => println!("Karere: Camera Access GRANTED by System/User."),
                            Err(e) => println!("Karere: Camera Access DENIED or FAILED: {:?}", e),
                        }

                        match proxy.is_present().await {
                            Ok(present) => println!("Karere: Camera Is Present: {}", present),
                            Err(e) => println!("Karere: Failed to check camera presence: {:?}", e),
                        }

                        match proxy.open_pipe_wire_remote().await {
                            Ok(fd) => println!("Karere: Successfully opened PipeWire remote via Portal (FD: {:?})", fd),
                            Err(e) => println!("Karere: Failed to open PipeWire remote: {:?}", e),
                        }
                    } else {
                        println!("Karere: Failed to connect to Camera Portal.");
                    }
                }).await;
            });

            // One-time: Handle Window Focus (Clear Unread for active account)
            obj.connect_is_active_notify(move |window| {
                if window.is_active() {
                     if let Some(app) = window.application() {
                         app.activate_action("set-unread", Some(&false.to_variant()));
                     }
                     // Clear unread for the active account
                     let account_id = window.imp().active_account_id.borrow().clone();
                     if let Some(id) = account_id {
                         if let Some(mgr) = window.imp().account_manager.borrow().as_ref() {
                             let _ = mgr.set_account_unread(&id, false);
                         }
                     }
                     // Refresh account button to update unread indicators
                     window.imp().update_account_button();

                     // Withdraw all desktop notifications
                     let portal_ids: Vec<String> = window.imp().portal_notification_ids.borrow_mut().drain(..).collect();
                     if !portal_ids.is_empty() {
                         let proxy_cell = window.imp().notification_proxy.clone();
                         glib::MainContext::default().spawn_local(async move {
                             let _guard = crate::RUNTIME.enter();
                             if let Some(proxy) = proxy_cell.get() {
                                 for id in &portal_ids {
                                     let _ = proxy.remove_notification(id).await;
                                 }
                             }
                         });
                     }
                }
            });

            // Setup Actions (use dynamic webview() helper so they work after WebView replacement)
            let action_refresh = gio::SimpleAction::new("refresh", None);
            let obj_weak = obj.downgrade();
            action_refresh.connect_activate(move |_, _| {
                if let Some(obj) = obj_weak.upgrade() {
                    if let Some(webview) = obj.imp().webview() {
                        webview.reload();
                    }
                }
            });
            obj.add_action(&action_refresh);

            // New Chat Action (Ctrl+N -> Ctrl+Alt+N simulation)
            let action_new_chat = gio::SimpleAction::new("new-chat", None);
            let obj_weak_nc = obj.downgrade();
            action_new_chat.connect_activate(move |_, _| {
                if let Some(obj) = obj_weak_nc.upgrade() {
                    if let Some(webview) = obj.imp().webview() {
                        let js = r#"
                            (function() {
                                const ev = new KeyboardEvent('keydown', {
                                    bubbles: true, 
                                    cancelable: true, 
                                    view: window, 
                                    ctrlKey: true, 
                                    altKey: true, 
                                    shiftKey: false,
                                    metaKey: false,
                                    key: 'n',
                                    code: 'KeyN',
                                    keyCode: 78,
                                    which: 78
                                });
                                document.dispatchEvent(ev);
                            })();
                        "#;
                        webview.evaluate_javascript(js, None, None, Option::<&gio::Cancellable>::None, |_| {});
                    }
                }
            });
            obj.add_action(&action_new_chat);

            // Connect account button to toggle the bottom sheet
            let obj_weak_sheet = obj.downgrade();
            self.account_button.connect_clicked(move |_| {
                if let Some(obj) = obj_weak_sheet.upgrade() {
                    let sheet = &obj.imp().account_bottom_sheet;
                    sheet.set_open(!sheet.is_open());
                }
            });

            // Bind account button visibility to multi-account preference
            let app_id_ma = std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string());
            let settings_ma = gio::Settings::new(&app_id_ma);
            settings_ma.bind("enable-multi-account", &*self.account_button, "visible").build();

            // Connect account list row activation (for switching accounts)
            let obj_weak_list = obj.downgrade();
            self.accounts_list.connect_row_activated(move |_, row| {
                let account_id = row.widget_name();
                if !account_id.is_empty() {
                    if let Some(obj) = obj_weak_list.upgrade() {
                        obj.imp().switch_to_account(&account_id);
                    }
                }
            });

            // Setup Add Account Action
            let action_add_account = gio::SimpleAction::new("add-account", None);
            let obj_weak_acct = obj.downgrade();
            action_add_account.connect_activate(move |_, _| {
                if let Some(window) = obj_weak_acct.upgrade() {
                    window.show_add_account_dialog();
                }
            });
            obj.add_action(&action_add_account);

            // Update account button with current account info
            self.update_account_button();

            // Setup Settings Logic
            let app_id = std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string());
            let settings = gio::Settings::new(&app_id);

            // Restore Window State
            let width = settings.int("window-width");
            let height = settings.int("window-height");
            if width > 0 && height > 0 {
                obj.set_default_size(width, height);
                obj.imp().last_unmaximized_size.set((width, height));
            } else {
                obj.imp().last_unmaximized_size.set((800, 600));
            }

            if settings.boolean("is-maximized") {
                obj.maximize();
            }

            // Track Window Size (Resize Events) - Save only unmaximized size
            let obj_weak_resize = obj.downgrade();
            obj.connect_realize(move |window| {
                if let Some(surface) = window.surface() {
                    let obj_weak = obj_weak_resize.clone();
                    surface.connect_layout(move |_surface, _width, _height| {
                        if let Some(window) = obj_weak.upgrade() {
                            if !window.is_maximized() {
                                let w = window.width();
                                let h = window.height();
                                window.imp().last_unmaximized_size.set((w, h));
                            }
                        }
                    });
                }
            });


            // Window Resize Handler - Check for mobile/desktop transition in Auto mode
            // We need to use the surface's layout signal which fires on actual resize
            let settings_resize = settings.clone();
            let webview_resize = web_view.clone();
            let obj_weak_resize = obj.downgrade();
            obj.connect_realize(move |window| {
                if let Some(surface) = window.surface() {
                    let settings_layout = settings_resize.clone();
                    let webview_layout = webview_resize.clone();
                    let obj_weak_layout = obj_weak_resize.clone();
                    
                    // Initialize the mobile_layout_active state based on current width
                    // Initialize the mobile_layout_active state based on current width
                    let initial_width = window.width();
                    window.imp().mobile_layout_active.set(should_use_mobile_layout(&settings_layout, initial_width));
                    
                    surface.connect_layout(move |_, width, _| {
                        if let Some(window) = obj_weak_layout.upgrade() {
                            // Skip if we're already in the middle of a layout transition reload
                            if window.imp().mobile_layout_transitioning.get() {
                                return;
                            }

                            let was_mobile = window.imp().mobile_layout_active.get();
                            let is_mobile = should_use_mobile_layout(&settings_layout, width);

                            if is_mobile != was_mobile {
                                println!("Karere: Layout Mode Change Detected (Mobile: {} -> {}). Reloading...", was_mobile, is_mobile);
                                window.imp().mobile_layout_active.set(is_mobile);
                                window.imp().mobile_layout_transitioning.set(true);
                                webview_layout.reload();
                            }
                        }
                    });
                }
            });

            // Present Action (for notifications)
            let action_present = gio::SimpleAction::new("present", None);
            let obj_weak = obj.downgrade();
            action_present.connect_activate(move |_, _| {
                if let Some(obj) = obj_weak.upgrade() {
                    obj.present();
                }
            });
            obj.add_action(&action_present);

            // Handle Close Request (Background Mode & State Saving)
            let settings_close = settings.clone();
            obj.connect_close_request(move |window| {
                // Save Window State
                let is_maximized = window.is_maximized();
                let (width, height) = window.imp().last_unmaximized_size.get();

                if width > 0 && height > 0 {
                    let _ = settings_close.set_int("window-width", width);
                    let _ = settings_close.set_int("window-height", height);
                }
                let _ = settings_close.set_boolean("is-maximized", is_maximized);
                gio::Settings::sync();

                if window.imp().force_close.get() {
                    // Clean up WebViews on actual quit
                    let mut webviews = window.imp().webviews.borrow_mut();
                    for (_id, wv) in webviews.drain() {
                        window.imp().view_container.remove(&wv);
                    }
                    return glib::Propagation::Proceed;
                }

                // Check "Close Button Behavior" setting
                let close_action = settings_close.string("close-button-action");
                if close_action == "quit" {
                    // Clean up WebViews on actual quit
                    let mut webviews = window.imp().webviews.borrow_mut();
                    for (_id, wv) in webviews.drain() {
                        window.imp().view_container.remove(&wv);
                    }
                    return glib::Propagation::Proceed;
                }

                // Hide instead of close (Default/'background')
                window.set_visible(false);
                glib::Propagation::Stop
            });
            


            // 1. Theme
            let _style_manager = adw::StyleManager::default();
            // Bind setting to theme
            let _settings_clone_theme = settings.clone();
            let update_theme = move |settings: &gio::Settings, key: &str| {
                let theme = settings.string(key);
                let scheme = match theme.as_str() {
                    "light" => adw::ColorScheme::ForceLight,
                    "dark" => adw::ColorScheme::ForceDark,
                    _ => adw::ColorScheme::Default,
                };
                adw::StyleManager::default().set_color_scheme(scheme);
            };
            // Initial set
            update_theme(&settings, "theme");
            // Connect change
            settings.connect_changed(Some("theme"), update_theme);

            // Mobile Layout Change Handler - Reload webview when mode changes
            let obj_weak_mobile_setting = obj.downgrade();
            settings.connect_changed(Some("mobile-layout"), move |_settings, _| {
                if let Some(obj) = obj_weak_mobile_setting.upgrade() {
                    if let Some(webview) = obj.imp().webview() {
                        // Reload webview to apply/remove mobile layout
                        webview.reload();
                    }
                }
            });

            // WebKit settings and signals moved to setup_webview
            
            // 12. Dictionary Dropdown Logic (Quick Access)
            settings.bind("auto-detect-language", &*self.dictionary_dropdown, "visible")
                .flags(gio::SettingsBindFlags::INVERT_BOOLEAN)
                .build();
                
            let avail_dicts = crate::spellcheck::get_available_dictionaries();
            if !avail_dicts.is_empty() {
                let store = gtk::StringList::new(&avail_dicts.iter().map(|s| s.as_str()).collect::<Vec<&str>>());
                self.dictionary_dropdown.set_model(Some(&store));
                
                let current = settings.strv("spell-checking-languages");
                if let Some(first) = current.first() {
                    if let Some(idx) = avail_dicts.iter().position(|r| r == first.as_str()) {
                         self.dictionary_dropdown.set_selected(idx as u32);
                    }
                }
                
                let settings_dict = settings.clone();
                let dicts_clone = avail_dicts.clone();
                self.dictionary_dropdown.connect_selected_notify(move |dropdown| {
                     let idx = dropdown.selected() as usize;
                     if idx < dicts_clone.len() {
                         let selected = &dicts_clone[idx];
                         let _ = settings_dict.set_strv("spell-checking-languages", [selected.as_str()]);
                     }
                });
            }

            // Initialize background webviews for all non-active accounts
            // This enables notifications from every account without user interaction
            let background_accounts = {
                let mgr_ref = self.account_manager.borrow();
                if let Some(mgr) = mgr_ref.as_ref() {
                    mgr.get_accounts().unwrap_or_default()
                        .iter()
                        .filter(|a| !a.is_active)
                        .map(|a| (a.id.clone(), mgr.data_dir_for(&a.id), mgr.cache_dir_for(&a.id)))
                        .collect::<Vec<_>>()
                } else { vec![] }
            }; // borrow dropped

            for (id, data_dir, cache_dir) in background_accounts {
                println!("Karere: Initializing background webview for account '{}'", id);
                self.setup_webview(&id, data_dir, cache_dir, false);
            }

            // Global zoom-level listener: when the accessibility zoom floor changes,
            // apply the new floor to ALL webviews and reset all per-account zooms.
            let obj_weak_zoom_floor = obj.downgrade();
            settings.connect_changed(Some("zoom-level"), move |settings, _| {
                if !settings.boolean("webview-zoom") {
                    return; // Accessibility zoom not enabled, ignore
                }
                let floor = settings.double("zoom-level");
                if let Some(window) = obj_weak_zoom_floor.upgrade() {
                    // Reset all per-account zooms to the new floor
                    if let Some(mgr) = window.imp().account_manager.borrow().as_ref() {
                        if let Ok(accounts) = mgr.get_accounts() {
                            for account in &accounts {
                                let _ = mgr.set_account_zoom(&account.id, floor);
                            }
                        }
                    }
                    // Apply to all live webviews
                    for wv in window.imp().webviews.borrow().values() {
                        wv.set_zoom_level(floor);
                    }
                }
            });

            // When accessibility zoom is toggled on, apply the floor immediately
            let obj_weak_zoom_toggle = obj.downgrade();
            settings.connect_changed(Some("webview-zoom"), move |settings, _| {
                if !settings.boolean("webview-zoom") {
                    return;
                }
                let floor = settings.double("zoom-level");
                if let Some(window) = obj_weak_zoom_toggle.upgrade() {
                    if let Some(mgr) = window.imp().account_manager.borrow().as_ref() {
                        if let Ok(accounts) = mgr.get_accounts() {
                            for account in &accounts {
                                if account.zoom_level < floor {
                                    let _ = mgr.set_account_zoom(&account.id, floor);
                                }
                            }
                        }
                    }
                    for wv in window.imp().webviews.borrow().values() {
                        if wv.zoom_level() < floor {
                            wv.set_zoom_level(floor);
                        }
                    }
                }
            });
        }
    }

    impl KarereWindow {
        /// Get the currently active WebView (if any).
        pub fn webview(&self) -> Option<webkit6::WebView> {
            let id = self.active_account_id.borrow().clone()?;
            self.webviews.borrow().get(&id).cloned()
        }

        pub fn setup_webview(&self, account_id: &str, data_dir: std::path::PathBuf, cache_dir: std::path::PathBuf, is_foreground: bool) -> webkit6::WebView {
            // Hide current WebView (don't destroy it — we keep it in the pool)
            if is_foreground {
                if let Some(current_wv) = self.webview() {
                    current_wv.set_visible(false);
                }
            }

            // Ensure directories exist
            let _ = std::fs::create_dir_all(&data_dir);
            let _ = std::fs::create_dir_all(&cache_dir);

            // Create per-account NetworkSession
            let session = webkit6::NetworkSession::new(
                data_dir.to_str(),
                cache_dir.to_str()
            );

            // Create WebView Manually with Session
            let web_view = webkit6::WebView::builder()
                .network_session(&session)
                .build();

            // Enable Page Cache explicitly (Helpful for history navigation)
            if let Some(settings) = webkit6::prelude::WebViewExt::settings(&web_view) {
                // Memory Optimization: Disable Page Cache (Back/Forward Cache)
                settings.set_enable_page_cache(false);
                settings.set_enable_webrtc(true);
                settings.set_enable_media_stream(true);
                settings.set_enable_mediasource(true);

                // User Agent Spoofing (Chrome Linux) — must match for all accounts
                let user_agent = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";
                settings.set_user_agent(Some(user_agent));

                // Disable quirks to restore our manual Linux UA
                settings.set_enable_site_specific_quirks(false);
            }

            web_view.set_vexpand(true);
            web_view.set_hexpand(true);

            // Add to UI
            self.view_container.append(&web_view);
            web_view.set_visible(is_foreground);
            self.webviews.borrow_mut().insert(account_id.to_string(), web_view.clone());
            if is_foreground {
                *self.active_account_id.borrow_mut() = Some(account_id.to_string());
            }

            // Load URI
            // Load URI moved to end

            // Spoof Page Visibility API to ensure notifications firing in background
            if let Some(ucm) = web_view.user_content_manager() {
                let source = r#"
                    Object.defineProperty(document, 'hidden', {get: function() { return false; }});
                    Object.defineProperty(document, 'visibilityState', {get: function() { return 'visible'; }});
                    document.dispatchEvent(new Event('visibilitychange'));
                 "#;
                let script = webkit6::UserScript::new(
                    source,
                    webkit6::UserContentInjectedFrames::TopFrame,
                    webkit6::UserScriptInjectionTime::Start,
                    &[],
                    &[]
                );
                ucm.add_script(&script);

                // Prevent WhatsApp Web from closing notifications via JS.
                // When the active tab auto-closes a notification, WebKit removes
                // it from internal tracking and notification.clicked() becomes a
                // no-op. By making close() a no-op, the WebKit notification stays
                // alive so our Rust-side clicked() call can still dispatch the
                // click event back to the page/service-worker.
                // Tag-based replacement (new Notification with same tag) still
                // works — it's handled at creation time, not via close().
                let notif_hook = r#"
                    Notification.prototype.close = function() {};
                "#;
                let notif_script = webkit6::UserScript::new(
                    notif_hook,
                    webkit6::UserContentInjectedFrames::TopFrame,
                    webkit6::UserScriptInjectionTime::Start,
                    &[],
                    &[]
                );
                ucm.add_script(&notif_script);
            }

            let obj = self.obj();
            let app_id = std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string());
            let settings = gio::Settings::new(&app_id);
            // 2. WebKit Settings
            // Also ensure main switch is toggled on WebView settings
            if let Some(ws) = webkit6::prelude::WebViewExt::settings(&web_view) {
                 settings.bind("enable-developer-tools", &ws, "enable-developer-extras").build();
                 
                 // Memory Optimization: Disable Page Cache (Back/Forward Cache)
                 ws.set_enable_page_cache(false);

                 ws.set_enable_media_stream(true);
                 ws.set_enable_mediasource(true);
                 ws.set_enable_webrtc(true);

            // Debug: Monitor Camera Capture State
            web_view.connect_notify_local(Some("camera-capture-state"), |webview, _| {
                let state = webview.camera_capture_state();
                println!("Karere: Camera Capture State Changed: {:?}", state);
            });

            // Debug: Monitor Microphone Capture State
             web_view.connect_notify_local(Some("microphone-capture-state"), |webview, _| {
                let state = webview.microphone_capture_state();
                println!("Karere: Microphone Capture State Changed: {:?}", state);
            });

                 // User Agent Spoofing (Chrome Linux)
                 let user_agent = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";
                 ws.set_user_agent(Some(&user_agent));

                 // Disable quirks to restore our manual Linux UA
                 ws.set_enable_site_specific_quirks(false);

                 // Security hardening: restrict file and data URL access
                 ws.set_allow_file_access_from_file_urls(false);
                 ws.set_allow_universal_access_from_file_urls(false);
                 ws.set_allow_top_navigation_to_data_urls(false);

                 // CRITICAL: ANY JavaScript navigator override (userAgent OR platform) breaks dead key composition!
                 // Validated by user: platform-only override also causes dead key bug.
                 // We must NOT inject any scripts that modify navigator.
                 // The "Download for Mac" banner is cosmetic and unavoidable.
                 
                 if let Some(_ucm) = web_view.user_content_manager() {
                       // Notification Persistence Sync (Proxy Strategy) - REMOVED
                       // We now rely on Host-Driven Trigger (on Load Finished) and PersistentNetworkSession.
                       // The Proxy is no longer needed to "lie" because we can now get the "truth" (native grant) fast enough.
                   }
             }




            // Inject Notification Persistence (Native Logic)
            // With PersistentNetworkSession, we rely on WebKit saving the permission state.
            // When requestPermission is called by the site, our Rust handler (below) intercepts it.



            // Setup JS->Rust Logging Channel
            if let Some(ucm) = web_view.user_content_manager() {
                ucm.register_script_message_handler("log", None);

                // Inject Console Override Script
                let console_script = r#"
                    (function() {
                        function send(level, args) {
                            try {
                                var msg = Array.from(args).map(obj => String(obj)).join(' ');
                                window.webkit.messageHandlers.log.postMessage(level + ': ' + msg);
                            } catch(e) {}
                        }
                        var oldLog = console.log;
                        var oldWarn = console.warn;
                        var oldError = console.error;
                        console.log = function() { send('LOG', arguments); oldLog.apply(console, arguments); };
                        console.warn = function() { send('WARN', arguments); oldWarn.apply(console, arguments); };
                        console.error = function() { send('ERROR', arguments); oldError.apply(console, arguments); };
                    })();
                "#;
                let script = webkit6::UserScript::new(
                    console_script,
                    webkit6::UserContentInjectedFrames::TopFrame,
                    webkit6::UserScriptInjectionTime::Start,
                    &[],
                    &[]
                );
                ucm.add_script(&script);

                ucm.connect_script_message_received(Some("log"), |_, result| {
                    // result is a webkit6::javascriptcore6::Value
                    if result.is_string() {
                        let s = result.to_string();
                        println!("WEB-CONSOLE: {}", s);
                    }
                });
            }

            // 9. Developer Tools Action
            let obj_weak_dev = obj.downgrade();
            let action_devtools = gio::SimpleAction::new("show-devtools", None);
            action_devtools.connect_activate(move |_, _| {
                if let Some(obj) = obj_weak_dev.upgrade() {
                    if let Some(webview) = obj.imp().webview() {
                        if let Some(inspector) = webview.inspector() {
                            inspector.show();
                        }
                    }
                }
            });
            obj.add_action(&action_devtools);

            // Minimize Action (Ctrl+M)
            let obj_weak_min = obj.downgrade();
            let action_minimize = gio::SimpleAction::new("minimize", None);
            action_minimize.connect_activate(move |_, _| {
                if let Some(obj) = obj_weak_min.upgrade() {
                    obj.minimize();
                }
            });
            obj.add_action(&action_minimize);

            // Toggle Fullscreen Action (F11)
            let obj_weak_fs = obj.downgrade();
            let action_fullscreen = gio::SimpleAction::new("toggle-fullscreen", None);
            action_fullscreen.connect_activate(move |_, _| {
                if let Some(obj) = obj_weak_fs.upgrade() {
                    if obj.is_fullscreen() {
                        obj.unfullscreen();
                    } else {
                        obj.fullscreen();
                    }
                }
            });
            obj.add_action(&action_fullscreen);

            // Toggle High Contrast Action (Ctrl+Shift+H)
            let action_high_contrast = gio::SimpleAction::new("toggle-high-contrast", None);
            let settings_hc = settings.clone();
            action_high_contrast.connect_activate(move |_, _| {
                let current = settings_hc.boolean("high-contrast");
                let _ = settings_hc.set_boolean("high-contrast", !current);
            });
            obj.add_action(&action_high_contrast);

            // Toggle Focus Indicators Action (Ctrl+Shift+F)
            let action_focus = gio::SimpleAction::new("toggle-focus-indicators", None);
            let settings_fi = settings.clone();
            action_focus.connect_activate(move |_, _| {
                let current = settings_fi.boolean("focus-indicators");
                let _ = settings_fi.set_boolean("focus-indicators", !current);
            });
            obj.add_action(&action_focus);

            // Toggle Notifications Action (Ctrl+Shift+N)
            let action_notify = gio::SimpleAction::new("toggle-notifications", None);
            let settings_nt = settings.clone();
            action_notify.connect_activate(move |_, _| {
                let current = settings_nt.boolean("notifications-enabled");
                let _ = settings_nt.set_boolean("notifications-enabled", !current);
            });
            obj.add_action(&action_notify);

            // 3. Downloads
            if let Some(session) = web_view.network_session() {
                let settings_dl = settings.clone();
                let overlay = self.toast_overlay.get(); // Strong ref for clone! to downgrade
                let overlay_weak = overlay.downgrade();
                
                // Clone obj (self) which is strong, to pass to clone! for downgrading if needed, 
                // but actually we need to pass `obj` itself to clone! if we want it to handle weak ref creation.
                // However, `connect_closure` might take 'move', so we need to be careful.
                // The issue was `@weak overlay_weak` where overlay_weak was already weak.
                // We should pass `@weak overlay` where overlay is Strong.
                
                session.connect_closure(
                     "download-started",
                     false,
                     glib::closure_local! (move |_session: webkit6::NetworkSession, download: webkit6::Download| {
                          let settings_dl = settings_dl.clone();
                           let overlay_weak = overlay_weak.clone();
                           let settings_finished = settings_dl.clone();
                           let overlay_weak_fin = overlay_weak.clone();
                           let overlay_weak_fail = overlay_weak.clone();
                          
                          download.connect_closure(
                              "decide-destination",
                              false,
                              glib::closure_local! (move |download: webkit6::Download, filename: glib::GString| -> bool {
                                   let directory = settings_dl.string("download-directory");
                                   let path = if !directory.is_empty() {
                                       let mut path_str = directory.to_string();
                                       // Expand ~
                                       if path_str.starts_with("~") {
                                           let home = glib::home_dir();
                                           path_str = path_str.replacen("~", home.to_str().unwrap(), 1);
                                       }
                                       let p = std::path::PathBuf::from(path_str);
                                       if p.exists() { Some(p) } else { None }
                                   } else {
                                       None
                                   };

                                   // Fall back to XDG Downloads dir (always accessible in Flatpak sandbox)
                                   let path = path.or_else(|| glib::user_special_dir(glib::UserDirectory::Downloads).map(|p| p.to_path_buf()));

                                   if let Some(path) = path {
                                       let safe_name = std::path::Path::new(filename.as_str())
                                          .file_name()
                                          .unwrap_or(std::ffi::OsStr::new("download"));
                                      let dest = path.join(safe_name);
                                       let dest_file = gio::File::for_path(&dest);
                                       let uri_str = dest_file.uri();
                                       download.set_destination(&uri_str);
                                       return true;
                                   }
                                   false
                              }
                          ));
                          
                          // Handle Finished (Show Toast)
                          // let overlay_weak = overlay.downgrade();
                          download.connect_finished(move |download| {
                               let settings_dl = &settings_finished;
                               if let Some(overlay) = overlay_weak_fin.upgrade() {
                                   // Try to get path from URI first (standard), fallback if needed
                                   let file_path = if let Some(uri) = download.destination() {
                                       gio::File::for_uri(&uri).path()
                                           .or_else(|| gio::File::for_path(uri.as_str()).path()) // Fallback: maybe it was a path?
                                   } else {
                                       None
                                   };

                                   if let Some(file) = file_path {
                                            let filename = file.file_name().unwrap_or_default().to_string_lossy();
                                            // Downloads
                                            // Check Master & Download Toggles
                                            let master_enabled = settings_dl.boolean("notifications-enabled");
                                            let dl_enabled = settings_dl.boolean("notify-downloads-enabled");
                                            
                                            if master_enabled && dl_enabled {
                                                 let dl_type = settings_dl.string("notify-download-type");
                                                 let filename_str = &filename; // Use deref for Cow<str>

                                                 if dl_type == "system" {
                                                      // System Notification
                                                      let note = gio::Notification::new("Download Complete");
                                                      note.set_body(Some(filename_str));
                                                      note.set_icon(&gio::ThemedIcon::new("folder-download-symbolic"));
                                                      
                                                      // Add Open Action
                                                      // "app.present-window" brings up app.
                                                      // Use "app.open-download" with uri param
                                                      if let Some(uri) = download.destination() {
                                                          let uri_str = uri.to_string();
                                                          let detailed_action = gio::Action::print_detailed_name("app.open-download", Some(&uri_str.to_variant()));
                                                          note.set_default_action(&detailed_action);
                                                      }
                                                
                                                      if let Some(app) = gio::Application::default() {
                                                          app.send_notification(Some(&format!("dl-{}", glib::monotonic_time())), &note);
                                                      }
                                                 } else {
                                                      // Toast (Default)
                                                      let toast = adw::Toast::new(&format!("Download Complete: {}", filename_str));
                                                      // Hardcoding standard timeout 
                                                      toast.set_timeout(5); 

                                                      toast.set_button_label(Some("Open"));
                                                      if let Some(uri) = download.destination() {
                                                           let uri_str = uri.to_string();
                                                           toast.set_action_name(Some("app.open-download"));
                                                           toast.set_action_target_value(Some(&uri_str.to_variant()));
                                                      }
                                                      overlay.add_toast(toast);
                                                 }
                                            }
                                       }
                                   }
                               });
                          
                          // Handle Failed (Show Alert)
                          // Use `obj` (strong - KarereWindow wrapper) if available? 
                          // We are inside `constructed` method, so `obj` (Self) is available in outer scope.
                          // But we need to capture it.
                          // Let's assume we can capture `obj` from outer scope.
                          // BUT `obj` was defined way up.
                          // We need to pass it into the closure_local!
                          // Re-define window_strong to be safe.
                          
                          // Handle Failed (Show Alert)
                          // let overlay_weak2 = overlay.downgrade();
                          download.connect_failed(move |_, _error| {
                               if let Some(overlay) = overlay_weak_fail.upgrade() {
                               // Fallback to overlay if window logic is too complex to capture deep in closures without visual ref
                               // Actually, let's just use the overlay to find the window or just show a toast for error too slightly
                               // Or just use the overlay to show error toast?
                               // User asked for AlertDialog.
                               // Use proper weak ref to window.
                               // We need to capture `obj`.
                               // We can't easily access `obj` here unless we captured it.
                               // Let's modify the outer `glib::closure_local!` to capture `obj`.
                               
                               // Actually, `overlay` is attached to window.
                               if let Some(window) = overlay.root().and_then(|w| w.downcast::<gtk::Window>().ok()) {
                                   let dialog = adw::AlertDialog::builder()
                                       .heading(&gettext("Download Failed"))
                                       .body(&gettext("An error occurred while downloading the file."))
                                       .default_response("ok")
                                       .build();
                                   dialog.add_response("ok", &gettext("OK"));
                                   dialog.choose(Some(&window), gio::Cancellable::NONE, |_| {});
                               }
                           }
                      });
                     }
                ));
            }
            
            // 4. Notifications & Microphone Permissions (per-account persistence)
            let window_weak_perm = obj.downgrade();
            let account_id_perm = account_id.to_string();
            
            web_view.connect_permission_request(move |_, request| {
                let window_weak = window_weak_perm.clone();
                let account_id = account_id_perm.clone();

                // Helper: get account manager from window
                let get_mgr = |window: &super::KarereWindow| -> Option<crate::accounts::AccountManager> {
                    window.imp().account_manager.borrow().clone()
                };

                if let Some(req) = request.downcast_ref::<webkit6::NotificationPermissionRequest>() {
                    // Check per-account persistence
                    if let Some(window) = window_weak.upgrade() {
                        if let Some(mgr) = get_mgr(&window) {
                            let (asked, granted) = mgr.get_account_permission(&account_id, "notification");
                            
                            if asked {
                                if granted { req.allow(); } else { req.deny(); }
                                return true;
                            }

                            // Show Dialog
                            let dialog = adw::AlertDialog::builder()
                                .heading(&gettext("WhatsApp Web Notification Permission"))
                                .body(&gettext("WhatsApp Web wants to show desktop notifications for new messages. Would you like to allow notifications?"))
                                .default_response("allow")
                                .close_response("deny")
                                .build();

                            dialog.add_response("deny", &gettext("Deny"));
                            dialog.add_response("allow", &gettext("Allow"));
                            dialog.set_response_appearance("allow", adw::ResponseAppearance::Suggested);
                            
                            let req_clone = request.clone();
                            let account_id_d = account_id.clone();
                            let window_weak_d = window.downgrade();
                            dialog.choose(Some(&window), gio::Cancellable::NONE, move |response| {
                                let granted = response == "allow";
                                if let Some(w) = window_weak_d.upgrade() {
                                    if let Some(m) = get_mgr(&w) {
                                        let _ = m.set_account_permission(&account_id_d, "notification", true, granted);
                                    }
                                }
                                if granted { req_clone.allow(); } else { req_clone.deny(); }
                            });
                            return true;
                        }
                    }
                } else if let Some(req) = request.downcast_ref::<webkit6::UserMediaPermissionRequest>() {
                     let is_audio = req.is_for_audio_device();
                     let is_video = req.is_for_video_device();

                     if !is_audio && !is_video {
                         return false;
                     }

                     // Determine permission key and dialog text
                     let (perm_key, title, body) = if is_video {
                         if is_audio {
                             ("camera",
                              gettext("WhatsApp Web Camera & Microphone Permission"),
                              gettext("WhatsApp Web wants to access your camera and microphone. Would you like to allow access?"))
                         } else {
                             ("camera",
                              gettext("WhatsApp Web Camera Permission"),
                              gettext("WhatsApp Web wants to access your camera. Would you like to allow access?"))
                         }
                     } else {
                         ("microphone",
                          gettext("WhatsApp Web Microphone Permission"),
                          gettext("WhatsApp Web wants to access your microphone. Would you like to allow access?"))
                     };

                     if let Some(window) = window_weak.upgrade() {
                         if let Some(mgr) = get_mgr(&window) {
                             let (asked, granted) = mgr.get_account_permission(&account_id, perm_key);

                             if asked {
                                 if granted { req.allow(); } else { req.deny(); }
                                 return true;
                             }

                             // Show Dialog
                             let dialog = adw::AlertDialog::builder()
                                 .heading(&title)
                                 .body(&body)
                                 .default_response("allow")
                                 .close_response("deny")
                                 .build();

                             dialog.add_response("deny", &gettext("Deny"));
                             dialog.add_response("allow", &gettext("Allow"));
                             dialog.set_response_appearance("allow", adw::ResponseAppearance::Suggested);
                             
                             let req_clone = request.clone();
                             let account_id_d = account_id.clone();
                             let perm_key_owned = perm_key.to_string();
                             let window_weak_d = window.downgrade();
                             dialog.choose(Some(&window), gio::Cancellable::NONE, move |response| {
                                 let granted = response == "allow";
                                 if let Some(w) = window_weak_d.upgrade() {
                                     if let Some(m) = get_mgr(&w) {
                                         let _ = m.set_account_permission(&account_id_d, &perm_key_owned, true, granted);
                                         // If granting camera+audio, implicitly grant microphone too
                                         if granted && is_video && is_audio {
                                             let _ = m.set_account_permission(&account_id_d, "microphone", true, true);
                                         }
                                     }
                                 }
                                 if granted { req_clone.allow(); } else { req_clone.deny(); }
                             });
                             return true;
                         }
                     }
                }
                
                false
            });


            // Handle Show Notification signal (Bridge to Desktop)
            let settings_notify_msg = settings.clone(); // Clone for closure
            let window_weak = obj.downgrade();
            let account_id_notify_capture = account_id.to_string();
            web_view.connect_show_notification(move |_, notification| {
                
                // 1. Check Master Toggle
                if !settings_notify_msg.boolean("notifications-enabled") {
                    return true; // Suppress
                }
                
                // 2. Check Message Notifications Toggle
                if !settings_notify_msg.boolean("notify-messages") {
                    return true; // Suppress
                }

                if let Some(window) = window_weak.upgrade() {
                    if let Some(app) = window.application() {
                        // 3. Tray Icon Update (Check Toggle)
                        if settings_notify_msg.boolean("notify-tray-icon") {
                             app.activate_action("set-unread", Some(&true.to_variant()));
                        }

                        let title = notification.title().unwrap_or_else(|| glib::GString::from(gettext("WhatsApp")));
                        let mut body_text = notification.body().unwrap_or_else(|| glib::GString::from(gettext("New message"))).to_string();
                        
                        // 4. Message Preview Settings
                        let show_preview = settings_notify_msg.boolean("notify-preview-enabled");
                        if !show_preview {
                            body_text = gettext("New message received");
                        } else {
                            // Check Limit
                            let limit_enabled = settings_notify_msg.boolean("notify-preview-limit-enabled");
                            if limit_enabled {
                                let max_len = settings_notify_msg.int("notify-preview-length") as usize;
                                if body_text.chars().count() > max_len {
                                    body_text = body_text.chars().take(max_len).collect::<String>();
                                    body_text.push_str("...");
                                }
                            }
                        }
                        
                        // Use ashpd for Portal Notifications
                        let title_clone = title.clone();
                        let body_clone = body_text.clone();

                        // Tag notification with account ID for routing
                        // Tag notification with account ID for routing (FIX: Use captured ID, not active)
                        let account_id_notify = account_id_notify_capture.clone();

                        // Retrieve account info for icon generation
                        let (emoji, account_color, account_count) = if let Some(mgr) = window.imp().account_manager.borrow().as_ref() {
                            let count = mgr.get_account_count();
                            if let Ok(accounts) = mgr.get_accounts() {
                                if let Some(acct) = accounts.iter().find(|a| a.id == account_id_notify) {
                                    (acct.emoji.clone(), acct.color.clone(), count)
                                } else {
                                    ("💬".to_string(), DEFAULT_COLOR.to_string(), count)
                                }
                            } else { ("💬".to_string(), DEFAULT_COLOR.to_string(), count) }
                        } else { ("💬".to_string(), DEFAULT_COLOR.to_string(), 1) };

                        let display_title = title_clone.to_string();

                        // Create compound notification ID: account_id:original_tag
                        let original_tag = if let Some(tag) = notification.tag() {
                              tag.to_string()
                         } else {
                              format!("msg-{}", glib::monotonic_time())
                         };
                        let notification_id = format!("{}:{}", account_id_notify, original_tag);
                        // Unique portal ID so multiple notifications from the same
                        // chat stack instead of replacing each other.
                        let portal_id = format!("{}-{}", notification_id, glib::monotonic_time());

                        // Set account as having unread messages
                        if let Some(mgr) = window.imp().account_manager.borrow().as_ref() {
                            let _ = mgr.set_account_unread(&account_id_notify, true);
                        }

                        let notification_id_action = notification_id.clone();

                        // Get the proxy from the window impl (shared)
                        let proxy_cell = if let Some(window) = window_weak.upgrade() {
                             window.imp().notification_proxy.clone()
                        } else {
                             // Should not happen if we are here
                             return true;
                        };

                        let portal_id_track = portal_id.clone();

                        glib::MainContext::default().spawn_local(async move {
                            // Enter the tokio runtime context
                            let _guard = crate::RUNTIME.enter();

                            // Initialize Proxy if needed (Singleton pattern per window)
                            let proxy = match proxy_cell.get_or_try_init(|| async {
                                ashpd::desktop::notification::NotificationProxy::new().await
                                    .map_err(|e| {
                                        eprintln!("ERROR: Failed to create notification proxy: {}", e);
                                        e
                                    })
                            }).await {
                                Ok(p) => p,
                                Err(_) => return,
                            };

                            let app_id = std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string());

                            let mut notif = ashpd::desktop::notification::Notification::new(&display_title)
                                .body(body_clone.as_str())
                                .default_action("app.notification-clicked")
                                .default_action_target(notification_id_action.as_str())
                                .priority(ashpd::desktop::notification::Priority::Normal);

                            // Icon: colored circle + emoji for multi-account, app icon for single
                            if account_count > 1 {
                                if let Some(bytes) = create_account_icon_bytes(&account_color, &emoji, 64) {
                                    notif = notif.icon(ashpd::desktop::Icon::Bytes(bytes));
                                } else {
                                    notif = notif.icon(ashpd::desktop::Icon::with_names(&[&app_id]));
                                }
                            } else {
                                notif = notif.icon(ashpd::desktop::Icon::with_names(&[&app_id]));
                            }

                            if let Err(_e) = proxy.add_notification(&portal_id, notif).await {
                                // eprintln!("Failed to send portal notification: {}", e);
                            }
                        });

                        // Store notification for click handling.
                        // We intentionally do NOT remove on close — WhatsApp Web
                        // auto-closes notifications on the active tab, but we still
                        // need the reference so notification.clicked() can be attempted.
                        // Entries are naturally replaced when a new notification arrives
                        // with the same account:tag key.
                        if let Some(window) = window_weak.upgrade() {
                             window.imp().active_notifications.borrow_mut().insert(notification_id.clone(), notification.clone());
                             let mut ids = window.imp().portal_notification_ids.borrow_mut();
                             ids.push(portal_id_track);
                             let len = ids.len();
                             if len > 50 {
                                 ids.drain(..len - 50);
                             }
                        }
                        
                        // 5. Play Custom Sound (if enabled)
                        // Valid DND Check for GNOME
                        // 5. Play Custom Sound (if enabled)
                        // Valid DND Check for GNOME
                        let dnd_enabled = {
                             if let Some(source) = gio::SettingsSchemaSource::default() {
                                 if let Some(_schema) = source.lookup("org.gnome.desktop.notifications", true) {
                                      // Schema exists (likely GNOME or compatible)
                                      let settings = gio::Settings::new("org.gnome.desktop.notifications");
                                      // show-banners = false means DND is Likely ON (or just banners hidden, which implies quiet)
                                      !settings.boolean("show-banners")
                                 } else {
                                      false
                                 }
                             } else {
                                  false
                             }
                        };

                        if !dnd_enabled && settings_notify_msg.boolean("notify-sound-enabled") {
                            let sound_key = settings_notify_msg.string("notify-sound-file");
                            let sound_name = match sound_key.as_str() {
                                "pop" => "pop",
                                "alert" => "alert",
                                "soft" => "soft",
                                "start" => "start",
                                _ => "whatsapp",
                            };
                            
                            // Reuse spawn_local logic or just spawn separate one?
                            // Let's spawn separate local task to keep it async and clean
                            glib::MainContext::default().spawn_local(async move {
                                let resource_path = format!("/io/github/tobagin/karere/sounds/{}.oga", sound_name);
                                let temp_path = glib::user_runtime_dir().join("karere-notify.oga");
                                
                                // Only write if not exists or size is 0 (simple cache)
                                let needs_write = !temp_path.exists();
                                
                                if needs_write {
                                    if let Ok(bytes) = gio::resources_lookup_data(&resource_path, gio::ResourceLookupFlags::NONE) {
                                        let _ = tokio::fs::write(&temp_path, &bytes).await;
                                    }
                                }
                                
                                if temp_path.exists() {
                                    match tokio::process::Command::new("paplay")
                                        .arg(temp_path)
                                        .status() // awaits completion
                                        .await 
                                    {
                                        Ok(status) => {
                                            if !status.success() {
                                                eprintln!("paplay failed");
                                            }
                                        },
                                        Err(e) => eprintln!("Failed to run paplay: {}", e),
                                    }
                                }
                            });
                        }

                        return true;
                    }
                }
                false
            });


            // 5. Input Handling (Paste & Middle-click)
            
            // Image & File Paste (Ctrl+V)
            // WhatsApp Web often struggles with direct pasting from Linux/GDK clipboard in WebKit for images and files.
            // We manually detect them, encode, and inject synthetic Paste events.
            let key_controller = gtk::EventControllerKey::new();
            let webview_paste = web_view.clone();
            key_controller.connect_key_pressed(move |_, keyval, _keycode, state| {
                if state.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                    // COPY Fallback (Ctrl+C)
                    if keyval == gtk::gdk::Key::c || keyval == gtk::gdk::Key::C {
                         // Explicitly trigger copy standard behavior just in case
                         let webview = webview_paste.clone();
                         webview.evaluate_javascript("document.execCommand('copy');", None, None, Option::<&gio::Cancellable>::None, |_| {});
                         return glib::Propagation::Proceed;
                    }

                    // PASTE (Ctrl+V)
                    if keyval == gtk::gdk::Key::v || keyval == gtk::gdk::Key::V {
                         let clipboard = gtk::gdk::Display::default().and_then(|d| Some(d.clipboard()));
                         if let Some(clipboard) = clipboard {
                             let formats = clipboard.formats();
                             
                             // 1. Textures (Images)
                             if formats.contains_type(gtk::gdk::Texture::static_type()) {
                                 let webview = webview_paste.clone();
                                 clipboard.read_texture_async(gio::Cancellable::NONE, move |res: Result<Option<gtk::gdk::Texture>, glib::Error>| {
                                     if let Ok(Some(texture)) = res {
                                          let bytes = texture.save_to_png_bytes();
                                          let b64 = BASE64_STANDARD.encode(bytes.as_ref());
                                          let js = format!(r#"
                                              (function() {{
                                                  try {{
                                                      console.log("Karere: Injecting Image Paste...");
                                                      const b64 = "{}";
                                                      const byteCharacters = atob(b64);
                                                      const byteNumbers = new Array(byteCharacters.length);
                                                      for (let i = 0; i < byteCharacters.length; i++) {{
                                                          byteNumbers[i] = byteCharacters.charCodeAt(i);
                                                      }}
                                                      const byteArray = new Uint8Array(byteNumbers);
                                                      const blob = new Blob([byteArray], {{type: 'image/png'}});
                                                      const file = new File([blob], "paste.png", {{type: 'image/png', lastModified: new Date().getTime()}});
                                                      
                                                      const dataTransfer = new DataTransfer();
                                                      dataTransfer.items.add(file);
                                                      
                                                      const pasteEvent = new ClipboardEvent('paste', {{
                                                          bubbles: true,
                                                          cancelable: true,
                                                          clipboardData: dataTransfer
                                                      }});
                                                      
                                                      document.activeElement.dispatchEvent(pasteEvent);
                                                      console.log("Karere: Image Paste Dispatched.");
                                                  }} catch (e) {{
                                                     console.error("Karere Paste Injection Error:", e);
                                                  }}
                                              }})();
                                          "#, b64);
                                          webview.evaluate_javascript(&js, None, None, Option::<&gio::Cancellable>::None, |_| {});
                                     }
                                 });
                                 return glib::Propagation::Stop;
                             }
                             // 2. Files (FileList)
                             else if formats.contains_type(gtk::gdk::FileList::static_type()) {
                                 let webview = webview_paste.clone();
                                 clipboard.read_value_async(gtk::gdk::FileList::static_type(), glib::Priority::default(), gio::Cancellable::NONE, move |res| {
                                     if let Ok(value) = res {
                                         if let Ok(file_list) = value.get::<gtk::gdk::FileList>() {
                                             let files = file_list.files();
                                             
                                             for file in files {
                                                 let webview_clone = webview.clone();
                                                 let file_clone = file.clone();
                                                 file.load_contents_async(gio::Cancellable::NONE, move |res| {
                                                     match res {
                                                         Ok((bytes, _etag)) => {
                                                             let b64 = BASE64_STANDARD.encode(&bytes);
                                                             let filename = file_clone.basename().unwrap_or_else(|| std::path::Path::new("unknown").into()).to_string_lossy().to_string();
                                                             
                                                             // Basic Mime Guessing
                                                             let mime = if filename.ends_with(".png") { "image/png" }
                                                                        else if filename.ends_with(".jpg") || filename.ends_with(".jpeg") { "image/jpeg" }
                                                                        else if filename.ends_with(".pdf") { "application/pdf" }
                                                                        else if filename.ends_with(".mp4") { "video/mp4" }
                                                                        else { "application/octet-stream" };


                                                             // Escape filename and mime for safe JS string interpolation
                                                             let safe_filename = serde_json::Value::String(filename.clone()).to_string();
                                                             let safe_filename = &safe_filename[1..safe_filename.len()-1];
                                                             let safe_mime = serde_json::Value::String(mime.to_string()).to_string();
                                                             let safe_mime = &safe_mime[1..safe_mime.len()-1];

                                                             let js = format!(r#"
                                                                 (function() {{
                                                                     try {{
                                                                         console.log("Karere: Injecting File Paste: {}");
                                                                         const b64 = "{}";
                                                                         const byteCharacters = atob(b64);
                                                                         const byteNumbers = new Array(byteCharacters.length);
                                                                         for (let i = 0; i < byteCharacters.length; i++) {{
                                                                             byteNumbers[i] = byteCharacters.charCodeAt(i);
                                                                         }}
                                                                         const byteArray = new Uint8Array(byteNumbers);
                                                                         const blob = new Blob([byteArray], {{type: '{}'}});
                                                                         const file = new File([blob], "{}", {{type: '{}', lastModified: new Date().getTime()}});

                                                                         const dataTransfer = new DataTransfer();
                                                                         dataTransfer.items.add(file);

                                                                         const pasteEvent = new ClipboardEvent('paste', {{
                                                                             bubbles: true,
                                                                             cancelable: true,
                                                                             clipboardData: dataTransfer
                                                                         }});

                                                                         document.activeElement.dispatchEvent(pasteEvent);
                                                                     }} catch (e) {{
                                                                         console.error("Karere File Paste Error:", e);
                                                                     }}
                                                                 }})();
                                                             "#, safe_filename, b64, safe_mime, safe_filename, safe_mime);
                                                             webview_clone.evaluate_javascript(&js, None, None, Option::<&gio::Cancellable>::None, |_| {});
                                                         },
                                                         Err(e) => eprintln!("ERROR: Failed to read clipboard file: {}", e),
                                                     }
                                                 });
                                             }
                                         }
                                     }
                                 });
                                 return glib::Propagation::Stop;
                             }
                         }
                    }
                }
                glib::Propagation::Proceed
            });
            web_view.add_controller(key_controller);

            // Middle Click Paste (Primary Selection)
            let gesture_click = gtk::GestureClick::new();
            gesture_click.set_button(2); // Middle Mouse Button
            gesture_click.set_propagation_phase(gtk::PropagationPhase::Capture);
            let webview_mid = web_view.clone();
            gesture_click.connect_pressed(move |gesture, _, _, _| {
                 // Claim the gesture to stop propagation to WebView's default handler
                 gesture.set_state(gtk::EventSequenceState::Claimed);
                 
                 let clipboard = gtk::gdk::Display::default().and_then(|d| Some(d.primary_clipboard()));
                 if let Some(clipboard) = clipboard {
                     let webview = webview_mid.clone();
                     clipboard.read_text_async(gio::Cancellable::NONE, move |res: Result<Option<glib::GString>, glib::Error>| {
                         match res {
                             Ok(Some(text)) => {
                                 // Escape for JS string
                                 let safe_text = text.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n").replace("\r", "");
                                 let js = format!(r#"document.execCommand("insertText", false, "{}");"#, safe_text);
                                 webview.evaluate_javascript(&js, None, None, Option::<&gio::Cancellable>::None, |_| {});
                             },
                             Ok(None) => {},
                             Err(_) => {},
                         }
                     });
                 }
            });
            web_view.add_controller(gesture_click);

            // Drag and Drop (Files)
            // Explicitly handle file drops to bypass some Flatpak/WebView disconnects or just to provide unified injection.
            let drop_target = gtk::DropTarget::new(gtk::gdk::FileList::static_type(), gtk::gdk::DragAction::COPY);
            let webview_drop = web_view.clone();
            
            drop_target.connect_drop(move |_target, value, _x, _y| {
                if let Ok(file_list) = value.get::<gtk::gdk::FileList>() {
                     for file in file_list.files() {
                         let webview = webview_drop.clone();
                         let file_clone = file.clone();
                         file.load_contents_async(gio::Cancellable::NONE, move |res| {
                             match res {
                                 Ok((bytes, _etag)) => {
                                     let b64 = BASE64_STANDARD.encode(&bytes);
                                     let filename = file_clone.basename().unwrap_or_else(|| std::path::Path::new("dropped_file").into()).to_string_lossy().to_string();
                                     
                                     // Basic Mime Guessing (Same as Paste)
                                     let mime = if filename.ends_with(".png") { "image/png" }
                                                else if filename.ends_with(".jpg") || filename.ends_with(".jpeg") { "image/jpeg" }
                                                else if filename.ends_with(".pdf") { "application/pdf" }
                                                else if filename.ends_with(".mp4") { "video/mp4" }
                                                else { "application/octet-stream" };


                                     // Escape filename and mime for safe JS string interpolation
                                     let safe_filename = serde_json::Value::String(filename.clone()).to_string();
                                     let safe_filename = &safe_filename[1..safe_filename.len()-1];
                                     let safe_mime = serde_json::Value::String(mime.to_string()).to_string();
                                     let safe_mime = &safe_mime[1..safe_mime.len()-1];

                                     // Reuse the same JS injection strategy as Paste
                                     let js = format!(r#"
                                         (function() {{
                                             try {{
                                                 console.log("Karere: Injecting Dropped File: {}");
                                                 const b64 = "{}";
                                                 const byteCharacters = atob(b64);
                                                 const byteNumbers = new Array(byteCharacters.length);
                                                 for (let i = 0; i < byteCharacters.length; i++) {{
                                                     byteNumbers[i] = byteCharacters.charCodeAt(i);
                                                 }}
                                                 const byteArray = new Uint8Array(byteNumbers);
                                                 const blob = new Blob([byteArray], {{type: '{}'}});
                                                 const file = new File([blob], "{}", {{type: '{}', lastModified: new Date().getTime()}});

                                                 const dataTransfer = new DataTransfer();
                                                 dataTransfer.items.add(file);

                                                 // Use 'paste' event injection for attachments (proven to work)
                                                 const pasteEvent = new ClipboardEvent('paste', {{
                                                     bubbles: true,
                                                     cancelable: true,
                                                     clipboardData: dataTransfer
                                                 }});
                                                 document.activeElement.dispatchEvent(pasteEvent);

                                             }} catch (e) {{
                                                 console.error("Karere Drop Injection Error:", e);
                                             }}
                                         }})();
                                     "#, safe_filename, b64, safe_mime, safe_filename, safe_mime);
                                     webview.evaluate_javascript(&js, None, None, Option::<&gio::Cancellable>::None, |_| {});
                                 },
                                 Err(e) => eprintln!("ERROR: Failed to read dropped file: {}", e),
                             }
                         });
                     }
                     return true;
                }
                false
            });
            web_view.add_controller(drop_target);

            
            // 11. Setup Accessibility & Auto-Correct
            self.setup_accessibility(&web_view, settings.clone());

            // Per-account zoom level setup
            {
                let mut zoom = if let Some(mgr) = self.account_manager.borrow().as_ref() {
                    mgr.get_account_zoom(account_id)
                } else {
                    1.0
                };

                // Enforce minimum zoom floor if accessibility zoom is enabled
                if settings.boolean("webview-zoom") {
                    let floor = settings.double("zoom-level");
                    if zoom < floor {
                        zoom = floor;
                    }
                }

                if zoom > 0.0 {
                    web_view.set_zoom_level(zoom);
                }

                // Save per-account zoom on change, enforcing floor
                let obj_weak_zoom = obj.downgrade();
                web_view.connect_zoom_level_notify(move |webview| {
                    if let Some(window) = obj_weak_zoom.upgrade() {
                        let level = webview.zoom_level();
                        if let Some(id) = window.imp().active_account_id.borrow().clone() {
                            if let Some(mgr) = window.imp().account_manager.borrow().as_ref() {
                                let _ = mgr.set_account_zoom(&id, level);
                            }
                        }
                    }
                });
            }

            // Auto-restore notification permission and mobile layout on page load
            let settings_mobile_layout = settings.clone();
            let obj_weak_load = obj.downgrade();
            let account_id_load = account_id.to_string();

            web_view.connect_load_changed(move |webview, event| {
                if event == webkit6::LoadEvent::Finished {
                    // Reset transitioning flag unconditionally — the reload is done
                    // regardless of whether JS injection succeeds or fires its callback.
                    if let Some(window) = obj_weak_load.upgrade() {
                        window.imp().mobile_layout_transitioning.set(false);
                    }

                    // Get window width if available
                    let window_width = if let Some(window) = obj_weak_load.upgrade() {
                        window.width()
                    } else {
                        0
                    };

                    if should_use_mobile_layout(&settings_mobile_layout, window_width) {
                        let js_content = include_str!("mobile_responsive.js");
                        webview.evaluate_javascript(
                            &js_content,
                            None,
                            None,
                            Option::<&gio::Cancellable>::None,
                            move |result| {
                                if let Err(e) = result {
                                    eprintln!("ERROR: Failed to inject mobile_responsive.js: {}", e);
                                }
                            },
                        );
                        // Update state
                        if let Some(window) = obj_weak_load.upgrade() {
                            window.imp().mobile_layout_active.set(true);
                        }
                    } else {
                        // Update state
                        if let Some(window) = obj_weak_load.upgrade() {
                            window.imp().mobile_layout_active.set(false);
                        }
                    }

                    // Auto-restore notification permission if previously granted
                    // This prevents WhatsApp from re-prompting on every startup
                    let should_auto_grant = if let Some(window) = obj_weak_load.upgrade() {
                        if let Some(mgr) = window.imp().account_manager.borrow().as_ref() {
                            let (asked, granted) = mgr.get_account_permission(&account_id_load, "notification");
                            asked && granted
                        } else { false }
                    } else { false };

                    if should_auto_grant {
                        // Override Notification.permission to 'granted' first,
                        // then call requestPermission() in the callback to avoid race condition
                        let webview_perm = webview.clone();
                        webview.evaluate_javascript(
                            "if (typeof Notification !== 'undefined') { Object.defineProperty(Notification, 'permission', { value: 'granted', writable: false, configurable: true }); }",
                            None,
                            None,
                            Option::<&gio::Cancellable>::None,
                            move |_| {
                                webview_perm.evaluate_javascript(
                                    "Notification.requestPermission()",
                                    None,
                                    None,
                                    Option::<&gio::Cancellable>::None,
                                    |_| {}
                                );
                            }
                        );
                    } else {
                        webview.evaluate_javascript(
                            "Notification.requestPermission()",
                            None,
                            None,
                            Option::<&gio::Cancellable>::None,
                            |_| {}
                        );
                    }
                }
            });

            // Track unread counts from page title changes
            // WhatsApp Web sets titles like "(3) WhatsApp" when there are unread messages
            let account_id_title = account_id.to_string();
            let window_weak_title = obj.downgrade();
            web_view.connect_notify_local(Some("title"), move |webview, _| {
                let title = webview.title().unwrap_or_default().to_string();
                let has_unread = title.starts_with('(');

                if let Some(window) = window_weak_title.upgrade() {
                    if let Some(mgr) = window.imp().account_manager.borrow().as_ref() {
                        let _ = mgr.set_account_unread(&account_id_title, has_unread);
                    }
                    if let Some(app) = window.application() {
                        app.activate_action("set-unread", Some(&has_unread.to_variant()));
                    }
                    window.imp().update_account_button();
                }
            });

            // Handle Navigation Policy (External Links)
            web_view.connect_decide_policy(move |_, decision, decision_type| {
                match decision_type {
                     webkit6::PolicyDecisionType::NavigationAction | webkit6::PolicyDecisionType::NewWindowAction => {
                         if let Some(mut nav_action) = decision.downcast_ref::<webkit6::NavigationPolicyDecision>().and_then(|d| d.navigation_action()) {
                             if let Some(req) = nav_action.request() {
                                 if let Some(uri) = req.uri() {
                                     let uri_str = uri.as_str();

                                     // Enhanced V1-like Logic
                                     let is_internal = uri_str.contains("web.whatsapp.com") ||
                                                       uri_str.contains("whatsapp.com") ||
                                                       uri_str.contains("whatsapp.net") ||
                                                       uri_str.starts_with("data:") ||
                                                       uri_str.starts_with("blob:") ||
                                                       uri_str.starts_with("about:");

                                     if !is_internal {

                                         // Use generic GLib/GIO launcher which works via portal in Flatpak
                                         // This matches V1 logic (AppInfo.launch_default_for_uri)
                                         let uri_owned = uri_str.to_string();
                                         match gio::AppInfo::launch_default_for_uri(&uri_owned, Option::<&gio::AppLaunchContext>::None) {
                                             Ok(_) => {},
                                             Err(e) => eprintln!("WARNING: Failed to launch external URI: {}", e),
                                         }

                                         decision.ignore();
                                         return true;
                                     }
                                 }
                             }
                         }
                     }
                     _ => {}
                }
                false
            });

            web_view.load_uri("https://web.whatsapp.com");
            web_view
        }

        fn setup_accessibility(&self, web_view: &webkit6::WebView, settings: gio::Settings) {
             let obj = self.obj();
             
             // 1. High Contrast
             // AdwStyleManager::high-contrast is read-only, we cannot bind TO it.
             // Libadwaita automatically handles system high-contrast settings.
             // If manual override is needed, it requires a different approach, but usually not recommended.
             // let style_manager = adw::StyleManager::default();
             // settings.bind("high-contrast", &style_manager, "high-contrast").build();

             // 2. Reduce Motion
             let settings_anim = settings.clone();
             let update_anim = move |settings: &gio::Settings| {
                 if let Some(gtk_settings) = gtk::Settings::default() {
                     let reduce = settings.boolean("reduce-motion");
                     gtk_settings.set_gtk_enable_animations(!reduce);
                 }
             };
             update_anim(&settings_anim);
             settings.connect_changed(Some("reduce-motion"), move |s, _| update_anim(s));

             // 3. Header bar zoom controls visibility
             settings.bind("zoom-controls-headerbar", &*self.zoom_box, "visible").build();
             
             // 4. Screen Reader Support
             if let Some(webview_settings) = webkit6::prelude::WebViewExt::settings(web_view) {
                 settings.bind("screen-reader-opts", &webview_settings, "enable-caret-browsing").build();
             }
             
             // 5. Focus Indicators
             let provider = gtk::CssProvider::new();
             gtk::style_context_add_provider_for_display(
                 &gtk::gdk::Display::default().unwrap(),
                 &provider,
                 gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
             );
             let update_focus = move |settings: &gio::Settings| {
                 if settings.boolean("focus-indicators") {
                     provider.load_from_string(":focus { outline-width: 2px; outline-style: solid; outline-color: alpha(currentColor, 0.7); }");
                 } else {
                     provider.load_from_string("");
                 }
             };
             let uf = update_focus.clone();
             settings.connect_changed(Some("focus-indicators"), move |s, _| uf(s));
             update_focus(&settings);

             // 6. Spell Checking & Auto-Correct
             if let Some(context) = web_view.context() {
                  let update_spell = move |settings: &gio::Settings, _: &str| {
                      let enabled = settings.boolean("enable-spell-checking");
                      context.set_spell_checking_enabled(enabled);
                      
                      if enabled {
                          let auto = settings.boolean("auto-detect-language");
                          if !auto {
                               let configured = settings.strv("spell-checking-languages");
                               let refs: Vec<&str> = configured.iter().map(|s| s.as_str()).collect();
                               context.set_spell_checking_languages(&refs);
                          } else {
                               context.set_spell_checking_languages(&[]); 
                          }
                      }
                  };
                  
                  let us = update_spell.clone();
                  settings.connect_changed(Some("enable-spell-checking"), move |s, k| us(s, k));
                  let us = update_spell.clone();
                  settings.connect_changed(Some("auto-detect-language"), move |s, k| us(s, k));
                  let us = update_spell.clone();
                  settings.connect_changed(Some("spell-checking-languages"), move |s, k| us(s, k));
                   let us = update_spell.clone();
                  settings.connect_changed(Some("enable-auto-correct"), move |s, k| us(s, k));
                  
                  update_spell(&settings, "initial");
             }
             
             // 7. Zoom Controls Actions (use dynamic webview ref)
             // Ctrl++/- always work. The header bar buttons also trigger these.
             // Zoom out is clamped to the accessibility floor when enabled.
             let obj_weak_z = obj.downgrade();
             let action_zoom_in = gio::SimpleAction::new("zoom-in", None);
             action_zoom_in.connect_activate(move |_, _| {
                 if let Some(obj) = obj_weak_z.upgrade() {
                     if let Some(webview) = obj.imp().webview() {
                         let level = webview.zoom_level();
                         webview.set_zoom_level(level + 0.1);
                     }
                 }
             });
             obj.add_action(&action_zoom_in);

             let obj_weak_z = obj.downgrade();
             let settings_zoom_out = settings.clone();
             let action_zoom_out = gio::SimpleAction::new("zoom-out", None);
             action_zoom_out.connect_activate(move |_, _| {
                 if let Some(obj) = obj_weak_z.upgrade() {
                     if let Some(webview) = obj.imp().webview() {
                         let level = webview.zoom_level();
                         let floor = if settings_zoom_out.boolean("webview-zoom") {
                             settings_zoom_out.double("zoom-level")
                         } else {
                             0.2
                         };
                         let new_level = level - 0.1;
                         if new_level >= floor {
                             webview.set_zoom_level(new_level);
                         }
                     }
                 }
             });
             obj.add_action(&action_zoom_out);

             let obj_weak_z = obj.downgrade();
             let settings_zoom_reset = settings.clone();
             let action_zoom_reset = gio::SimpleAction::new("zoom-reset", None);
             action_zoom_reset.connect_activate(move |_, _| {
                 if let Some(obj) = obj_weak_z.upgrade() {
                     if let Some(webview) = obj.imp().webview() {
                         // Reset to floor if accessibility zoom is enabled, otherwise 1.0
                         let reset_to = if settings_zoom_reset.boolean("webview-zoom") {
                             settings_zoom_reset.double("zoom-level")
                         } else {
                             1.0
                         };
                         webview.set_zoom_level(reset_to);
                     }
                 }
             });
             obj.add_action(&action_zoom_reset);
        }

        pub fn update_account_button(&self) {
            if self.updating_accounts.replace(true) {
                return;
            }

            self.reset_account_button_default();
            self.clear_account_list();

            let app_id = std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string());
            let settings = gio::Settings::new(&app_id);
            let limit_enabled = settings.boolean("account-limit-enabled");
            let limit = settings.int("account-limit") as usize;

            if let Some(mgr) = self.account_manager.borrow().as_ref() {
                if let Ok(accounts) = mgr.get_accounts_sorted() {
                    // If limit is enabled, only show accounts up to the limit in the UI
                    let visible_accounts: Vec<_> = if limit_enabled && limit > 0 {
                        accounts.iter().take(limit).cloned().collect()
                    } else {
                        accounts.clone()
                    };
                    self.populate_account_list(&visible_accounts);

                    // Hide "Add account" button when at or over the limit
                    let at_limit = limit_enabled && accounts.len() >= limit;
                    self.add_account_button.set_visible(!at_limit);
                }
                if let Ok(Some(account)) = mgr.get_active_account() {
                    self.set_account_button_active(&account);
                }
            }

            self.updating_accounts.set(false);

            // Keep tray menu in sync with account list
            if let Some(app) = self.obj().application() {
                app.activate_action("refresh-tray-accounts", None);
            }
        }

        fn reset_account_button_default(&self) {
            self.account_avatar.set_custom_image(None::<&gtk::gdk::Paintable>);
            self.account_avatar.set_icon_name(Some("user-info-symbolic"));
            self.account_avatar.set_text(Some(" "));
            self.account_button.set_tooltip_text(Some(&gettext("No account")));
        }

        fn set_account_button_active(&self, account: &Account) {
            self.account_button.set_tooltip_text(Some(&account.name));
            self.account_avatar.set_icon_name(None::<&str>);
            apply_avatar_texture(&self.account_avatar, &account.color, &account.emoji);
        }

        fn clear_account_list(&self) {
            while let Some(row) = self.accounts_list.first_child() {
                self.accounts_list.remove(&row);
            }
        }

        fn populate_account_list(&self, accounts: &[Account]) {
            // For reorder buttons: only non-default accounts can be reordered,
            // so compute first/last among non-default accounts only
            let non_default: Vec<&str> = accounts.iter()
                .filter(|a| a.id != DEFAULT_ACCOUNT_ID)
                .map(|a| a.id.as_str())
                .collect();

            for account in accounts {
                let non_default_idx = non_default.iter().position(|id| *id == account.id);
                let is_first_reorderable = non_default_idx == Some(0);
                let is_last_reorderable = non_default_idx == Some(non_default.len().saturating_sub(1));
                let (row, edit_btn, delete_btn, up_btn, down_btn) = build_account_row(account, is_first_reorderable, is_last_reorderable);

                let account_id_del = account.id.clone();
                let account_name_del = account.name.clone();
                let obj_weak_del = self.obj().downgrade();
                delete_btn.connect_clicked(move |_| {
                    if let Some(obj) = obj_weak_del.upgrade() {
                        obj.confirm_delete_account(&account_id_del, &account_name_del);
                    }
                });

                let account_clone = account.clone();
                let obj_weak_edit = self.obj().downgrade();
                edit_btn.connect_clicked(move |_| {
                    if let Some(obj) = obj_weak_edit.upgrade() {
                        obj.show_account_dialog(Some(&account_clone));
                    }
                });

                // Move up
                let account_id_up = account.id.clone();
                let obj_weak_up = self.obj().downgrade();
                up_btn.connect_clicked(move |_| {
                    if let Some(obj) = obj_weak_up.upgrade() {
                        if let Some(mgr) = obj.imp().account_manager.borrow().as_ref() {
                            let _ = mgr.reorder_account(&account_id_up, -1);
                        }
                        obj.imp().update_account_button();
                    }
                });

                // Move down
                let account_id_down = account.id.clone();
                let obj_weak_down = self.obj().downgrade();
                down_btn.connect_clicked(move |_| {
                    if let Some(obj) = obj_weak_down.upgrade() {
                        if let Some(mgr) = obj.imp().account_manager.borrow().as_ref() {
                            let _ = mgr.reorder_account(&account_id_down, 1);
                        }
                        obj.imp().update_account_button();
                    }
                });

                self.accounts_list.append(&row);
            }
        }

        pub fn switch_to_account(&self, account_id: &str) {
            // Already active — nothing to do
            if self.active_account_id.borrow().as_deref() == Some(account_id) {
                self.account_bottom_sheet.set_open(false);
                return;
            }

            // Hide the current WebView (keep it in the pool)
            if let Some(current_wv) = self.webview() {
                current_wv.set_visible(false);
            }

            // Update active account in manager
            if let Some(mgr) = self.account_manager.borrow().as_ref() {
                let _ = mgr.set_active_account(account_id);
                
                // Update session state for the account we're switching to
                let _ = mgr.update_session_state(account_id);
            }

            // Check if we already have a WebView for this account
            let has_webview = self.webviews.borrow().contains_key(account_id);

            if has_webview {
                // Show the existing WebView and restore per-account zoom level
                if let Some(wv) = self.webviews.borrow().get(account_id) {
                    wv.set_visible(true);
                    let mut zoom = if let Some(mgr) = self.account_manager.borrow().as_ref() {
                        mgr.get_account_zoom(account_id)
                    } else {
                        1.0
                    };
                    // Enforce floor
                    let app_id = std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string());
                    let settings = gio::Settings::new(&app_id);
                    if settings.boolean("webview-zoom") {
                        let floor = settings.double("zoom-level");
                        if zoom < floor { zoom = floor; }
                    }
                    if zoom > 0.0 {
                        wv.set_zoom_level(zoom);
                    }
                }
                *self.active_account_id.borrow_mut() = Some(account_id.to_string());
            } else {
                // Create a new WebView for this account
                let dirs = if let Some(mgr) = self.account_manager.borrow().as_ref() {
                    Some((mgr.data_dir_for(account_id), mgr.cache_dir_for(account_id)))
                } else {
                    None
                };

                if let Some((data_dir, cache_dir)) = dirs {
                    self.setup_webview(account_id, data_dir, cache_dir, true);
                }
            }

            self.update_account_button();
            self.account_bottom_sheet.set_open(false);
        }
    }

    impl WidgetImpl for KarereWindow {}
    impl WindowImpl for KarereWindow {}
    impl ApplicationWindowImpl for KarereWindow {}
    impl AdwApplicationWindowImpl for KarereWindow {}
}


glib::wrapper! {
    pub struct KarereWindow(ObjectSubclass<imp::KarereWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl KarereWindow {
    pub fn new(app: &adw::Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    pub fn force_close(&self) {
        self.imp().force_close.set(true);
        self.close();
    }

    /// Public method for switching accounts, delegates to imp
    pub fn switch_to_account(&self, account_id: &str) {
        self.imp().switch_to_account(account_id);
    }

    pub fn process_notification_click(&self, id: &str) {
        let imp = self.imp();

        // Parse compound notification ID: "account_id:original_tag"
        let (account_id, original_tag) = if let Some((aid, tag)) = id.split_once(':') {
            (aid, tag)
        } else {
            (DEFAULT_ACCOUNT_ID, id)
        };

        println!("Karere: Notification clicked — Account: {}, Tag: {}", account_id, original_tag);

        // Switch to the correct account first
        let current_account = imp.active_account_id.borrow().clone();
        if current_account.as_deref() != Some(account_id) {
            self.switch_to_account(account_id);
        }

        // Fire the WebKit notification's click handler.
        // We inject a Notification.prototype.close no-op so WhatsApp Web can't
        // close notifications — this keeps them alive in WebKit's internal
        // tracking, allowing clicked() to dispatch the event back to the page.
        if let Some(notification) = imp.active_notifications.borrow().get(id) {
            notification.clicked();
        }

        // Clear unread for this account
        if let Some(mgr) = imp.account_manager.borrow().as_ref() {
            let _ = mgr.set_account_unread(account_id, false);
        }
        imp.update_account_button();
    }

    fn show_account_dialog(&self, existing: Option<&Account>) {
        self.imp().account_bottom_sheet.set_open(false);

        let is_edit = existing.is_some();

        // Check account limit (only for new accounts)
        if !is_edit {
            let app_id = std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string());
            let settings = gio::Settings::new(&app_id);
            let limit_enabled = settings.boolean("account-limit-enabled");
            let limit = settings.int("account-limit") as usize;

            if limit_enabled {
                if let Some(mgr) = self.imp().account_manager.borrow().as_ref() {
                    let count = mgr.get_account_count();
                    if count >= limit {
                        let warning = adw::AlertDialog::builder()
                            .heading(&gettext("Account Limit Reached"))
                            .body(&format!(
                                "{} ({}/{}). {}",
                                gettext("You have reached the maximum number of accounts"),
                                count, limit,
                                gettext("You can increase or disable this limit in Preferences → Accounts.")
                            ))
                            .default_response("ok")
                            .close_response("ok")
                            .build();
                        warning.add_response("ok", &gettext("_OK"));
                        warning.present(Some(self));
                        return;
                    }
                }
            }
        }

        let title = if is_edit { gettext("Edit Account") } else { gettext("Add New Account") };
        let btn_label = if is_edit { gettext("Save") } else { gettext("Add Account") };

        let initial_name = existing.map(|a| a.name.clone()).unwrap_or_default();
        let initial_emoji = existing.map(|a| a.emoji.clone()).unwrap_or_else(|| DEFAULT_EMOJI.to_string());
        let initial_color = existing.map(|a| a.color.clone()).unwrap_or_else(|| DEFAULT_COLOR.to_string());
        let account_id = existing.map(|a| a.id.clone());

        // Shared state for selected color and emoji
        let selected_emoji = std::rc::Rc::new(std::cell::RefCell::new(initial_emoji.clone()));
        let selected_color = std::rc::Rc::new(std::cell::RefCell::new(initial_color.clone()));

        // Build a proper Adw.Dialog with header bar
        let dialog = adw::Dialog::builder()
            .title(&title)
            .content_width(360)
            .content_height(-1)
            .build();

        // Action button (goes in bottom bar)
        let action_btn = gtk::Button::with_label(&btn_label);
        action_btn.add_css_class("suggested-action");
        action_btn.add_css_class("pill");
        action_btn.set_sensitive(!initial_name.is_empty());

        // Top header bar (with close button)
        let header = adw::HeaderBar::new();

        // Bottom header bar (with action button as title widget, no window buttons)
        let bottom_bar = adw::HeaderBar::new();
        bottom_bar.set_show_start_title_buttons(false);
        bottom_bar.set_show_end_title_buttons(false);
        bottom_bar.set_title_widget(Some(&action_btn));

        // ToolbarView wrapping header + content + bottom bar
        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.add_bottom_bar(&bottom_bar);

        // Content
        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.set_margin_start(24);
        content.set_margin_end(24);
        content.set_margin_top(12);
        content.set_margin_bottom(12);

        // === Live Avatar Preview (PNG texture) ===
        let preview_avatar = adw::Avatar::builder()
            .size(96)
            .halign(gtk::Align::Center)
            .build();
        apply_avatar_texture(&preview_avatar, &initial_color, &initial_emoji);
        content.append(&preview_avatar);

        // Shared closure to update the preview avatar
        let preview_avatar_update = preview_avatar.clone();
        let color_for_preview = selected_color.clone();
        let emoji_for_preview = selected_emoji.clone();
        let update_preview = std::rc::Rc::new(move || {
            let color = color_for_preview.borrow().clone();
            let emoji = emoji_for_preview.borrow().clone();
            apply_avatar_texture(&preview_avatar_update, &color, &emoji);
        });

        // === Preferences Group with proper rows ===
        let group = adw::PreferencesGroup::new();

        // --- Account Name EntryRow ---
        let name_row = adw::EntryRow::builder()
            .title(&gettext("Account Name"))
            .text(&initial_name)
            .build();
        name_row.connect_map(|e| {
            e.grab_focus();
        });

        // Enable button only when name is non-empty
        let action_btn_weak = action_btn.downgrade();
        name_row.connect_changed(move |entry| {
            if let Some(btn) = action_btn_weak.upgrade() {
                btn.set_sensitive(!entry.text().is_empty());
            }
        });

        group.add(&name_row);

        // --- Emoji ActionRow with EmojiChooser ---
        let emoji_label = gtk::Label::new(Some(&initial_emoji));
        emoji_label.add_css_class("title-2");

        let emoji_chooser = gtk::EmojiChooser::new();
        let emoji_button = gtk::MenuButton::builder()
            .popover(&emoji_chooser)
            .valign(gtk::Align::Center)
            .child(&emoji_label)
            .build();
        emoji_button.add_css_class("flat");

        let emoji_row = adw::ActionRow::builder()
            .title(&gettext("Emoji"))
            .activatable_widget(&emoji_button)
            .build();
        emoji_row.add_suffix(&emoji_button);

        // Connect emoji picked signal
        let selected_emoji_e = selected_emoji.clone();
        let emoji_label_e = emoji_label.clone();
        let update_preview_emoji = update_preview.clone();
        emoji_chooser.connect_emoji_picked(move |_, emoji_str| {
            let emoji = emoji_str.to_string();
            *selected_emoji_e.borrow_mut() = emoji.clone();
            emoji_label_e.set_label(&emoji);
            update_preview_emoji();
        });

        group.add(&emoji_row);

        // --- Color ActionRow with ColorDialogButton ---
        let color_dialog = gtk::ColorDialog::builder()
            .title(&gettext("Choose Avatar Color"))
            .with_alpha(false)
            .build();

        let initial_rgba = gtk::gdk::RGBA::parse(&initial_color).unwrap_or_else(|_| {
            gtk::gdk::RGBA::parse(DEFAULT_COLOR).unwrap()
        });

        let color_button = gtk::ColorDialogButton::builder()
            .dialog(&color_dialog)
            .rgba(&initial_rgba)
            .valign(gtk::Align::Center)
            .build();

        let color_row = adw::ActionRow::builder()
            .title(&gettext("Color"))
            .activatable_widget(&color_button)
            .build();
        color_row.add_suffix(&color_button);

        // Connect color change signal
        let selected_color_c = selected_color.clone();
        let update_preview_color = update_preview.clone();
        color_button.connect_rgba_notify(move |btn| {
            let rgba = btn.rgba();
            let hex = format!(
                "#{:02x}{:02x}{:02x}",
                (rgba.red() * 255.0) as u8,
                (rgba.green() * 255.0) as u8,
                (rgba.blue() * 255.0) as u8,
            );
            *selected_color_c.borrow_mut() = hex;
            update_preview_color();
        });

        group.add(&color_row);

        content.append(&group);
        toolbar_view.set_content(Some(&content));
        dialog.set_child(Some(&toolbar_view));

        // Action button handler
        let window_weak = self.downgrade();
        let selected_emoji_final = selected_emoji.clone();
        let selected_color_final = selected_color.clone();
        let dialog_weak = dialog.downgrade();
        let name_row_ref = name_row.clone();
        let account_id_clone = account_id.clone();
        action_btn.connect_clicked(move |_| {
            let name = name_row_ref.text();
            if !name.is_empty() {
                if let Some(window) = window_weak.upgrade() {
                    let emoji = selected_emoji_final.borrow().clone();
                    let color = selected_color_final.borrow().clone();
                    if let Some(ref id) = account_id_clone {
                        // Edit mode: update existing account
                        if let Some(mgr) = window.imp().account_manager.borrow().as_ref() {
                            let _ = mgr.update_account_identity(id, name.as_str(), &emoji, &color);
                        }
                        window.imp().update_account_button();
                    } else {
                        // Add mode: create new account
                        window.create_account(name.as_str(), &color, &emoji);
                    }
                }
            }
            if let Some(d) = dialog_weak.upgrade() {
                d.close();
            }
        });

        // Enter key in name entry triggers action
        let action_btn_enter = action_btn.clone();
        name_row.connect_entry_activated(move |_| {
            if action_btn_enter.is_sensitive() {
                action_btn_enter.emit_clicked();
            }
        });

        dialog.present(Some(self));
    }

    fn show_add_account_dialog(&self) {
        self.show_account_dialog(None);
    }


    fn confirm_delete_account(&self, account_id: &str, account_name: &str) {
        self.imp().account_bottom_sheet.set_open(false);

        let dialog = adw::AlertDialog::builder()
            .heading(&format!("{} \"{}\"?", gettext("Remove account"), account_name))
            .body(&gettext("This action cannot be undone."))
            .default_response("cancel")
            .close_response("cancel")
            .build();

        dialog.add_response("cancel", &gettext("_Cancel"));
        dialog.add_response("remove", &gettext("_Remove"));
        dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);

        let account_id = account_id.to_string();
        let window_weak = self.downgrade();
        dialog.choose(Some(self), gio::Cancellable::NONE, move |response| {
            if response == "remove" {
                if let Some(window) = window_weak.upgrade() {
                    let imp = window.imp();

                    // Remove the deleted account's WebView from pool and container
                    if let Some(wv) = imp.webviews.borrow_mut().remove(&account_id) {
                        imp.view_container.remove(&wv);
                    }

                    // Collect info for the new active account while borrow is held
                    let new_info = if let Some(mgr) = imp.account_manager.borrow().as_ref() {
                        let _ = mgr.remove_account(&account_id);

                        if let Ok(Some(new_active)) = mgr.get_active_account() {
                            Some((new_active.id.clone(), mgr.data_dir_for(&new_active.id), mgr.cache_dir_for(&new_active.id)))
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    // borrow dropped

                    if let Some((new_id, data_dir, cache_dir)) = new_info {
                        // Check if the new active account already has a WebView in the pool
                        let has_webview = imp.webviews.borrow().contains_key(&new_id);
                        if has_webview {
                            if let Some(wv) = imp.webviews.borrow().get(&new_id) {
                                wv.set_visible(true);
                                let mut zoom = if let Some(mgr) = imp.account_manager.borrow().as_ref() {
                                    mgr.get_account_zoom(&new_id)
                                } else {
                                    1.0
                                };
                                let app_id = std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string());
                                let settings = gio::Settings::new(&app_id);
                                if settings.boolean("webview-zoom") {
                                    let floor = settings.double("zoom-level");
                                    if zoom < floor { zoom = floor; }
                                }
                                if zoom > 0.0 {
                                    wv.set_zoom_level(zoom);
                                }
                            }
                            *imp.active_account_id.borrow_mut() = Some(new_id);
                        } else {
                            imp.setup_webview(&new_id, data_dir, cache_dir, true);
                        }
                    } else {
                        *imp.active_account_id.borrow_mut() = None;
                    }

                    imp.update_account_button();
                }
            }
        });
    }

    fn create_account(&self, account_name: &str, color: &str, emoji: &str) {
        let imp = self.imp();

        // Collect dirs and account_id while borrow is held, then drop before setup_webview
        let info = if let Some(mgr) = imp.account_manager.borrow().as_ref() {
            let new_account_id = format!("account_{}", chrono::Local::now().timestamp());
            let account = Account::new(new_account_id.clone(), account_name.to_string(), color.to_string(), emoji.to_string());

            if mgr.add_account(account).is_ok() {
                let _ = mgr.set_active_account(&new_account_id);
                
                // Update session state immediately after creation
                let _ = mgr.update_session_state(&new_account_id);
                
                Some((new_account_id.clone(), mgr.data_dir_for(&new_account_id), mgr.cache_dir_for(&new_account_id)))
            } else {
                None
            }
        } else {
            None
        };
        // borrow dropped

        if let Some((account_id, data_dir, cache_dir)) = info {
            imp.setup_webview(&account_id, data_dir, cache_dir, true);
        }

        imp.update_account_button();
    }
}
