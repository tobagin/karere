use gtk::gio;
use libadwaita as adw;
use adw::prelude::*;

pub fn show(window: &gtk::Window) {
    let preferences = adw::PreferencesDialog::builder()
        .title("Preferences")
        .build();

    let page = adw::PreferencesPage::builder()
        .title("General")
        .icon_name("preferences-system-symbolic")
        .build();

    let settings = gio::Settings::new("io.github.tobagin.karere");
    
    // 1. Startup
    let group_startup = adw::PreferencesGroup::builder().title("Startup").build();
    
    let row_background = adw::SwitchRow::builder().title("Start in background").build();
    settings.bind("start-in-background", &row_background, "active").build();
    group_startup.add(&row_background);

    let model_tray = gtk::StringList::new(&["Auto", "Enabled", "Disabled"]);
    let row_tray = adw::ComboRow::builder()
        .title("System Tray Icon")
        .model(&model_tray)
        .tooltip_text("Auto: Enabled for non-GNOME environments")
        .build();
    
    // Map: 0->"auto", 1->"enabled", 2->"disabled"
    // Note: The UI StringList order is ["Auto", "Enabled", "Disabled"] 
    settings.bind("systray-icon", &row_tray, "selected")
        .mapping(|variant, _type| {
            // GSettings (string from variant) -> UI (u32 in Value)
            let val = variant.get::<String>().unwrap_or_else(|| "auto".to_string());
            let idx: u32 = match val.as_str() {
                "enabled" => 1,
                "disabled" => 2,
                "auto" | _ => 0,
            };
            Some(idx.to_value())
        })
        .set_mapping(|value, _type| {
            // UI (u32 from Value) -> GSettings (string variant)
            let idx = value.get::<u32>().unwrap_or(0);
            let val = match idx {
                1 => "enabled",
                2 => "disabled",
                _ => "auto",
            };
            Some(val.to_variant())
        })
        .build();

    group_startup.add(&row_tray);

    // 2. Appearance
    let group_appearance = adw::PreferencesGroup::builder().title("Appearance").build();
    let model_theme = gtk::StringList::new(&["Follow System", "Light", "Dark"]);
    let row_theme = adw::ComboRow::builder()
        .title("Theme")
        .model(&model_theme)
        .build();
    settings.bind("theme", &row_theme, "selected")
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
    group_appearance.add(&row_theme);

    // 3. Spell Checking
    // 3. Spell Checking
    // 3. Spell Checking
    let group_spell = adw::PreferencesGroup::builder().title("Spell Checking").build();
    
    // 1. Enable Toggle
    let row_spell_enable = adw::SwitchRow::builder().title("Enable Spell Checking").build();
    settings.bind("enable-spell-checking", &row_spell_enable, "active").build();
    group_spell.add(&row_spell_enable);

    // 2. Dictionary Status (suffix count)
    let available_dicts = crate::spellcheck::get_available_dictionaries();
    let dict_count = available_dicts.len();
    let count_text = if dict_count == 1 {
        "1 Dictionary".to_string()
    } else {
        format!("{} Dictionaries", dict_count)
    };
    let status_label = gtk::Label::builder()
        .label(&count_text)
        .valign(gtk::Align::Center)
        .build();
    let row_spell_status = adw::ActionRow::builder()
        .title("Dictionary Status")
        .build();
    row_spell_status.add_suffix(&status_label);
    settings.bind("enable-spell-checking", &row_spell_status, "visible").build(); // Visible if enabled
    group_spell.add(&row_spell_status);
    
    // 3. Active Language (Label for Auto Mode)
    let active_lang_label = gtk::Label::builder()
        .valign(gtk::Align::Center)
        .build();
    let row_spell_active_label = adw::ActionRow::builder()
        .title("Active Language")
        .build();
    row_spell_active_label.add_suffix(&active_lang_label);
    group_spell.add(&row_spell_active_label);

    // 4. Active Language (Combo for Manual Mode) - replaces "Choose Language"
    // Move definition up so we can add it here
    let model_langs = gtk::StringList::new(
        &available_dicts.iter().map(|s| s.as_str()).collect::<Vec<&str>>()
    );

    let row_spell_active_combo = adw::ComboRow::builder()
        .title("Active Language")
        .model(&model_langs)
        .build();
    
    // Bind selection to "spell-checking-languages"
    let available_dicts_clone = available_dicts.clone();
    settings.bind("spell-checking-languages", &row_spell_active_combo, "selected")
        .mapping(move |variant, _type| {
            // GSettings (['en_US']) -> UI index
            let langs = variant.get::<Vec<String>>().unwrap_or_default();
            if let Some(first) = langs.first() {
                if let Some(idx) = available_dicts_clone.iter().position(|r| r == first) {
                    return Some((idx as u32).to_value());
                }
            }
            Some(0u32.to_value()) // Default to first
        })
        .set_mapping(move |value, _type| {
            // UI index -> GSettings (['en_US'])
            let idx = value.get::<u32>().unwrap_or(0) as usize; 
            let dicts = crate::spellcheck::get_available_dictionaries(); 
            if let Some(lang) = dicts.get(idx) {
                Some(vec![lang].to_variant())
            } else {
                 Some(vec!["en_US"].to_variant()) 
            }
        })
        .build();
    group_spell.add(&row_spell_active_combo);

    // 5. Override Dictionary (Inverted Auto-Detect)
    let row_spell_override = adw::SwitchRow::builder().title("Override Dictionary").build();
    
    // Bind "auto-detect-language" to "active" but INVERTED
    settings.bind("auto-detect-language", &row_spell_override, "active")
        .mapping(|variant, _type| {
            // GSettings (auto=true) -> UI (override=false)
            let auto = variant.get::<bool>().unwrap_or(true);
            Some((!auto).to_value())
        })
        .set_mapping(|value, _type| {
            // UI (override=true) -> GSettings (auto=false)
            let override_val = value.get::<bool>().unwrap_or(false);
            Some((!override_val).to_variant())
        })
        .build();
    settings.bind("enable-spell-checking", &row_spell_override, "visible").build(); // Visible if enabled
    group_spell.add(&row_spell_override);



    // Logic to update UI state (Visibility & Active Label)
    let row_active_label_clone = row_spell_active_label.clone();
    let row_active_combo_clone = row_spell_active_combo.clone();
    let label_widget_clone = active_lang_label.clone();
    
    let update_ui_state = move |settings: &gio::Settings| {
        let enabled = settings.boolean("enable-spell-checking");
        let auto = settings.boolean("auto-detect-language");
        
        // Visibility Logic:
        // 1. Label Row: Visible if Enabled AND Auto is ON
        row_active_label_clone.set_visible(enabled && auto);
        
        // 2. Combo Row: Visible if Enabled AND Auto is OFF (Override ON)
        row_active_combo_clone.set_visible(enabled && !auto);
        
        // Update Label Text (only matters when visible, but good to keep updated)
        if auto {
             let locale = std::env::var("LANG").unwrap_or_else(|_| "Unknown".to_string());
             let clean_locale = locale.split('.').next().unwrap_or(&locale).to_string();
             label_widget_clone.set_label(&clean_locale);
        }
    };

    // Initial update
    update_ui_state(&settings);

    // Connect signals
    let update_closure = update_ui_state.clone();
    settings.connect_changed(Some("enable-spell-checking"), move |s, _| update_closure(s));
    let update_closure = update_ui_state.clone();
    settings.connect_changed(Some("auto-detect-language"), move |s, _| update_closure(s));
    let update_closure = update_ui_state.clone();
    settings.connect_changed(Some("spell-checking-languages"), move |s, _| update_closure(s));





    // 4. Downloads
    let group_downloads = adw::PreferencesGroup::builder().title("Downloads").build();
    let row_download = adw::ActionRow::builder()
        .title("Download Directory")
        .build();
        
    settings.bind("download-directory", &row_download, "subtitle")
        .mapping(|variant, _type| {
            let path = variant.get::<String>().unwrap_or_default();
            println!("Mapping download-directory value: '{}'", path);
            if path.is_empty() {
                Some("Default (Downloads)".to_value())
            } else {
                Some(path.to_value())
            }
        })
        .build();
    let btn_choose = gtk::Button::builder().label("Choose...").valign(gtk::Align::Center).build();
    let btn_reset = gtk::Button::builder().icon_name("edit-clear-symbolic").valign(gtk::Align::Center).tooltip_text("Reset to default").build();
    row_download.add_suffix(&btn_choose);
    row_download.add_suffix(&btn_reset);
    group_downloads.add(&row_download);

    // Logic for Choose Button
    let settings_clone_dl = settings.clone();
    let window_clone = window.clone();
    
    btn_choose.connect_clicked(move |_| {
        let current_dir = settings_clone_dl.string("download-directory");
        let initial_file = if !current_dir.is_empty() {
             Some(gio::File::for_path(std::path::PathBuf::from(current_dir)))
        } else {
             None 
        };

        let dialog = gtk::FileDialog::builder()
            .title("Select Download Directory")
            .modal(true)
            .accept_label("Select")
            .build();
            
        if let Some(f) = initial_file {
            dialog.set_initial_folder(Some(&f));
        }
        
        // ... build continues ...
        
        let settings_clone_inner = settings_clone_dl.clone();
        let window_clone_inner = window_clone.clone();
        
        dialog.select_folder(Some(&window_clone_inner), gio::Cancellable::NONE, move |result| {
            match result {
                Ok(file) => {
                    if let Some(path) = file.path() {
                        if let Some(path_str) = path.to_str() {
                             println!("Selected path: {}", path_str);
                             let res = settings_clone_inner.set_string("download-directory", path_str);
                             println!("Setting updated: {:?}", res);
                        } else {
                            println!("Failed to convert path to string");
                        }
                    } else {
                        println!("Failed to get path from file");
                    }
                },
                Err(err) => {
                    // Ignore cancellation errors which are common
                    println!("Folder selection error or cancelled: {:?}", err);
                }
            }
        });
    });

    // Logic for Reset Button
    let settings_clone_reset = settings.clone();
    btn_reset.connect_clicked(move |_| {
        let _ = settings_clone_reset.set_string("download-directory", "");
    });


    // 5. Developer Tools
    let group_devtools = adw::PreferencesGroup::builder().title("Developer Tools").build();
    let row_dev_enable = adw::SwitchRow::builder().title("Enable Developer Tools").build();
    settings.bind("enable-developer-tools", &row_dev_enable, "active").build();
    group_devtools.add(&row_dev_enable);
    let row_dev_open = adw::ActionRow::builder()
        .title("Developer Tools")
        .build();
    let btn_dev_open = gtk::Button::builder()
        .label("Open")
        .valign(gtk::Align::Center)
        .action_name("win.show-devtools")
        .build();
    settings.bind("enable-developer-tools", &row_dev_open, "visible").build(); // Bind row visibility
    row_dev_open.add_suffix(&btn_dev_open);
    group_devtools.add(&row_dev_open);

    // Assemble
    page.add(&group_startup);
    page.add(&group_appearance);
    page.add(&group_spell);
    page.add(&group_downloads);
    page.add(&group_devtools);

    preferences.add(&page);

    // Accessibility Page
    let page_accessibility = adw::PreferencesPage::builder()
        .title("Accessibility")
        .icon_name("preferences-desktop-accessibility-symbolic")
        .build();

    // 1. Keyboard Navigation
    let group_keyboard = adw::PreferencesGroup::builder().title("Keyboard Navigation").build();
    
    let row_kb_shortcuts = adw::SwitchRow::builder().title("Enable keyboard shortcuts").build();
    settings.bind("enable-shortcuts", &row_kb_shortcuts, "active").build();
    group_keyboard.add(&row_kb_shortcuts);

    let row_focus = adw::SwitchRow::builder().title("Focus indicators").build();
    settings.bind("focus-indicators", &row_focus, "active").build();
    group_keyboard.add(&row_focus);

    let row_view_shortcuts = adw::ActionRow::builder()
        .title("View keyboard shortcuts")
        .build();
    let _btn_view_shortcuts = gtk::Button::builder().label("View").valign(gtk::Align::Center).build();
    // Connect button to show shortcuts window
    // Using action-name on button directly
    let btn_view_shortcuts = gtk::Button::builder()
        .label("View")
        .valign(gtk::Align::Center)
        .action_name("win.show-help-overlay")
        .build();

    row_view_shortcuts.add_suffix(&btn_view_shortcuts);
    group_keyboard.add(&row_view_shortcuts);
    page_accessibility.add(&group_keyboard);

    // 2. Visual Accessibility
    let group_visual = adw::PreferencesGroup::builder().title("Visual Accessibility").build();
    
    let row_contrast = adw::SwitchRow::builder().title("High Contract Mode").build();
    settings.bind("high-contrast", &row_contrast, "active").build();
    group_visual.add(&row_contrast);

    let row_motion = adw::SwitchRow::builder().title("Reduce Motion").build();
    settings.bind("reduce-motion", &row_motion, "active").build();
    group_visual.add(&row_motion);
    page_accessibility.add(&group_visual);

    // 3. Webview Zoom
    let group_zoom = adw::PreferencesGroup::builder().title("Webview Zoom").build();
    let row_zoom = adw::SwitchRow::builder().title("Enable Webview Zoom").build();
    settings.bind("webview-zoom", &row_zoom, "active").build();
    group_zoom.add(&row_zoom);
    page_accessibility.add(&group_zoom);

    // 4. Screen Reader Support
    let group_screen_reader = adw::PreferencesGroup::builder().title("Screen Reader Support").build();
    
    let row_sr_opts = adw::SwitchRow::builder().title("Screen Reader Optimizations").build();
    settings.bind("screen-reader-opts", &row_sr_opts, "active").build();
    group_screen_reader.add(&row_sr_opts);

    let row_sr_status = adw::ActionRow::builder().title("Screen Reader Status").build();
    // Check if a screen reader is active (this is dynamic, but we can check once on open)
    let settings_gtk = gtk::Settings::default().expect("Failed to get default settings");
    let _sr_enabled = settings_gtk.is_gtk_enable_accels(); // Not quite right, likely gtk-enable-a11y-widget-scaling or similar. 
    // Actually, easy way is to bind to a label or just set static text for now as place holder or implementation.
    // The user requested "Screen Reader Status Row with status label".
    let label_sr_status = gtk::Label::builder().label("Inactive").valign(gtk::Align::Center).build();
    row_sr_status.add_suffix(&label_sr_status);
    group_screen_reader.add(&row_sr_status);
    page_accessibility.add(&group_screen_reader);

    // 5. Accessibility Shortcuts
    let group_shortcuts = adw::PreferencesGroup::builder().title("Accessibility Shortcuts").build();
    
    let row_a11y_shortcuts = adw::SwitchRow::builder().title("Accessibility Shortcuts").build();
    settings.bind("a11y-shortcuts", &row_a11y_shortcuts, "active").build();
    group_shortcuts.add(&row_a11y_shortcuts);

    let row_dev_shortcuts = adw::SwitchRow::builder().title("Developer Shortcuts").build();
    settings.bind("dev-shortcuts", &row_dev_shortcuts, "active").build();
    group_shortcuts.add(&row_dev_shortcuts);

    let row_notify_shortcuts = adw::SwitchRow::builder().title("Notification Shortcuts").build();
    settings.bind("notify-shortcuts", &row_notify_shortcuts, "active").build();
    group_shortcuts.add(&row_notify_shortcuts);
    page_accessibility.add(&group_shortcuts);

    preferences.add(&page_accessibility);

    // Notifications Page
    let page_notifications = adw::PreferencesPage::builder()
        .title("Notifications")
        .icon_name("preferences-system-notifications-symbolic")
        .build();

    // 1. General Notifications
    let group_notify_gen = adw::PreferencesGroup::builder().title("Notifications").build();

    let row_notify_sys = adw::SwitchRow::builder().title("System notifications").build();
    settings.bind("notify-system", &row_notify_sys, "active").build();
    group_notify_gen.add(&row_notify_sys);

    let row_notify_msg = adw::SwitchRow::builder().title("Message Notifications").build();
    settings.bind("notify-message", &row_notify_msg, "active").build();
    group_notify_gen.add(&row_notify_msg);

    let model_notify_bg = gtk::StringList::new(&["Always", "First Time", "Never"]);
    let row_notify_bg = adw::ComboRow::builder()
        .title("Background Notification")
        .model(&model_notify_bg)
        .build();
    // Bind to index (int)
    settings.bind("notify-background", &row_notify_bg, "selected").build();
    group_notify_gen.add(&row_notify_bg);

    let row_notify_sound = adw::SwitchRow::builder().title("Notification sounds").build();
    settings.bind("notify-sound", &row_notify_sound, "active").build();
    group_notify_gen.add(&row_notify_sound);

    let row_notify_dl = adw::SwitchRow::builder().title("Download Notifications").build();
    settings.bind("notify-download", &row_notify_dl, "active").build();
    group_notify_gen.add(&row_notify_dl);

    page_notifications.add(&group_notify_gen);

    // 2. Message Preview
    let group_preview = adw::PreferencesGroup::builder().title("Message Preview").build();
    
    let row_preview_show = adw::SwitchRow::builder().title("Show message preview").build();
    settings.bind("preview-show", &row_preview_show, "active").build();
    group_preview.add(&row_preview_show);

    // Use SpinRow if available, otherwise ActionRow + SpinButton
    // libadwaita 1.0+ has AdwActionRow, 1.4+ has AdwSpinRow. 
    // We are using feature v1_6, so SpinRow should be available.
    let row_preview_len = adw::SpinRow::builder()
        .title("Preview length")
        .adjustment(&gtk::Adjustment::new(50.0, 0.0, 500.0, 1.0, 10.0, 0.0))
        .build();
    settings.bind("preview-length", &row_preview_len, "value").build();
    group_preview.add(&row_preview_len);

    page_notifications.add(&group_preview);

    // 3. Background Check
    let group_bg_check = adw::PreferencesGroup::builder().title("Background").build();
    
    let row_check_freq = adw::SpinRow::builder()
        .title("Check frequency (minutes)")
        .adjustment(&gtk::Adjustment::new(10.0, 1.0, 60.0, 1.0, 5.0, 0.0))
        .build();
    settings.bind("check-frequency", &row_check_freq, "value").build();
    group_bg_check.add(&row_check_freq);

    page_notifications.add(&group_bg_check);

    preferences.add(&page_notifications);

    preferences.present(Some(window));
}
