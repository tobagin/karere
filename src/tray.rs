use ksni;
use std::error::Error;
use gtk::prelude::*;
use gtk::{gio, glib};
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
        ksni::Category::ApplicationStatus
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        println!("Tray clicked!");
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
                                    if window.is_visible() {
                                        window.set_visible(false);
                                    } else {
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
