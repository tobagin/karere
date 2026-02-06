use gtk::{gio, glib, prelude::*, subclass::prelude::*};
use gettextrs::gettext;
use libadwaita as adw;
use adw::prelude::*;
use adw::subclass::prelude::*;
use base64::prelude::*;
use webkit6::prelude::*;

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
        
        // Manual storage for the WebView
        pub web_view: std::cell::OnceCell<webkit6::WebView>,
        
        // Force Close Flag
        pub force_close: std::cell::Cell<bool>,

        // Active Notifications Storage (Map ID -> WebKitNotification)
        pub active_notifications: std::cell::RefCell<std::collections::HashMap<String, webkit6::Notification>>,
        
        // Mobile Layout State (tracks if mobile layout is currently active)
        pub mobile_layout_active: std::cell::Cell<bool>,

        // Notification Proxy (Reusable)
        pub notification_proxy: Rc<OnceCell<ashpd::desktop::notification::NotificationProxy<'static>>>,

        // Window State Persistence
        pub last_unmaximized_size: std::cell::Cell<(i32, i32)>,

        // Resize Debounce Timer
        pub resize_timer: std::cell::RefCell<Option<glib::SourceId>>,
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

            // 0. Setup Persistent Network Session
            let data_dir = glib::user_data_dir().join("karere").join("webkit");
            let cache_dir = glib::user_cache_dir().join("karere").join("webkit");
            
            let session = webkit6::NetworkSession::new(
                data_dir.to_str(),
                cache_dir.to_str()
            );

            // 1. Create WebView Manually with Session
            let web_view = webkit6::WebView::builder()
                .network_session(&session)
                .build();
            
            web_view.set_vexpand(true);
            web_view.set_hexpand(true);

            // Enable Page Cache explicitly (Helpful for history navigation)
            if let Some(settings) = webkit6::prelude::WebViewExt::settings(&web_view) {
                settings.set_enable_page_cache(true);
                settings.set_enable_webrtc(true);
                settings.set_enable_media_stream(true);
            }
            
            // 2. Add to UI
            self.view_container.append(&web_view);
            
            // 3. Store reference
            let _ = self.web_view.set(web_view.clone());

            // 4. Load URI
            web_view.load_uri("https://web.whatsapp.com");

            // 5. Pre-emptive Camera Permission Request (Fix for "0 Devices" / Catch-22)
            // We must ask for permission *before* WebKit initializes GStreamer, otherwise
            // GStreamer sees 0 devices and WebKit never asks for permission.
            let ctx = glib::MainContext::default();
            ctx.spawn_local(async move {
                 // Use the global Tokio runtime from main.rs to prevent "no reactor running" panic
                 // This is necessary because we have Tokio enabled in dependencies, causing zbus to expect it.
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
            }
            
            // Setup Actions
            let action_refresh = gio::SimpleAction::new("refresh", None);
            let obj_weak = obj.downgrade();
            action_refresh.connect_activate(move |_, _| {
                if let Some(obj) = obj_weak.upgrade() {
                    if let Some(webview) = obj.imp().web_view.get() {
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
                    if let Some(webview) = obj.imp().web_view.get() {
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
                    surface.connect_layout(move |_surface, width, height| {
                        if let Some(window) = obj_weak.upgrade() {
                            if !window.is_maximized() {
                                window.imp().last_unmaximized_size.set((width, height));
                            }
                        }
                    });
                }
            });


            // Host-Driven Permission Trigger (User Suggestion)
            // Attempt to force the permission request from the Host side on load completion.
            let settings_mobile_layout = settings.clone(); // Clone for closure
            
            // Helper function to determine if mobile layout should be used
            // window_width: current window width in pixels (use 0 if unknown)
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
            
            // Clone obj for closures
            let obj_weak_mobile = obj.downgrade();
            
            web_view.connect_load_changed(move |webview, event| {
                if event == webkit6::LoadEvent::Finished {
                    // Get window width if available
                    let window_width = if let Some(window) = obj_weak_mobile.upgrade() {
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
                            |result| {
                                match result {
                                    Ok(_) => {},
                                    Err(e) => eprintln!("ERROR: Failed to inject mobile_responsive.js: {}", e),
                                }
                            },
                        );
                        // Update state
                        if let Some(window) = obj_weak_mobile.upgrade() {
                            window.imp().mobile_layout_active.set(true);
                        }
                    } else {
                        // Update state
                        if let Some(window) = obj_weak_mobile.upgrade() {
                            window.imp().mobile_layout_active.set(false);
                        }
                    }

                    webview.evaluate_javascript(
                        "Notification.requestPermission()", 
                        None, 
                        None, 
                        Option::<&gio::Cancellable>::None, 
                        |_| {}
                    );
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
                    const MOBILE_WIDTH_THRESHOLD: i32 = 768;
                    let initial_width = window.width();
                    window.imp().mobile_layout_active.set(initial_width < MOBILE_WIDTH_THRESHOLD);
                    
                    /* Mobile layout logic temporarily removed to fix reload loop regression */
                    surface.connect_layout(move |_, width, _| {
                        if let Some(window) = obj_weak_layout.upgrade() {
                            let was_mobile = window.imp().mobile_layout_active.get();
                            let is_mobile = should_use_mobile_layout(&settings_layout, width);

                            if is_mobile != was_mobile {
                                println!("Karere: Layout Mode Change Detected (Mobile: {} -> {}). Reloading...", was_mobile, is_mobile);
                                window.imp().mobile_layout_active.set(is_mobile);
                                webview_layout.reload();
                            }
                        }
                    });
                }
            });

            // Handle Navigation Policy (External Links)
            let _obj_weak_nav = obj.downgrade();
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
                    return glib::Propagation::Proceed;
                }

                // Check "Close Button Behavior" setting
                let close_action = settings_close.string("close-button-action");
                if close_action == "quit" {
                    // Quit Application
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
            let webview_mobile_reload = self.web_view.get().unwrap().clone();
            settings.connect_changed(Some("mobile-layout"), move |_settings, _| {
                // Reload webview to apply/remove mobile layout
                webview_mobile_reload.reload();
            });

            // 2. WebKit Settings
            // Also ensure main switch is toggled on WebView settings
            let webview = self.web_view.get().expect("WebView should be initialized");
            if let Some(ws) = webkit6::prelude::WebViewExt::settings(webview) {
                 settings.bind("enable-developer-tools", &ws, "enable-developer-extras").build();
                 
                 // Memory Optimization: Disable Page Cache (Back/Forward Cache)
                 ws.set_enable_page_cache(false);
                 
                 ws.set_enable_media_stream(true);
                 ws.set_enable_mediasource(true);
                 ws.set_enable_webrtc(true);

            // Debug: Monitor Camera Capture State
            webview.connect_notify_local(Some("camera-capture-state"), |webview, _| {
                let state = webview.camera_capture_state();
                println!("Karere: Camera Capture State Changed: {:?}", state);
            });
            
            // Debug: Monitor Microphone Capture State
             webview.connect_notify_local(Some("microphone-capture-state"), |webview, _| {
                let state = webview.microphone_capture_state();
                println!("Karere: Microphone Capture State Changed: {:?}", state);
            });
                 
                 // User Agent Spoofing (Chrome Linux)
                 let _version = "2.0.0"; 
                 let user_agent = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
                 ws.set_user_agent(Some(&user_agent));
                 
                 // Disable quirks to restore our manual Linux UA
                 ws.set_enable_site_specific_quirks(false);
                 
                 // CRITICAL: ANY JavaScript navigator override (userAgent OR platform) breaks dead key composition!
                 // Validated by user: platform-only override also causes dead key bug.
                 // We must NOT inject any scripts that modify navigator.
                 // The "Download for Mac" banner is cosmetic and unavoidable.
                 
                 if let Some(_ucm) = self.web_view.get().unwrap().user_content_manager() {
                       // Notification Persistence Sync (Proxy Strategy) - REMOVED
                       // We now rely on Host-Driven Trigger (on Load Finished) and PersistentNetworkSession.
                       // The Proxy is no longer needed to "lie" because we can now get the "truth" (native grant) fast enough.
                   }
             }




            // Inject Notification Persistence (Native Logic)
            // With PersistentNetworkSession, we rely on WebKit saving the permission state.
            // When requestPermission is called by the site, our Rust handler (below) intercepts it.



            // Setup JS->Rust Logging Channel
            if let Some(ucm) = self.web_view.get().unwrap().user_content_manager() {
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
            let webview_dev = self.web_view.get().unwrap().clone();
            let action_devtools = gio::SimpleAction::new("show-devtools", None);
            action_devtools.connect_activate(move |_, _| {
                if let Some(inspector) = webview_dev.inspector() {
                    inspector.show();
                }
            });
            obj.add_action(&action_devtools);
            


            // 3. Downloads
            if let Some(session) = self.web_view.get().unwrap().network_session() {
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
                                   if !directory.is_empty() {
                                       let mut path_str = directory.to_string();
                                       // Expand ~
                                       if path_str.starts_with("~") {
                                           let home = glib::home_dir(); // Returns PathBuf directly
                                           path_str = path_str.replacen("~", home.to_str().unwrap(), 1);
                                       }
                                       
                                       let path = std::path::PathBuf::from(path_str);
                                       if path.exists() {
                                            let dest = path.join(filename.as_str());
                                            // WebKitGTK set_destination expects a URI, validation for it returning one.
                                            let dest_file = gio::File::for_path(&dest);
                                            let uri_str = dest_file.uri();
                                            download.set_destination(&uri_str);
                                            return true;
                                        }
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
            
            // 4. Notifications (Permissions)
            // 4. Notifications & Microphone Permissions (with Persistence and Dialogs)
            let settings_notify = settings.clone();
            let window_weak = obj.downgrade();
            
            self.web_view.get().unwrap().connect_permission_request(move |_, request| {
                // Debug logs
                let settings = settings_notify.clone();
                let window_weak = window_weak.clone();

                if let Some(req) = request.downcast_ref::<webkit6::NotificationPermissionRequest>() {
                    // Check persistence
                    let asked = settings.boolean("web-notification-permission-asked");
                    let granted = settings.boolean("web-notification-permission-granted");
                    
                    
                    if asked {
                        if granted {
                            req.allow();
                        } else {
                            req.deny();
                        }
                        return true;
                    }
                    
                    // Show Dialog
                    if let Some(window) = window_weak.upgrade() {
                        let dialog = adw::AlertDialog::builder()
                            .heading(&gettext("WhatsApp Web Notification Permission"))
                            .body(&gettext("WhatsApp Web wants to show desktop notifications for new messages. Would you like to allow notifications?"))
                            .default_response("allow")
                            .close_response("deny")
                            .build();

                        dialog.add_response("deny", &gettext("Deny"));
                        dialog.add_response("allow", &gettext("Allow"));
                        dialog.set_response_appearance("allow", adw::ResponseAppearance::Suggested);
                        
                        let req_clone = request.clone(); // Upcast to PermissionRequest object
                        dialog.choose(Some(&window), gio::Cancellable::NONE, move |response| {
                            let granted = response == "allow";
                            
                            if let Err(e) = settings.set_boolean("web-notification-permission-asked", true) {
                                eprintln!("ERROR: Failed to save 'asked' setting: {}", e);
                            }
                            if let Err(e) = settings.set_boolean("web-notification-permission-granted", granted) {
                                eprintln!("ERROR: Failed to save 'granted' setting: {}", e);
                            }
                            
                            // Force sync to ensure it hits disk
                            gio::Settings::sync();
                            
                            if granted {
                                req_clone.allow();
                            } else {
                                req_clone.deny();
                            }
                        });
                        return true; // We are handling it (async)
                    }
                } else if let Some(req) = request.downcast_ref::<webkit6::UserMediaPermissionRequest>() {
                     let is_audio = req.is_for_audio_device();
                     let is_video = req.is_for_video_device();

                     // Skip if neither (shouldn't happen for UserMedia)
                     if !is_audio && !is_video {
                         return false;
                     }

                     // Manual Portal Request for Video (Camera)
                     // The OS-level permission is handled at startup to ensure device visibility.
                     // We now fall through to the standard app dialog below to ask the user
                     // for site-specific permission, maintaining UX consistency with Microphone.

                     // Determine keys and dialog text
                     let (asked_key, granted_key, title, body) = if is_video {
                         // Video implies Camera (may include Audio too)
                         // If it's both, we just ask for Camera access (which implies call usually) or say "Camera and Microphone"
                         // For simplicity and matching typical "app" behavior:
                         if is_audio {
                             ("web-camera-permission-asked", "web-camera-permission-granted", 
                              gettext("WhatsApp Web Camera & Microphone Permission"),
                              gettext("WhatsApp Web wants to access your camera and microphone. Would you like to allow access?"))
                         } else {
                             ("web-camera-permission-asked", "web-camera-permission-granted",
                              gettext("WhatsApp Web Camera Permission"),
                              gettext("WhatsApp Web wants to access your camera. Would you like to allow access?"))
                         }
                     } else {
                         // Audio only
                         ("web-microphone-permission-asked", "web-microphone-permission-granted",
                          gettext("WhatsApp Web Microphone Permission"),
                          gettext("WhatsApp Web wants to access your microphone. Would you like to allow access?"))
                     };

                     // Check persistence
                     let asked = settings.boolean(asked_key);
                     let granted = settings.boolean(granted_key);


                     if asked {
                         if granted {
                             req.allow();
                         } else {
                             req.deny();
                         }
                         return true;
                     }
                    
                    // Show Dialog
                    if let Some(window) = window_weak.upgrade() {
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
                        dialog.choose(Some(&window), gio::Cancellable::NONE, move |response| {
                            let granted = response == "allow";
                            
                            // Save settings
                            let _ = settings.set_boolean(asked_key, true);
                            let _ = settings.set_boolean(granted_key, granted);
                            
                            // If granting Video+Audio, we should implicitly grant Audio-only too?
                            // Yes, to avoid double prompt if next time it asks only Audio.
                            if granted && is_video && is_audio {
                                let _ = settings.set_boolean("web-microphone-permission-asked", true);
                                let _ = settings.set_boolean("web-microphone-permission-granted", true);
                            }
                            
                            gio::Settings::sync();
                            
                            if granted {
                                req_clone.allow();
                            } else {
                                req_clone.deny();
                            }
                        });
                        return true;
                    }
                }
                
                false
            });

            // Handle Show Notification signal (Bridge to Desktop)
            let settings_notify_msg = settings.clone(); // Clone for closure
            let window_weak = obj.downgrade();
            self.web_view.get().unwrap().connect_show_notification(move |_, notification| {
                
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
                         let notification_id = if let Some(tag) = notification.tag() {
                              tag.to_string()
                         } else {
                              format!("msg-{}", glib::monotonic_time())
                         };

                        let notification_id_portal = notification_id.clone();
                        
                        // Get the proxy from the window impl (shared)
                        let proxy_cell = if let Some(window) = window_weak.upgrade() {
                             window.imp().notification_proxy.clone()
                        } else {
                             // Should not happen if we are here
                             return true;
                        };

                        glib::MainContext::default().spawn_local(async move {
                            // Enter the tokio runtime context
                            let _guard = crate::RUNTIME.enter();
                            
                            // Initialize Proxy if needed (Singleton pattern per window)
                            let proxy = proxy_cell.get_or_init(|| async {
                                match ashpd::desktop::notification::NotificationProxy::new().await {
                                    Ok(p) => p,
                                    Err(e) => {
                                        eprintln!("ERROR: Failed to create notification proxy: {}", e);
                                        // We have to return something to satisfy the type, but this is cached.
                                        // If this fails, we are in trouble anyway. 
                                        // Ideally we retry, but OnceCell is once.
                                        // For simplicity, we might crash or just panic here if strict, 
                                        // but better to handle it. 
                                        // Actually ashpd Proxy new() shouldn't fail often unless DBus is dead.
                                        // Let's rely on it working or we'll get a broken proxy maybe?
                                        // Wait, we can't easily return a Result here if get_or_init expects value.
                                        // Let's panic to restart app state? No.
                                        // Let's try to construct it.
                                        // If it fails, subsequent calls will not re-try with standard OnceCell.
                                        // But we can just log and panic for now as it's critical infrastructure.
                                        panic!("Failed to init notification proxy: {}", e);
                                    }
                                }
                            }).await;

                            let notif = ashpd::desktop::notification::Notification::new(&title_clone)
                                .body(body_clone.as_str())
                                .icon(ashpd::desktop::Icon::with_names(&["dialog-information-symbolic"]))
                                .default_action("app.notification-clicked")
                                .default_action_target(notification_id_portal.as_str())
                                .priority(ashpd::desktop::notification::Priority::Normal);

                            if let Err(_e) = proxy.add_notification(&notification_id_portal, notif).await {
                                // eprintln!("Failed to send portal notification: {}", e); // Removed debug log
                            }
                        });
                        
                        // Store notification for click handling
                        if let Some(window) = window_weak.upgrade() {
                             window.imp().active_notifications.borrow_mut().insert(notification_id.clone(), notification.clone());
                             
                             // Cleanup on close
                             let window_weak_close = window.downgrade();
                             let id_close = notification_id.clone();
                             notification.connect_closed(move |_| {
                                 if let Some(window) = window_weak_close.upgrade() {
                                     window.imp().active_notifications.borrow_mut().remove(&id_close);
                                 }
                             });
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
                                let temp_path = std::env::temp_dir().join("karere-notify.oga");
                                
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

            // Handle Window Focus (Clear Unread)
            obj.connect_is_active_notify(move |window| {
                if window.is_active() {
                     if let Some(app) = window.application() {
                         app.activate_action("set-unread", Some(&false.to_variant()));
                     }
                }
            });




            // 5. Input Handling (Paste & Middle-click)
            
            // Image & File Paste (Ctrl+V)
            // WhatsApp Web often struggles with direct pasting from Linux/GDK clipboard in WebKit for images and files.
            // We manually detect them, encode, and inject synthetic Paste events.
            let key_controller = gtk::EventControllerKey::new();
            let webview_paste = self.web_view.get().unwrap().clone();
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
                                                             "#, filename, b64, mime, filename, mime);
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
            self.web_view.get().unwrap().add_controller(key_controller);

            // Middle Click Paste (Primary Selection)
            let gesture_click = gtk::GestureClick::new();
            gesture_click.set_button(2); // Middle Mouse Button
            gesture_click.set_propagation_phase(gtk::PropagationPhase::Capture);
            let webview_mid = self.web_view.get().unwrap().clone();
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
            self.web_view.get().unwrap().add_controller(gesture_click);

            // Drag and Drop (Files)
            // Explicitly handle file drops to bypass some Flatpak/WebView disconnects or just to provide unified injection.
            let drop_target = gtk::DropTarget::new(gtk::gdk::FileList::static_type(), gtk::gdk::DragAction::COPY);
            let webview_drop = self.web_view.get().unwrap().clone();
            
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
                                     "#, filename, b64, mime, filename, mime);
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
            self.web_view.get().unwrap().add_controller(drop_target);

            
            // 11. Setup Accessibility & Auto-Correct
            self.setup_accessibility(web_view, settings.clone());
            
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
        }
    }

    impl KarereWindow {
        fn setup_accessibility(&self, web_view: webkit6::WebView, settings: gio::Settings) {
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

             // 3. Webview Zoom Visibility
             settings.bind("webview-zoom", &*self.zoom_box, "visible").build();
             
             // 4. Screen Reader Support
             if let Some(webview_settings) = webkit6::prelude::WebViewExt::settings(&web_view) {
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
             
             // 7. Zoom Controls Actions
             let webview_z = web_view.clone();
             let action_zoom_in = gio::SimpleAction::new("zoom-in", None);
             action_zoom_in.connect_activate(move |_, _| {
                 let level = webview_z.zoom_level();
                 webview_z.set_zoom_level(level + 0.1);
             });
             obj.add_action(&action_zoom_in);

             let webview_z = web_view.clone();
             let action_zoom_out = gio::SimpleAction::new("zoom-out", None);
             action_zoom_out.connect_activate(move |_, _| {
                 let level = webview_z.zoom_level();
                 if level > 0.2 {
                     webview_z.set_zoom_level(level - 0.1);
                 }
             });
             obj.add_action(&action_zoom_out);

             let webview_z = web_view.clone();
             let action_zoom_reset = gio::SimpleAction::new("zoom-reset", None);
             action_zoom_reset.connect_activate(move |_, _| {
                 webview_z.set_zoom_level(1.0);
             });
             obj.add_action(&action_zoom_reset);
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

    pub fn process_notification_click(&self, id: &str) {
        let imp = self.imp();
        if let Some(notification) = imp.active_notifications.borrow().get(id) {
            notification.clicked();
        } else {
            eprintln!("Notification ID not found for click: {}", id);
        }
    }
}
