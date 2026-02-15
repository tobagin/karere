use std::error::Error;
use gtk::prelude::*;
use gtk::{gio, glib};
use libadwaita as adw;

use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};

use gettextrs::gettext;

/// Account info tuple: (id, name, emoji, has_unread)
pub type AccountInfo = (String, String, String, bool);

pub struct KarereTray {
    pub visible: Arc<AtomicBool>,
    pub has_unread: Arc<AtomicBool>,
    pub accounts: Arc<Mutex<Vec<AccountInfo>>>,
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
            if let Some(app) = gio::Application::default()
                && let Ok(gtk_app) = app.downcast::<gtk::Application>()
                    && let Some(window) = gtk_app.windows().first() {
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

        let mut items: Vec<ksni::MenuItem<Self>> = vec![
            StandardItem {
                label,
                activate: Box::new(|_| {
                    glib::MainContext::default().invoke(move || {
                        if let Some(app) = gio::Application::default()
                            && let Ok(gtk_app) = app.downcast::<gtk::Application>()
                                && let Some(window) = gtk_app.windows().first() {
                                    let app_id = std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string());
                                    let settings = gio::Settings::new(&app_id);
                                    
                                    if window.is_visible() {
                                        let width = window.width();
                                        let height = window.height();
                                        let _ = settings.set_int("window-width", width);
                                        let _ = settings.set_int("window-height", height);
                                        window.set_visible(false);
                                    } else {
                                        let width = settings.int("window-width");
                                        let height = settings.int("window-height");
                                        if let Ok(adw_window) = window.clone().downcast::<adw::ApplicationWindow>() {
                                            adw_window.set_default_size(width, height);
                                        }
                                        window.present();
                                    }
                                }
                    });
                }),
                ..Default::default()
            }
            .into(),
        ];

        // Add account entries when multiple accounts exist
        let accounts = self.accounts.lock().unwrap_or_else(|e| e.into_inner());
        if accounts.len() > 1 {
            // Separator
            items.push(MenuItem::Separator);

            for (id, name, emoji, has_unread) in accounts.iter() {
                let label = if *has_unread {
                    format!("● {} {}", emoji, name)
                } else {
                    format!("{} {}", emoji, name)
                };
                let account_id = id.clone();
                items.push(
                    StandardItem {
                        label,
                        activate: Box::new(move |_| {
                            let account_id = account_id.clone();
                            glib::MainContext::default().invoke(move || {
                                if let Some(app) = gio::Application::default() {
                                    app.activate_action("switch-account", Some(&account_id.to_variant()));
                                }
                            });
                        }),
                        ..Default::default()
                    }
                    .into(),
                );
            }

            // Separator before Quit
            items.push(MenuItem::Separator);
        }

        items.push(
            StandardItem {
                label: gettext("Quit"),
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
        );

        items
    }
}

use ksni::TrayMethods;

pub fn spawn_tray(
    visible: Arc<AtomicBool>,
    has_unread: Arc<AtomicBool>,
    accounts: Arc<Mutex<Vec<AccountInfo>>>,
) -> Result<ksni::Handle<KarereTray>, Box<dyn Error>> {
    let tray = KarereTray { visible, has_unread, accounts };
    let handle = crate::RUNTIME.block_on(
        tray.disable_dbus_name(true)
            .assume_sni_available(true)
            .spawn(),
    )?;
    Ok(handle)
}
