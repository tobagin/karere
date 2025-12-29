use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use gtk::{gio, glib};
use std::sync::atomic::Ordering;

mod tray;
mod window;
mod preferences;
mod spellcheck;

use window::KarereWindow;

fn main() -> anyhow::Result<()> {
    env_logger::init();
    
    // Ensure compiled schemas are found (fix for running locally from cargo)
    // ONLY do this if NOT inside Flatpak, otherwise we break system schemas!
    if std::env::var("FLATPAK_ID").is_err() {
        println!("DEBUG: Not in Flatpak. Applying Schema Override.");
        let current_dir = std::env::current_dir().expect("Failed to get current directory");
        let schema_dir = current_dir.join("data");
        // Only set if the directory actually exists to be safe
        if schema_dir.exists() {
             unsafe {
                 std::env::set_var("GSETTINGS_SCHEMA_DIR", schema_dir);
             }
        }
    } else {
        println!("DEBUG: Flatpak detected ({:?}). Skipping Schema Override.", std::env::var("FLATPAK_ID"));
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

    println!("Starting main... App ID: {}", app_id);

    app.connect_startup(move |app| {
        println!("Startup signal received.");
        adw::init().expect("Failed to initialize Libadwaita");

        // Request background permission
        std::thread::spawn(|| {
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async {
                    println!("Requesting background permission...");
                    match ashpd::desktop::background::Background::request()
                        .reason("Karere needs to run in the background to receive notifications.")
                        .auto_start(true)
                        .send()
                        .await
                    {
                        Ok(response) => {
                             println!("Background permission requested: {:?}", response.response());
                             async fn set_status_msg() -> ashpd::Result<()> {
                                 // Use zbus directly to call SetStatus
                                 let connection = zbus::Connection::session().await?;
                                 let proxy = zbus::Proxy::new(
                                     &connection, 
                                     "org.freedesktop.portal.Desktop", 
                                     "/org/freedesktop/portal/desktop", 
                                     "org.freedesktop.portal.Background"
                                 ).await?;
                                 
                                 let mut options = std::collections::HashMap::new();
                                 options.insert("message", zbus::zvariant::Value::from("Running in background"));
                                 
                                 proxy.call_method("SetStatus", &(options)).await?;
                                 Ok(())
                             }

                             if let Err(e) = set_status_msg().await {
                                 // "NotAllowed" is expected if running outside sandbox (e.g. cargo run)
                                 eprintln!("Warning: Failed to set background status (normal if not sandboxed): {}", e);
                             } else {
                                 println!("Background status set.");
                             }
                        }
                        Err(e) => {
                             eprintln!("Failed to request background permission: {}", e);
                        }
                    }
                });
            } else {
                eprintln!("Failed to create Tokio runtime for background permission request.");
            }
        });

        // Register icons
        if let Some(display) = gtk::gdk::Display::default() {
            let icon_theme = gtk::IconTheme::for_display(&display);
            icon_theme.add_resource_path("/io/github/tobagin/karere/icons");
        }

        // Quit Action
        let action_quit = gio::SimpleAction::new("quit", None);
        let app_weak = app.downgrade();
        action_quit.connect_activate(move |_, _| {
            if let Some(app) = app_weak.upgrade() {
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
                    let developers = vec!["Thiago Fernandes", "Aman9Das", "Cameo"];
                    let designers = vec!["Thiago Fernandes"];
                    let artists = vec!["Thiago Fernandes"];
                    
                    let is_devel = app_id_clone.contains("Devel");
                    let app_name = if is_devel { "Karere (Dev)" } else { "Karere" };
                    let comments = if is_devel {
                        "A modern, native GTK4/LibAdwaita wrapper for WhatsApp Web that provides seamless desktop integration with comprehensive logging and crash reporting capabilities (Development Version)"
                    } else {
                        "A modern, native GTK4/LibAdwaita wrapper for WhatsApp Web that provides seamless desktop integration with comprehensive logging and crash reporting capabilities"
                    };

                    let about = adw::AboutDialog::builder()
                        .application_name(app_name)
                        .developer_name("The Karere Team") 
                        .version("2.0.0")
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
                        .release_notes(&get_release_notes("2.0.0"))
                        .build();
                        
                    about.add_link("Source", "https://github.com/tobagin/karere");
                    
                    about.add_acknowledgement_section(
                        Some("Special Thanks"), 
                        &["The GNOME Project", "The WebKitGTK Team", "WhatsApp Inc.", "LibAdwaita Contributors", "Vala Programming Language Team"]
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
                     let builder = gtk::Builder::from_resource("/io/github/tobagin/karere/help-overlay.ui");
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
        app.set_accels_for_action("win.refresh", &["<Control>r"]);
        app.set_accels_for_action("app.preferences", &["<Control>comma"]);
        app.set_accels_for_action("app.show-help-overlay", &["<Control>question", "<Control>slash"]);
        app.set_accels_for_action("app.about", &["F1"]);
        app.set_accels_for_action("app.quit", &["<Control>q"]);

        // Present Window Action (for Notifications)
        let action_present = gio::SimpleAction::new("present-window", None);
        let app_weak = app.downgrade();
        action_present.connect_activate(move |_, _| {
            if let Some(app) = app_weak.upgrade() {
                if let Some(window) = app.active_window() {
                    window.present();
                } else if let Some(window) = app.windows().first() {
                     window.present();
                }
            }
        });
        app.add_action(&action_present);
    });

    // Start Tray Icon
    println!("Initializing settings...");
    let settings = gio::Settings::new("io.github.tobagin.karere");
    println!("Settings initialized.");
    
    let tray_behavior = settings.string("systray-icon");
    println!("Tray behavior: {}", tray_behavior);
    
    let should_spawn_tray = match tray_behavior.as_str() {
        "disabled" => false,
        "enabled" => true,
        "auto" | _ => {
             // Simple auto detection: Check XDG_CURRENT_DESKTOP
             let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();
             !desktop.contains("gnome")
        }
    };
    println!("Should spawn tray: {}", should_spawn_tray);

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


    println!("Spawning tray handle...");
    let tray_handle = if should_spawn_tray {
        match tray::spawn_tray(visible.clone(), has_unread.clone()) {
            Ok(handle) => Some(handle),
            Err(e) => {
                eprintln!("Warning: Failed to start tray icon: {}", e);
                // If tray fails, ensure window is visible so user can access app
                visible.store(true, Ordering::Relaxed);
                None
            }
        }
    } else {
        None
    };
    println!("Tray handle spawned.");
    
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
                 println!("Tray icon updated (unread={})", is_unread);
            }
        }
    });
    app.add_action(&action_set_unread);

    app.connect_activate(move |app| {
        println!("Activate signal received.");
        let window = KarereWindow::new(app);

        // TEST NOTIFICATION REMOVED

        
        let visible = visible_clone.clone();
        if visible.load(std::sync::atomic::Ordering::Relaxed) {
             window.present();
        } else {
             window.set_visible(false);
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
