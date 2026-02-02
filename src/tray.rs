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

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        // Only provide pixmap on KDE - let GNOME use icon_name
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        if !desktop.contains("KDE") {
            return Vec::new();
        }

        //  Detect theme via environment variable (set by KDE)
        let is_dark = detect_dark_theme();
        
        // Get icon name and render it
        let app_id = std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string());
        let icon_name = if self.has_unread.load(Ordering::Relaxed) {
            format!("{}-new-message-symbolic", app_id)
        } else {
            format!("{}-symbolic", app_id)
        };

        // Render SVG with appropriate color
        match render_svg_icon(&icon_name, 22, is_dark) {
            Ok(pixmap) => vec![pixmap],
            Err(_) => Vec::new(),
        }
    }

/// Detect dark theme via KDE environment variables
fn detect_dark_theme() -> bool {
    // We'll just use a medium gray that works on both themes
    // Return value doesn't matter since we use the same color
    false
}

/// Render SVG icon with color replacement based on theme
fn render_svg_icon(icon_name: &str, size: i32, _is_dark: bool) -> Result<ksni::Icon, Box<dyn Error>> {
    // Find the SVG file
    let icon_path = find_icon_path(icon_name)?;
    
    // Read and modify SVG content
    let svg_content = fs::read_to_string(&icon_path)?;
    
    // Use medium gray (#6e6e6e) that works on both light and dark backgrounds
    let color = "#6e6e6e";
    
    // Replace both currentColor and hardcoded colors
    let modified_svg = svg_content
        .replace("currentColor", color)
        .replace("#2e3436", color);
    
    // Render SVG to pixmap
    let handle = rsvg::Loader::new().read_stream::<gio::MemoryInputStream, gio::File, gio::Cancellable>(
        &gio::MemoryInputStream::from_bytes(&glib::Bytes::from(modified_svg.as_bytes())),
        None::<&gio::File>,
        None::<&gio::Cancellable>,
    )?;
    
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, size, size)?;
    let width = surface.width();
    let height = surface.height();
    
    {
        let cr = cairo::Context::new(&surface)?;
        let renderer = rsvg::CairoRenderer::new(&handle);
        let viewport = cairo::Rectangle::new(0.0, 0.0, size as f64, size as f64);
        renderer.render_document(&cr, &viewport)?;
    }
    
    let data = surface.data()?;
    
    Ok(ksni::Icon {
        width,
        height,
        data: data.to_vec(),
    })
}

/// Find icon file path
fn find_icon_path(icon_name: &str) -> Result<PathBuf, Box<dyn Error>> {
    let paths = vec![
        format!("/app/share/icons/hicolor/symbolic/apps/{}.svg", icon_name),
        format!("/usr/share/icons/hicolor/symbolic/apps/{}.svg", icon_name),
    ];
    
    for path in paths {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Ok(p);
        }
    }
    
    Err("Icon not found".into())
}
