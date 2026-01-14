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
    use webkit6::prelude::*;

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

            // Apply devel class if needed
            // Fallback to env var since obj.application() might be None in constructed
            let app_id = obj.application()
                .and_then(|app| app.application_id())
                .map(|s| s.to_string())
                .or_else(|| std::env::var("FLATPAK_ID").ok())
                .unwrap_or_default();

            println!("DEBUG: Resolved App ID: '{}'", app_id);
            if app_id.contains("Dev") || app_id.contains("Devel") {
                 println!("DEBUG: 'Dev/Devel' detected. Adding 'devel' css class.");
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
            
            // 2. Add to UI
            self.view_container.append(&web_view);
            
            // 3. Store reference
            let _ = self.web_view.set(web_view.clone());

            // 4. Load URI
            web_view.load_uri("https://web.whatsapp.com");
            
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
                        println!("DEBUG: Triggering New Chat (Ctrl+Alt+N simulation)");
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


            // Host-Driven Permission Trigger (User Suggestion)
            // Attempt to force the permission request from the Host side on load completion.
            let settings_mobile_layout = settings.clone(); // Clone for closure
            let webview_inj = web_view.clone();
            web_view.connect_load_changed(move |_, event| {
                if event == webkit6::LoadEvent::Finished {

                    let mobile_layout = settings_mobile_layout.boolean("mobile-layout");
                    if mobile_layout {
                        let js_content = include_str!("mobile_responsive.js");
                        webview_inj.evaluate_javascript(
                            &js_content,
                            None,
                            None,
                            Option::<&gio::Cancellable>::None,
                            |result| {
                                match result {
                                    Ok(_) => println!("INFO: mobile_responsive.js injected successfully."),
                                    Err(e) => eprintln!("ERROR: Failed to inject mobile_responsive.js: {}", e),
                                }
                            },
                        );                        
                    }

                    println!("DEBUG: Load Finished. Attempting Host-Driven Permission Request...");
                    // We use the proxy's requestPermission if available, or native.
                    // Since we injected the proxy at Start, window.Notification should be our proxy.
                    webview_inj.evaluate_javascript(
                        "Notification.requestPermission()", 
                        None, 
                        None, 
                        Option::<&gio::Cancellable>::None, 
                        |result| {
                            match result {
                                Ok(_) => println!("DEBUG: Host-driven JS execution success."),
                                Err(e) => println!("DEBUG: Host-driven JS execution failed: {}", e),
                            }
                        }
                    );
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
                                     println!("DEBUG: Policy Decision: Type={:?}, URI={}", decision_type, uri_str);

                                     // Enhanced V1-like Logic
                                     let is_internal = uri_str.contains("web.whatsapp.com") || 
                                                       uri_str.contains("whatsapp.com") ||
                                                       uri_str.contains("whatsapp.net") ||
                                                       uri_str.starts_with("data:") ||
                                                       uri_str.starts_with("blob:") ||
                                                       uri_str.starts_with("about:");

                                     if !is_internal {
                                         println!("DEBUG: External Link detected. Opening: {}", uri_str);
                                         
                                         // Use generic GLib/GIO launcher which works via portal in Flatpak
                                         // This matches V1 logic (AppInfo.launch_default_for_uri)
                                         let uri_owned = uri_str.to_string();
                                         match gio::AppInfo::launch_default_for_uri(&uri_owned, Option::<&gio::AppLaunchContext>::None) {
                                             Ok(_) => println!("DEBUG: External URI launched successfully via AppInfo."),
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

            // Handle Close Request (Background Mode)
            obj.connect_close_request(|window| {
                if window.imp().force_close.get() {
                    return glib::Propagation::Proceed;
                }
                
                // Hide instead of close
                window.set_visible(false);
                glib::Propagation::Stop
            });
            
            // DEBUG: Check persistence at startup
            let asked_start = settings.boolean("web-notification-permission-asked");
            let granted_start = settings.boolean("web-notification-permission-granted");
            println!("DEBUG: Startup GSettings - Asked: {}, Granted: {}", asked_start, granted_start);
            
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

            // 2. WebKit Settings
            // Also ensure main switch is toggled on WebView settings
            if let Some(ws) = webkit6::prelude::WebViewExt::settings(self.web_view.get().unwrap()) {
                 settings.bind("enable-developer-tools", &ws, "enable-developer-extras").build();
                 
                 // User Agent Spoofing (Chrome Linux)
                 let _version = "2.0.0"; 
                 let user_agent = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
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
                ucm.connect_script_message_received(Some("log"), |_, result| {
                     // result is &Simple (or &Value). 
                     // In webkit6-rs, the signature handles the conversion efficiently or gives access to JSCValue.
                     // The error said `result` is `&webkit6::javascriptcore6::Value`.
                     // Let's rely on standard debug print or robust string conversion.
                     let msg = result.to_string();
                     println!("JS_LOG: {}", msg);
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
                          // Wait, `obj` is `self.obj()` in `constructed`.
                          // We need to capture it here.
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
                               // We need to capture `obj` in the outer closure.
                               // Let's skip AlertDialog for now and use Toast for error to handle the compile error simply?
                               // NO, user explicitly asked for AlertDialog.
                               // We need `obj`.
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
                println!("Requesting notification permission...");
                let settings = settings_notify.clone();
                let window_weak = window_weak.clone();

                if let Some(req) = request.downcast_ref::<webkit6::NotificationPermissionRequest>() {
                    // Check persistence
                    let asked = settings.boolean("web-notification-permission-asked");
                    let granted = settings.boolean("web-notification-permission-granted");
                    
                    println!("DEBUG: Permission Request. Asked={}, Granted={}", asked, granted);
                    
                    if asked {
                        if granted {
                            println!("DEBUG: Auto-Allowing notification permission");
                            req.allow();
                        } else {
                            println!("DEBUG: Auto-Denying notification permission");
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

                     println!("DEBUG: Media Request. Audio={}, Video={}, Asked={}, Granted={}", is_audio, is_video, asked, granted);

                     if asked {
                         if granted {
                             println!("DEBUG: Auto-Allowing media permission");
                             req.allow();
                         } else {
                             println!("DEBUG: Auto-Denying media permission");
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
                println!("DEBUG: connect_show_notification TRIGGERED!");
                
                // 1. Check Master Toggle
                if !settings_notify_msg.boolean("notifications-enabled") {
                    println!("DEBUG: Notifications disabled globally.");
                    return true; // Suppress
                }
                
                // 2. Check Message Notifications Toggle
                if !settings_notify_msg.boolean("notify-messages") {
                    println!("DEBUG: Message notifications disabled.");
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
                        glib::MainContext::default().spawn_local(async move {
                            // Enter the tokio runtime context for ashpd/zbus
                            let _guard = crate::RUNTIME.enter();
                            
                            // Create proxy
                            match ashpd::desktop::notification::NotificationProxy::new().await {
                                Ok(proxy) => {
                                    let notif = ashpd::desktop::notification::Notification::new(&title_clone)
                                        .body(body_clone.as_str())
                                        .icon(ashpd::desktop::Icon::with_names(&["dialog-information-symbolic"]))
                                        .default_action("app.notification-clicked")
                                        .default_action_target(notification_id_portal.as_str())
                                        .priority(ashpd::desktop::notification::Priority::High);

                                    // Check if we can add an ID (ashpd 0.4+ uses send_notification with ID usually, let's check basic usage)
                                    // wrapper: "add_notification(&self, id: &str, notification: &Notification)"
                                    
                                    if let Err(e) = proxy.add_notification(&notification_id_portal, notif).await {
                                        eprintln!("Failed to send portal notification: {}", e);
                                    }
                                },
                                Err(e) => eprintln!("ERROR: Failed to connect to notification portal: {}", e),
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
                        if settings_notify_msg.boolean("notify-sound-enabled") {
                            let sound_key = settings_notify_msg.string("notify-sound-file");
                            let sound_name = match sound_key.as_str() {
                                "pop" => "pop",
                                "alert" => "alert",
                                "soft" => "soft",
                                "start" => "start",
                                _ => "whatsapp",
                            };
                            
                            // Play Logic (Extract & Spawn)
                            // Ideally extraction happens once or cached, but tmp write is fast enough.
                            let resource_path = format!("/io/github/tobagin/karere/sounds/{}.oga", sound_name);
                            if let Ok(bytes) = gio::resources_lookup_data(&resource_path, gio::ResourceLookupFlags::NONE) {
                                let temp_path = std::env::temp_dir().join("karere-notify.oga");
                                if std::fs::write(&temp_path, &bytes).is_ok() {
                                     let _ = std::process::Command::new("paplay")
                                         .arg(temp_path)
                                         .spawn();
                                }
                            }
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
            
            // Image Paste (Ctrl+V)
            // WhatsApp Web often struggles with direct image pasting from Linux/GDK clipboard in WebKit.
            // We manually detect image data, encode it, and inject a synthetic Paste event.
            let key_controller = gtk::EventControllerKey::new();
            let webview_paste = self.web_view.get().unwrap().clone();
            key_controller.connect_key_pressed(move |_, keyval, _keycode, state| {
                if state.contains(gtk::gdk::ModifierType::CONTROL_MASK) && (keyval == gtk::gdk::Key::v || keyval == gtk::gdk::Key::V) {
                     let clipboard = gtk::gdk::Display::default().and_then(|d| Some(d.clipboard()));
                     if let Some(clipboard) = clipboard {
                         // Check if clipboard has an image (GdkTexture)
                         let formats = clipboard.formats();
                         if formats.contains_type(gtk::gdk::Texture::static_type()) {
                             println!("DEBUG: Image detected in clipboard. Intercepting Ctrl+V for manual injection.");
                             
                             let webview = webview_paste.clone();
                             clipboard.read_texture_async(gio::Cancellable::NONE, move |res: Result<Option<gtk::gdk::Texture>, glib::Error>| {
                                 if let Ok(Some(texture)) = res {
                                      // Convert to PNG bytes
                                      let bytes = texture.save_to_png_bytes();
                                      let b64 = BASE64_STANDARD.encode(bytes.as_ref());
                                      
                                      // Inject JS to create File and dispatch Paste event
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
                 
                 println!("DEBUG: Middle Click Detected. Attempting Primary Paste...");
                 let clipboard = gtk::gdk::Display::default().and_then(|d| Some(d.primary_clipboard()));
                 if let Some(clipboard) = clipboard {
                     let webview = webview_mid.clone();
                     clipboard.read_text_async(gio::Cancellable::NONE, move |res: Result<Option<glib::GString>, glib::Error>| {
                         match res {
                             Ok(Some(text)) => {
                                 println!("DEBUG: Primary Selection Retrieved: {} chars", text.len());
                                 // Escape for JS string
                                 let safe_text = text.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n").replace("\r", "");
                                 let js = format!(r#"document.execCommand("insertText", false, "{}");"#, safe_text);
                                 webview.evaluate_javascript(&js, None, None, Option::<&gio::Cancellable>::None, |_| {});
                             },
                             Ok(None) => println!("DEBUG: Primary Selection Empty"),
                             Err(e) => println!("DEBUG: Failed to read Primary Selection: {}", e),
                         }
                     });
                 }
            });
            self.web_view.get().unwrap().add_controller(gesture_click);

            
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
                     provider.load_from_data(":focus { outline-width: 2px; outline-style: solid; outline-color: alpha(currentColor, 0.7); }");
                 } else {
                     provider.load_from_data(""); 
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
