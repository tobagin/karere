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
        // Leaked: must outlive `main` for portal callbacks. One per process.
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

/// `app.present-window`: toggle the window on visibility (not `is_active` — the
/// tray steals focus, so `is_active` would never hide it). Driven by tray.
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
    action.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| {
            let dialog = crate::preferences::KarerePreferencesDialog::new(&app);
            dialog.present(app.active_window().as_ref());
        }
    ));
    app.add_action(&action);
}

fn register_help_overlay(app: &KarereApplication) {
    let action = gio::SimpleAction::new("show-help-overlay", None);
    action.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| {
            let builder =
                gtk::Builder::from_resource("/io/github/tobagin/karere/ui/keyboard-shortcuts.ui");
            // AdwShortcutsDialog (1.8) has no binding under v1_6; load as AdwDialog.
            let Some(dialog) = builder.object::<adw::Dialog>("shortcuts_dialog") else {
                log::warn!("app.show-help-overlay: keyboard-shortcuts.ui has no shortcuts_dialog");
                return;
            };
            dialog.present(app.active_window().as_ref());
        }
    ));
    app.add_action(&action);
}

/// `app.open-download <path>`: open a downloaded file (or its folder) via the
/// OpenURI portal, falling back to `AppInfo::launch_default_for_uri`.
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
                // gio must run on the glib main thread.
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

/// Open `path` via the OpenURI portal (works under Flatpak without fs holes).
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

/// `app.notification-clicked <tag>`: raise the window and call
/// `__karereActivateNotif('<tag>')` so WhatsApp opens the originating chat.
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

/// `app.set-unread <u32>`: update the tray unread count (icon + tooltip).
/// No-op when no tray is running.
fn register_set_unread(app: &KarereApplication) {
    let action = gio::SimpleAction::new("set-unread", Some(glib::VariantTy::UINT32));
    action.connect_activate(|_, param| {
        let count = param.and_then(|v| v.get::<u32>()).unwrap_or(0);
        crate::tray::set_unread(count);
    });
    app.add_action(&action);
}

/// `app.refresh-tray-accounts`: request a `ksni` menu redraw. The window pushes
/// fresh summaries via `tray::set_accounts`; this just triggers the rerender.
fn register_refresh_tray_accounts(app: &KarereApplication) {
    let action = gio::SimpleAction::new("refresh-tray-accounts", None);
    action.connect_activate(|_, _| {
        crate::tray::refresh_accounts();
    });
    app.add_action(&action);
}

/// `app.switch-account <id>`: switch the window to account `id` (tray entries).
fn register_switch_account(app: &KarereApplication) {
    let action = gio::SimpleAction::new("switch-account", Some(glib::VariantTy::STRING));
    action.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, param| {
            let id = param.and_then(|v| v.str()).unwrap_or_default();
            if id.is_empty() {
                return;
            }
            let Some(win) = app
                .active_window()
                .or_else(|| app.windows().into_iter().next())
            else {
                return;
            };
            // Always surface (never toggle): "show this account", not "toggle".
            win.set_visible(true);
            win.present();
            if let Some(win) = win.downcast_ref::<crate::window::KarereWindow>() {
                win.switch_account(id);
            }
        }
    ));
    app.add_action(&action);
}

fn register_about(app: &KarereApplication) {
    use gettextrs::gettext;
    let action = gio::SimpleAction::new("about", None);
    action.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| {
            // Like-for-like with v3's About; only the version differs.
            let developers = [
                "Thiago Fernandes https://github.com/tobagin",
                "Aman9Das https://github.com/Aman9das",
                "Pascal Dietrich https://github.com/",
                "Sabri Ünal https://github.com/yakushabb",
                "Enrico https://github.com/account1009",
                "Leandro Marques https://github.com/leandromqrs",
                "Muhammed Al-Basha https://github.com/mu7basha",
                "Jimmy Scionti https://github.com/amivaleo",
                "AnmiTaliDev https://github.com/AnmiTaliDev",
            ];
            let designers = ["Thiago Fernandes https://github.com/tobagin"];
            let artists = [
                "Thiago Fernandes https://github.com/tobagin",
                "Rosabel https://github.com/oiimrosabel",
            ];

            let dialog = adw::AboutDialog::builder()
                .application_name(gettext("Karere"))
                .application_icon(APP_ID)
                .developer_name("The Karere Team")
                .version(env!("CARGO_PKG_VERSION"))
                .comments(gettext(
                    "A modern, native GTK4/LibAdwaita wrapper for WhatsApp Web that provides seamless desktop integration with comprehensive logging and crash reporting capabilities",
                ))
                .website("https://tobagin.github.io/apps/karere")
                .issue_url("https://github.com/tobagin/karere/issues")
                .support_url("https://github.com/tobagin/karere/discussions")
                .license_type(gtk::License::Gpl30)
                .copyright("© 2025 The Karere Team")
                .developers(developers.iter().map(|s| String::from(*s)).collect::<Vec<_>>())
                .designers(designers.iter().map(|s| String::from(*s)).collect::<Vec<_>>())
                .artists(artists.iter().map(|s| String::from(*s)).collect::<Vec<_>>())
                .translator_credits(
                    "Thiago Fernandes https://github.com/tobagin\n\
                     Muhammed Al-Basha https://github.com/mu7basha\n\
                     Jimmy Scionti https://github.com/amivaleo\n\
                     AnmiTaliDev https://github.com/AnmiTaliDev\n\
                     Sabri Ünal https://github.com/yakushabb",
                )
                .build();

            dialog.add_link(gettext("Source").as_str(), "https://github.com/tobagin/karere");

            dialog.add_acknowledgement_section(
                Some(gettext("Special Thanks").as_str()),
                &[
                    "The GNOME Project",
                    "The Chromium Embedded Framework",
                    "WhatsApp Inc.",
                    "LibAdwaita Contributors",
                    "The Rust Project",
                ],
            );

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
    // First <release> block, then its <description>.
    let release_open = xml.find("<release")?;
    let tail = &xml[release_open..];
    let release_end = tail.find("</release>")?;
    let block = &tail[..release_end];
    let desc_open_tag = block.find("<description>")?;
    let after_open = &block[desc_open_tag + "<description>".len()..];
    let desc_close = after_open.find("</description>")?;
    Some(strip_localized(after_open[..desc_close].trim()))
}

/// Drop translated `<p xml:lang="…">…</p>` / `<li xml:lang=…>` variants that
/// `i18n.merge_file` injects into the installed metainfo. The What's-New dialog
/// renders the already-localized default markup and rejects `xml:lang`
/// attributes ("attribute 'xml:lang' invalid for element 'p'").
fn strip_localized(desc: &str) -> String {
    let mut out = String::with_capacity(desc.len());
    let mut rest = desc;
    while let Some(attr) = rest.find("xml:lang=") {
        let tag_start = rest[..attr].rfind('<').unwrap_or(0);
        let name_start = tag_start + 1;
        let name_end = rest[name_start..]
            .find([' ', '>', '/', '\t', '\n'])
            .map(|i| name_start + i)
            .unwrap_or(rest.len());
        let tag = &rest[name_start..name_end];
        out.push_str(&rest[..tag_start]);
        let close = format!("</{tag}>");
        rest = match rest[tag_start..].find(&close) {
            Some(c) => &rest[tag_start + c + close.len()..],
            None => {
                // No close tag found — skip past this opening tag only.
                let gt = rest[tag_start..]
                    .find('>')
                    .map(|i| tag_start + i + 1)
                    .unwrap_or(rest.len());
                &rest[gt..]
            }
        };
    }
    out.push_str(rest);
    // Collapse the blank lines left where variants were removed.
    out.lines()
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
