//! System-tray `StatusNotifierItem` via the `ksni` crate. State lives in one
//! process-wide `Arc<Mutex<TrayState>>` that outlives any tray instance: the
//! tokio tray task reads it, main-thread code writes it then refreshes. A tray
//! re-created from Preferences therefore starts from the last pushed state.

use std::sync::{Arc, Mutex, OnceLock};

use gettextrs::{gettext, ngettext};
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
    /// Unread chats in this account (page-title count).
    pub unread: u32,
    /// The account shown in the window.
    pub active: bool,
    pub icon_png: Option<Vec<u8>>,
}

/// Cross-thread tray state (tokio reads, main thread writes).
#[derive(Default)]
pub struct TrayState {
    /// Sum of the unmuted accounts' unread counts.
    pub unread_count: u32,
    pub accounts: Vec<AccountSummary>,
    /// Drives the `Show / Hide` menu label.
    pub window_visible: bool,
    /// Do Not Disturb (`notifications-enabled` off) checkmark.
    pub dnd: bool,
}

/// The process-wide tray state, kept across tray stop/start.
fn state() -> &'static Arc<Mutex<TrayState>> {
    static STATE: OnceLock<Arc<Mutex<TrayState>>> = OnceLock::new();
    STATE.get_or_init(Arc::default)
}

fn state_lock() -> std::sync::MutexGuard<'static, TrayState> {
    state().lock().unwrap_or_else(|e| e.into_inner())
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
            let n = state.unread_count;
            ksni::ToolTip {
                title: gettext("Karere"),
                // Translators: tray tooltip body; {} is the number of unread chats.
                description: ngettext("{} unread chat", "{} unread chats", n)
                    .replace("{}", &n.to_string()),
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

        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let toggle_label = if state.window_visible {
            gettext("Hide Window")
        } else {
            gettext("Show Window")
        };

        // One radio group: the active account is the selected item, each label
        // carries its unread count. Out-of-range `selected` = none checked.
        let ids: Vec<String> = state.accounts.iter().map(|a| a.id.clone()).collect();
        let accounts = RadioGroup {
            selected: state
                .accounts
                .iter()
                .position(|a| a.active)
                .unwrap_or(usize::MAX),
            select: Box::new(move |_, i| {
                if let Some(id) = ids.get(i) {
                    activate_app_action("switch-account", Some(id.to_variant()));
                }
            }),
            options: state
                .accounts
                .iter()
                .map(|a| RadioItem {
                    label: account_label(&a.name, a.unread),
                    icon_data: a.icon_png.clone().unwrap_or_default(),
                    ..Default::default()
                })
                .collect(),
        };
        let has_accounts = !state.accounts.is_empty();
        let dnd = state.dnd;
        drop(state);

        let mut items: Vec<ksni::MenuItem<Self>> = vec![
            StandardItem {
                label: toggle_label,
                activate: Box::new(|_| activate_app_action("present-window", None)),
                ..Default::default()
            }
            .into(),
        ];
        if has_accounts {
            items.push(MenuItem::Separator);
            items.push(accounts.into());
        }
        items.extend([
            MenuItem::Separator,
            CheckmarkItem {
                label: gettext("Do Not Disturb"),
                checked: dnd,
                activate: Box::new(|_| activate_app_action("do-not-disturb", None)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: gettext("Preferences"),
                activate: Box::new(|_| activate_dialog_action("preferences")),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: gettext("Keyboard Shortcuts"),
                activate: Box::new(|_| activate_dialog_action("show-help-overlay")),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: gettext("About Karere"),
                activate: Box::new(|_| activate_dialog_action("about")),
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

/// Like `activate_app_action`, but shows a hidden window first so the dialog
/// has a visible parent (#200).
fn activate_dialog_action(name: &str) {
    let name = name.to_owned();
    glib::MainContext::default().invoke(move || {
        let Some(app) = gio::Application::default().and_downcast::<gtk::Application>() else {
            return;
        };
        if let Some(win) = app.windows().into_iter().next() {
            if !win.is_visible() {
                win.present();
            }
        }
        app.activate_action(&name, None);
    });
}

/// The running tray, if any. `starting` covers the async registration window;
/// `epoch` is bumped by `stop()` so a registration that finishes after a stop
/// shuts itself down instead of resurrecting the tray.
struct TraySlot {
    handle: Option<ksni::Handle<KarereTray>>,
    starting: bool,
    epoch: u64,
}

static TRAY: Mutex<TraySlot> = Mutex::new(TraySlot {
    handle: None,
    starting: false,
    epoch: 0,
});

fn tray_lock() -> std::sync::MutexGuard<'static, TraySlot> {
    TRAY.lock().unwrap_or_else(|e| e.into_inner())
}

/// Start the tray service per `systray-icon`, the GNOME skip policy, and
/// `KARERE_FORCE_TRAY=1`. Idempotent and non-blocking: the D-Bus work (watcher
/// probe, registration) runs on the tokio runtime, and the tray reads the
/// shared state when it comes up, so anything pushed meanwhile is shown.
pub fn start() {
    let mode = gio::Settings::new(APP_ID).string("systray-icon");
    if mode == "disabled" {
        log::info!("tray disabled via systray-icon setting");
        return;
    }
    let force = mode == "enabled" || std::env::var("KARERE_FORCE_TRAY").as_deref() == Ok("1");

    let epoch = {
        let mut slot = tray_lock();
        if slot.handle.is_some() || slot.starting {
            return;
        }
        slot.starting = true;
        slot.epoch
    };

    crate::actions::runtime().spawn(async move {
        if !force && should_skip_on_gnome().await {
            log::info!("tray skipped (GNOME w/o AppIndicator)");
            let mut slot = tray_lock();
            if slot.epoch == epoch {
                slot.starting = false;
            }
            return;
        }
        let tray = KarereTray {
            state: state().clone(),
        };
        // disable_dbus_name + assume_sni_available so the item registers under
        // Flatpak and when the watcher appears slightly after us.
        let result = tray
            .disable_dbus_name(true)
            .assume_sni_available(true)
            .spawn()
            .await;
        let mut slot = tray_lock();
        let current = slot.epoch == epoch;
        if current {
            slot.starting = false;
        }
        match result {
            Ok(handle) if current => {
                refresh(&handle);
                slot.handle = Some(handle);
                log::info!("tray service started");
            }
            Ok(handle) => {
                // Stopped while registering.
                handle.shutdown();
            }
            Err(err) => log::warn!("tray service failed to start: {err}"),
        }
    });
}

/// Stop the running (or starting) tray service; a later `start()` can
/// re-create it with the same state.
pub fn stop() {
    let handle = {
        let mut slot = tray_lock();
        slot.epoch = slot.epoch.wrapping_add(1);
        slot.starting = false;
        slot.handle.take()
    };
    if let Some(handle) = handle {
        handle.shutdown();
        log::info!("tray service stopped");
    }
}

/// Apply the current `systray-icon` setting live. Any enabled mode restarts the
/// tray: "auto" must re-run the async GNOME watcher probe, and a restart also
/// cancels a probe still running for the previous mode. State is kept.
pub fn apply_setting() {
    stop();
    start();
}

/// Skip the tray on GNOME unless an SNI watcher (AppIndicator extension) is present.
async fn should_skip_on_gnome() -> bool {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    // May be colon-separated (e.g. "ubuntu:GNOME").
    let is_gnome = desktop
        .split(':')
        .any(|entry| entry.eq_ignore_ascii_case("GNOME"));
    if !is_gnome {
        return false;
    }
    !watcher_present().await
}

/// Whether `org.kde.StatusNotifierWatcher` has an owner on the session bus.
async fn watcher_present() -> bool {
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
}

/// Menu label for an account: its name (underscores escaped, ksni treats `_`
/// as a mnemonic marker) plus the unread count when there is one.
fn account_label(name: &str, unread: u32) -> String {
    let name = name.replace('_', "__");
    if unread > 0 {
        format!("{name} ({})", crate::accounts::unread_badge_text(unread))
    } else {
        name
    }
}

/// Write the tray unread total into the state, refreshing only on a change.
pub fn set_unread(count: u32) {
    {
        let mut state = state_lock();
        if state.unread_count == count {
            return;
        }
        state.unread_count = count;
    }
    refresh_running();
}

/// Recompute the tray total from the accounts (unmuted only) and push it.
/// Zero when "change tray icon on new message" is off. Main thread only.
pub fn sync_unread() {
    let on = gio::Settings::new(APP_ID).boolean("notify-tray-icon");
    set_unread(if on {
        crate::accounts::unread_total_unmuted()
    } else {
        0
    });
}

/// Sync window visibility into tray state, refreshing only on a real change.
pub fn set_window_visible(visible: bool) {
    {
        let mut state = state_lock();
        if state.window_visible == visible {
            return;
        }
        state.window_visible = visible;
    }
    refresh_running();
}

/// Sync the Do Not Disturb checkmark, refreshing only on a real change.
pub fn set_dnd(dnd: bool) {
    {
        let mut state = state_lock();
        if state.dnd == dnd {
            return;
        }
        state.dnd = dnd;
    }
    refresh_running();
}

/// Replace the per-account menu entries and re-render.
pub fn set_accounts(accounts: Vec<AccountSummary>) {
    state_lock().accounts = accounts;
    refresh_running();
}

/// Refresh the tray if one is running.
fn refresh_running() {
    if let Some(handle) = tray_lock().handle.as_ref() {
        refresh(handle);
    }
}

/// Poke `ksni` to re-pull `icon_name`/`tool_tip`/`menu`.
fn refresh(handle: &ksni::Handle<KarereTray>) {
    let handle = handle.clone();
    crate::actions::runtime().spawn(async move {
        handle.update(|_| {}).await;
    });
}

#[cfg(test)]
mod tests {
    use super::account_label;

    #[test]
    fn account_label_appends_capped_count_and_escapes_mnemonics() {
        assert_eq!(account_label("Work", 0), "Work");
        assert_eq!(account_label("Work", 3), "Work (3)");
        assert_eq!(account_label("Work", 250), "Work (99+)");
        assert_eq!(account_label("my_acct", 0), "my__acct");
    }
}
