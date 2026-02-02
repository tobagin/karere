use ksni;
use std::error::Error;
use gtk::prelude::*;
use gtk::{gio, glib};
use libadwaita as adw;
use std::fs;
use std::path::PathBuf;
use cairo;


use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use gettextrs::gettext;

pub struct KarereTray {
    pub visible: Arc<AtomicBool>,
    pub has_unread: Arc<AtomicBool>,
}

impl ksni::Tray for KarereTray {
    fn icon_name(&self) -> String {
        let app_id = std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string());
        if self.has_unread.load(Ordering::Relaxed) {
             format!("{}-new-message-symbolic", app_id)
        } else {
             format!("{}-symbolic", app_id)
        }
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        // Only provide pixmap on KDE - let GNOME use icon_name
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        log::info!("icon_pixmap called, XDG_CURRENT_DESKTOP: {}", desktop);
        
        if !desktop.contains("KDE") {
            log::info!("Not KDE, returning empty pixmap vector");
            return Vec::new();
        }

        // Determine if we're on dark theme
        let is_dark = is_dark_theme();
        log::info!("Theme is dark: {}", is_dark);
        
        // Get icon name and render it
        let app_id = std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string());
        let icon_name = if self.has_unread.load(Ordering::Relaxed) {
            format!("{}-new-message-symbolic", app_id)
        } else {
            format!("{}-symbolic", app_id)
        };
        
        log::info!("Rendering icon: {}", icon_name);

        // Render SVG with appropriate color
        match render_svg_icon(&icon_name, 22, is_dark) {
            Ok(pixmap) => {
                log::info!("Successfully rendered icon pixmap: {}x{}", pixmap.width, pixmap.height);
                vec![pixmap]
            }
            Err(e) => {
                log::error!("Failed to render icon pixmap: {}", e);
                Vec::new()
            }
        }
    }


    fn title(&self) -> String {
        gettext("Karere")
    }

    fn id(&self) -> String {
        std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string())
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::Communications
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        // Toggle window visibility on tray icon click
        glib::MainContext::default().invoke(move || {
            if let Some(app) = gio::Application::default() {
                if let Ok(gtk_app) = app.downcast::<gtk::Application>() {
                    if let Some(window) = gtk_app.windows().first() {
                        let app_id = std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string());
                        let settings = gio::Settings::new(&app_id);
                        
                        if window.is_visible() {
                            // Save window size before hiding
                            let width = window.width();
                            let height = window.height();
                            let _ = settings.set_int("window-width", width);
                            let _ = settings.set_int("window-height", height);
                            window.set_visible(false);
                        } else {
                            // Restore window size and present
                            let width = settings.int("window-width");
                            let height = settings.int("window-height");
                            if let Ok(adw_window) = window.clone().downcast::<adw::ApplicationWindow>() {
                                adw_window.set_default_size(width, height);
                            }
                            window.present();
                        }
                    }
                }
            }
        });
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: gettext("Karere"),
            description: gettext("Running via Rust & GTK4"),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        let label = if self.visible.load(Ordering::Relaxed) {
             gettext("Hide Window")
        } else {
             gettext("Show Window")
        };
        vec![
            StandardItem {
                label: label.into(),
                activate: Box::new(|_| {
                    glib::MainContext::default().invoke(move || {
                        if let Some(app) = gio::Application::default() {
                            if let Ok(gtk_app) = app.downcast::<gtk::Application>() {
                                if let Some(window) = gtk_app.windows().first() {
                                    let app_id = std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string());
                                    let settings = gio::Settings::new(&app_id);
                                    
                                    if window.is_visible() {
                                        // Save window size before hiding
                                        let width = window.width();
                                        let height = window.height();
                                        let _ = settings.set_int("window-width", width);
                                        let _ = settings.set_int("window-height", height);
                                        window.set_visible(false);
                                    } else {
                                        // Restore window size and present
                                        let width = settings.int("window-width");
                                        let height = settings.int("window-height");
                                        if let Ok(adw_window) = window.clone().downcast::<adw::ApplicationWindow>() {
                                            adw_window.set_default_size(width, height);
                                        }
                                        window.present();
                                    }
                                }
                            }
                        }
                    });
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: gettext("Quit").into(),
                activate: Box::new(|_| {
                     glib::MainContext::default().invoke(move || {
                        if let Some(app) = gio::Application::default() {
                            app.activate_action("quit", None);
                        }
                     });
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

use ksni::TrayMethods;

pub fn spawn_tray(visible: Arc<AtomicBool>, has_unread: Arc<AtomicBool>) -> Result<ksni::Handle<KarereTray>, Box<dyn Error>> {
    let tray = KarereTray { visible, has_unread };
    let rt = tokio::runtime::Runtime::new()?;
    // We block on spawn. Note: If ksni requires the runtime to stay alive for the tray to function, 
    // this might fail at runtime. But let's try to compile first.
    // Actually, to be safe, let's leak the runtime so it keeps running background tasks?
    // Or assume ksni/zbus handles its own threads.
    // Using block_on for now.
    let handle = rt.block_on(tray.disable_dbus_name(true).spawn())?;
    // We might need to keep rt alive? 
    // Let's store it in a static if needed, but simplest fix first.
    // Actually, zbus 5 usually requires an async executor running. 
    // If we drop `rt`, the executor stops.
    // So we should probably leak `rt` or store it.
    std::mem::forget(rt); 
    Ok(handle)
}

/// Detect if the system is using a dark theme
fn is_dark_theme() -> bool {
    // TODO: Detect theme without GTK (which isn't initialized yet when tray spawns)
    // For now, default to light theme
    // We could use XDG portal or read KDE/GNOME settings directly
    false
}

/// Render SVG icon with color replacement based on theme
fn render_svg_icon(icon_name: &str, size: i32, is_dark: bool) -> Result<ksni::Icon, Box<dyn Error>> {
    // Find the SVG file in the icon theme
    let icon_path = find_icon_path(icon_name)?;
    
    // Read and modify SVG content based on theme
    let svg_content = fs::read_to_string(&icon_path)?;
    let color = if is_dark { "#ffffff" } else { "#000000" };
    let modified_svg = svg_content.replace("#2e3436", color);
    
    // Render SVG to pixmap using librsvg and cairo
    let handle = rsvg::Loader::new().read_stream::<gio::MemoryInputStream, gio::File, gio::Cancellable>(
        &gio::MemoryInputStream::from_bytes(&glib::Bytes::from(modified_svg.as_bytes())),
        None::<&gio::File>,
        None::<&gio::Cancellable>,
    )?;
    
    
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, size, size)?;
    
    // Get dimensions first
    let width = surface.width();
    let height = surface.height();
    
    // Render in a scope so cr gets dropped
    {
        let cr = cairo::Context::new(&surface)?;
        let renderer = rsvg::CairoRenderer::new(&handle);
        let viewport = cairo::Rectangle::new(0.0, 0.0, size as f64, size as f64);
        renderer.render_document(&cr, &viewport)?;
    } // cr dropped here
    
    // Now we can get exclusive access to surface data
    let data = surface.data()?;
    
    Ok(ksni::Icon {
        width,
        height,
        data: data.to_vec(),
    })
}

/// Find icon file path in the icon theme directories  
fn find_icon_path(icon_name: &str) -> Result<PathBuf, Box<dyn Error>> {
    let home = std::env::var("HOME")?;
    let home_path = format!("{}/.local/share/icons/hicolor/symbolic/apps", home);
    let icon_dirs = vec![
        "/app/share/icons/hicolor/symbolic/apps",  // Flatpak location
        "/usr/share/icons/hicolor/symbolic/apps",
        &home_path,
    ];
    
    for dir in icon_dirs {
        let path = PathBuf::from(dir).join(format!("{}.svg", icon_name));
        if path.exists() {
            return Ok(path);
        }
    }
    
    Err(format!("Icon {} not found", icon_name).into())
}
