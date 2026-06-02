use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use libadwaita as adw;

use crate::window::KarereWindow;

pub const APP_ID: &str = "io.github.tobagin.karere";
pub const RESOURCE_BASE_PATH: &str = "/io/github/tobagin/karere";

glib::wrapper! {
    pub struct KarereApplication(ObjectSubclass<imp::KarereApplication>)
        @extends adw::Application, gtk::Application, gio::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl KarereApplication {
    pub fn new() -> Self {
        glib::Object::builder()
            .property("application-id", APP_ID)
            .property("resource-base-path", RESOURCE_BASE_PATH)
            .property("flags", gio::ApplicationFlags::HANDLES_COMMAND_LINE)
            .build()
    }

    pub fn activate_with_url(&self, url: &str) {
        if let Some(win) = self
            .active_window()
            .or_else(|| self.windows().into_iter().next())
        {
            win.present();
            return;
        }
        let app: &adw::Application = self.upcast_ref();
        let window = KarereWindow::new(app, url);

        let settings = gio::Settings::new(APP_ID);
        if settings.boolean("start-in-background") && crate::tray::is_active() {
            log::info!(
                "start-in-background=true and tray configured — window built but not presented"
            );
        } else {
            if settings.boolean("start-in-background") && !crate::tray::is_active() {
                log::info!(
                    "start-in-background=true but no tray configured — presenting window so it is reachable"
                );
            }
            window.present();
        }
    }
}

impl Default for KarereApplication {
    fn default() -> Self {
        Self::new()
    }
}

fn map_theme(value: &str) -> adw::ColorScheme {
    match value {
        "system" => adw::ColorScheme::Default,
        "light" => adw::ColorScheme::ForceLight,
        "dark" => adw::ColorScheme::ForceDark,
        other => {
            log::warn!("unknown theme GSetting value {other:?}; falling back to system");
            adw::ColorScheme::Default
        }
    }
}

fn apply_theme(settings: &gio::Settings) {
    let value = settings.string("theme");
    adw::StyleManager::default().set_color_scheme(map_theme(value.as_str()));
}

fn register_accels(app: &KarereApplication) {
    let gtk_app: &gtk::Application = app.upcast_ref();
    let bindings: &[(&str, &[&str])] = &[
        ("app.quit", &["<Primary>q"]),
        ("app.preferences", &["<Primary>comma"]),
        ("app.show-help-overlay", &["<Primary>question"]),
        ("win.toggle-fullscreen", &["F11", "<Alt>Return"]),
        ("win.minimize", &["<Primary>m"]),
        ("win.close", &["<Primary>w"]),
        ("win.refresh", &["<Primary>r", "F5"]),
        ("win.refresh-hard", &["<Primary><Shift>r"]),
        ("win.show-devtools", &["F12", "<Primary><Shift>i"]),
        ("win.inspect-element", &["<Primary><Shift>c"]),
        ("win.find-in-page", &["<Primary>f"]),
        ("win.next-account", &["<Primary>Tab", "<Primary>Page_Down"]),
        ("win.prev-account", &["<Primary><Shift>Tab", "<Primary>Page_Up"]),
        ("win.switch-account-index(1)", &["<Alt>1"]),
        ("win.switch-account-index(2)", &["<Alt>2"]),
        ("win.switch-account-index(3)", &["<Alt>3"]),
        ("win.switch-account-index(4)", &["<Alt>4"]),
        ("win.switch-account-index(5)", &["<Alt>5"]),
        ("win.switch-account-index(6)", &["<Alt>6"]),
        ("win.switch-account-index(7)", &["<Alt>7"]),
        ("win.switch-account-index(8)", &["<Alt>8"]),
        ("win.switch-account-index(9)", &["<Alt>9"]),
        (
            "win.zoom-in",
            &["<Primary>plus", "<Primary>equal", "<Primary>KP_Add"],
        ),
        (
            "win.zoom-out",
            &["<Primary>minus", "<Primary>KP_Subtract"],
        ),
        ("win.zoom-reset", &["<Primary>0", "<Primary>KP_0"]),
    ];
    for (action, accels) in bindings {
        gtk_app.set_accels_for_action(action, accels);
    }
}

mod imp {
    use std::cell::OnceCell;

    use super::*;
    use libadwaita::subclass::prelude::*;

    #[derive(Default)]
    pub struct KarereApplication {
        pub settings: OnceCell<gio::Settings>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for KarereApplication {
        const NAME: &'static str = "KarereApplication";
        type Type = super::KarereApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for KarereApplication {}

    impl ApplicationImpl for KarereApplication {
        fn startup(&self) {
            self.parent_startup();
            let app = self.obj();

            crate::actions::register_app_actions(&app);
            register_accels(&app);

            // M19 enhanced focus indicators: load the bundled stylesheet once,
            // for the whole process, at APPLICATION priority (so user theme
            // overrides still win). The rules are scoped under `.enhanced-focus`;
            // the window toggles that class from the `focus-indicators` GSetting,
            // so this provider is inert until a window opts in. No per-window
            // CssProvider allocation occurs.
            if let Some(display) = gtk::gdk::Display::default() {
                let provider = gtk::CssProvider::new();
                provider.load_from_resource(&format!("{RESOURCE_BASE_PATH}/style.css"));
                gtk::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }

            // Start the SNI tray (M15). Honors the GNOME skip policy and the
            // KARERE_FORCE_TRAY override internally; safe to call unconditionally.
            crate::tray::start();

            let settings = gio::Settings::new(APP_ID);
            apply_theme(&settings);
            settings.connect_changed(
                Some("theme"),
                |s, _| apply_theme(s),
            );
            // Keep the Settings instance alive so the change handler keeps firing.
            let _ = self.settings.set(settings);
        }
    }

    impl GtkApplicationImpl for KarereApplication {}
    impl AdwApplicationImpl for KarereApplication {}
}
