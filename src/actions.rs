use std::sync::OnceLock;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::application::{APP_ID, KarereApplication};

static RUNTIME: OnceLock<&'static tokio::runtime::Runtime> = OnceLock::new();

pub(crate) fn runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        let rt = tokio::runtime::Runtime::new()
            .expect("failed to create tokio runtime for portal calls");
        // Deliberately leaked — the runtime needs to outlive `main` for portal
        // callbacks. One runtime per process; not freed at shutdown.
        Box::leak(Box::new(rt))
    })
}

pub fn register_app_actions(app: &KarereApplication) {
    register_quit(app);
    register_about(app);
    register_preferences(app);
    register_help_overlay(app);
    register_present_window(app);
    register_sync_autostart(app);
    register_notification_clicked(app);
    register_switch_account(app);
    register_set_unread(app);
    register_refresh_tray_accounts(app);
    register_open_download(app);
}

fn register_quit(app: &KarereApplication) {
    let action = gio::SimpleAction::new("quit", None);
    action.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| {
            if let Some(win) = app.active_window().and_downcast::<crate::window::KarereWindow>() {
                win.quit_now();
            } else {
                app.quit();
            }
        }
    ));
    app.add_action(&action);
}

/// `app.present-window`: toggle the primary chrome window on visibility. Hide it
/// when it is visible; otherwise show and present it. Gating on visibility alone
/// (not `is_active`) is required for the tray: clicking the tray menu removes
/// focus from the window, so an `is_active` check would never hide it. Driven by
/// tray left-click and the `Show / Hide Window` menu item.
fn register_present_window(app: &KarereApplication) {
    let action = gio::SimpleAction::new("present-window", None);
    action.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| {
            let Some(win) = app
                .active_window()
                .or_else(|| app.windows().into_iter().next())
            else {
                log::warn!("app.present-window: no window to present");
                return;
            };
            if win.is_visible() {
                win.set_visible(false);
            } else {
                win.present();
            }
        }
    ));
    app.add_action(&action);
}

fn register_preferences(app: &KarereApplication) {
    let action = gio::SimpleAction::new("preferences", None);
    action.connect_activate(|_, _| {
        log::warn!("action app.preferences not yet implemented (milestone M22)");
    });
    app.add_action(&action);
}

fn register_help_overlay(app: &KarereApplication) {
    let action = gio::SimpleAction::new("show-help-overlay", None);
    action.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| {
            let Some(win) = app.active_window() else {
                log::warn!("app.show-help-overlay: no active window");
                return;
            };
            let builder =
                gtk::Builder::from_resource("/io/github/tobagin/karere/ui/keyboard-shortcuts.ui");
            if let Some(shortcuts) = builder.object::<gtk::ShortcutsWindow>("help_overlay") {
                shortcuts.set_transient_for(Some(&win));
                shortcuts.present();
            } else {
                log::warn!(
                    "action app.show-help-overlay has no help_overlay object (milestone Mxx)"
                );
            }
        }
    ));
    app.add_action(&action);
}

/// `app.open-download <path>`: open a downloaded file (or its parent directory,
/// for "Show in Folder") in the default application via the OpenURI portal,
/// falling back to `AppInfo::launch_default_for_uri` when no portal is available.
fn register_open_download(app: &KarereApplication) {
    let action = gio::SimpleAction::new("open-download", Some(glib::VariantTy::STRING));
    action.connect_activate(|_, param| {
        let Some(path) = param.and_then(|v| v.str()) else {
            log::warn!("app.open-download invoked without a path argument");
            return;
        };
        let path = path.to_owned();
        runtime().spawn(async move {
            if let Err(err) = open_via_portal(&path).await {
                log::warn!(
                    "open-download: portal open_file({path}) failed ({err}); \
                     falling back to AppInfo"
                );
                // gio calls must run on the glib main thread.
                glib::MainContext::default().invoke(move || {
                    let uri = format!("file://{path}");
                    if let Err(err) = gio::AppInfo::launch_default_for_uri(
                        &uri,
                        None::<&gio::AppLaunchContext>,
                    ) {
                        log::warn!("open-download: fallback launch_default_for_uri failed: {err}");
                    }
                });
            }
        });
    });
    app.add_action(&action);
}

/// Open `path` through the FreeDesktop OpenURI portal so it works under Flatpak
/// without filesystem holes. Works for both files and directories.
async fn open_via_portal(path: &str) -> anyhow::Result<()> {
    use ashpd::desktop::open_uri::OpenFileRequest;
    use std::os::fd::AsFd;

    let file = std::fs::File::open(path)?;
    OpenFileRequest::default()
        .ask(false)
        .send_file(&file.as_fd())
        .await?;
    Ok(())
}

/// `app.notification-clicked <tag>`: the user clicked a Karere-branded banner.
/// Raise the window and re-enter the page via `__karereActivateNotif('<tag>')`
/// so WhatsApp opens the originating chat (M14 3.7 / 7.2).
fn register_notification_clicked(app: &KarereApplication) {
    let action = gio::SimpleAction::new("notification-clicked", Some(glib::VariantTy::STRING));
    action.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, param| {
            let tag = param.and_then(|v| v.str()).unwrap_or_default().to_owned();
            let Some(win) = app
                .active_window()
                .or_else(|| app.windows().into_iter().next())
            else {
                log::warn!("notification-clicked: no window to raise");
                return;
            };
            win.present();
            if let Some(kw) = win.downcast_ref::<crate::window::KarereWindow>() {
                let script = format!(
                    "window.__karereActivateNotif({})",
                    crate::notifications::js_string(&tag)
                );
                kw.run_page_js(&script);
            }
        }
    ));
    app.add_action(&action);
}

/// `app.set-unread <u32>`: write the new unread count into the shared tray
/// state and trigger a tray refresh (icon + tooltip). No-op when no tray is
/// running (GNOME without AppIndicator).
fn register_set_unread(app: &KarereApplication) {
    let action = gio::SimpleAction::new("set-unread", Some(glib::VariantTy::UINT32));
    action.connect_activate(|_, param| {
        let count = param.and_then(|v| v.get::<u32>()).unwrap_or(0);
        crate::tray::set_unread(count);
    });
    app.add_action(&action);
}

/// `app.refresh-tray-accounts`: re-read accounts into the tray state and ask
/// `ksni` to re-render the menu. A no-op on state until M20 provides an
/// `AccountManager`; the refresh still fires so the next `menu()` reflects any
/// future change.
fn register_refresh_tray_accounts(app: &KarereApplication) {
    let action = gio::SimpleAction::new("refresh-tray-accounts", None);
    action.connect_activate(|_, _| {
        crate::tray::refresh_accounts();
    });
    app.add_action(&action);
}

/// `app.switch-account <id>`: registered no-op stub so per-account menu items
/// can target it without runtime errors. M20 fills in the account switch.
fn register_switch_account(app: &KarereApplication) {
    let action = gio::SimpleAction::new("switch-account", Some(glib::VariantTy::STRING));
    action.connect_activate(|_, param| {
        let id = param.and_then(|v| v.str()).unwrap_or_default();
        log::debug!("app.switch-account({id:?}) — no-op stub (milestone M20)");
    });
    app.add_action(&action);
}

fn register_about(app: &KarereApplication) {
    let action = gio::SimpleAction::new("about", None);
    action.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| {
            let dialog = adw::AboutDialog::builder()
                .application_name("Karere")
                .application_icon(APP_ID)
                .developer_name("Thiago Avila Fernandes")
                .version(env!("CARGO_PKG_VERSION"))
                .website("https://github.com/tobagin/karere")
                .issue_url("https://github.com/tobagin/karere/issues")
                .license_type(gtk::License::Gpl30)
                .build();

            if let Some(notes) = load_release_notes() {
                dialog.set_release_notes(&notes);
            }

            let parent = app.active_window();
            dialog.present(parent.as_ref());
        }
    ));
    app.add_action(&action);
}

fn register_sync_autostart(app: &KarereApplication) {
    let action = gio::SimpleAction::new("sync-autostart", None);
    action.connect_activate(move |_, _| {
        let settings = gio::Settings::new(APP_ID);
        let want = settings.boolean("run-on-startup");
        runtime().spawn(async move {
            use ashpd::desktop::background::Background;
            let result = if want {
                Background::request()
                    .reason("Karere can keep running in the background to deliver messages.")
                    .auto_start(true)
                    .command(["karere"])
                    .send()
                    .await
            } else {
                Background::request().auto_start(false).send().await
            };
            match result {
                Ok(_) => log::info!(
                    "ashpd Background::request completed (auto_start={want})"
                ),
                Err(err) => log::warn!("ashpd Background::request failed: {err}"),
            }
        });
    });
    app.add_action(&action);
}

fn load_release_notes() -> Option<String> {
    let path = if std::env::var("FLATPAK_ID").is_ok() {
        std::path::PathBuf::from(format!("/app/share/metainfo/{APP_ID}.metainfo.xml"))
    } else {
        let prefix = std::env::var("KARERE_DATA_PREFIX").unwrap_or_else(|_| "/usr".into());
        std::path::PathBuf::from(format!("{prefix}/share/metainfo/{APP_ID}.metainfo.xml"))
    };
    let xml = std::fs::read_to_string(&path).ok()?;
    extract_first_release_description(&xml)
}

fn extract_first_release_description(xml: &str) -> Option<String> {
    // Find the first <release ...>...</release> block, then its <description>...</description>.
    let release_open = xml.find("<release")?;
    let tail = &xml[release_open..];
    let release_end = tail.find("</release>")?;
    let block = &tail[..release_end];
    let desc_open_tag = block.find("<description>")?;
    let after_open = &block[desc_open_tag + "<description>".len()..];
    let desc_close = after_open.find("</description>")?;
    Some(after_open[..desc_close].trim().to_owned())
}
