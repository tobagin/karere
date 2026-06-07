//! `KarerePreferencesDialog`: surfaces every GSetting plus the permission registry.

use std::cell::RefCell;

use gettextrs::gettext;
use gtk::gio;
use gtk::gio::prelude::{SettingsExt, SettingsExtManual};
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::ObjectSubclassIsExt;
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::application::{APP_ID, KarereApplication};

glib::wrapper! {
    pub struct KarerePreferencesDialog(ObjectSubclass<imp::KarerePreferencesDialog>)
        @extends adw::PreferencesDialog, adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl KarerePreferencesDialog {
    /// Build a dialog bound to the app's GSettings.
    pub fn new(app: &KarereApplication) -> Self {
        let obj: Self = glib::Object::new();
        obj.imp().setup(app);
        obj
    }
}

/// Bind an `AdwComboRow` to a string GSetting; model order matches `values`.
fn bind_combo_string(settings: &gio::Settings, key: &str, row: &adw::ComboRow, values: &[&str]) {
    let current = settings.string(key);
    if let Some(idx) = values.iter().position(|v| *v == current.as_str()) {
        row.set_selected(idx as u32);
    }
    let values: Vec<String> = values.iter().map(|s| s.to_string()).collect();
    let key = key.to_owned();
    let settings = settings.clone();
    row.connect_selected_notify(move |row| {
        let sel = row.selected() as usize;
        if let Some(v) = values.get(sel) {
            let _ = settings.set_string(&key, v);
        }
    });
}

/// Bind an `AdwComboRow` to an enum GSetting; integer values match model order.
fn bind_combo_enum(settings: &gio::Settings, key: &str, row: &adw::ComboRow) {
    let current = settings.enum_(key);
    if current >= 0 {
        row.set_selected(current as u32);
    }
    let key = key.to_owned();
    let settings = settings.clone();
    row.connect_selected_notify(move |row| {
        let _ = settings.set_enum(&key, row.selected() as i32);
    });
}

fn bind_switch(settings: &gio::Settings, key: &str, row: &adw::SwitchRow) {
    settings
        .bind(key, row, "active")
        .flags(gio::SettingsBindFlags::DEFAULT)
        .build();
}

/// The active window's live webview, if any.
fn active_web_view() -> Option<crate::web_view::KarereWebView> {
    let app = gio::Application::default()?
        .downcast::<gtk::Application>()
        .ok()?;
    let win = app
        .active_window()?
        .downcast::<crate::window::KarereWindow>()
        .ok()?;
    let wv = win.imp().web_view.borrow().clone();
    wv
}

fn active_window() -> Option<gtk::Window> {
    let app = gio::Application::default()?
        .downcast::<gtk::Application>()
        .ok()?;
    app.active_window()
}

/// Ask whether to restart now to apply the UI-language change (it only takes
/// effect at startup). Shown instead of a toast so the choice is explicit.
fn prompt_restart(parent: gtk::Widget) {
    let dialog = adw::AlertDialog::new(
        Some(&gettext("Restart Karere?")),
        Some(&gettext(
            "The language change takes effect after Karere restarts.",
        )),
    );
    dialog.add_response("later", &gettext("Later"));
    dialog.add_response("restart", &gettext("Restart Now"));
    dialog.set_response_appearance("restart", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("restart"));
    dialog.set_close_response("later");
    dialog.connect_response(None, move |_, response| {
        if response == "restart" {
            restart_app();
        }
    });
    dialog.present(Some(&parent));
}

/// Relaunch Karere. The app is single-instance, so a fresh launch would just
/// activate the existing one — spawn a detached relauncher that waits for this
/// instance to quit (releasing the D-Bus name) before starting a new one. Under
/// Flatpak this must go through the host (`flatpak-spawn --host`).
fn restart_app() {
    if let Ok(id) = std::env::var("FLATPAK_ID") {
        let _ = std::process::Command::new("flatpak-spawn")
            .arg("--host")
            .arg("sh")
            .arg("-c")
            .arg(format!("sleep 1; exec flatpak run {id}"))
            .spawn();
    } else if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("sleep 1; exec {exe:?}"))
            .spawn();
    }
    if let Some(app) = gio::Application::default() {
        app.quit();
    }
}

mod imp {
    use super::*;
    use gtk::subclass::prelude::*;
    use libadwaita::subclass::prelude::*;

    #[derive(gtk::CompositeTemplate, Default)]
    #[template(resource = "/io/github/tobagin/karere/ui/preferences.ui")]
    pub struct KarerePreferencesDialog {
        #[template_child]
        pub row_startup: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_background: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_close_action: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub row_tray: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub row_language: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub row_theme: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub row_mobile_layout: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub row_dev_enable: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub btn_dev_open: TemplateChild<gtk::Button>,

        #[template_child]
        pub row_master_toggle: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_notify_msg: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_tray_anim: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_preview_name: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_preview_message: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_preview_len: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub row_sound_enable: TemplateChild<adw::SwitchRow>,

        #[template_child]
        pub row_download: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub btn_download_choose: TemplateChild<gtk::Button>,
        #[template_child]
        pub btn_download_reset: TemplateChild<gtk::Button>,
        #[template_child]
        pub row_dl_enable: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_dl_type: TemplateChild<adw::ComboRow>,

        #[template_child]
        pub row_spell_enable: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_auto_detect: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_auto_correct: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_spell_headerbar: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_spell_lang: TemplateChild<adw::ComboRow>,

        #[template_child]
        pub group_permissions: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub btn_clear_all: TemplateChild<gtk::Button>,

        #[template_child]
        pub row_motion: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_focus: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_contrast: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_zoom: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_zoom_level: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub row_zoom_headerbar: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_sr_opts: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_kb_shortcuts: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_a11y_shortcuts: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_dev_shortcuts: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub row_notify_shortcuts: TemplateChild<adw::SwitchRow>,

        /// Permission rows currently shown, so a rebuild can clear them first.
        pub perm_rows: RefCell<Vec<adw::ActionRow>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for KarerePreferencesDialog {
        const NAME: &'static str = "KarerePreferencesDialog";
        type Type = super::KarerePreferencesDialog;
        type ParentType = adw::PreferencesDialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for KarerePreferencesDialog {}
    impl WidgetImpl for KarerePreferencesDialog {}
    impl AdwDialogImpl for KarerePreferencesDialog {}
    impl PreferencesDialogImpl for KarerePreferencesDialog {}

    impl KarerePreferencesDialog {
        pub fn setup(&self, app: &KarereApplication) {
            let settings = gio::Settings::new(APP_ID);
            self.bind_general(&settings, app);
            self.bind_notifications(&settings);
            self.bind_downloads(&settings);
            self.bind_spellcheck(&settings);
            self.bind_privacy();
            self.bind_accessibility(&settings);
        }

        fn bind_general(&self, settings: &gio::Settings, app: &KarereApplication) {
            self.bind_language(settings);
            bind_switch(settings, "start-in-background", &self.row_background);
            bind_combo_string(
                settings,
                "close-button-action",
                &self.row_close_action,
                &["background", "quit"],
            );
            bind_combo_string(
                settings,
                "systray-icon",
                &self.row_tray,
                &["auto", "enabled", "disabled"],
            );
            bind_combo_string(
                settings,
                "theme",
                &self.row_theme,
                &["system", "light", "dark"],
            );
            bind_combo_string(
                settings,
                "mobile-layout",
                &self.row_mobile_layout,
                &["auto", "enabled", "disabled"],
            );
            bind_switch(settings, "enable-developer-tools", &self.row_dev_enable);

            bind_switch(settings, "run-on-startup", &self.row_startup);
            let app_weak = app.downgrade();
            settings.connect_changed(
                Some("run-on-startup"),
                move |_, _| {
                    if let Some(app) = app_weak.upgrade() {
                        app.activate_action("sync-autostart", None);
                    }
                },
            );

            // Present first so the OSR view is realized before DevTools attaches.
            self.btn_dev_open.connect_clicked(|_| {
                if let Some(win) = active_window() {
                    win.present();
                    if let Err(e) = win.activate_action("win.show-devtools", None) {
                        log::warn!("show-devtools action failed: {e}");
                    }
                }
            });
        }

        /// UI-language override. Labels are language names; values are gettext
        /// locale codes (`""` = follow system). Applied at startup, so a change
        /// raises a "restart to apply" toast.
        fn bind_language(&self, settings: &gio::Settings) {
            let mut codes: Vec<String> = vec![String::new()];
            let mut labels: Vec<String> = vec![gettext("System Default")];
            for (code, name) in crate::i18n::ui_locales() {
                codes.push(code);
                labels.push(name);
            }
            let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
            self.row_language
                .set_model(Some(&gtk::StringList::new(&label_refs)));

            let current = settings.string("app-language");
            if let Some(idx) = codes.iter().position(|c| c == current.as_str()) {
                self.row_language.set_selected(idx as u32);
            }

            let settings = settings.clone();
            let dialog = self.obj().downgrade();
            self.row_language.connect_selected_notify(move |row| {
                let Some(code) = codes.get(row.selected() as usize) else {
                    return;
                };
                if settings.string("app-language").as_str() == code {
                    return;
                }
                let _ = settings.set_string("app-language", code);
                if let Some(dialog) = dialog.upgrade() {
                    prompt_restart(dialog.upcast());
                }
            });
        }

        fn bind_notifications(&self, settings: &gio::Settings) {
            bind_switch(settings, "notifications-enabled", &self.row_master_toggle);
            bind_switch(settings, "notify-messages", &self.row_notify_msg);
            bind_switch(settings, "notify-tray-icon", &self.row_tray_anim);
            bind_switch(settings, "notify-preview-name", &self.row_preview_name);
            bind_switch(settings, "notify-preview-message", &self.row_preview_message);
            bind_combo_enum(settings, "notify-preview-length", &self.row_preview_len);
            bind_switch(settings, "notify-sound-enabled", &self.row_sound_enable);

            // Master toggle gates each dependent's `sensitive`.
            let dependents: [&gtk::Widget; 6] = [
                self.row_notify_msg.upcast_ref(),
                self.row_tray_anim.upcast_ref(),
                self.row_preview_name.upcast_ref(),
                self.row_preview_message.upcast_ref(),
                self.row_preview_len.upcast_ref(),
                self.row_sound_enable.upcast_ref(),
            ];
            for w in dependents {
                settings
                    .bind("notifications-enabled", w, "sensitive")
                    .flags(gio::SettingsBindFlags::GET)
                    .build();
            }
        }

        fn bind_downloads(&self, settings: &gio::Settings) {
            bind_switch(settings, "notify-downloads-enabled", &self.row_dl_enable);
            bind_combo_enum(settings, "notify-download-type", &self.row_dl_type);

            self.update_download_subtitle(settings);

            let settings_choose = settings.clone();
            let row = self.row_download.downgrade();
            self.btn_download_choose.connect_clicked(move |_| {
                let dialog = gtk::FileDialog::builder()
                    .title("Select Download Folder")
                    .modal(true)
                    .build();
                let settings = settings_choose.clone();
                let row = row.clone();
                dialog.select_folder(
                    active_window().as_ref(),
                    gio::Cancellable::NONE,
                    move |res| {
                        if let Ok(folder) = res
                            && let Some(path) = folder.path()
                        {
                            let _ = settings
                                .set_string("download-directory", &path.to_string_lossy());
                            if let Some(row) = row.upgrade() {
                                row.set_subtitle(&path.to_string_lossy());
                            }
                        }
                    },
                );
            });

            let settings_reset = settings.clone();
            let row_reset = self.row_download.downgrade();
            self.btn_download_reset.connect_clicked(move |_| {
                let _ = settings_reset.set_string("download-directory", "");
                if let Some(row) = row_reset.upgrade() {
                    row.set_subtitle("Default (Downloads)");
                }
            });
        }

        fn update_download_subtitle(&self, settings: &gio::Settings) {
            let dir = settings.string("download-directory");
            if dir.is_empty() {
                self.row_download.set_subtitle("Default (Downloads)");
            } else {
                self.row_download.set_subtitle(&dir);
            }
        }

        fn bind_spellcheck(&self, settings: &gio::Settings) {
            use crate::spellcheck_ui::{self, SpellLang};
            use std::rc::Rc;

            bind_switch(settings, "enable-spell-checking", &self.row_spell_enable);
            bind_switch(settings, "auto-detect-language", &self.row_auto_detect);
            bind_switch(settings, "enable-auto-correct", &self.row_auto_correct);
            bind_switch(settings, "spellcheck-headerbar", &self.row_spell_headerbar);

            let favorites: Vec<String> = settings
                .strv("favorite-spell-check-languages")
                .iter()
                .map(|s| s.to_string())
                .collect();
            let store = spellcheck_ui::build_store(&favorites);
            let sorter = spellcheck_ui::build_sorter();
            let sort_model = gtk::SortListModel::new(Some(store), Some(sorter.clone()));

            let row = self.row_spell_lang.get();
            row.set_model(Some(&sort_model));
            row.set_factory(Some(&spellcheck_ui::build_button_factory()));

            // Star toggle → persist favorites and re-sort.
            let on_toggle: Rc<dyn Fn(&SpellLang, bool)> = Rc::new({
                let settings = settings.clone();
                let sorter = sorter.clone();
                move |lang: &SpellLang, now: bool| {
                    lang.set_favorite(now);
                    let mut favs: Vec<String> = settings
                        .strv("favorite-spell-check-languages")
                        .iter()
                        .map(|s| s.to_string())
                        .collect();
                    let code = lang.code();
                    if now {
                        if !favs.contains(&code) {
                            favs.push(code);
                        }
                    } else {
                        favs.retain(|c| c != &code);
                    }
                    let refs: Vec<&str> = favs.iter().map(String::as_str).collect();
                    let _ = settings.set_strv("favorite-spell-check-languages", refs);
                    sorter.changed(gtk::SorterChange::Different);
                }
            });
            row.set_list_factory(Some(&spellcheck_ui::build_list_factory(on_toggle)));

            let explicit: Vec<String> = settings
                .strv("spell-checking-languages")
                .iter()
                .map(|s| s.to_string())
                .collect();
            if let Some(want) = explicit.first() {
                let n = sort_model.n_items();
                for i in 0..n {
                    if let Some(lang) = sort_model.item(i).and_downcast::<SpellLang>()
                        && &lang.code() == want
                    {
                        row.set_selected(i);
                        break;
                    }
                }
            }

            // Selection → persist + live-switch the running browser (no reload).
            let settings_sel = settings.clone();
            row.connect_selected_item_notify(move |row| {
                let Some(lang) = row.selected_item().and_downcast::<SpellLang>() else {
                    return;
                };
                let code = lang.code();
                let _ = settings_sel.set_strv("spell-checking-languages", [code.as_str()]);
                if let Some(web) = active_web_view() {
                    web.set_spellcheck_languages(&[code], true);
                }
            });
        }

        fn bind_privacy(&self) {
            self.rebuild_permissions();
            let obj = self.obj().downgrade();
            self.btn_clear_all.connect_clicked(move |_| {
                crate::permissions_store::clear();
                if let Some(obj) = obj.upgrade() {
                    obj.imp().rebuild_permissions();
                }
            });
        }

        /// Render the permission registry as one row per (origin, bit) entry.
        fn rebuild_permissions(&self) {
            let group = self.group_permissions.get();
            for row in self.perm_rows.borrow_mut().drain(..) {
                group.remove(&row);
            }

            let entries = crate::permissions_store::entries();
            for (origin, bit, state) in entries {
                let perm = crate::handlers::permission::permission_label(bit);
                let verdict = match state {
                    crate::permissions_store::State::Allow => "Allowed",
                    crate::permissions_store::State::Deny => "Denied",
                    crate::permissions_store::State::Ask => "Ask",
                };
                let row = adw::ActionRow::builder()
                    .title(&origin)
                    .subtitle(format!("{perm} — {verdict}"))
                    .build();

                let remove = gtk::Button::builder()
                    .icon_name("user-trash-symbolic")
                    .valign(gtk::Align::Center)
                    .tooltip_text("Remove this decision")
                    .build();
                remove.add_css_class("flat");

                let obj = self.obj().downgrade();
                let origin_c = origin.clone();
                remove.connect_clicked(move |_| {
                    crate::permissions_store::remove(&origin_c, bit);
                    if let Some(obj) = obj.upgrade() {
                        obj.imp().rebuild_permissions();
                    }
                });
                row.add_suffix(&remove);

                group.add(&row);
                self.perm_rows.borrow_mut().push(row);
            }
        }

        fn bind_accessibility(&self, settings: &gio::Settings) {
            bind_switch(settings, "reduce-motion", &self.row_motion);
            bind_switch(settings, "focus-indicators", &self.row_focus);
            bind_switch(settings, "high-contrast", &self.row_contrast);
            bind_switch(settings, "webview-zoom", &self.row_zoom);
            settings
                .bind("zoom-level", &self.row_zoom_level.get(), "value")
                .flags(gio::SettingsBindFlags::DEFAULT)
                .build();
            bind_switch(settings, "zoom-controls-headerbar", &self.row_zoom_headerbar);
            bind_switch(settings, "screen-reader-opts", &self.row_sr_opts);
            bind_switch(settings, "enable-shortcuts", &self.row_kb_shortcuts);
            bind_switch(settings, "a11y-shortcuts", &self.row_a11y_shortcuts);
            bind_switch(settings, "dev-shortcuts", &self.row_dev_shortcuts);
            bind_switch(settings, "notify-shortcuts", &self.row_notify_shortcuts);
        }
    }
}
