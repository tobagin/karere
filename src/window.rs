use gtk::{gio, glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use adw::prelude::*;
use adw::subclass::prelude::*;

mod imp {
    use super::*;
    use gtk::gio;
    use webkit6::prelude::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/tobagin/karere/window.ui")]
    pub struct KarereWindow {
        #[template_child]
        pub view_container: TemplateChild<gtk::Box>,
        #[template_child]
        pub zoom_box: TemplateChild<gtk::Box>,
        
        // Manual storage for the WebView
        pub web_view: std::cell::OnceCell<webkit6::WebView>,
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
                window.set_visible(false);
                glib::Propagation::Stop
            });

            // Setup Settings Logic
            let settings = gio::Settings::new("io.github.tobagin.karere");
            
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
                 
                 // User Agent Parity with Karere v1
                 let version = "2.0.0"; // Hardcoded or env!("CARGO_PKG_VERSION")
                 let user_agent = format!("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Karere/{}", version);
                 ws.set_user_agent(Some(&user_agent));
                 
                 // Inject User Agent via UserScript for Parity
                  if let Some(ucm) = self.web_view.get().unwrap().user_content_manager() {
                       // 1. User Agent Spoofing
                       let js_ua = format!(r#"
                           Object.defineProperty(navigator, 'userAgent', {{
                               get: function() {{ return '{}'; }},
                               configurable: false,
                               enumerable: true
                           }});
                           Object.defineProperty(navigator, 'platform', {{
                               get: function() {{ return 'Linux x86_64'; }},
                               configurable: false,
                               enumerable: true
                           }});
                       "#, user_agent);
                       
                       let script_ua = webkit6::UserScript::new(
                           &js_ua,
                           webkit6::UserContentInjectedFrames::AllFrames,
                           webkit6::UserScriptInjectionTime::Start,
                           &[],
                           &[],
                       );
                       ucm.add_script(&script_ua);

                       // 2. Notification Persistence Sync (Proxy Strategy)
                       // Check if we already granted permission in GSettings
                       let already_granted = settings.boolean("web-notification-permission-granted");
                       
                       if already_granted {
                           let js_perm = r#"
                               (function() {
                                   // Redirect logging to Rust
                                   const oldLog = console.log;
                                   console.log = function(...args) {
                                       oldLog.apply(console, args);
                                       try {
                                           window.webkit.messageHandlers.log.postMessage(args.map(a => String(a)).join(' '));
                                       } catch(e) {}
                                   };
                                   
                                   console.log('Karere: JS Injection Started.');

                                   if (window.KarereInjected) {
                                       console.log('Karere: Already injected, skipping.');
                                       return;
                                   }
                                   window.KarereInjected = true;
                                   
                                   const NativeNotification = window.Notification;
                                   if (!NativeNotification) {
                                       console.log('Karere: ERR - No Notification object found!');
                                       return;
                                   }
                                   
                                   console.log('Karere: Creating Proxy...');
                                   
                                   // Create a proxy constructor
                                   const ProxyNotification = function(title, options) {
                                       return new NativeNotification(title, options);
                                   };
                                   
                                   // Copy prototype chain
                                   ProxyNotification.prototype = NativeNotification.prototype;
                                   
                                   // Spoof permission to 'granted' to hide banner
                                   Object.defineProperty(ProxyNotification, 'permission', {
                                       get: () => {
                                           console.log('Karere: Permission property accessed, returning granted.');
                                           return 'granted';
                                       },
                                       configurable: true,
                                       enumerable: true
                                   });
                                   
                                   // Pass-through requestPermission to native to trigger Rust signal
                                   ProxyNotification.requestPermission = function(cb) {
                                       console.log('Karere: Proxy requestPermission called, forwarding to native...');
                                       return NativeNotification.requestPermission(cb);
                                   };
                                   
                                   // Copy other static properties
                                   for (let prop in NativeNotification) {
                                        if (prop !== 'permission' && prop !== 'requestPermission' && prop !== 'prototype') {
                                            try {
                                               ProxyNotification[prop] = NativeNotification[prop];
                                            } catch(e) {}
                                        }
                                   }

                                   // Replace global Notification object
                                   try {
                                       window.Notification = ProxyNotification;
                                       console.log('Karere: Notification object replaced successfully.');
                                   } catch(e) {
                                       console.log('Karere: Failed to replace Notification object:', e);
                                   }
                                   
                                   // Force sync native permission state
                                   console.log('Karere: Attempting initial native sync...');
                                   const doSync = () => {
                                        console.log('Karere: doSync called');
                                        NativeNotification.requestPermission().then(p => {
                                            console.log('Karere: Native sync result:', p);
                                        }).catch(e => {
                                            console.log('Karere: Sync failed:', e);
                                        });
                                   };
                                   
                                   doSync();
                                   
                                   // Retry on user interaction (to bypass User Activation protections)
                                   const lazySync = () => {
                                        console.log('Karere: User interaction detected, retrying sync...');
                                        doSync();
                                        window.removeEventListener('click', lazySync);
                                        window.removeEventListener('keydown', lazySync);
                                   };
                                   window.addEventListener('click', lazySync);
                                   window.addEventListener('keydown', lazySync);

                               })();
                           "#;
                           
                           let script_perm = webkit6::UserScript::new(
                               js_perm,
                               webkit6::UserContentInjectedFrames::AllFrames,
                               webkit6::UserScriptInjectionTime::Start,
                               &[],
                               &[],
                           );
                           ucm.add_script(&script_perm);
                       }

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
            
            // Add accelerator (F11)
            if let Some(app) = obj.application().and_then(|a| a.downcast::<gtk::Application>().ok()) {
                app.set_accels_for_action("win.show-devtools", &["F11"]);
            }

            // 3. Downloads
            if let Some(session) = self.web_view.get().unwrap().network_session() {
                let settings_dl = settings.clone();
                session.connect_closure(
                     "download-started",
                     false,
                     glib::closure_local!(move |_session: webkit6::NetworkSession, download: webkit6::Download| {
                          let settings_dl = settings_dl.clone();
                          download.connect_closure(
                              "decide-destination",
                              false,
                              glib::closure_local!(move |download: webkit6::Download, filename: glib::GString| -> bool {
                                   let directory = settings_dl.string("download-directory");
                                   if !directory.is_empty() {
                                       let path = std::path::PathBuf::from(directory.as_str());
                                       if path.exists() {
                                           let dest = path.join(filename.as_str());
                                           let dest_uri = format!("file://{}", dest.to_string_lossy());
                                           download.set_destination(&dest_uri);
                                           return true;
                                       }
                                   }
                                   false                     
                              })
                          );
                     })
                );
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
                            .heading("WhatsApp Web Notification Permission")
                            .body("WhatsApp Web wants to show desktop notifications for new messages. Would you like to allow notifications?")
                            .default_response("allow")
                            .close_response("deny")
                            .build();

                        dialog.add_response("deny", "Deny");
                        dialog.add_response("allow", "Allow");
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
                     // Only handle Audio
                     if !req.is_for_audio_device() {
                         return false; 
                     }

                    // Check persistence
                    let asked = settings.boolean("web-microphone-permission-asked");
                    let granted = settings.boolean("web-microphone-permission-granted");     

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
                            .heading("WhatsApp Web Microphone Permission")
                            .body("WhatsApp Web wants to access your microphone for voice messages and calls. Would you like to allow microphone access?")
                            .default_response("allow")
                            .close_response("deny")
                            .build();

                        dialog.add_response("deny", "Deny");
                        dialog.add_response("allow", "Allow");
                        dialog.set_response_appearance("allow", adw::ResponseAppearance::Suggested);
                        
                        let req_clone = request.clone(); 
                        dialog.choose(Some(&window), gio::Cancellable::NONE, move |response| {
                            let granted = response == "allow";
                            settings.set_boolean("web-microphone-permission-asked", true).unwrap();
                            settings.set_boolean("web-microphone-permission-granted", granted).unwrap();
                            
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
            let window_weak = obj.downgrade();
            self.web_view.get().unwrap().connect_show_notification(move |_, notification| {
                println!("DEBUG: connect_show_notification TRIGGERED!");
                if let Some(window) = window_weak.upgrade() {
                    if let Some(app) = window.application() {
                        // Notify Tray: Set Unread = true
                        app.activate_action("set-unread", Some(&true.to_variant()));

                        let title = notification.title().unwrap_or_else(|| glib::GString::from("WhatsApp"));
                        let body = notification.body().unwrap_or_else(|| glib::GString::from("New message"));
                        
                        let note = gio::Notification::new(&title);
                        note.set_body(Some(&body));
                        note.set_icon(&gio::ThemedIcon::new("dialog-information-symbolic"));
                        
                        // When clicked, activate the window
                        note.set_default_action("win.present"); // Action on the window itself
                        
                        // Assign a unique ID to allow multiple? Or just constant ID "whatsapp-msg"?
                        // WhatsApp uses unique checks, so probably unique is better.
                        // Using tag if available?
                        let id = if let Some(tag) = notification.tag() {
                             format!("karere-{}", tag)
                        } else {
                             format!("karere-msg-{}", glib::monotonic_time())
                        };
                        
                        println!("Sending notification to OS: ID={}", id); 

                        app.send_notification(Some(&id), &note);
                        
                        // We handled it
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

            // 5. Sound (Notify Sound) -> Mute logic
            let webview_clone = self.web_view.get().unwrap().clone();
            let update_sound = move |settings: &gio::Settings, key: &str| {
                let enabled = settings.boolean(key);
                webview_clone.set_is_muted(!enabled);
            };
            update_sound(&settings, "notify-sound"); 
            settings.connect_changed(Some("notify-sound"), update_sound);

            // 6. Accessibility & System Overrides
            let update_a11y = move |settings: &gio::Settings, _: &str| {
                if let Some(gtk_settings) = gtk::Settings::default() {
                     let reduce_motion = settings.boolean("reduce-motion");
                     gtk_settings.set_gtk_enable_animations(!reduce_motion);
                }
            };
            settings.connect_changed(Some("reduce-motion"), update_a11y);
            // Initial call
            if let Some(gtk_settings) = gtk::Settings::default() {
                 let reduce_motion = settings.boolean("reduce-motion");
                 gtk_settings.set_gtk_enable_animations(!reduce_motion);
            }

            // 7. Zoom Controls
            settings.bind("webview-zoom", &*self.zoom_box, "visible").build();

            let webview_z = self.web_view.get().unwrap().clone();
            let action_zoom_in = gio::SimpleAction::new("zoom-in", None);
            action_zoom_in.connect_activate(move |_, _| {
                let level = webview_z.zoom_level();
                webview_z.set_zoom_level(level + 0.1);
            });
            obj.add_action(&action_zoom_in);

            let webview_z = self.web_view.get().unwrap().clone();
            let action_zoom_out = gio::SimpleAction::new("zoom-out", None);
            action_zoom_out.connect_activate(move |_, _| {
                let level = webview_z.zoom_level();
                if level > 0.2 {
                    webview_z.set_zoom_level(level - 0.1);
                }
            });
            obj.add_action(&action_zoom_out);

            let webview_z = self.web_view.get().unwrap().clone();
            let action_zoom_reset = gio::SimpleAction::new("zoom-reset", None);
            action_zoom_reset.connect_activate(move |_, _| {
                webview_z.set_zoom_level(1.0);
            });
            obj.add_action(&action_zoom_reset);

            // 8. Spell Checking
            if let Some(context) = self.web_view.get().unwrap().context() {
                  let update_spell = move |settings: &gio::Settings, _: &str| {
                      let enabled = settings.boolean("enable-spell-checking");
                      
                      context.set_spell_checking_enabled(enabled);

                      // Handle auto-detect
                      let auto = settings.boolean("auto-detect-language");
                       if enabled {
                           let available_dicts = crate::spellcheck::get_available_dictionaries();

                           if auto {
                               // Auto-detect logic
                               let locale = std::env::var("LANG").unwrap_or_else(|_| "en_US".to_string());
                               if let Some(lang) = crate::spellcheck::match_locale_to_dictionary(&locale, &available_dicts) {
                                   println!("Auto-detected spell checking language: {}", lang);
                                   context.set_spell_checking_languages(&[&lang]);
                               } else {
                                   eprintln!("Warning: No dictionary found for locale '{}'", locale);
                               }
                           } else {
                               // Manual selection logic
                               let languages = settings.strv("spell-checking-languages");
                               let available_set: std::collections::HashSet<&str> = available_dicts.iter().map(|s| s.as_str()).collect();
                               let mut valid_langs: Vec<&str> = Vec::new();
                               for lang in &languages {
                                   if available_set.contains(lang.as_str()) {
                                       valid_langs.push(lang.as_str());
                                   }
                               }
                               context.set_spell_checking_languages(&valid_langs);
                           }
                      }
                 };
                 // Initial call
                 update_spell(&settings, "enable-spell-checking");
                 
                 // Connect
                 settings.connect_changed(Some("enable-spell-checking"), update_spell.clone());
                 settings.connect_changed(Some("auto-detect-language"), update_spell.clone());
                 settings.connect_changed(Some("spell-checking-languages"), update_spell);
            }
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
}
