use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use libadwaita as adw;

use crate::web_view::KarereWebView;

glib::wrapper! {
    pub struct KarereWindow(ObjectSubclass<imp::KarereWindow>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager, gio::ActionGroup, gio::ActionMap;
}

impl KarereWindow {
    pub fn new(app: &adw::Application, url: &str) -> Self {
        let _ = KarereWebView::static_type();
        let obj: Self = glib::Object::builder().property("application", app).build();
        obj.imp().init_web_view(url);
        obj
    }

    /// Force a real quit, bypassing the `close-button-action=background` branch.
    /// Runs the teardown directly instead of via `close()`: GTK's `close()` is a
    /// silent no-op on a hidden/never-realized window, which left the tray's
    /// "Quit" dead while running in the background (#175).
    pub fn quit_now(&self) {
        self.imp().force_quit.set(true);
        self.imp().quit_teardown();
    }

    /// Switch the active account (driven by the tray's per-account entries).
    pub fn switch_account(&self, id: &str) {
        self.imp().switch_account(id);
    }

    /// Re-apply notification-sound muting on a live settings change.
    pub fn apply_audio_mute(&self) {
        if let Some(web) = self.imp().web_view.borrow().as_ref() {
            web.apply_audio_mute();
        }
    }

    /// Reload every account's browser so each re-evaluates the layout gate on
    /// its next `on_load_end` (live `mobile-layout` change).
    pub fn reload_all_accounts(&self) {
        if let Some(web) = self.imp().web_view.borrow().as_ref() {
            web.reload_all();
        }
    }

    /// Re-emulate the web content color scheme on this window's web view after a
    /// theme change. (#160)
    pub fn reapply_web_color_scheme(&self) {
        if let Some(web) = self.imp().web_view.borrow().as_ref() {
            web.reapply_color_scheme();
        }
    }

    /// Pre-warm all account browsers so WhatsApp loads and notifications work
    /// before the window is ever shown.
    pub fn prewarm(&self) {
        if let Some(web) = self.imp().web_view.borrow().as_ref() {
            web.prewarm();
        }
    }

    /// Load `url` in the active account's browser.
    pub fn load_url(&self, url: &str) {
        if let Some(web) = self.imp().web_view.borrow().as_ref() {
            web.load_url(url);
        }
    }

    /// Show a plain toast in this window.
    pub fn show_toast(&self, text: &str) {
        self.imp().toast_overlay.add_toast(adw::Toast::new(text));
    }

    /// Run `script` in `account_id`'s browser, foreground or not (notification
    /// click routing for a specific account).
    pub fn run_account_js(&self, account_id: &str, script: &str) {
        if let Some(web) = self.imp().web_view.borrow().as_ref() {
            web.run_js_in_account(account_id, script);
        }
    }
}

mod imp {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::sync::Arc;

    use parking_lot::Mutex;

    use adw::subclass::prelude::*;
    use glib::clone;
    use gtk::gio;
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{CompositeTemplate, TemplateChild};
    use libadwaita as adw;
    use libadwaita::prelude::*;

    use crate::application::{APP_ID, PROFILE};
    use crate::handlers::{CrashDialog, DownloadCompleted, DownloadFailed};
    use crate::web_view::KarereWebView;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/io/github/tobagin/karere/ui/window.ui")]
    pub struct KarereWindow {
        #[template_child]
        pub header_bar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub view_container: TemplateChild<gtk::Box>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub split: TemplateChild<gtk::Paned>,
        #[template_child]
        pub devtools_container: TemplateChild<gtk::Box>,
        #[template_child]
        pub search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub find_prev_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub find_next_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub find_counter_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub dictionary_dropdown: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub account_bottom_sheet: TemplateChild<adw::BottomSheet>,
        #[template_child]
        pub account_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub account_avatar: TemplateChild<adw::Avatar>,
        #[template_child]
        pub accounts_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub zoom_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub zoom_label: TemplateChild<gtk::Label>,

        /// Wide-variant switcher popover, parented to `account_button`; its list
        /// mirrors `accounts_list`.
        pub account_popover: RefCell<Option<gtk::Popover>>,
        pub accounts_list_popover: RefCell<Option<gtk::ListBox>>,
        pub web_view: RefCell<Option<KarereWebView>>,
        /// Embedded DevTools OSR view, present only while DevTools is open.
        pub devtools_view: RefCell<Option<KarereWebView>>,
        /// Most recent query, reused by Next/Prev so Chromium cycles the match set.
        pub last_query: RefCell<String>,
        pub closing: std::cell::Cell<bool>,
        pub force_quit: std::cell::Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for KarereWindow {
        const NAME: &'static str = "KarereWindow";
        type Type = super::KarereWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            KarereWebView::ensure_type();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for KarereWindow {
        fn dispose(&self) {
            // The popover is parented to the header button by hand.
            if let Some(popover) = self.account_popover.take() {
                popover.unparent();
            }
        }

        fn constructed(&self) {
            self.parent_constructed();
            let window = self.obj();

            // Development builds get the striped Adwaita "devel" headerbar.
            if PROFILE == "development" {
                window.add_css_class("devel");
            }

            let settings = gio::Settings::new(APP_ID);
            settings
                .bind("window-width", &*window, "default-width")
                .flags(gio::SettingsBindFlags::DEFAULT)
                .build();
            settings
                .bind("window-height", &*window, "default-height")
                .flags(gio::SettingsBindFlags::DEFAULT)
                .build();
            settings
                .bind("is-maximized", &*window, "maximized")
                .flags(gio::SettingsBindFlags::DEFAULT)
                .build();

            // Headerbar zoom-box visibility follows the opt-in setting.
            settings
                .bind("zoom-controls-headerbar", &*self.zoom_box, "visible")
                .flags(gio::SettingsBindFlags::GET)
                .build();
            // Seed the label from the active account's persisted zoom (floor-lifted).
            self.update_zoom_label(Self::load_zoom_for_active_account().max(Self::zoom_floor()));

            // Reduce-motion: drive process-wide `gtk-enable-animations`
            // (inverted) from the GSetting, with live propagation. App-global by
            // design.
            apply_reduce_motion(&settings);
            settings.connect_changed(Some("reduce-motion"), |s, _key| {
                apply_reduce_motion(s);
            });

            // Enhanced focus indicators: toggle the `enhanced-focus` CSS class on
            // the root window from the GSetting; the stylesheet scopes its rules
            // under that class.
            apply_focus_indicators(&settings, &window);
            settings.connect_changed(
                Some("focus-indicators"),
                clone!(
                    #[weak]
                    window,
                    move |s, _key| apply_focus_indicators(s, &window)
                ),
            );

            let settings_for_close = settings.clone();
            window.connect_close_request(clone!(
                #[weak(rename_to = this)]
                self,
                #[upgrade_or]
                glib::Propagation::Stop,
                move |win| {
                    let action = settings_for_close.string("close-button-action");
                    if action.as_str() == "background" && !this.force_quit.get() {
                        win.set_visible(false);
                        return glib::Propagation::Stop;
                    }

                    // "quit"/unknown/forced: destroy after the CEF-close gate.
                    this.quit_teardown();
                    glib::Propagation::Stop
                }
            ));

            self.register_win_actions();
            self.setup_search();
            self.setup_focus_withdraw();
            self.setup_spellcheck();
            self.setup_account_switcher();
            self.setup_fullscreen_headerbar();

            // Keep the tray Show/Hide label in sync with visibility, and drive
            // notification-sound muting (a hidden window mutes the foreground
            // browser's ding when sounds are off).
            window.connect_visible_notify(clone!(
                #[weak(rename_to = this)]
                self,
                move |win| {
                    // First, so CEF work requested by the re-show below is
                    // already under the normal backstop.
                    crate::cef_pump::set_idle_mode(!win.is_visible());
                    crate::tray::set_window_visible(win.is_visible());
                    if let Some(web) = this.web_view.borrow().as_ref() {
                        web.set_window_visible(win.is_visible());
                    }
                    // Re-assert the theme when the window is first shown: a
                    // start-in-background window is built (light, after CEF reset
                    // the color scheme) and presented later, so apply the dark
                    // selection at present time. (#160)
                    if win.is_visible() {
                        crate::application::apply_theme(&gio::Settings::new(
                            crate::application::APP_ID,
                        ));
                    }
                }
            ));
            crate::tray::set_window_visible(window.is_visible());
            crate::cef_pump::set_idle_mode(!window.is_visible());
        }
    }

    impl WidgetImpl for KarereWindow {}
    impl WindowImpl for KarereWindow {}
    impl ApplicationWindowImpl for KarereWindow {}

    /// Hard cap on linked accounts (matches the Alt+1..9 switch shortcuts).
    const MAX_ACCOUNTS: usize = 9;

    /// Primary display name for an account row / avatar / tray: the discovered
    /// WhatsApp name (pushname) first, then the user label, then a placeholder.
    fn row_title(account: &crate::accounts::Account) -> String {
        account
            .pushname
            .clone()
            .or_else(|| crate::accounts::display_label(account))
            .unwrap_or_else(|| gettextrs::gettext("Account"))
    }

    /// Decode account PNG bytes into a `gdk::Texture` for `Adw.Avatar`'s
    /// custom-image. Pixbuf loader avoids relying on `Texture::from_bytes`.
    fn texture_from_png(bytes: &[u8]) -> Option<gtk::gdk::Texture> {
        let loader = gtk::gdk_pixbuf::PixbufLoader::new();
        loader.write(bytes).ok()?;
        loader.close().ok()?;
        let pixbuf = loader.pixbuf()?;
        Some(gtk::gdk::Texture::for_pixbuf(&pixbuf))
    }
    impl AdwApplicationWindowImpl for KarereWindow {}

    impl KarereWindow {
        pub fn init_web_view(&self, url: &str) {
            let web = KarereWebView::new();
            web.set_hexpand(true);
            web.set_vexpand(true);

            // Overlay an offline status page over the GLArea so a failed load
            // shows a message, not a blank GLArea. The `.background` box paints an
            // opaque fill that fully covers the view underneath.
            let overlay = gtk::Overlay::new();
            overlay.set_hexpand(true);
            overlay.set_vexpand(true);
            overlay.set_child(Some(&web));

            let offline = gtk::Box::new(gtk::Orientation::Vertical, 0);
            offline.add_css_class("background");
            offline.set_hexpand(true);
            offline.set_vexpand(true);
            offline.set_visible(false);
            offline.append(
                &adw::StatusPage::builder()
                    .icon_name("network-offline-symbolic")
                    .title(&gettextrs::gettext("No connection"))
                    .description(&gettextrs::gettext("Waiting for the network — retrying…"))
                    .vexpand(true)
                    .build(),
            );
            overlay.add_overlay(&offline);

            self.view_container.append(&overlay);
            web.load_url(url);
            self.start_state_poll(&web, &offline);
            *self.web_view.borrow_mut() = Some(web);
        }

        /// Surface the shared handler state on the GTK side: crash toasts,
        /// crash-storm dialog, find counter, downloads, JS fullscreen, offline
        /// overlay.
        ///
        /// Not polled. Every producer is a CEF callback, and those only run
        /// inside the pump's `do_message_loop_work`, so a `cef_pump` after-work
        /// hook spots new state and queues one drain on idle. A producer ever
        /// reached synchronously from GTK code is still seen on the pump's next
        /// backstop turn, so nothing is lost.
        ///
        /// The overlay tracks `GNetworkMonitor` as well as the load-error
        /// `offline` flag: WhatsApp's service worker serves a cached page when
        /// offline so a load error never fires — the network monitor is the only
        /// reliable offline signal. It is followed through `network-changed`.
        fn start_state_poll(&self, web: &KarereWebView, offline: &gtk::Box) {
            use gtk::gio::prelude::NetworkMonitorExt;

            let toast_overlay = self.toast_overlay.get();
            let web_weak = web.downgrade();
            let win_weak = self.obj().downgrade();
            let offline_weak = offline.downgrade();
            let counter_weak = self.find_counter_label.downgrade();
            let monitor = gtk::gio::NetworkMonitor::default();
            let dialog_open = Rc::new(Cell::new(false));
            let was_net_down = Rc::new(Cell::new(!monitor.is_network_available()));
            // Load-error flag as of the last drain, so a change alone wakes one.
            let last_offline = Rc::new(Cell::new(false));
            let drain_queued = Rc::new(Cell::new(false));
            let hook_web = web.downgrade();
            let net_web = web.downgrade();
            let drain_monitor = monitor.clone();
            let drain_last_offline = last_offline.clone();

            let drain: Rc<dyn Fn()> = Rc::new(move || {
                let Some(web) = web_weak.upgrade() else {
                    return;
                };
                let shared = web.shared();
                let (toast, dialog, flag_offline, find, completed, failed, fullscreen) = {
                    let mut s = shared.lock();
                    (
                        s.crash_toast.take(),
                        s.crash_dialog_request.take(),
                        s.offline,
                        s.find_result.take(),
                        std::mem::take(&mut s.downloads_completed),
                        std::mem::take(&mut s.downloads_failed),
                        s.fullscreen_request.take(),
                    )
                };

                // JS-initiated fullscreen (e.g. WhatsApp video call) from the
                // display handler. Apply window state on the GTK main thread; the
                // headerbar follows the resulting `notify::fullscreened`.
                if let Some(on) = fullscreen
                    && let Some(win) = win_weak.upgrade()
                {
                    if on {
                        win.fullscreen();
                    } else {
                        win.unfullscreen();
                    }
                }

                if let Some(find) = find
                    && let Some(label) = counter_weak.upgrade()
                {
                    if find.count == 0 {
                        label.set_visible(false);
                    } else {
                        label.set_text(
                            &gettextrs::gettext("{active} of {count}")
                                .replace("{active}", &find.active.to_string())
                                .replace("{count}", &find.count.to_string()),
                        );
                        label.set_visible(true);
                    }
                }

                if let Some(text) = toast {
                    toast_overlay.add_toast(adw::Toast::new(&text));
                }
                if let Some(req) = dialog
                    && !dialog_open.get()
                    && let Some(win) = win_weak.upgrade()
                {
                    dialog_open.set(true);
                    show_crash_dialog(&win, req, dialog_open.clone());
                }

                if (!completed.is_empty() || !failed.is_empty())
                    && let Some(win) = win_weak.upgrade()
                {
                    // Honour the "Notification Type" setting: toast, system
                    // notification, or both.
                    let dl_type = gtk::gio::Settings::new(crate::application::APP_ID)
                        .string("notify-download-type");
                    for dl in completed {
                        match dl_type.as_str() {
                            "notification" => show_download_notification(&dl),
                            "both" => {
                                show_download_toast(&toast_overlay, &win, dl.clone());
                                show_download_notification(&dl);
                            }
                            // "toast" and any unexpected value
                            _ => show_download_toast(&toast_overlay, &win, dl),
                        }
                    }
                    for fail in failed {
                        show_download_failed(&win, fail);
                    }
                }

                let net_down = !drain_monitor.is_network_available();
                if let Some(page) = offline_weak.upgrade() {
                    page.set_visible(flag_offline || net_down);
                }
                drain_last_offline.set(flag_offline);
            });

            let schedule_drain: Rc<dyn Fn()> = Rc::new({
                let drain = drain.clone();
                let queued = drain_queued.clone();
                move || {
                    if queued.replace(true) {
                        return;
                    }
                    let drain = drain.clone();
                    let queued = queued.clone();
                    glib::idle_add_local_once(move || {
                        queued.set(false);
                        drain();
                    });
                }
            });

            let net_drain = drain.clone();
            monitor.connect_network_changed(move |monitor, _available| {
                let net_down = !monitor.is_network_available();
                // Network just came back: reload so the cached page reconnects.
                if was_net_down.get()
                    && !net_down
                    && let Some(web) = net_web.upgrade()
                {
                    web.reload();
                }
                was_net_down.set(net_down);
                net_drain();
            });

            let hook_drain = schedule_drain.clone();
            crate::cef_pump::on_after_work(move || {
                let Some(web) = hook_web.upgrade() else {
                    return false;
                };
                if drain_queued.get() {
                    return true;
                }
                let shared = web.shared();
                let pending = {
                    let s = shared.lock();
                    s.crash_toast.is_some()
                        || s.crash_dialog_request.is_some()
                        || s.find_result.is_some()
                        || !s.downloads_completed.is_empty()
                        || !s.downloads_failed.is_empty()
                        || s.fullscreen_request.is_some()
                        || s.offline != last_offline.get()
                };
                if pending {
                    hook_drain();
                }
                true
            });
            // First drain: show the overlay when starting without a network.
            schedule_drain();
        }

        /// Hide the headerbar while fullscreen, restore on exit. Tied to
        /// `notify::fullscreened` (not the request) so every exit path — JS
        /// `exitFullscreen()`, Esc, F11, WM toggles — converges. `set_visible` is
        /// idempotent, so it never fights the poll-loop drain.
        fn setup_fullscreen_headerbar(&self) {
            let header_bar = self.header_bar.get();
            self.obj().connect_fullscreened_notify(move |win| {
                header_bar.set_visible(!win.is_fullscreen());
            });
        }

        /// Withdraw live notification banners when the window regains focus.
        /// A short debounce coalesces rapid focus toggles so we don't stampede
        /// `execute_java_script` on every flicker.
        fn setup_focus_withdraw(&self) {
            let window = self.obj();
            let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

            window.connect_is_active_notify(move |win| {
                if !win.is_active() {
                    return;
                }
                // Unread counts are not touched here: they follow the page
                // title, which WhatsApp updates once chats are actually read.
                // Restart the debounce timer on each activation edge.
                if let Some(id) = pending.borrow_mut().take() {
                    id.remove();
                }
                let win_weak = win.downgrade();
                let pending_inner = pending.clone();
                let id = glib::timeout_add_local_once(
                    std::time::Duration::from_millis(150),
                    move || {
                        pending_inner.borrow_mut().take();
                        let Some(win) = win_weak.upgrade() else {
                            return;
                        };
                        let web = win.imp().web_view.borrow().clone();
                        // Only the account in front of the user: background
                        // accounts keep their banners.
                        if let Some(web) = web
                            && let Some(active) = crate::accounts::manager().active()
                        {
                            crate::notifications::tracker().on_account_viewed(&active.id, |js| {
                                web.run_js_in_account(&active.id, js)
                            });
                        }
                    },
                );
                *pending.borrow_mut() = Some(id);
            });
        }

        // ---- account switcher ----------------------------------------------

        pub fn setup_account_switcher(&self) {
            // Instantiate the wide-variant popover and parent it to the header
            // account button.
            let builder =
                gtk::Builder::from_resource("/io/github/tobagin/karere/ui/account_switcher.ui");
            if let Some(popover) = builder.object::<gtk::Popover>("account_switcher_popover") {
                popover.set_parent(&*self.account_button);
                *self.account_popover.borrow_mut() = Some(popover);
            }
            *self.accounts_list_popover.borrow_mut() =
                builder.object::<gtk::ListBox>("accounts_list_popover");

            // Wide windows: popover; narrow: bottom sheet.
            self.account_button.connect_clicked(clone!(
                #[weak(rename_to = this)]
                self,
                move |btn| {
                    if btn.width().max(this.obj().width()) >= 600
                        && let Some(pop) = this.account_popover.borrow().as_ref()
                    {
                        pop.popup();
                    } else {
                        this.account_bottom_sheet.set_open(true);
                    }
                }
            ));

            // Rebuild rows on every account-list / runtime-state change.
            let mgr = crate::accounts::manager();
            mgr.connect_local(
                "accounts-changed",
                false,
                clone!(
                    #[weak(rename_to = this)]
                    self,
                    #[upgrade_or]
                    None,
                    move |_| {
                        this.rebuild_account_rows();
                        None
                    }
                ),
            );
            self.rebuild_account_rows();
        }

        fn rebuild_account_rows(&self) {
            let mgr = crate::accounts::manager();
            let accounts = mgr.get_accounts_sorted();

            // Disable "Add account" at the hard cap.
            if let Some(add) = self
                .obj()
                .lookup_action("add-account")
                .and_downcast::<gio::SimpleAction>()
            {
                add.set_enabled(accounts.len() < MAX_ACCOUNTS);
            }

            // Populate both the bottom-sheet list and the popover list.
            let mut lists: Vec<gtk::ListBox> = vec![self.accounts_list.get()];
            if let Some(pop_list) = self.accounts_list_popover.borrow().as_ref() {
                lists.push(pop_list.clone());
            }
            for list in &lists {
                while let Some(child) = list.first_child() {
                    list.remove(&child);
                }
                let can_remove = accounts.len() > 1;
                for account in &accounts {
                    list.append(&self.make_account_row(account, can_remove));
                }
            }

            // Mirror the account list into the tray: name, unread count, active
            // mark, avatar pixmap; plus the unmuted unread total.
            let summaries = accounts
                .iter()
                .map(|a| crate::tray::AccountSummary {
                    id: a.id.clone(),
                    name: row_title(a),
                    unread: crate::accounts::runtime_state(&a.id).unread,
                    active: a.is_active,
                    icon_png: a.avatar_png.clone(),
                })
                .collect();
            crate::tray::set_accounts(summaries);
            crate::tray::sync_unread();

            // Sync the header avatar to the active (or MRU-first) account.
            let active = mgr.active().or_else(|| mgr.mru_first());
            if let Some(a) = active {
                let label = row_title(&a);
                self.account_avatar.set_text(Some(&label));
                match a.avatar_png.as_deref().and_then(texture_from_png) {
                    Some(tex) => self.account_avatar.set_custom_image(Some(&tex)),
                    None => self
                        .account_avatar
                        .set_custom_image(None::<&gtk::gdk::Texture>),
                }
            }
        }

        fn make_account_row(
            &self,
            account: &crate::accounts::Account,
            can_remove: bool,
        ) -> adw::ActionRow {
            let runtime = crate::accounts::runtime_state(&account.id);
            let title = row_title(account);
            let id = account.id.clone();

            let row = adw::ActionRow::builder()
                .title(glib::markup_escape_text(&title))
                .activatable(true)
                .build();
            // Title = pushname; subtitle = user label. Before pairing (no
            // pushname yet) show the waiting state instead.
            if runtime.awaiting_pairing && account.pushname.is_none() {
                row.set_subtitle(&gettextrs::gettext("Waiting for QR scan…"));
            } else if let Some(label) = crate::accounts::display_label(account) {
                row.set_subtitle(&glib::markup_escape_text(&label));
            }

            // Prefix: avatar (custom image or initials).
            let avatar = adw::Avatar::new(36, Some(&title), true);
            if let Some(tex) = account.avatar_png.as_deref().and_then(texture_from_png) {
                avatar.set_custom_image(Some(&tex));
            }
            row.add_prefix(&avatar);

            // Awaiting-pairing spinner.
            if runtime.awaiting_pairing {
                let spinner = gtk::Spinner::new();
                spinner.set_spinning(true);
                spinner.set_valign(gtk::Align::Center);
                row.add_suffix(&spinner);
            }

            // Unread badge: the page title's count of unread chats ("99+" cap).
            // Muted accounts keep their badge; only the tray total skips them.
            if runtime.unread > 0 {
                let text = gettextrs::ngettext(
                    "{} unread chat",
                    "{} unread chats",
                    runtime.unread,
                )
                .replace("{}", &runtime.unread.to_string());
                let badge = gtk::Label::new(Some(&crate::accounts::unread_badge_text(
                    runtime.unread,
                )));
                badge.add_css_class("unread-badge");
                badge.set_valign(gtk::Align::Center);
                badge.set_tooltip_text(Some(&text));
                badge.update_property(&[gtk::accessible::Property::Label(&text)]);
                row.add_suffix(&badge);
            }

            // Per-account notification mute toggle. Icons are the app's shipped
            // symbolic pair (app-id-prefixed so Flatpak exports them).
            fn apply_mute_visual(btn: &gtk::ToggleButton) {
                let app_id = crate::application::APP_ID;
                if btn.is_active() {
                    btn.set_icon_name(&format!("{app_id}-do-not-disturb-symbolic"));
                    btn.set_tooltip_text(Some(&gettextrs::gettext(
                        "Notifications muted — click to unmute",
                    )));
                } else {
                    btn.set_icon_name(&format!("{app_id}-notification-symbolic"));
                    btn.set_tooltip_text(Some(&gettextrs::gettext("Mute notifications")));
                }
            }
            let mute = gtk::ToggleButton::new();
            mute.add_css_class("flat");
            mute.set_valign(gtk::Align::Center);
            // set_active before connect: programmatic state must not fire toggled.
            mute.set_active(account.muted);
            apply_mute_visual(&mute);
            mute.connect_toggled(clone!(
                #[weak(rename_to = this)]
                self,
                #[strong]
                id,
                move |btn| {
                    crate::accounts::manager().set_muted(&id, btn.is_active());
                    apply_mute_visual(btn);
                    // Muted accounts drop out of the tray total.
                    crate::tray::sync_unread();
                    // Re-push the ding-mute flag now so it takes effect live, not
                    // just on the next page load. Each browser re-evaluates its
                    // own account's mute state.
                    if let Some(web) = this.web_view.borrow().as_ref() {
                        web.apply_audio_mute();
                    }
                }
            ));
            row.add_suffix(&mute);

            // Edit + remove affordances (no reorder controls — creation order).
            let edit = gtk::Button::from_icon_name("document-edit-symbolic");
            edit.add_css_class("flat");
            edit.set_valign(gtk::Align::Center);
            edit.set_tooltip_text(Some(&gettextrs::gettext("Edit account")));
            edit.connect_clicked(clone!(
                #[weak(rename_to = this)]
                self,
                #[strong]
                id,
                move |_| this.open_account_dialog(&id)
            ));
            row.add_suffix(&edit);

            let remove = gtk::Button::from_icon_name("user-trash-symbolic");
            remove.add_css_class("flat");
            remove.set_valign(gtk::Align::Center);
            // The last remaining account cannot be removed.
            remove.set_sensitive(can_remove);
            remove.set_tooltip_text(Some(&if can_remove {
                gettextrs::gettext("Remove account")
            } else {
                gettextrs::gettext("Can't remove the only account")
            }));
            remove.connect_clicked(clone!(
                #[weak(rename_to = this)]
                self,
                #[strong]
                id,
                move |_| this.confirm_remove_account(&id)
            ));
            row.add_suffix(&remove);

            // Row activation switches to the account.
            row.connect_activated(clone!(
                #[weak(rename_to = this)]
                self,
                #[strong]
                id,
                move |_| this.switch_account(&id)
            ));
            row
        }

        /// Close both switcher surfaces (popover + bottom sheet). Done before
        /// presenting a dialog so the autohide popover's input grab does not
        /// swallow the dialog's events.
        fn close_switcher(&self) {
            if let Some(pop) = self.account_popover.borrow().as_ref() {
                pop.popdown();
            }
            self.account_bottom_sheet.set_open(false);
        }

        /// Switch to the account at `idx` in stable list order (no-op if out of range).
        fn switch_to_index(&self, idx: usize) {
            if let Some(a) = crate::accounts::manager().get_accounts_sorted().get(idx) {
                self.switch_account(&a.id);
            }
        }

        /// Cycle the active account by `delta` (+1 next, -1 previous) in stable
        /// list order — the same order the switcher shows (#166).
        fn cycle_account(&self, delta: i32) {
            let mgr = crate::accounts::manager();
            let accts = mgr.get_accounts_sorted();
            if accts.len() < 2 {
                return;
            }
            let cur = mgr.active().map(|a| a.id);
            let here = cur
                .and_then(|id| accts.iter().position(|a| a.id == id))
                .unwrap_or(0) as i32;
            let n = accts.len() as i32;
            let next = ((here + delta) % n + n) % n;
            self.switch_account(&accts[next as usize].id);
        }

        /// Make `id` the active account and bring its browser to the foreground.
        pub fn switch_account(&self, id: &str) {
            log::info!("switch_account({id})");
            self.close_switcher();
            crate::accounts::manager().activate(id);
            if let Some(web) = self.web_view.borrow().as_ref() {
                web.spawn_account(id, true);
                // Reflect the new account's zoom. A loaded browser retains its CEF
                // zoom across hide/show; a fresh one re-applies on its first
                // `on_load_end`. Either way the headerbar label must track it.
                let z = Self::load_zoom_for_active_account().max(Self::zoom_floor());
                web.set_zoom_linear(z);
                self.update_zoom_label(z);
                // Viewing an account in a focused window withdraws its banners
                // (an unfocused window is handled by the focus-gained path).
                if self.obj().is_active() {
                    crate::notifications::tracker()
                        .on_account_viewed(id, |js| web.run_js_in_account(id, js));
                }
            }
        }

        /// `win.add-account`: create an account with a default label, give it the
        /// foreground browser so its QR shows immediately, close the switcher. No
        /// upfront prompt — the real name fills in from the Store hook on pairing.
        fn add_account(&self) {
            let mgr = crate::accounts::manager();
            let count = mgr.get_accounts_sorted().len();
            if count >= MAX_ACCOUNTS {
                self.close_switcher();
                self.toast_overlay
                    .add_toast(adw::Toast::new(&gettextrs::gettext(
                        "Maximum of 9 accounts reached",
                    )));
                return;
            }
            // One save + one switcher rebuild; label is the lowest unused
            // "Account N" so removals never lead to duplicates.
            let account = mgr.add_active_with_default_label();
            if let Some(web) = self.web_view.borrow().as_ref() {
                web.spawn_account(&account.id, true);
            }
            self.close_switcher();
        }

        /// Add/edit dialog: only `user_label` is editable; identity fields are
        /// shown read-only.
        fn open_account_dialog(&self, id: &str) {
            let mgr = crate::accounts::manager();
            let Some(account) = mgr.get(id) else { return };
            let id = account.id.clone();

            let page = adw::PreferencesPage::new();
            let group = adw::PreferencesGroup::new();

            let shown_label = crate::accounts::display_label(&account).unwrap_or_default();
            let stored_label = account.user_label.clone();
            let label_row = adw::EntryRow::builder()
                .title(gettextrs::gettext("Label"))
                .text(shown_label.as_str())
                .build();
            group.add(&label_row);

            let dim = |title: &str, value: Option<&str>| -> adw::ActionRow {
                let r = adw::ActionRow::builder()
                    .title(title)
                    .subtitle(value.unwrap_or("—"))
                    .build();
                r.set_sensitive(false); // read-only
                r.add_css_class("property");
                r
            };
            group.add(&dim(
                &gettextrs::gettext("Name"),
                account.pushname.as_deref(),
            ));
            group.add(&dim(
                &gettextrs::gettext("WhatsApp ID"),
                account.wid.as_deref(),
            ));
            page.add(&group);

            let order_row = adw::ActionRow::builder()
                .title(gettextrs::gettext("Position"))
                .build();
            let up = gtk::Button::with_label(&gettextrs::gettext("Move Up"));
            let down = gtk::Button::with_label(&gettextrs::gettext("Move Down"));
            for b in [&up, &down] {
                b.set_valign(gtk::Align::Center);
                order_row.add_suffix(b);
            }
            let sync_order = clone!(
                #[weak]
                up,
                #[weak]
                down,
                #[strong]
                id,
                move || {
                    let list = crate::accounts::manager().get_accounts_sorted();
                    let idx = list.iter().position(|a| a.id == id);
                    up.set_sensitive(idx.is_some_and(|i| i > 0));
                    down.set_sensitive(idx.is_some_and(|i| i + 1 < list.len()));
                }
            );
            sync_order();
            up.connect_clicked(clone!(
                #[strong]
                id,
                #[strong]
                sync_order,
                move |_| {
                    crate::accounts::manager().move_up(&id);
                    sync_order();
                }
            ));
            down.connect_clicked(clone!(
                #[strong]
                id,
                #[strong]
                sync_order,
                move |_| {
                    crate::accounts::manager().move_down(&id);
                    sync_order();
                }
            ));
            group.add(&order_row);

            let toolbar = adw::ToolbarView::new();
            let header = adw::HeaderBar::new();
            header.set_show_end_title_buttons(false);
            header.set_show_start_title_buttons(false);
            let cancel = gtk::Button::with_label(&gettextrs::gettext("Cancel"));
            let save = gtk::Button::with_label(&gettextrs::gettext("Save"));
            save.add_css_class("suggested-action");
            header.pack_start(&cancel);
            header.pack_end(&save);
            toolbar.add_top_bar(&header);
            toolbar.set_content(Some(&page));

            let dialog = adw::Dialog::builder()
                .title(gettextrs::gettext("Edit Account"))
                .content_width(420)
                .child(&toolbar)
                .build();

            cancel.connect_clicked(clone!(
                #[weak]
                dialog,
                move |_| {
                    dialog.close();
                }
            ));
            save.connect_clicked(clone!(
                #[weak]
                dialog,
                #[strong]
                id,
                move |_| {
                    let text = label_row.text();
                    // An untouched translated default keeps its stored form.
                    let label = if text.as_str() == shown_label {
                        stored_label.clone()
                    } else {
                        (!text.trim().is_empty()).then(|| text.to_string())
                    };
                    crate::accounts::manager().update_user_label(&id, label);
                    dialog.close();
                }
            ));

            dialog.present(Some(&*self.obj()));
        }

        fn confirm_remove_account(&self, id: &str) {
            log::info!("confirm_remove_account({id})");
            let mgr = crate::accounts::manager();
            if mgr.get_accounts_sorted().len() <= 1 {
                log::info!("refusing to remove the only account");
                return;
            }
            self.close_switcher();
            let name = mgr.get(id).map(|a| row_title(&a)).unwrap_or_default();
            let dialog = adw::AlertDialog::new(
                Some(&gettextrs::gettext("Remove account?")),
                Some(
                    &gettextrs::gettext(
                        "This removes the local session for {name}. To fully unlink the device, open WhatsApp on your phone, go to Linked devices, select this device and choose Log out.",
                    )
                    .replace("{name}", &name),
                ),
            );
            dialog.add_response("cancel", &gettextrs::gettext("Cancel"));
            dialog.add_response("remove", &gettextrs::gettext("Remove"));
            dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");
            dialog.connect_response(
                None,
                clone!(
                    #[weak(rename_to = this)]
                    self,
                    #[strong(rename_to = id)]
                    id.to_owned(),
                    move |_, resp| {
                        if resp != "remove" {
                            return;
                        }
                        let mgr = crate::accounts::manager();
                        let was_active = mgr.active().map(|a| a.id) == Some(id.clone());
                        if let Some(web) = this.web_view.borrow().as_ref() {
                            web.close_account(&id);
                        }
                        mgr.remove(&id);
                        // Wipe the on-disk session after a delay (let CEF release
                        // the profile first).
                        let id_del = id.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(1500),
                            move || crate::accounts::delete_session_dir(&id_del),
                        );
                        // Removing the foreground account leaves no visible
                        // browser; promote the MRU-first survivor.
                        if was_active && let Some(next) = mgr.mru_first() {
                            this.switch_account(&next.id);
                        }
                    }
                ),
            );
            dialog.present(Some(&*self.obj()));
        }

        fn register_win_actions(&self) {
            let window = self.obj();

            let toggle_fullscreen = gio::SimpleAction::new("toggle-fullscreen", None);
            toggle_fullscreen.connect_activate(clone!(
                #[weak]
                window,
                move |_, _| {
                    window.set_fullscreened(!window.is_fullscreen());
                }
            ));
            window.add_action(&toggle_fullscreen);

            let minimize = gio::SimpleAction::new("minimize", None);
            minimize.connect_activate(clone!(
                #[weak]
                window,
                move |_, _| {
                    window.minimize();
                }
            ));
            window.add_action(&minimize);

            let close = gio::SimpleAction::new("close", None);
            close.connect_activate(clone!(
                #[weak]
                window,
                move |_, _| {
                    window.close();
                }
            ));
            window.add_action(&close);

            let show_devtools = gio::SimpleAction::new("show-devtools", None);
            show_devtools.connect_activate(clone!(
                #[weak(rename_to = this)]
                self,
                move |_, _| this.toggle_devtools()
            ));
            window.add_action(&show_devtools);

            let inspect_element = gio::SimpleAction::new("inspect-element", None);
            inspect_element.connect_activate(clone!(
                #[weak(rename_to = this)]
                self,
                move |_, _| this.inspect_element()
            ));
            window.add_action(&inspect_element);

            let close_devtools = gio::SimpleAction::new("close-devtools", None);
            close_devtools.connect_activate(clone!(
                #[weak(rename_to = this)]
                self,
                move |_, _| this.close_devtools()
            ));
            window.add_action(&close_devtools);

            let find_in_page = gio::SimpleAction::new("find-in-page", None);
            find_in_page.connect_activate(clone!(
                #[weak(rename_to = this)]
                self,
                move |_, _| {
                    this.search_bar.set_search_mode(true);
                    this.search_entry.grab_focus();
                }
            ));
            window.add_action(&find_in_page);

            let refresh = gio::SimpleAction::new("refresh", None);
            refresh.connect_activate(clone!(
                #[weak(rename_to = this)]
                self,
                move |_, _| {
                    if let Some(web) = this.web_view.borrow().as_ref() {
                        web.reload();
                    }
                }
            ));
            window.add_action(&refresh);

            let refresh_hard = gio::SimpleAction::new("refresh-hard", None);
            refresh_hard.connect_activate(clone!(
                #[weak(rename_to = this)]
                self,
                move |_, _| {
                    if let Some(web) = this.web_view.borrow().as_ref() {
                        web.reload_hard();
                    }
                }
            ));
            window.add_action(&refresh_hard);

            let add_account = gio::SimpleAction::new("add-account", None);
            add_account.connect_activate(clone!(
                #[weak(rename_to = this)]
                self,
                move |_, _| this.add_account()
            ));
            window.add_action(&add_account);

            // Account-switch shortcuts: cycle next/prev + jump-to-Nth (stable order).
            let next_account = gio::SimpleAction::new("next-account", None);
            next_account.connect_activate(clone!(
                #[weak(rename_to = this)]
                self,
                move |_, _| this.cycle_account(1)
            ));
            window.add_action(&next_account);

            let prev_account = gio::SimpleAction::new("prev-account", None);
            prev_account.connect_activate(clone!(
                #[weak(rename_to = this)]
                self,
                move |_, _| this.cycle_account(-1)
            ));
            window.add_action(&prev_account);

            let switch_index =
                gio::SimpleAction::new("switch-account-index", Some(glib::VariantTy::INT32));
            switch_index.connect_activate(clone!(
                #[weak(rename_to = this)]
                self,
                move |_, param| {
                    let n = param.and_then(|v| v.get::<i32>()).unwrap_or(0);
                    if n >= 1 {
                        this.switch_to_index((n - 1) as usize);
                    }
                }
            ));
            window.add_action(&switch_index);

            // Zoom actions: step the active account's linear zoom, apply through
            // CEF (respecting the accessibility floor), persist, refresh label.
            for verb in ["zoom-in", "zoom-out", "zoom-reset"] {
                let action = gio::SimpleAction::new(verb, None);
                let verb_owned = verb.to_string();
                action.connect_activate(clone!(
                    #[weak(rename_to = this)]
                    self,
                    move |_, _| this.zoom_step(&verb_owned)
                ));
                window.add_action(&action);
            }

            // Ctrl+B: flip the header bar zoom controls (the headerbar box is
            // bound to this key).
            let toggle_zoom = gio::SimpleAction::new("toggle-zoom-controls", None);
            toggle_zoom.connect_activate(|_, _| {
                let s = gio::Settings::new(APP_ID);
                let _ = s.set_boolean(
                    "zoom-controls-headerbar",
                    !s.boolean("zoom-controls-headerbar"),
                );
            });
            window.add_action(&toggle_zoom);

            // The accessibility floor changed in Preferences: re-apply live.
            let zoom_settings = gio::Settings::new(APP_ID);
            for key in ["webview-zoom", "zoom-level"] {
                zoom_settings.connect_changed(
                    Some(key),
                    clone!(
                        #[weak(rename_to = this)]
                        self,
                        move |_, _| {
                            let cur = Self::load_zoom_for_active_account();
                            this.apply_and_persist_zoom(cur);
                        }
                    ),
                );
            }
            // The handlers die with the Settings object; keep it for the process.
            std::mem::forget(zoom_settings);
        }
    }

    impl KarereWindow {
        /// Effective accessibility floor (linear); delegates to
        /// `web_view::zoom_floor`.
        fn zoom_floor() -> f64 {
            crate::web_view::zoom_floor()
        }

        /// Persisted linear zoom for the active account, defaulting to `1.0`.
        fn load_zoom_for_active_account() -> f64 {
            crate::accounts::manager()
                .active()
                .map(|a| a.zoom_level)
                .unwrap_or(1.0)
                .clamp(crate::web_view::ZOOM_MIN, crate::web_view::ZOOM_MAX)
        }

        /// Persist `linear` as the active account's zoom. Does not emit
        /// `accounts-changed` (see `AccountManager::set_zoom`).
        fn persist_zoom(linear: f64) {
            if let Some(a) = crate::accounts::manager().active() {
                crate::accounts::manager().set_zoom(&a.id, linear);
            }
        }

        /// Apply `linear` (lifted to the floor) to the live browser, persist the
        /// effective value, and update the headerbar label.
        fn apply_and_persist_zoom(&self, linear: f64) {
            let effective = linear.clamp(Self::zoom_floor(), crate::web_view::ZOOM_MAX);
            if let Some(web) = self.web_view.borrow().as_ref() {
                web.set_zoom_linear(effective);
            }
            Self::persist_zoom(effective);
            self.update_zoom_label(effective);
        }

        /// Handle `win.zoom-in` / `win.zoom-out` / `win.zoom-reset`.
        fn zoom_step(&self, verb: &str) {
            let floor = Self::zoom_floor().max(crate::web_view::ZOOM_MIN);
            let cur = Self::load_zoom_for_active_account().max(floor);
            let target = match verb {
                "zoom-in" => cur * 1.1,
                // Clamp the step-down at the floor; never cross it.
                "zoom-out" => cur / 1.1,
                _ => 1.0,
            };
            // Stored value stays within [max(floor, ZOOM_MIN), ZOOM_MAX].
            self.apply_and_persist_zoom(target.clamp(floor, crate::web_view::ZOOM_MAX));
        }

        /// Refresh the headerbar `<int>%` label from a linear factor.
        fn update_zoom_label(&self, linear: f64) {
            self.zoom_label
                .set_label(&format!("{}%", (linear * 100.0).round() as i32));
        }
    }

    impl KarereWindow {
        /// F12 / Ctrl+Shift+I: open the embedded DevTools pane, or close it if
        /// already open.
        fn toggle_devtools(&self) {
            if self.devtools_view.borrow().is_some() {
                self.close_devtools();
            } else {
                self.open_devtools();
            }
        }

        /// Ctrl+Shift+C: open DevTools if closed; otherwise forward Ctrl+Shift+C
        /// into the DevTools view so its frontend toggles the element picker.
        /// (First press opens — the frontend loads async — second press inspects.)
        fn inspect_element(&self) {
            let open = self.devtools_view.borrow().clone();
            match open {
                Some(dv) => dv.dispatch_inspect_shortcut(),
                None => self.open_devtools(),
            }
        }

        /// Open embedded DevTools: dock a fresh OSR view in the bottom pane and
        /// load the CDP DevTools frontend for the active page into it.
        ///
        /// CEF can't render `ShowDevTools` windowless, so DevTools is the
        /// frontend page served by `--remote-debugging-port`, loaded as a URL.
        /// The target URL is resolved off the main thread.
        fn open_devtools(&self) {
            if self.web_view.borrow().is_none() {
                log::warn!("open_devtools: no web view");
                return;
            }
            // F12 needs the CDP debugging port + relaxed network gates, which ship
            // only with --debug (a website could otherwise DNS-rebind to the port
            // and drive the session). Notifications don't use the port.
            if !crate::cef_runtime::debug_enabled() {
                log::info!("open_devtools: disabled (relaunch with --debug to enable)");
                self.toast_overlay
                    .get()
                    .add_toast(adw::Toast::new(&gettextrs::gettext("DevTools available only with --debug")));
                return;
            }

            let dv = KarereWebView::new_devtools();
            dv.set_hexpand(true);
            dv.set_vexpand(true);
            self.devtools_container.append(&dv);
            self.devtools_container.set_visible(true);

            // Dock at ~60% so the page keeps the majority of the height.
            let height = self.obj().height();
            if height > 0 {
                self.split.set_position((height as f64 * 0.6) as i32);
            }

            *self.devtools_view.borrow_mut() = Some(dv.clone());

            // Resolve the frontend URL on a worker thread; a short poll loads it
            // once ready (or toasts + collapses the pane on failure).
            let slot: Arc<Mutex<Option<Result<String, String>>>> = Arc::new(Mutex::new(None));
            let slot_bg = slot.clone();
            std::thread::spawn(move || {
                let res = crate::devtools::fetch_frontend_url(crate::devtools::DEVTOOLS_PORT);
                *slot_bg.lock() = Some(res);
            });

            let dv_weak = dv.downgrade();
            let toast = self.toast_overlay.get();
            let win_weak = self.obj().downgrade();
            glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                let Some(res) = slot.lock().take() else {
                    return glib::ControlFlow::Continue;
                };
                match res {
                    Ok(url) => {
                        log::info!("devtools frontend: {url}");
                        if let Some(dv) = dv_weak.upgrade() {
                            dv.load_url(&url);
                        }
                    }
                    Err(e) => {
                        log::warn!("devtools frontend unavailable: {e}");
                        toast.add_toast(adw::Toast::new(&gettextrs::gettext("DevTools unavailable: {error}").replace("{error}", &e.to_string())));
                        if let Some(win) = win_weak.upgrade() {
                            win.imp().close_devtools();
                        }
                    }
                }
                glib::ControlFlow::Break
            });
        }

        /// Close DevTools + all browsers, then destroy the window once CEF
        /// confirms the close. Does not go through GTK `close()`/close-request,
        /// so it works on a hidden or never-realized window too (#175).
        pub(super) fn quit_teardown(&self) {
            if self.closing.get() {
                return; // teardown already in flight
            }
            self.closing.set(true);
            // Held so the wait below also covers the DevTools browser.
            let devtools = self.devtools_view.borrow().clone();
            self.close_devtools();
            let web_opt = self.web_view.borrow().clone();
            let Some(web) = web_opt else {
                self.obj().destroy();
                return;
            };
            web.shutdown_browsers();
            // Wait for every browser to report closed so CEF flushes cookies and
            // IndexedDB before shutdown; never hang quit on a stuck browser.
            const CLOSE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
            let started = std::time::Instant::now();
            let win_weak = self.obj().downgrade();
            let web_weak = web.downgrade();
            glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                let Some(win) = win_weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                let closed = web_weak.upgrade().is_none_or(|web| web.is_browser_closed())
                    && devtools.as_ref().is_none_or(|dv| dv.is_browser_closed());
                if closed {
                    win.destroy();
                    glib::ControlFlow::Break
                } else if started.elapsed() >= CLOSE_TIMEOUT {
                    log::warn!(
                        "quit: browsers did not report closed within {CLOSE_TIMEOUT:?}; tearing down anyway"
                    );
                    win.destroy();
                    glib::ControlFlow::Break
                } else {
                    glib::ControlFlow::Continue
                }
            });
        }

        /// Close the DevTools view and collapse the bottom pane.
        fn close_devtools(&self) {
            let Some(dv) = self.devtools_view.borrow_mut().take() else {
                return;
            };
            dv.close_browser();
            self.devtools_container.remove(&dv);
            self.devtools_container.set_visible(false);

            // Removing the second GLArea leaves the main view without a fresh
            // render; force a repaint so it doesn't stay black.
            if let Some(main) = self.web_view.borrow().as_ref() {
                let main = main.clone();
                glib::idle_add_local_once(move || main.queue_render());
            }
        }

        /// Wire the headerbar spellcheck-language dropdown: a sorted view of
        /// `KNOWN_LANGUAGES` with star-pin favorites. Selection persists
        /// `spell-checking-languages` and switches the live browser (no reload);
        /// toggling a star persists `favorite-spell-check-languages` and re-sorts.
        fn setup_spellcheck(&self) {
            use crate::spellcheck_ui::{self, SpellLang};

            let settings = gio::Settings::new(APP_ID);
            let dropdown = self.dictionary_dropdown.get();

            // Visible only when spellcheck is enabled AND the headerbar control is
            // opted in; track both keys.
            let spell_dropdown_visible = |s: &gio::Settings| {
                s.boolean("enable-spell-checking") && s.boolean("spellcheck-headerbar")
            };
            dropdown.set_visible(spell_dropdown_visible(&settings));
            for key in ["enable-spell-checking", "spellcheck-headerbar"] {
                settings.connect_changed(
                    Some(key),
                    clone!(
                        #[weak]
                        dropdown,
                        move |s, _| dropdown.set_visible(spell_dropdown_visible(s))
                    ),
                );
            }

            // Live auto-correct toggle: push the flag into the running page (the
            // load handler re-seeds it on every navigation).
            let win_ac = self.obj().downgrade();
            settings.connect_changed(Some("enable-auto-correct"), move |s, _| {
                let on = s.boolean("enable-auto-correct");
                if let Some(win) = win_ac.upgrade()
                    && let Some(web) = win.imp().web_view.borrow().as_ref()
                {
                    web.run_js(&format!("window.__karereAutoCorrect = {on};"));
                }
            });

            let binding = spellcheck_ui::SpellBinding::new(&settings);

            // Set both factories BEFORE assigning the model — a DropDown with a
            // model but no factory yet emits "factory or expression must be set"
            // once per item.
            let (button_factory, list_factory) = binding.factories();
            dropdown.set_factory(Some(&button_factory));
            dropdown.set_list_factory(Some(&list_factory));
            dropdown.set_model(Some(binding.model()));

            // Show the effective language (explicit, else auto-detected) and keep
            // it in step with changes made from Preferences. The load handler
            // applies the startup language on the first main-frame `on_load_end`
            // (setting it before the spellcheck service is ready is ignored).
            binding.sync(|i| dropdown.set_selected(i));
            let dd_weak = dropdown.downgrade();
            binding.watch(move |i| {
                if let Some(dd) = dd_weak.upgrade() {
                    dd.set_selected(i);
                }
            });

            let win = self.obj().downgrade();
            dropdown.connect_selected_item_notify(move |dd| {
                let Some(lang) = dd.selected_item().and_downcast::<SpellLang>() else {
                    return;
                };
                let Some(langs) = binding.selected(&lang) else {
                    binding.sync(|i| dd.set_selected(i));
                    return;
                };
                if let Some(win) = win.upgrade()
                    && let Some(web) = win.imp().web_view.borrow().as_ref()
                {
                    web.set_spellcheck_languages(&langs, true);
                }
            });
        }

        /// Wire the find-in-page search bar to Chromium's `BrowserHost::find`.
        fn setup_search(&self) {
            // Reveal/hide + Escape handled by GtkSearchBar.
            self.search_bar.connect_entry(&*self.search_entry);

            // Fresh search per keystroke (find_next=false restarts the match set).
            self.search_entry.connect_search_changed(clone!(
                #[weak(rename_to = this)]
                self,
                move |entry| {
                    let text = entry.text().to_string();
                    *this.last_query.borrow_mut() = text.clone();
                    let Some(web) = this.web_view.borrow().clone() else {
                        return;
                    };
                    if text.is_empty() {
                        web.stop_finding();
                        this.find_counter_label.set_visible(false);
                    } else {
                        web.find(&text, true, false);
                    }
                }
            ));

            // Next/Prev reuse the last query (find_next=true) so Chromium cycles.
            self.find_next_button.connect_clicked(clone!(
                #[weak(rename_to = this)]
                self,
                move |_| {
                    let query = this.last_query.borrow().clone();
                    if query.is_empty() {
                        return;
                    }
                    if let Some(web) = this.web_view.borrow().as_ref() {
                        web.find(&query, true, true);
                    }
                }
            ));
            self.find_prev_button.connect_clicked(clone!(
                #[weak(rename_to = this)]
                self,
                move |_| {
                    let query = this.last_query.borrow().clone();
                    if query.is_empty() {
                        return;
                    }
                    if let Some(web) = this.web_view.borrow().as_ref() {
                        web.find(&query, false, true);
                    }
                }
            ));

            // Escape hides the bar and drops highlights.
            self.search_bar.connect_search_mode_enabled_notify(clone!(
                #[weak(rename_to = this)]
                self,
                move |bar| {
                    if !bar.is_search_mode() {
                        this.find_counter_label.set_visible(false);
                        if let Some(web) = this.web_view.borrow().as_ref() {
                            web.stop_finding();
                        }
                    }
                }
            ));
        }
    }

    /// Mirror `reduce-motion` onto process-wide
    /// `GtkSettings::gtk-enable-animations` (inverted). App-global by design.
    fn apply_reduce_motion(settings: &gio::Settings) {
        use gtk::prelude::SettingsExt;
        let reduce = settings.boolean("reduce-motion");
        if let Some(gtk_settings) = gtk::Settings::default() {
            gtk_settings.set_property("gtk-enable-animations", !reduce);
        }
    }

    /// Toggle the `enhanced-focus` CSS class on the root window to match
    /// `focus-indicators`; the stylesheet scopes its focus rules under it.
    fn apply_focus_indicators(settings: &gio::Settings, window: &super::KarereWindow) {
        use gtk::prelude::{SettingsExt, WidgetExt};
        if settings.boolean("focus-indicators") {
            window.add_css_class("enhanced-focus");
        } else {
            window.remove_css_class("enhanced-focus");
        }
    }

    fn show_crash_dialog(win: &super::KarereWindow, req: CrashDialog, dialog_open: Rc<Cell<bool>>) {
        let dialog = adw::AlertDialog::new(Some(&req.title), Some(&req.body));
        dialog.add_response("cancel", &gettextrs::gettext("Cancel"));
        dialog.add_response("diagnostics", &gettextrs::gettext("Copy Diagnostic Info"));
        dialog.add_response("logs", &gettextrs::gettext("Open logs"));
        dialog.set_response_appearance("logs", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("logs"));
        dialog.set_close_response("cancel");
        dialog.connect_response(None, move |_dialog, response| {
            dialog_open.set(false);
            if response == "logs" {
                open_logs();
            } else if response == "diagnostics"
                && let Some(app) = gio::Application::default()
            {
                app.activate_action("copy-diagnostics", None);
            }
        });
        dialog.present(Some(win));
    }

    /// Completion toast for a finished download: `"<name> downloaded"` with
    /// "Open" (file) and "Show in Folder" (parent) buttons, both routed through
    /// the `app.open-download` action.
    fn show_download_toast(
        overlay: &adw::ToastOverlay,
        win: &super::KarereWindow,
        dl: DownloadCompleted,
    ) {
        let DownloadCompleted { path, name } = dl;
        let file_path = path.to_string_lossy().into_owned();
        let show_path = file_path.clone();

        // AdwToast has only one native action button; a custom title widget lets
        // us offer both "Open" and "Show in Folder".
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.set_valign(gtk::Align::Center);
        let label = gtk::Label::new(Some(
            &gettextrs::gettext("{name} downloaded").replace("{name}", &name),
        ));
        label.set_hexpand(true);
        label.set_xalign(0.0);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let open = gtk::Button::with_label(&gettextrs::gettext("Open"));
        open.add_css_class("flat");
        let show = gtk::Button::with_label(&gettextrs::gettext("Show in Folder"));
        show.add_css_class("flat");
        row.append(&label);
        row.append(&open);
        row.append(&show);

        let toast = adw::Toast::new("");
        toast.set_custom_title(Some(&row));
        toast.set_timeout(6);

        open.connect_clicked(clone!(
            #[weak]
            win,
            #[weak]
            toast,
            move |_| {
                invoke_open_download(&win, &file_path);
                toast.dismiss();
            }
        ));
        show.connect_clicked(clone!(
            #[weak]
            win,
            #[weak]
            toast,
            move |_| {
                if let Some(app) = win.application() {
                    app.activate_action("show-download", Some(&show_path.to_variant()));
                }
                toast.dismiss();
            }
        ));

        overlay.add_toast(toast);
    }

    /// Completed download as a system notification with an Open action (for the
    /// "System Notification" / "Both" download type).
    fn show_download_notification(dl: &DownloadCompleted) {
        let Some(app) = gtk::gio::Application::default() else {
            return;
        };
        let name = &dl.name;
        let file_path = dl.path.to_string_lossy().into_owned();

        let notif = gtk::gio::Notification::new(&gettextrs::gettext("Download complete"));
        notif.set_body(Some(name));
        notif.set_icon(&gtk::gio::ThemedIcon::new("folder-download-symbolic"));
        // Click / Open → app.open-download with the file path.
        notif.set_default_action_and_target_value(
            "app.open-download",
            Some(&file_path.to_variant()),
        );
        notif.add_button_with_target_value(
            &gettextrs::gettext("Open"),
            "app.open-download",
            Some(&file_path.to_variant()),
        );
        app.send_notification(Some(&format!("download-{name}")), &notif);
    }

    /// Activate `app.open-download <path>` from a widget callback.
    fn invoke_open_download(win: &super::KarereWindow, path: &str) {
        if let Some(app) = win.application() {
            app.activate_action("open-download", Some(&path.to_variant()));
        } else {
            log::warn!("open-download: window has no application");
        }
    }

    /// Surface a failed download as an `AdwAlertDialog`.
    fn show_download_failed(win: &super::KarereWindow, fail: DownloadFailed) {
        let title = gettextrs::gettext("Download failed: {reason}").replace("{reason}", &fail.reason);
        let dialog = if fail.name.is_empty() {
            adw::AlertDialog::new(Some(&title), None)
        } else {
            let body = gettextrs::gettext("“{name}” could not be saved.").replace("{name}", &fail.name);
            adw::AlertDialog::new(Some(&title), Some(&body))
        };
        dialog.add_response("ok", &gettextrs::gettext("OK"));
        dialog.set_default_response(Some("ok"));
        dialog.set_close_response("ok");
        dialog.present(Some(win));
    }

    /// Best-effort: open the directory where logs are written.
    fn open_logs() {
        let dir = crate::log_dir();
        let uri = gio::File::for_path(&dir).uri();
        if let Err(err) = gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>)
        {
            log::warn!("open-logs: could not open {uri}: {err}");
        }
    }
}
