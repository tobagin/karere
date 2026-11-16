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

    /// Handle one launch (first or forwarded). `link` is a validated WhatsApp
    /// Web URL from a deep link; `background` is the autostart flag.
    pub fn activate_with_url(&self, url: &str, link: Option<&str>, background: bool) {
        if let Some(win) = any_window(self) {
            if let Some(link) = link {
                if let Some(kw) = win.downcast_ref::<KarereWindow>() {
                    kw.load_url(link);
                }
            } else if background {
                // An autostart launch that finds a running instance must not
                // pop its window up.
                return;
            }
            win.set_visible(true);
            win.present();
            return;
        }
        let app: &adw::Application = self.upcast_ref();
        let window = KarereWindow::new(app, link.unwrap_or(url));

        let settings = gio::Settings::new(APP_ID);
        // Re-assert the color scheme here: CEF initialization (between `startup`
        // and this point) can reset the GTK/Adwaita color scheme, so a dark
        // selection chosen at `startup` is lost — the window then opens light
        // until the user toggles the theme. Re-applying after CEF init makes it
        // stick, including the start-in-background case below. (#160)
        apply_theme(&settings);
        // Only autostart launches (`--background`) stay hidden; a manual launch
        // always shows the window. A deep link also always shows it.
        if background && link.is_none() && settings.boolean("start-in-background") {
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

thread_local! {
    /// CSS provider for the `match-whatsapp-colors` tint.
    static WA_BG_PROVIDER: gtk::CssProvider = gtk::CssProvider::new();
    /// The page's own header background as `#rrggbb`, once a page reported it.
    static PAGE_BG: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// A computed opaque CSS colour, `rgb(r, g, b)`, as `#rrggbb`. Anything else
/// (translucent, other syntaxes) is rejected, so page-supplied text never
/// reaches GTK's CSS parser.
fn opaque_css_rgb_to_hex(color: &str) -> Option<String> {
    let inner = color.trim().strip_prefix("rgb(")?.strip_suffix(')')?;
    let mut parts = inner.split(',').map(|part| part.trim().parse::<u8>());
    let (r, g, b) = (
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    );
    parts
        .next()
        .is_none()
        .then(|| format!("#{r:02x}{g:02x}{b:02x}"))
}

/// Tint the GTK chrome (window, header bar and dialog backgrounds) with the
/// page's background when `match-whatsapp-colors` is on: the colour the page
/// reported, so a user stylesheet is followed, else WhatsApp Web's stock one.
/// Driven off AdwStyleManager, not a CSS media query, so the stock fallback
/// follows Karere's theme setting and not just the desktop scheme. (#168)
fn apply_wa_bg() {
    let css = if gio::Settings::new(APP_ID).boolean("match-whatsapp-colors") {
        let stock = if adw::StyleManager::default().is_dark() {
            "#1d1f1f"
        } else {
            "#f7f5f3"
        };
        let bg = PAGE_BG
            .with_borrow(Clone::clone)
            .unwrap_or_else(|| stock.to_owned());
        // Modern libadwaita styles via CSS variables; keep the legacy
        // @define-color for older runtimes.
        format!(
            ":root {{ --window-bg-color: {bg}; }}\n\
             @define-color window_bg_color {bg};"
        )
    } else {
        String::new()
    };
    WA_BG_PROVIDER.with(|provider| provider.load_from_string(&css));
}

/// A page reported its header background (a computed CSS colour). GTK main
/// thread only.
pub(crate) fn set_page_background(computed: &str) {
    let Some(hex) = opaque_css_rgb_to_hex(computed) else {
        return;
    };
    let changed = PAGE_BG.with_borrow_mut(|current| {
        let changed = current.as_deref() != Some(hex.as_str());
        *current = Some(hex);
        changed
    });
    if changed {
        apply_wa_bg();
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
    reapply_web_color_scheme_everywhere();
}

/// Re-push the effective color scheme into every window's web content.
fn reapply_web_color_scheme_everywhere() {
    if let Some(app) = gio::Application::default().and_downcast::<gtk::Application>() {
        for win in app.windows() {
            if let Some(kw) = win.downcast_ref::<KarereWindow>() {
                kw.reapply_web_color_scheme();
            }
        }
    }
}

/// The active window, or any window when none is active (a hidden window is
/// not "active"). (#175)
pub(crate) fn any_window(app: &impl IsA<gtk::Application>) -> Option<gtk::Window> {
    app.active_window()
        .or_else(|| app.windows().into_iter().next())
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
        ("app.do-not-disturb", &["<Primary><Shift>n"]),
    ];
    for (action, accels) in bindings {
        gtk_app.set_accels_for_action(action, accels);
    }
}

mod imp {
    use std::cell::OnceCell;
    use std::rc::Rc;

    use super::*;
    use libadwaita::subclass::prelude::*;

    #[derive(Default)]
    pub struct KarereApplication {
        pub settings: OnceCell<gio::Settings>,
        // Keeps the live user-stylesheet reloader alive for the app's lifetime;
        // drops (and cleans up) with the application object.
        user_css: OnceCell<Rc<crate::gtk_css::UserCss>>,
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

            // Live-reload the user's GTK stylesheet (`gtk.css` in the user
            // config directory) so Noctalia-style theme changes reach the
            // native chrome immediately, without restarting. No-op (with a
            // warning) when the directory cannot be monitored.
            if let Some(user_css) = crate::gtk_css::UserCss::start() {
                let _ = self.user_css.set(user_css);
            }

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

            // Optional tint of the GTK chrome; see `apply_wa_bg`. (#168)
            {
                if let Some(display) = gtk::gdk::Display::default() {
                    WA_BG_PROVIDER.with(|provider| {
                        gtk::style_context_add_provider_for_display(
                            &display,
                            provider,
                            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                        );
                    });
                }
                apply_wa_bg();
                adw::StyleManager::default().connect_dark_notify(|_| apply_wa_bg());
                // GSettings signals die with the object — keep this one alive
                // for the app's lifetime or the toggle only applies on restart.
                let settings_wa = gio::Settings::new(APP_ID);
                settings_wa
                    .connect_changed(Some("match-whatsapp-colors"), |_, _| apply_wa_bg());
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
                reapply_web_color_scheme_everywhere();
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
            // Existing autostart entries predate `--background`; re-register once
            // so they gain it (the portal updates the entry in place).
            if settings.boolean("run-on-startup") && !settings.boolean("autostart-migrated") {
                app.activate_action("sync-autostart", None);
            }
            let _ = settings.set_boolean("autostart-migrated", true);
            // Keep Settings alive so the change handlers keep firing.
            let _ = self.settings.set(settings);
        }
    }

    impl GtkApplicationImpl for KarereApplication {}
    impl AdwApplicationImpl for KarereApplication {}
}

#[cfg(test)]
mod tests {
    use super::opaque_css_rgb_to_hex;

    #[test]
    fn only_opaque_computed_rgb_becomes_a_hex_colour() {
        assert_eq!(
            opaque_css_rgb_to_hex("rgb(46, 52, 64)").as_deref(),
            Some("#2e3440")
        );
        assert_eq!(
            opaque_css_rgb_to_hex(" rgb(0,0,255) ").as_deref(),
            Some("#0000ff")
        );
        for rejected in [
            "rgba(46, 52, 64, 0.5)",
            "rgb(46, 52)",
            "rgb(46, 52, 64, 1)",
            "rgb(256, 0, 0)",
            "#2e3440",
            "red; } * { color: red",
            "rgb(1, 2, 3); } window { background: red",
            "",
        ] {
            assert_eq!(opaque_css_rgb_to_hex(rejected), None, "{rejected}");
        }
    }
}
