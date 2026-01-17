use gtk::{gio, glib, prelude::*, subclass::prelude::*};
use gettextrs::gettext;
use libadwaita as adw;
use adw::prelude::*;
use adw::subclass::prelude::*;

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/tobagin/karere/ui/preferences.ui")]
    pub struct KarerePreferencesWindow {
        // Startup
        #[template_child] pub row_startup: TemplateChild<adw::SwitchRow>,
        #[template_child] pub row_background: TemplateChild<adw::SwitchRow>,
        #[template_child] pub row_tray: TemplateChild<adw::ComboRow>,
        
        // Appearance
        #[template_child] pub row_theme: TemplateChild<adw::ComboRow>,
        #[template_child] pub row_mobile_layout: TemplateChild<adw::ComboRow>,
        
        // Spell Checking
        #[template_child] pub row_spell_enable: TemplateChild<adw::SwitchRow>,
        #[template_child] pub row_spell_status: TemplateChild<adw::ActionRow>,
        #[template_child] pub label_spell_status: TemplateChild<gtk::Label>,
        #[template_child] pub row_spell_active_label: TemplateChild<adw::ActionRow>,
        #[template_child] pub label_active_lang: TemplateChild<gtk::Label>,
        #[template_child] pub row_spell_active_combo: TemplateChild<adw::ComboRow>,
        #[template_child] pub row_spell_override: TemplateChild<adw::SwitchRow>,
        #[template_child] pub row_auto_correct: TemplateChild<adw::SwitchRow>,
        
        // Downloads
        #[template_child] pub row_download: TemplateChild<adw::ActionRow>,
        #[template_child] pub btn_download_choose: TemplateChild<gtk::Button>,
        #[template_child] pub btn_download_reset: TemplateChild<gtk::Button>,
        
        // Developers
        #[template_child] pub row_dev_enable: TemplateChild<adw::SwitchRow>,
        #[template_child] pub row_dev_open: TemplateChild<adw::ActionRow>,
        
        // Accessibility
        #[template_child] pub row_kb_shortcuts: TemplateChild<adw::SwitchRow>,
        #[template_child] pub row_focus: TemplateChild<adw::SwitchRow>,
        #[template_child] pub row_contrast: TemplateChild<adw::SwitchRow>,
        #[template_child] pub row_motion: TemplateChild<adw::SwitchRow>,
        #[template_child] pub row_zoom: TemplateChild<adw::SwitchRow>,
        #[template_child] pub row_sr_opts: TemplateChild<adw::SwitchRow>,
        #[template_child] pub row_a11y_shortcuts: TemplateChild<adw::SwitchRow>,
        #[template_child] pub row_dev_shortcuts: TemplateChild<adw::SwitchRow>,
        #[template_child] pub row_notify_shortcuts: TemplateChild<adw::SwitchRow>,
        
        // Notifications
        #[template_child] pub row_master_toggle: TemplateChild<adw::SwitchRow>,
        #[template_child] pub group_master: TemplateChild<adw::PreferencesGroup>, // Not really needed if binding handles visibility
        #[template_child] pub group_messages: TemplateChild<adw::PreferencesGroup>,
        #[template_child] pub row_notify_msg: TemplateChild<adw::SwitchRow>,
        #[template_child] pub row_tray_anim: TemplateChild<adw::SwitchRow>,
        #[template_child] pub group_preview: TemplateChild<adw::PreferencesGroup>,
        #[template_child] pub row_preview_show: TemplateChild<adw::SwitchRow>,
        #[template_child] pub row_preview_limit: TemplateChild<adw::SwitchRow>,
        #[template_child] pub row_preview_len: TemplateChild<adw::SpinRow>,
        #[template_child] pub group_sounds: TemplateChild<adw::PreferencesGroup>,
        #[template_child] pub row_sound_enable: TemplateChild<adw::SwitchRow>,
        #[template_child] pub row_sound_file: TemplateChild<adw::ComboRow>,
        #[template_child] pub group_dl_notify: TemplateChild<adw::PreferencesGroup>,
        #[template_child] pub row_dl_enable: TemplateChild<adw::SwitchRow>,
        #[template_child] pub row_dl_type: TemplateChild<adw::ComboRow>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for KarerePreferencesWindow {
        const NAME: &'static str = "KarerePreferencesWindow";
        type Type = super::KarerePreferencesWindow;
        type ParentType = adw::PreferencesDialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for KarerePreferencesWindow {
        fn constructed(&self) {
             self.parent_constructed();
             let obj = self.obj();
             obj.setup_bindings();
             obj.setup_downloads();
             obj.setup_spellcheck();
        }
    }
    impl WidgetImpl for KarerePreferencesWindow {}
    impl AdwDialogImpl for KarerePreferencesWindow {}
    impl PreferencesDialogImpl for KarerePreferencesWindow {}
}

glib::wrapper! {
    pub struct KarerePreferencesWindow(ObjectSubclass<imp::KarerePreferencesWindow>)
        @extends gtk::Widget, adw::Dialog, adw::PreferencesDialog,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl KarerePreferencesWindow {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    fn setup_bindings(&self) {
         let imp = self.imp();
         let app_id = std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string());
         let settings = gio::Settings::new(&app_id);

         // 1. Startup
         settings.bind("run-on-startup", &*imp.row_startup, "active").build();
         settings.connect_changed(Some("run-on-startup"), move |settings, _| {
            let enabled = settings.boolean("run-on-startup");
            // Call the action defined in main.rs
            if let Some(app) = gio::Application::default() {
                app.activate_action("sync-autostart", Some(&enabled.to_variant()));
            } else {
                eprintln!("Failed to get default application to sync autostart.");
            }
         });

         settings.bind("start-in-background", &*imp.row_background, "active").build();
         
         settings.bind("systray-icon", &*imp.row_tray, "selected")
            .mapping(|variant, _type| {
                let val = variant.get::<String>().unwrap_or_else(|| "auto".to_string());
                let idx: u32 = match val.as_str() {
                    "enabled" => 1,
                    "disabled" => 2,
                    "auto" | _ => 0,
                };
                Some(idx.to_value())
            })
            .set_mapping(|value, _type| {
                let idx = value.get::<u32>().unwrap_or(0);
                let val = match idx {
                    1 => "enabled",
                    2 => "disabled",
                    _ => "auto",
                };
                Some(val.to_variant())
            })
            .build();

         // 2. Appearance
         settings.bind("theme", &*imp.row_theme, "selected")
            .mapping(|variant, _type| {
                let val = variant.get::<String>().unwrap_or_else(|| "system".to_string());
                let idx: u32 = match val.as_str() {
                    "light" => 1,
                    "dark" => 2,
                    "system" | _ => 0,
                };
                Some(idx.to_value())
            })
            .set_mapping(|value, _type| {
                let idx = value.get::<u32>().unwrap_or(0);
                let val = match idx {
                    1 => "light",
                    2 => "dark",
                    _ => "system",
                };
                Some(val.to_variant())
            })
            .build();

         settings.bind("mobile-layout", &*imp.row_mobile_layout, "selected")
            .mapping(|variant, _type| {
                let val = variant.get::<String>().unwrap_or_else(|| "auto".to_string());
                let idx: u32 = match val.as_str() {
                    "enabled" => 1,
                    "disabled" => 2,
                    "auto" | _ => 0,
                };
                Some(idx.to_value())
            })
            .set_mapping(|value, _type| {
                let idx = value.get::<u32>().unwrap_or(0);
                let val = match idx {
                    1 => "enabled",
                    2 => "disabled",
                    _ => "auto",
                };
                Some(val.to_variant())
            })
            .build();

         // 3. Spell Checking
         settings.bind("enable-spell-checking", &*imp.row_spell_enable, "active").build();
         settings.bind("enable-spell-checking", &*imp.row_spell_status, "visible").build();
         
         settings.bind("auto-detect-language", &*imp.row_spell_override, "active")
            .mapping(|variant, _type| {
                let auto = variant.get::<bool>().unwrap_or(true);
                Some((!auto).to_value())
            })
            .set_mapping(|value, _type| {
                let override_val = value.get::<bool>().unwrap_or(false);
                Some((!override_val).to_variant())
            })
            .build();
         settings.bind("enable-spell-checking", &*imp.row_spell_override, "visible").build();
         
         settings.bind("enable-auto-correct", &*imp.row_auto_correct, "active").build();
         settings.bind("enable-spell-checking", &*imp.row_auto_correct, "visible").build();
         
         // Visibility Logic for Spellcheck (UI state)
         // We do this via signals as per original code
         let label_widget = imp.label_active_lang.clone(); // Template child is a wrapper, clone is cheap
         let row_active_label = imp.row_spell_active_label.clone();
         let row_active_combo = imp.row_spell_active_combo.clone();
         
         let update_spell_ui = move |settings: &gio::Settings| {
             let enabled = settings.boolean("enable-spell-checking");
             let auto = settings.boolean("auto-detect-language");
             
             row_active_label.set_visible(enabled && auto);
             row_active_combo.set_visible(enabled && !auto);
             
             if auto {
                  let locale = std::env::var("LANG").unwrap_or_else(|_| "Unknown".to_string());
                  let clean_locale = locale.split('.').next().unwrap_or(&locale).to_string();
                  label_widget.set_label(&clean_locale);
             }
         };
         
         let up = update_spell_ui.clone();
         settings.connect_changed(Some("enable-spell-checking"), move |s, _| up(s));
         let up = update_spell_ui.clone();
         settings.connect_changed(Some("auto-detect-language"), move |s, _| up(s));
         let up = update_spell_ui.clone();
         settings.connect_changed(Some("spell-checking-languages"), move |s, _| up(s));
         update_spell_ui(&settings);

         // 4. Downloads
         settings.bind("download-directory", &*imp.row_download, "subtitle")
            .mapping(|variant, _type| {
                let path_str = variant.get::<String>().unwrap_or_default();
                if path_str.is_empty() {
                    Some(gettext("Default (Downloads)").to_value())
                } else {
                    // Prettify path if it's a Flatpak doc portal path
                    // Portal paths look like: /run/user/1000/doc/ID/Name
                    if path_str.starts_with("/run/user/") && path_str.contains("/doc/") {
                        let path = std::path::Path::new(&path_str);
                        if let Some(name) = path.file_name() {
                             // Heuristic: Display as ~/Name (assuming it's likely a home folder)
                             // or just Name if we want to be strictly accurate.
                             // User asked for /home/username/Documents style.
                             // Let's try to reconstruct a friendly path.
                             let home = glib::home_dir();
                             let friendly = home.join(name);
                             return Some(friendly.to_string_lossy().to_value());
                        }
                    }
                    Some(path_str.to_value())
                }
            })
            .build();
         
         // 5. Dev Tools
         settings.bind("enable-developer-tools", &*imp.row_dev_enable, "active").build();
         settings.bind("enable-developer-tools", &*imp.row_dev_open, "visible").build();
         
         // 6. Accessibility
         settings.bind("enable-shortcuts", &*imp.row_kb_shortcuts, "active").build();
         settings.bind("focus-indicators", &*imp.row_focus, "active").build();
         settings.bind("high-contrast", &*imp.row_contrast, "active").build();
         settings.bind("reduce-motion", &*imp.row_motion, "active").build();
         settings.bind("webview-zoom", &*imp.row_zoom, "active").build();
         settings.bind("screen-reader-opts", &*imp.row_sr_opts, "active").build();
         
         // Shortcuts Sub-settings: Bind sensitivity to master toggle
         settings.bind("a11y-shortcuts", &*imp.row_a11y_shortcuts, "active").build();
         settings.bind("enable-shortcuts", &*imp.row_a11y_shortcuts, "sensitive").build();
         
         settings.bind("dev-shortcuts", &*imp.row_dev_shortcuts, "active").build();
         settings.bind("enable-shortcuts", &*imp.row_dev_shortcuts, "sensitive").build();
         
         settings.bind("notify-shortcuts", &*imp.row_notify_shortcuts, "active").build();
         settings.bind("enable-shortcuts", &*imp.row_notify_shortcuts, "sensitive").build();

         // 7. Notifications
         settings.bind("notifications-enabled", &*imp.row_master_toggle, "active").build();
         settings.bind("notifications-enabled", &*imp.group_messages, "visible").build();
         
         settings.bind("notify-messages", &*imp.row_notify_msg, "active").build();
         settings.bind("notify-tray-icon", &*imp.row_tray_anim, "active").build();
         settings.bind("notify-messages", &*imp.row_tray_anim, "visible").build();
         
         
         settings.bind("notify-preview-enabled", &*imp.row_preview_show, "active").build();
         settings.bind("notify-preview-limit-enabled", &*imp.row_preview_limit, "active").build();
         settings.bind("notify-preview-enabled", &*imp.row_preview_limit, "visible").build();
         settings.bind("notify-preview-length", &*imp.row_preview_len, "value").build();
         settings.bind("notify-preview-limit-enabled", &*imp.row_preview_len, "visible").build();
         
         settings.bind("notifications-enabled", &*imp.group_sounds, "visible").build();
         settings.bind("notify-sound-enabled", &*imp.row_sound_enable, "active").build();
         settings.bind("notify-sound-file", &*imp.row_sound_file, "selected")
             .mapping(|variant, _type| {
                 let val = variant.get::<String>().unwrap_or_default();
                 let idx = match val.as_str() {
                     "pop" => 1,
                     "alert" => 2,
                     "soft" => 3,
                     "start" => 4,
                     "whatsapp" | "default" | _ => 0,
                 };
                 Some(idx.to_value())
             })
             .set_mapping(|value, _type| {
                 let idx = value.get::<u32>().unwrap_or(0);
                 let val = match idx {
                     1 => "pop",
                     2 => "alert",
                     3 => "soft",
                     4 => "start",
                     _ => "whatsapp",
                 };
                 Some(val.to_variant())
             })
             .build();
         settings.bind("notify-sound-enabled", &*imp.row_sound_file, "visible").build();
         
         settings.bind("notifications-enabled", &*imp.group_dl_notify, "visible").build();
         settings.bind("notify-downloads-enabled", &*imp.row_dl_enable, "active").build();
         settings.bind("notify-download-type", &*imp.row_dl_type, "selected")
             .mapping(|variant, _type| {
                 let val = variant.get::<String>().unwrap_or_default();
                 let idx = if val == "system" { 1 } else { 0 };
                 Some(idx.to_value())
             })
             .set_mapping(|value, _type| {
                 let idx = value.get::<u32>().unwrap_or(0);
                 let val = if idx == 1 { "system" } else { "toast" };
                 Some(val.to_variant())
             })
             .build();
         settings.bind("notify-downloads-enabled", &*imp.row_dl_type, "visible").build();
         
         // Preview Visibility Logic (Master AND Messages)
         let group_preview = imp.group_preview.clone();
         let update_prev = move |settings: &gio::Settings| {
             let master = settings.boolean("notifications-enabled");
             let msg = settings.boolean("notify-messages");
             group_preview.set_visible(master && msg);
         };
         let upv = update_prev.clone();
         settings.connect_changed(Some("notifications-enabled"), move |s, _| upv(s));
         let upv = update_prev.clone();
         settings.connect_changed(Some("notify-messages"), move |s, _| upv(s));
         update_prev(&settings);
         
         // Sound Preview
         imp.row_sound_file.connect_selected_notify(move |row| {
             let idx = row.selected();
             let sound_name = match idx {
                 1 => "pop",
                 2 => "alert",
                 3 => "soft",
                 4 => "start",
                 _ => "whatsapp",
             };
             let resource_path = format!("/io/github/tobagin/karere/sounds/{}.oga", sound_name);
             if let Ok(bytes) = gio::resources_lookup_data(&resource_path, gio::ResourceLookupFlags::NONE) {
                  let temp_path = std::env::temp_dir().join("karere-preview.oga");
                  if std::fs::write(&temp_path, &bytes).is_ok() {
                       let _ = std::process::Command::new("paplay")
                           .arg(temp_path)
                           .spawn();
                  }
             }
         });
    }

    fn setup_spellcheck(&self) {
        let imp = self.imp();
        let app_id = std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string());
        let settings = gio::Settings::new(&app_id);
        
        let available_dicts = crate::spellcheck::get_available_dictionaries();
        let dict_count = available_dicts.len();
        let count_text = if dict_count == 1 { 
            gettext("1 Dictionary") 
        } else { 
            // In a real app we'd use ngettext for plurals, but for now simple gettext
            format!("{} {}", dict_count, gettext("Dictionaries"))
        };
        imp.label_spell_status.set_label(&count_text);
        
        let model_langs = gtk::StringList::new(
             &available_dicts.iter().map(|s| s.as_str()).collect::<Vec<&str>>()
        );
        imp.row_spell_active_combo.set_model(Some(&model_langs));
        
        // Bind active combo
        let available_dicts_clone = available_dicts.clone();
        settings.bind("spell-checking-languages", &*imp.row_spell_active_combo, "selected")
            .mapping(move |variant, _type| {
                let langs = variant.get::<Vec<String>>().unwrap_or_default();
                if let Some(first) = langs.first() {
                    if let Some(idx) = available_dicts_clone.iter().position(|r| r == first) {
                        return Some((idx as u32).to_value());
                    }
                }
                Some(0u32.to_value())
            })
            .set_mapping(move |value, _type| {
                let idx = value.get::<u32>().unwrap_or(0) as usize; 
                let dicts = crate::spellcheck::get_available_dictionaries(); 
                if let Some(lang) = dicts.get(idx) {
                    Some(vec![lang].to_variant())
                } else {
                     Some(vec!["en_US"].to_variant()) 
                }
            })
            .build();
    }
    
    fn setup_downloads(&self) {
        let imp = self.imp();
        let app_id = std::env::var("FLATPAK_ID").unwrap_or_else(|_| "io.github.tobagin.karere".to_string());
        let settings = gio::Settings::new(&app_id);
        
        let settings_clone_dl = settings.clone();
        imp.btn_download_choose.connect_clicked(move |btn| {
             // We need a window parent. 
             // We can get root?
             let root = btn.root().and_then(|r| r.downcast::<gtk::Window>().ok());
             
             let current_dir = settings_clone_dl.string("download-directory");
             let initial_file = if !current_dir.is_empty() {
                  Some(gio::File::for_path(std::path::PathBuf::from(current_dir)))
             } else { None };
             
             let dialog = gtk::FileDialog::builder()
                 .title(&gettext("Select Download Directory"))
                 .modal(true)
                 .accept_label(&gettext("Select"))
                 .build();
             if let Some(f) = initial_file {
                 dialog.set_initial_folder(Some(&f));
             }
             
             let settings_inner = settings_clone_dl.clone();
             dialog.select_folder(root.as_ref(), gio::Cancellable::NONE, move |result| {
                 match result {
                     Ok(file) => {
                         if let Some(path) = file.path() {
                             if let Some(path_str) = path.to_str() {
                                 let _ = settings_inner.set_string("download-directory", path_str);
                             }
                         }
                     },
                     Err(_) => {}
                 }
             });
        });

        let settings_clone_reset = settings.clone();
        imp.btn_download_reset.connect_clicked(move |_| {
            let _ = settings_clone_reset.set_string("download-directory", "");
        });
    }
}

pub fn show(window: &gtk::Window) {
    let prefs = KarerePreferencesWindow::new();
    prefs.present(Some(window));
}
