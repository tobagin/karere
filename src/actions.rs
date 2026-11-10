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
    register_notification_mute_account(app);
    register_do_not_disturb(app);
    register_refresh_tray_accounts(app);
    register_open_download(app);
    register_show_download(app);
}

fn register_quit(app: &KarereApplication) {
    let action = gio::SimpleAction::new("quit", None);
    action.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| {
            // A hidden window may not be "active" — fall back to the window list. (#175)
            if let Some(win) = app
                .active_window()
                .or_else(|| app.windows().into_iter().next())
                .and_downcast::<crate::window::KarereWindow>()
            {
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
            crate::preferences::KarerePreferencesDialog::show(&app);
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
                    let uri = gio::File::for_path(&path).uri();
                    if let Err(err) =
                        gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>)
                    {
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
        .ask(needs_open_confirmation(path))
        .send_file(&file.as_fd())
        .await?;
    Ok(())
}

/// File types that can run code when opened: always let the portal ask which
/// application to use instead of launching the default handler directly.
fn needs_open_confirmation(path: &str) -> bool {
    const RISKY: &[&str] = &[
        "desktop",
        "sh",
        "run",
        "appimage",
        "py",
        "jar",
        "exe",
        "bat",
        "deb",
        "rpm",
        "flatpakref",
    ];
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| RISKY.contains(&e.to_ascii_lowercase().as_str()))
}

/// `app.show-download <path>`: reveal a downloaded file in the file manager
/// (highlighted) via the OpenURI portal's OpenDirectory request.
fn register_show_download(app: &KarereApplication) {
    let action = gio::SimpleAction::new("show-download", Some(glib::VariantTy::STRING));
    action.connect_activate(|_, param| {
        let Some(path) = param.and_then(|v| v.str()) else {
            log::warn!("app.show-download invoked without a path argument");
            return;
        };
        let path = path.to_owned();
        runtime().spawn(async move {
            if let Err(err) = show_via_portal(&path).await {
                log::warn!(
                    "show-download: portal open_directory({path}) failed ({err}); \
                     falling back to the parent folder"
                );
                glib::MainContext::default().invoke(move || {
                    let parent = std::path::Path::new(&path)
                        .parent()
                        .map(std::path::Path::to_path_buf)
                        .unwrap_or_else(|| std::path::PathBuf::from("/"));
                    let uri = gio::File::for_path(parent).uri();
                    if let Err(err) =
                        gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>)
                    {
                        log::warn!("show-download: fallback launch_default_for_uri failed: {err}");
                    }
                });
            }
        });
    });
    app.add_action(&action);
}

async fn show_via_portal(path: &str) -> anyhow::Result<()> {
    use ashpd::desktop::open_uri::OpenDirectoryRequest;
    use std::os::fd::AsFd;

    let file = std::fs::File::open(path)?;
    OpenDirectoryRequest::default().send(&file.as_fd()).await?;
    Ok(())
}

/// `app.notification-clicked (account_id, tag)`: raise the window, switch to
/// the banner's account, and call `__karereActivateNotif('<tag>')` in that
/// account's browser so WhatsApp opens the originating chat.
fn register_notification_clicked(app: &KarereApplication) {
    let target_ty = glib::VariantTy::new(crate::notifications::ACTIVATE_TARGET_TYPE)
        .expect("notification target type is a valid GVariant type");
    let action = gio::SimpleAction::new("notification-clicked", Some(target_ty));
    action.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, param| {
            let Some((account_id, tag)) =
                param.and_then(crate::notifications::parse_activate_target)
            else {
                log::warn!("notification-clicked: malformed target");
                return;
            };
            let Some(win) = app
                .active_window()
                .or_else(|| app.windows().into_iter().next())
            else {
                log::warn!("notification-clicked: no window to raise");
                return;
            };
            win.present();
            // Withdraw the clicked banner and drop it from the tracker before
            // the switch, so the switch's withdraw pass does not close the stub
            // the click is about to activate.
            crate::notifications::tracker().on_activated(&account_id, &tag);
            let mgr = crate::accounts::manager();
            if mgr.get(&account_id).is_none() {
                // Account removed since the banner was shown: just raise.
                return;
            }
            if let Some(kw) = win.downcast_ref::<crate::window::KarereWindow>() {
                if mgr.active().is_none_or(|a| a.id != account_id) {
                    kw.switch_account(&account_id);
                }
                let script = format!(
                    "window.__karereActivateNotif({})",
                    crate::notifications::js_string(&tag)
                );
                kw.run_account_js(&account_id, &script);
            }
        }
    ));
    app.add_action(&action);
}

/// `app.notification-mute-account (account_id, tag)`: a banner's "Mute
/// Account" button. Mutes that account's notifications (the switcher keeps
/// counting it, the tray total drops it) and withdraws the banner.
fn register_notification_mute_account(app: &KarereApplication) {
    let target_ty = glib::VariantTy::new(crate::notifications::ACTIVATE_TARGET_TYPE)
        .expect("notification target type is a valid GVariant type");
    let name = crate::notifications::MUTE_ACCOUNT_ACTION
        .strip_prefix("app.")
        .expect("mute action is an app action");
    let action = gio::SimpleAction::new(name, Some(target_ty));
    action.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, param| {
            let Some((account_id, tag)) =
                param.and_then(crate::notifications::parse_activate_target)
            else {
                log::warn!("notification-mute-account: malformed target");
                return;
            };
            crate::notifications::tracker().on_activated(&account_id, &tag);
            let mgr = crate::accounts::manager();
            if mgr.get(&account_id).is_none() {
                return;
            }
            mgr.set_muted(&account_id, true);
            // `set_muted` does not emit (see its doc); outside the row's own
            // toggle it is safe to rebuild the switcher, which also re-pushes
            // the tray total.
            mgr.emit_by_name::<()>("accounts-changed", &[]);
            for win in app.windows() {
                if let Some(kw) = win.downcast_ref::<crate::window::KarereWindow>() {
                    kw.apply_audio_mute();
                }
            }
        }
    ));
    app.add_action(&action);
}

/// `app.do-not-disturb`: boolean-state toggle, the inverse of the
/// `notifications-enabled` key. On: no banners and no notification sound;
/// unread counts keep updating. Kept in sync when the key changes elsewhere
/// (Preferences), and mirrored into the tray's checkmark.
fn register_do_not_disturb(app: &KarereApplication) {
    let settings = gio::Settings::new(APP_ID);
    let dnd = !settings.boolean("notifications-enabled");
    crate::tray::set_dnd(dnd);
    let action = gio::SimpleAction::new_stateful("do-not-disturb", None, &dnd.to_variant());
    action.connect_activate(|action, _| {
        let on = !action.state().and_then(|v| v.get::<bool>()).unwrap_or(false);
        // The `changed` handler below updates the state, tray and sound mute.
        if let Err(err) = gio::Settings::new(APP_ID).set_boolean("notifications-enabled", !on) {
            log::warn!("do-not-disturb: cannot write notifications-enabled: {err}");
        }
    });
    settings.connect_changed(
        Some("notifications-enabled"),
        glib::clone!(
            #[weak]
            action,
            move |s, key| {
                let dnd = !s.boolean(key);
                action.set_state(&dnd.to_variant());
                crate::tray::set_dnd(dnd);
            }
        ),
    );
    // GSettings signals die with the object; keep it for the app's lifetime.
    std::mem::forget(settings);
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
                "Riccardo Marotti https://github.com/riccardomarotti",
                "LeoXia https://github.com/hz-xiaxz",
            ];
            let designers = ["Thiago Fernandes https://github.com/tobagin"];
            let artists = [
                "Thiago Fernandes https://github.com/tobagin",
                "Rosabel https://github.com/oiimrosabel",
            ];

            let dialog = adw::AboutDialog::builder()
                .application_name(gettext("Karere"))
                .application_icon(APP_ID)
                .developer_name("Thiago Fernandes")
                .version(env!("CARGO_PKG_VERSION"))
                .comments(gettext(
                    "A modern, native GTK4/LibAdwaita wrapper for WhatsApp Web that provides seamless desktop integration",
                ))
                .website("https://tobagin.github.io/apps/karere")
                .issue_url("https://github.com/tobagin/karere/issues")
                .support_url("https://github.com/tobagin/karere/discussions")
                .license_type(gtk::License::Gpl30)
                .copyright("© 2025-2026 Thiago Fernandes")
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
                // `--background` marks autostart launches so only those stay hidden.
                Background::request()
                    .reason("Karere can keep running in the background to deliver messages.")
                    .auto_start(true)
                    .command(["karere", "--background"])
                    .send()
                    .await
            } else {
                Background::request().auto_start(false).send().await
            };
            let outcome = result.and_then(|r| r.response());
            match outcome {
                Ok(bg) => {
                    log::info!(
                        "ashpd Background::request completed (auto_start={want}, granted={})",
                        bg.auto_start()
                    );
                    if want && !bg.auto_start() {
                        revert_autostart_setting();
                    }
                }
                Err(err) => {
                    log::warn!("ashpd Background::request failed: {err}");
                    if want {
                        revert_autostart_setting();
                    }
                }
            }
        });
    });
    app.add_action(&action);
}

/// The portal refused (or could not register) autostart: turn the setting back
/// off and tell the user. Runs on the main thread.
fn revert_autostart_setting() {
    glib::MainContext::default().invoke(|| {
        let settings = gio::Settings::new(APP_ID);
        if !settings.boolean("run-on-startup") {
            return;
        }
        if let Err(err) = settings.set_boolean("run-on-startup", false) {
            log::warn!("could not reset run-on-startup: {err}");
        }
        let win = gio::Application::default()
            .and_downcast::<gtk::Application>()
            .and_then(|app| app.active_window().or_else(|| app.windows().into_iter().next()))
            .and_downcast::<crate::window::KarereWindow>();
        if let Some(win) = win {
            win.show_toast(&gettextrs::gettext(
                "Could not enable running on startup. Allow Karere to run in the background in your system settings.",
            ));
        }
    });
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

#[cfg(test)]
mod tests {
    use super::needs_open_confirmation;

    #[test]
    fn risky_types_ask() {
        for p in [
            "dl/a.desktop",
            "dl/a.SH",
            "dl/Foo.AppImage",
            "dl/x.py",
            "dl/x.flatpakref",
            "dl/x.deb",
        ] {
            assert!(needs_open_confirmation(p), "{p}");
        }
        for p in ["dl/a.pdf", "dl/a.txt", "dl/noext", "dl/.sh", "dl/a.sh.txt"] {
            assert!(!needs_open_confirmation(p), "{p}");
        }
    }
}
