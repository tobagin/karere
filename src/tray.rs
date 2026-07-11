//! System-tray `StatusNotifierItem` via the `ksni` crate. State lives in a shared
//! `Arc<Mutex<TrayState>>`: the tokio tray task reads it, main-thread GAction
//! handlers write it then refresh.

use std::sync::{Arc, Mutex};

use gettextrs::gettext;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use ksni::TrayMethods;

use crate::application::APP_ID;

/// One account row in the tray's right-click menu.
#[derive(Clone)]
pub struct AccountSummary {
    pub id: String,
    pub name: String,
    pub has_unread: bool,
    pub icon_png: Option<Vec<u8>>,
}

/// Cross-thread tray state (tokio reads, main thread writes).
#[derive(Default)]
pub struct TrayState {
    pub unread_count: u32,
    pub accounts: Vec<AccountSummary>,
    /// Drives the `Show / Hide` menu label.
    pub window_visible: bool,
}

pub struct KarereTray {
    state: Arc<Mutex<TrayState>>,
}

impl ksni::Tray for KarereTray {
    fn id(&self) -> String {
        APP_ID.to_owned()
    }

    fn title(&self) -> String {
        gettext("Karere")
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::Communications
    }

    fn icon_name(&self) -> String {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        // Must be app-id-prefixed: Flatpak only exports `$FLATPAK_ID*` icons, so a
        // bare name is dropped and the panel can't resolve it.
        let base = std::env::var("FLATPAK_ID").unwrap_or_else(|_| APP_ID.to_owned());
        if state.unread_count > 0 {
            format!("{base}-tray-unread-symbolic")
        } else {
            format!("{base}-tray-symbolic")
        }
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.unread_count > 0 {
            ksni::ToolTip {
                title: gettext("Karere"),
                // Translators: tray tooltip body; leading number is the unread count.
                description: format!("{} {}", state.unread_count, gettext("unread")),
                ..Default::default()
            }
        } else {
            ksni::ToolTip {
                title: gettext("Karere"),
                ..Default::default()
            }
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        activate_app_action("present-window", None);
    }

    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        activate_app_action("present-window", None);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;

        let toggle_label = {
            let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.window_visible {
                gettext("Hide Window")
            } else {
                gettext("Show Window")
            }
        };

        let account_items: Vec<ksni::MenuItem<Self>> = {
            let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state
                .accounts
                .iter()
                .map(|a| {
                    let id = a.id.clone();
                    let label = if a.has_unread {
                        format!("● {}", a.name)
                    } else {
                        a.name.clone()
                    };
                    StandardItem {
                        label,
                        icon_data: a.icon_png.clone().unwrap_or_default(),
                        activate: Box::new(move |_| {
                            activate_app_action("switch-account", Some(id.to_variant()));
                        }),
                        ..Default::default()
                    }
                    .into()
                })
                .collect()
        };

        let mut items: Vec<ksni::MenuItem<Self>> = vec![
            StandardItem {
                label: toggle_label,
                activate: Box::new(|_| activate_app_action("present-window", None)),
                ..Default::default()
            }
            .into(),
        ];
        if !account_items.is_empty() {
            items.push(MenuItem::Separator);
            items.extend(account_items);
        }
        items.extend([
            MenuItem::Separator,
            StandardItem {
                label: gettext("Preferences"),
                activate: Box::new(|_| activate_app_action("preferences", None)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: gettext("Keyboard Shortcuts"),
                activate: Box::new(|_| activate_app_action("show-help-overlay", None)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: gettext("About Karere"),
                activate: Box::new(|_| activate_app_action("about", None)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: gettext("Quit"),
                activate: Box::new(|_| activate_app_action("quit", None)),
                ..Default::default()
            }
            .into(),
        ]);
        items
    }
}

/// Activate an app GAction from a tray callback; hops to the glib main context.
fn activate_app_action(name: &str, target: Option<glib::Variant>) {
    let name = name.to_owned();
    glib::MainContext::default().invoke(move || {
        if let Some(app) = gio::Application::default() {
            app.activate_action(&name, target.as_ref());
        }
    });
}

struct TrayHolder {
    state: Arc<Mutex<TrayState>>,
    handle: ksni::Handle<KarereTray>,
}

static TRAY: Mutex<Option<TrayHolder>> = Mutex::new(None);

fn tray_lock() -> std::sync::MutexGuard<'static, Option<TrayHolder>> {
    TRAY.lock().unwrap_or_else(|e| e.into_inner())
}

/// Start the tray service per `systray-icon`, the GNOME skip policy, and
/// `KARERE_FORCE_TRAY=1`. Idempotent.
pub fn start() {
    if tray_lock().is_some() {
        return;
    }

    let mode = gio::Settings::new(APP_ID).string("systray-icon");
    if mode == "disabled" {
        log::info!("tray disabled via systray-icon setting");
        return;
    }
    let force = mode == "enabled" || std::env::var("KARERE_FORCE_TRAY").as_deref() == Ok("1");
    if !force && should_skip_on_gnome() {
        log::info!("tray skipped (GNOME w/o AppIndicator)");
        return;
    }

    let state = Arc::new(Mutex::new(TrayState::default()));
    let tray = KarereTray {
        state: state.clone(),
    };
    // disable_dbus_name + assume_sni_available so the item registers under Flatpak
    // and when the watcher appears slightly after us.
    match crate::actions::runtime().block_on(
        tray.disable_dbus_name(true)
            .assume_sni_available(true)
            .spawn(),
    ) {
        Ok(handle) => {
            *tray_lock() = Some(TrayHolder { state, handle });
            log::info!("tray service started");
        }
        Err(err) => log::warn!("tray service failed to start: {err}"),
    }
}

/// Stop the running tray service; a later `start()` can re-create it.
pub fn stop() {
    let holder = tray_lock().take();
    if let Some(holder) = holder {
        holder.handle.shutdown();
        log::info!("tray service stopped");
    }
}

/// Apply the current `systray-icon` setting live.
pub fn apply_setting() {
    let mode = gio::Settings::new(APP_ID).string("systray-icon");
    let want = match mode.as_str() {
        "disabled" => false,
        "enabled" => true,
        _ => {
            std::env::var("KARERE_FORCE_TRAY").as_deref() == Ok("1") || !should_skip_on_gnome()
        }
    };
    if want {
        start();
    } else {
        stop();
    }
}

/// Skip the tray on GNOME unless an SNI watcher (AppIndicator extension) is present.
fn should_skip_on_gnome() -> bool {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    // May be colon-separated (e.g. "ubuntu:GNOME").
    let is_gnome = desktop
        .split(':')
        .any(|entry| entry.eq_ignore_ascii_case("GNOME"));
    if !is_gnome {
        return false;
    }
    !watcher_present()
}

/// Whether `org.kde.StatusNotifierWatcher` has an owner on the session bus.
fn watcher_present() -> bool {
    crate::actions::runtime().block_on(async {
        let conn = match zbus::Connection::session().await {
            Ok(conn) => conn,
            Err(err) => {
                log::warn!("tray: session bus unavailable while probing watcher: {err}");
                return false;
            }
        };
        let proxy = match zbus::fdo::DBusProxy::new(&conn).await {
            Ok(proxy) => proxy,
            Err(err) => {
                log::warn!("tray: DBusProxy unavailable while probing watcher: {err}");
                return false;
            }
        };
        let name = match zbus::names::BusName::try_from("org.kde.StatusNotifierWatcher") {
            Ok(name) => name,
            Err(_) => return false,
        };
        proxy.name_has_owner(name).await.unwrap_or(false)
    })
}

/// Current unread count held in tray state (0 when no tray is running).
pub fn unread_count() -> u32 {
    tray_lock()
        .as_ref()
        .map(|tray| {
            tray.state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .unread_count
        })
        .unwrap_or(0)
}

/// Write `count` into tray state and request a refresh (`app.set-unread`).
pub fn set_unread(count: u32) {
    let guard = tray_lock();
    let Some(tray) = guard.as_ref() else { return };
    tray.state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .unread_count = count;
    refresh(&tray.handle);
}

/// Sync window visibility into tray state, refreshing only on a real change.
pub fn set_window_visible(visible: bool) {
    let guard = tray_lock();
    let Some(tray) = guard.as_ref() else { return };
    {
        let mut state = tray.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.window_visible == visible {
            return;
        }
        state.window_visible = visible;
    }
    refresh(&tray.handle);
}

/// Replace the per-account menu entries and re-render.
pub fn set_accounts(accounts: Vec<AccountSummary>) {
    let guard = tray_lock();
    let Some(tray) = guard.as_ref() else { return };
    tray.state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .accounts = accounts;
    refresh(&tray.handle);
}

/// Re-render the menu after an account-list change (`app.refresh-tray-accounts`).
pub fn refresh_accounts() {
    let guard = tray_lock();
    let Some(tray) = guard.as_ref() else { return };
    refresh(&tray.handle);
}

/// Poke `ksni` to re-pull `icon_name`/`tool_tip`/`menu`.
fn refresh(handle: &ksni::Handle<KarereTray>) {
    let handle = handle.clone();
    crate::actions::runtime().spawn(async move {
        handle.update(|_| {}).await;
    });
}
