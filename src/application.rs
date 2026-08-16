use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use libadwaita as adw;

use crate::window::KarereWindow;

// App-id varies by build profile (io.github.tobagin.karere[.Devel]); set by
// build.rs from meson's KARERE_APP_ID. RESOURCE_BASE_PATH is the bundled
// gresource prefix and stays fixed regardless of profile.
pub const APP_ID: &str = env!("KARERE_APP_ID");
pub const PROFILE: &str = env!("KARERE_PROFILE");
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
        // Re-assert the color scheme here: CEF initialization (between `startup`
        // and this point) can reset the GTK/Adwaita color scheme, so a dark
        // selection chosen at `startup` is lost — the window then opens light
        // until the user toggles the theme. Re-applying after CEF init makes it
        // stick, including the start-in-background case below. (#160)
        apply_theme(&settings);
        // Honored even without a tray icon: relaunching the app presents the
        // existing window (see the top of this fn), so it stays reachable. (#170)
        if settings.boolean("start-in-background") {
            log::info!("start-in-background=true — window built but not presented");
            // OSR browsers only spawn when the window is shown; pre-warm so
            // WhatsApp loads and notifies while we stay in the background.
            window.prewarm();
        } else {
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

pub(crate) fn apply_theme(settings: &gio::Settings) {
    let value = settings.string("theme");
    let mgr = adw::StyleManager::default();
    mgr.set_color_scheme(map_theme(value.as_str()));
    // Mirror the effective scheme to the web content: WhatsApp Web (on its
    // default "System" theme) follows prefers-color-scheme, which CEF never
    // derives from the libadwaita theme — so emulate it over CDP. (#160)
    crate::cdp::set_dark_preference(mgr.is_dark());
    if let Some(app) = gio::Application::default().and_downcast::<gtk::Application>() {
        for win in app.windows() {
            if let Some(kw) = win.downcast_ref::<KarereWindow>() {
                kw.reapply_web_color_scheme();
            }
        }
    }
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
        (
            "win.prev-account",
            &["<Primary><Shift>Tab", "<Primary>Page_Up"],
        ),
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
        ("win.zoom-out", &["<Primary>minus", "<Primary>KP_Subtract"]),
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

            // Load the focus-indicator stylesheet once at APPLICATION priority
            // (user themes still win). Scoped under `.enhanced-focus`, which the
            // window toggles from the `focus-indicators` GSetting — inert until
            // a window opts in.
            if let Some(display) = gtk::gdk::Display::default() {
                let provider = gtk::CssProvider::new();
                provider.load_from_resource(&format!("{RESOURCE_BASE_PATH}/style.css"));
                gtk::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }

            // Optional tint of the GTK chrome (window/headerbar/dialog
            // backgrounds) with WhatsApp Web's own background so the shell
            // blends with the page (`match-whatsapp-colors`). Driven off
            // AdwStyleManager rather than a CSS media query so it follows
            // Karere's theme setting, not just the desktop scheme. (#168)
            {
                let wa_bg = gtk::CssProvider::new();
                if let Some(display) = gtk::gdk::Display::default() {
                    gtk::style_context_add_provider_for_display(
                        &display,
                        &wa_bg,
                        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                    );
                }
                let apply_wa_bg = move || {
                    let css = if gio::Settings::new(APP_ID).boolean("match-whatsapp-colors") {
                        let bg = if adw::StyleManager::default().is_dark() {
                            "#1d1f1f"
                        } else {
                            "#f7f5f3"
                        };
                        // Modern libadwaita styles via CSS variables; keep the
                        // legacy @define-color for older runtimes.
                        format!(
                            ":root {{ --window-bg-color: {bg}; }}\n\
                             @define-color window_bg_color {bg};"
                        )
                    } else {
                        String::new()
                    };
                    wa_bg.load_from_string(&css);
                };
                apply_wa_bg();
                let apply = apply_wa_bg.clone();
                adw::StyleManager::default().connect_dark_notify(move |_| apply());
                // GSettings signals die with the object — keep this one alive
                // for the app's lifetime or the toggle only applies on restart.
                let settings_wa = gio::Settings::new(APP_ID);
                settings_wa
                    .connect_changed(Some("match-whatsapp-colors"), move |_, _| apply_wa_bg());
                std::mem::forget(settings_wa);
            }

            // Start the SNI tray; honors GNOME skip policy + KARERE_FORCE_TRAY
            // internally, so it's safe to call unconditionally.
            crate::tray::start();

            let settings = gio::Settings::new(APP_ID);
            apply_theme(&settings);
            settings.connect_changed(Some("theme"), |s, _| apply_theme(s));
            // On the "system" theme, the effective dark state flips when the
            // desktop scheme changes; re-mirror it to the web content. (#160)
            adw::StyleManager::default().connect_dark_notify(|mgr| {
                crate::cdp::set_dark_preference(mgr.is_dark());
                if let Some(app) = gio::Application::default().and_downcast::<gtk::Application>() {
                    for win in app.windows() {
                        if let Some(kw) = win.downcast_ref::<KarereWindow>() {
                            kw.reapply_web_color_scheme();
                        }
                    }
                }
            });
            // Live tray enable/disable/auto.
            settings.connect_changed(Some("systray-icon"), |_, _| {
                crate::tray::apply_setting();
            });
            // Live mobile-layout: reload accounts so the page gate re-evaluates.
            settings.connect_changed(
                Some("mobile-layout"),
                glib::clone!(
                    #[weak(rename_to = app)]
                    app,
                    move |_, _| {
                        for win in app.windows() {
                            if let Some(kw) = win.downcast_ref::<KarereWindow>() {
                                kw.reload_all_accounts();
                            }
                        }
                    }
                ),
            );
            // Live notification-sound / master-toggle: re-apply audio muting.
            settings.connect_changed(
                None,
                glib::clone!(
                    #[weak(rename_to = app)]
                    app,
                    move |_, key| {
                        if key == "notify-sound-enabled" || key == "notifications-enabled" {
                            for win in app.windows() {
                                if let Some(kw) = win.downcast_ref::<KarereWindow>() {
                                    kw.apply_audio_mute();
                                }
                            }
                        }
                    }
                ),
            );
            // Keep Settings alive so the change handlers keep firing.
            let _ = self.settings.set(settings);
        }
    }

    impl GtkApplicationImpl for KarereApplication {}
    impl AdwApplicationImpl for KarereApplication {}
}
