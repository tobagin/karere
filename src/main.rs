use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use gtk::{gio, glib};
use std::sync::atomic::Ordering;
use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

use gettextrs::*;

mod tray;
mod window;
mod preferences;
mod spellcheck;

use window::KarereWindow;

pub static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    Runtime::new().expect("Failed to create Tokio runtime")
});

fn main() -> anyhow::Result<()> {
    env_logger::init();

    // Initialize gettext
    textdomain("karere")?;
    bind_textdomain_codeset("karere", "UTF-8")?;
    
    let locale_dir = if std::env::var("FLATPAK_ID").is_ok() {
        std::path::PathBuf::from("/app/share/locale")
    } else {
        // Fallback for local dev - try to find target dir or use current dir
        // This is a bit hacky for creating a general path, but sufficient for now
        // Better: just expect them to be installed or ignore for raw cargo run unless we setup a specific install step.
        // Let's use a local 'locale' folder if it exists
        std::env::current_dir()?.join("locale")
    };
    
    bindtextdomain("karere", locale_dir)?;

    
    // Ensure compiled schemas are found (fix for running locally from cargo)
    // ONLY do this if NOT inside Flatpak, otherwise we break system schemas!
    if std::env::var("FLATPAK_ID").is_err() {
        let current_dir = std::env::current_dir().expect("Failed to get current directory");
        let schema_dir = current_dir.join("data");
        if schema_dir.exists() {
             unsafe {
                 std::env::set_var("GSETTINGS_SCHEMA_DIR", schema_dir);
             }
        }
    }

    // Load resources
    let resources_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/karere.gresource"));
    let resource = gio::Resource::from_data(&glib::Bytes::from_static(resources_bytes))
        .expect("Failed to load resources");
    gio::resources_register(&resource);

    // Initialize the application
    let app_id = std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string());
    let app = adw::Application::builder()
        .application_id(&app_id)
        .build();

    app.connect_startup(move |app| {
        adw::init().expect("Failed to initialize Libadwaita");

        // Sync Autostart Status
        // We ensure the portal permission matches the user preference on every startup.
        // We ensure the portal permission matches the user preference on every startup.
        // app_id is captured from outer scope
        let settings = gio::Settings::new(&app_id);
        let enabled = settings.boolean("run-on-startup");
        app.activate_action("sync-autostart", Some(&enabled.to_variant()));

        // Register icons
        if let Some(display) = gtk::gdk::Display::default() {
            let icon_theme = gtk::IconTheme::for_display(&display);
            icon_theme.add_resource_path("/io/github/tobagin/karere/icons");
        }

        // Sync Autostart Action
        let action_sync_autostart = gio::SimpleAction::new("sync-autostart", Some(glib::VariantTy::BOOLEAN));
        action_sync_autostart.connect_activate(|_, parameter| {
             let enabled = parameter.unwrap().get::<bool>().unwrap();
             
             std::thread::spawn(move || {
                // Use global runtime to reuse resources/connections context potential
                RUNTIME.block_on(async {
                    let request_future = ashpd::desktop::background::Background::request()
                        .reason("Syncing autostart preference")
                        .auto_start(enabled)
                        .send();
                        
                    // Wrap in timeout
                    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), request_future).await;
                });
            });
        });
        app.add_action(&action_sync_autostart);

        // Quit Action
        let action_quit = gio::SimpleAction::new("quit", None);
        let app_weak = app.downgrade();
        action_quit.connect_activate(move |_, _| {
            if let Some(app) = app_weak.upgrade() {
                for window in app.windows() {
                    if let Ok(win) = window.clone().downcast::<KarereWindow>() {
                        win.force_close();
                    } else {
                        window.close();
                    }
                }
                app.quit();
            }
        });
        app.add_action(&action_quit);

        // About Action
        let action_about = gio::SimpleAction::new("about", None);
        let app_weak = app.downgrade();
        // Capture app_id for closure
        let app_id_clone = app_id.clone();
        action_about.connect_activate(move |_, _| {
            if let Some(app) = app_weak.upgrade() {
                if let Some(window) = app.active_window() {
                    let developers = vec![
                        "Thiago Fernandes https://github.com/tobagin", 
                        "Aman9Das https://github.com/Aman9das", 
                        "Pascal Dietrich https://github.com/", 
                        "Sabri Ünal https://github.com/yakushabb",
                        "Enrico https://github.com/account1009"
                    ];
                    let designers = vec!["Thiago Fernandes https://github.com/tobagin"];
                    let artists = vec![
                        "Thiago Fernandes https://github.com/tobagin", 
                        "Rosabel https://github.com/oiimrosabel"
                    ];
                    
                    let is_devel = app_id_clone.contains("Devel") || app_id_clone.ends_with(".Dev");
                    let app_name = if is_devel { gettext("Karere (Dev)") } else { gettext("Karere") };
                    let comments = if is_devel {
                        gettext("A modern, native GTK4/LibAdwaita wrapper for WhatsApp Web that provides seamless desktop integration with comprehensive logging and crash reporting capabilities (Development Version)")
                    } else {
                        gettext("A modern, native GTK4/LibAdwaita wrapper for WhatsApp Web that provides seamless desktop integration with comprehensive logging and crash reporting capabilities")
                    };

                    let about = adw::AboutDialog::builder()
                        .application_name(app_name)
                        .developer_name("The Karere Team") 
                        .version(env!("CARGO_PKG_VERSION"))
                        .comments(comments)
                        .website("https://tobagin.github.io/apps/karere")
                        .issue_url("https://github.com/tobagin/karere/issues")
                        .support_url("https://github.com/tobagin/karere/discussions")
                        .license_type(gtk::License::Gpl30)
                        .copyright("© 2025 The Karere Team")
                        .application_icon(&app_id_clone)
                        .developers(developers.iter().map(|s| String::from(*s)).collect::<Vec<_>>())
                        .designers(designers.iter().map(|s| String::from(*s)).collect::<Vec<_>>())
                        .artists(artists.iter().map(|s| String::from(*s)).collect::<Vec<_>>())
                        .translator_credits("Thiago Fernandes")
                        .release_notes(&get_release_notes(env!("CARGO_PKG_VERSION")))
                        .build();
                        
                    about.add_link(gettext("Source").as_str(), "https://github.com/tobagin/karere");
                    
                    about.add_acknowledgement_section(
                        Some(gettext("Special Thanks").as_str()), 
                        &["The GNOME Project", "The WebKitGTK Team", "WhatsApp Inc.", "LibAdwaita Contributors", "The Rust Project"]
                    );

                    about.present(Some(&window));
                }
            }
        });
        app.add_action(&action_about);

        // Preferences Action
        let action_preferences = gio::SimpleAction::new("preferences", None);
        let app_weak = app.downgrade();
        action_preferences.connect_activate(move |_, _| {
             if let Some(app) = app_weak.upgrade() {
                if let Some(window) = app.active_window() {
                    preferences::show(&window);
                }
            }
        });
        app.add_action(&action_preferences);

        // Help Overlay Action (App Scope)
        let action_help = gio::SimpleAction::new("show-help-overlay", None);
        let app_weak = app.downgrade();
        action_help.connect_activate(move |_, _| {
            if let Some(app) = app_weak.upgrade() {
                 if let Some(window) = app.active_window() {
                     let builder = gtk::Builder::from_resource("/io/github/tobagin/karere/ui/keyboard-shortcuts.ui");
                     // Support both Adw.Dialog (newer) and Gtk.Window (fallback)
                     if let Some(dialog) = builder.object::<adw::Dialog>("shortcuts_dialog") {
                        dialog.present(Some(&window));
                     } else if let Some(dialog) = builder.object::<gtk::Window>("shortcuts_dialog") {
                        dialog.set_transient_for(Some(&window));
                        dialog.present();
                     } else {
                         eprintln!("Failed to load shortcuts_dialog");
                     }
                 }
            }
        });
        app.add_action(&action_help);

        // Set Accelerators
        app.set_accels_for_action("win.new-chat", &["<Control>n"]);
        app.set_accels_for_action("win.refresh", &["<Control>r"]);
        app.set_accels_for_action("app.preferences", &["<Control>comma"]);
        app.set_accels_for_action("app.show-help-overlay", &["<Control>question", "<Control>slash"]);
        app.set_accels_for_action("app.about", &["F1"]);
        app.set_accels_for_action("app.quit", &["<Control>q"]);
        app.set_accels_for_action("win.show-devtools", &["F12"]);

        app.set_accels_for_action("win.show-devtools", &["F12"]);

        // Open Download Action
        let action_open_download = gio::SimpleAction::new("open-download", Some(glib::VariantTy::STRING));
        let app_weak = app.downgrade();
        action_open_download.connect_activate(move |_, parameter| {
             if let Some(app) = app_weak.upgrade() {
                 let uri_str = parameter
                     .expect("Could not get parameter")
                     .get::<String>()
                     .expect("The value is not a string");
                 
                 // Extract file path from URI or treat as direct path
                 let file_obj = gio::File::for_uri(&uri_str);
                 let path_opt = if file_obj.path().is_some() {
                     file_obj.path()
                 } else {
                     gio::File::for_path(&uri_str).path()
                 };

                 if let Some(path) = path_opt {
                     if let Ok(file) = std::fs::File::open(&path) {
                         let window = app.active_window();
                         if let Some(win) = window {
                             let win_ref = win.clone();
                             glib::MainContext::default().spawn_local(async move {
                                     // Enter Tokio context for zbus/ashpd, preventing "no reactor" panic
                                     let _guard = RUNTIME.enter();
                                     
                                     let request = ashpd::desktop::open_uri::OpenFileRequest::default();
                                     let request = if let Some(native) = win_ref.native() {
                                         request.identifier(ashpd::WindowIdentifier::from_native(&native).await)
                                     } else {
                                         request
                                     };
                                     
                                     let result = request
                                         .ask(true) 
                                         .send_file(&file).await;
                                         
                                     if let Err(e) = result {
                                         eprintln!("Failed to open file via portal: {}", e);
                                     }
                             });
                         }
                     } else {
                         eprintln!("Failed to open file for reading: {}", path.display());
                     }
                 } else {
                     eprintln!("Invalid file URI/Path: {}", uri_str);
                 }
             }
        });
        app.add_action(&action_open_download);

        // Present Window Action (for Notifications)
        let action_present = gio::SimpleAction::new("present-window", None);
        let app_weak = app.downgrade();
        action_present.connect_activate(move |_, _| {
            if let Some(app) = app_weak.upgrade() {
                if let Some(window) = app.active_window() {
                    window.set_visible(true); // Force visible
                    window.present();
                } else if let Some(window) = app.windows().first() {
                     window.set_visible(true); // Force visible
                     window.present();
                }
            }
        });
        app.add_action(&action_present);
        // Notification Click Action
        let action_notification_click = gio::SimpleAction::new("notification-clicked", Some(glib::VariantTy::VARIANT));
        let app_weak = app.downgrade();
        action_notification_click.connect_activate(move |_, parameter| {
             if let Some(app) = app_weak.upgrade() {
                 let id = if let Some(v) = parameter {
                     // Try extracting as string directly
                     if let Some(s) = v.get::<String>() {
                         s
                     } 
                     // Try extracting if it's a tuple containing a string (s)
                     else if let Some(s) = v.child_value(0).get::<String>() {
                         s
                     }
                     else {
                         String::new()
                     }
                 } else {
                     String::new()
                 };
                 


                 if !id.is_empty() {
                     if let Some(window) = app.active_window() {
                         if let Ok(win) = window.downcast::<KarereWindow>() {
                             win.process_notification_click(&id);
                             win.set_visible(true);
                             win.present();
                         }
                     } else if let Some(window) = app.windows().first() {
                         if let Ok(win) = window.clone().downcast::<KarereWindow>() {
                             win.process_notification_click(&id);
                             win.set_visible(true);
                             win.present();
                         }
                     }
                 }
             }
        });
        
        app.add_action(&action_notification_click);

    });

    // Start Tray Icon
    let app_id_files = std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string());
    let settings = gio::Settings::new(&app_id_files);
    
    let tray_behavior = settings.string("systray-icon");
    let should_spawn_tray = match tray_behavior.as_str() {
        "disabled" => false,
        "enabled" => true,
        "auto" | _ => {
             let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();
             !desktop.contains("gnome")
        }
    };

    // Visibility state
    let start_hidden = settings.boolean("start-in-background");
    // If tray is NOT spawned, we MUST be visible, otherwise there is no way to access the window.
    let initial_visibility = if !should_spawn_tray {
        true
    } else {
        !start_hidden
    };
    
    let visible = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(initial_visibility));
    let visible_clone = visible.clone();
    
    let has_unread = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));


    let tray_handle = if should_spawn_tray {
        match tray::spawn_tray(visible.clone(), has_unread.clone()) {
            Ok(handle) => Some(handle),
            Err(e) => {
                eprintln!("Warning: Failed to start tray icon: {}", e);
                visible.store(true, Ordering::Relaxed);
                None
            }
        }
    } else {
        None
    };
    
    // Action: Set Unread Status
    let action_set_unread = gio::SimpleAction::new("set-unread", Some(&*glib::VariantTy::BOOLEAN));
    let tray_handle_clone = tray_handle.clone();
    let has_unread_clone_2 = has_unread.clone();
    
    action_set_unread.connect_activate(move |_, parameter| {
        let is_unread = parameter
            .expect("Could not get parameter")
            .get::<bool>()
            .expect("The value is not a boolean");
            
        // Only update if changed
        let was_unread = has_unread_clone_2.load(Ordering::Relaxed);
        if was_unread != is_unread {
            has_unread_clone_2.store(is_unread, Ordering::Relaxed);
            if let Some(handle) = &tray_handle_clone {
                 let handle = handle.clone();
                 std::thread::spawn(move || {
                     if let Ok(rt) = tokio::runtime::Runtime::new() {
                         rt.block_on(async move {
                             let _ = handle.update(|_| {}).await;
                         });
                     }
                 });

            }
        }
    });
    app.add_action(&action_set_unread);

    app.connect_activate(move |app| {
        if let Some(window) = app.active_window() {
            window.set_visible(true);
            window.present();
            return;
        }
        
        if let Some(window) = app.windows().first() {
             window.set_visible(true);
             window.present();
             return;
        }

        let window = KarereWindow::new(app);

        // TEST NOTIFICATION REMOVED

        
        let visible = visible_clone.clone();
        if visible.load(std::sync::atomic::Ordering::Relaxed) {
             window.present();
        } else {
             window.set_visible(false);
             
             // Ensure window is realized so WebKit loads resources even if hidden
             gtk::prelude::WidgetExt::realize(&window);
        }

        // Sync visibility

        let visible = visible_clone.clone();
        let tray_handle = tray_handle.clone();
        window.connect_notify_local(Some("visible"), move |window, _| {
            visible.store(window.is_visible(), std::sync::atomic::Ordering::Relaxed);
            if let Some(handle) = &tray_handle {
                let handle = handle.clone();
                std::thread::spawn(move || {
                     if let Ok(rt) = tokio::runtime::Runtime::new() {
                         rt.block_on(async move {
                             let _ = handle.update(|_| {}).await;
                         });
                     }
                });
            }
        });
    });


    // Run the application
    app.run();

    Ok(())
}

fn get_release_notes(version: &str) -> String {
    // Try possible paths for metainfo to read release notes
    let paths = [
        format!("/app/share/metainfo/io.github.tobagin.karere.metainfo.xml"), // Flatpak sandbox path (renamed during build, but might be original name depending on install)
        // Actually, in Devel build we rename it to io.github.tobagin.karere2.Devel.metainfo.xml
        // We should try checking env vars or multiple names
        format!("/app/share/metainfo/io.github.tobagin.karere2.Devel.metainfo.xml"),
        format!("/usr/share/metainfo/io.github.tobagin.karere.metainfo.xml"),
        format!("data/io.github.tobagin.karere.metainfo.xml"), // Local dev
    ];

    for path in paths {
        if let Ok(content) = std::fs::read_to_string(&path) {
            let version_tag = format!("version=\"{}\"", version);
            if let Some(pos) = content.find(&version_tag) {
                if let Some(desc_start) = content[pos..].find("<description>") {
                     if let Some(desc_end) = content[pos + desc_start..].find("</description>") {
                         let raw = &content[pos + desc_start + 13 .. pos + desc_start + desc_end];
                         return raw.trim().to_string();
                     }
                }
            }
        }
    }
    String::new()
}
