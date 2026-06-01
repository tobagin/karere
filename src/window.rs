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
    pub fn quit_now(&self) {
        self.imp().force_quit.set(true);
        self.close();
    }

    /// Switch the active account (driven by the tray's per-account entries).
    pub fn switch_account(&self, id: &str) {
        self.imp().switch_account(id);
    }

    /// Run `script` in the page's main frame (used by notification click
    /// routing to re-enter the page via `__karereActivateNotif`).
    pub fn run_page_js(&self, script: &str) {
        if let Some(web) = self.imp().web_view.borrow().as_ref() {
            web.run_js(script);
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

    use crate::application::APP_ID;
    use crate::handlers::{CrashDialog, DownloadCompleted, DownloadFailed};
    use crate::web_view::KarereWebView;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/io/github/tobagin/karere/ui/window.ui")]
    pub struct KarereWindow {
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

        /// Wide-variant switcher popover (from `account_switcher.blp`), parented
        /// to `account_button`; its list mirrors `accounts_list`.
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
        fn constructed(&self) {
            self.parent_constructed();
            let window = self.obj();

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

            // M18 6.3: headerbar zoom-box visibility follows the opt-in setting.
            settings
                .bind("zoom-controls-headerbar", &*self.zoom_box, "visible")
                .flags(gio::SettingsBindFlags::GET)
                .build();
            // Seed the label from the active account's persisted zoom (floor-lifted).
            self.update_zoom_label(
                Self::load_zoom_for_active_account().max(Self::zoom_floor()),
            );

            settings.connect_changed(
                Some("close-button-action"),
                |s, _key| {
                    log::info!(
                        "close-button-action changed to {:?}",
                        s.string("close-button-action").as_str()
                    );
                },
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

                    // "quit", unknown, or forced quit: fall through to M4 CEF-close gate.
                    // Tear down embedded DevTools first so its browser closes too.
                    this.close_devtools();
                    let web_opt = this.web_view.borrow().clone();
                    let Some(web) = web_opt else {
                        return glib::Propagation::Proceed;
                    };
                    if this.closing.get() && web.is_browser_closed() {
                        return glib::Propagation::Proceed;
                    }
                    if !this.closing.get() {
                        this.closing.set(true);
                        web.close_browser();
                        let win_weak = win.downgrade();
                        let web_weak = web.downgrade();
                        glib::timeout_add_local(
                            std::time::Duration::from_millis(50),
                            move || {
                                let (Some(win), Some(web)) = (win_weak.upgrade(), web_weak.upgrade()) else {
                                    return glib::ControlFlow::Break;
                                };
                                if web.is_browser_closed() {
                                    win.close();
                                    glib::ControlFlow::Break
                                } else {
                                    glib::ControlFlow::Continue
                                }
                            },
                        );
                    }
                    glib::Propagation::Stop
                }
            ));

            self.register_win_actions();
            self.setup_search();
            self.setup_focus_withdraw();
            self.setup_spellcheck();
            self.setup_account_switcher();

            // M15: keep the tray menu's Show/Hide label in sync with window
            // visibility (fires on every show/hide, incl. close-to-background).
            window.connect_visible_notify(|win| {
                crate::tray::set_window_visible(win.is_visible());
            });
            crate::tray::set_window_visible(window.is_visible());
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
            .or_else(|| account.user_label.clone())
            .unwrap_or_else(|| gettextrs::gettext("Account"))
    }

    /// Decode account PNG bytes into a `gdk::Texture` for `Adw.Avatar`'s
    /// custom-image. Uses a Pixbuf loader so it works regardless of the GTK
    /// `Texture::from_bytes` availability.
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
            // shows a message instead of the blank GLArea / Chromium error page.
            // Wrap the status page in a `.background` box so it paints an opaque
            // window-coloured fill that fully covers the view underneath.
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
                    .title("No connection")
                    .description("Waiting for the network — retrying…")
                    .vexpand(true)
                    .build(),
            );
            overlay.add_overlay(&offline);

            self.view_container.append(&overlay);
            web.load_url(url);
            self.start_state_poll(&web, &offline);
            *self.web_view.borrow_mut() = Some(web);
        }

        /// Poll the shared handler state at 100 ms and surface it on the GTK
        /// side: crash toasts, the crash-storm dialog, and the offline overlay.
        ///
        /// The overlay is driven by the OS network state (via `GNetworkMonitor`)
        /// as well as the load-error `offline` flag: WhatsApp Web's service
        /// worker serves a cached page when offline, so a load error never
        /// fires and the network monitor is the only reliable offline signal.
        fn start_state_poll(&self, web: &KarereWebView, offline: &gtk::Box) {
            use gtk::gio::prelude::NetworkMonitorExt;

            let toast_overlay = self.toast_overlay.get();
            let web_weak = web.downgrade();
            let win_weak = self.obj().downgrade();
            let offline_weak = offline.downgrade();
            let counter_weak = self.find_counter_label.downgrade();
            let monitor = gtk::gio::NetworkMonitor::default();
            let dialog_open = Rc::new(Cell::new(false));
            let was_net_down = Rc::new(Cell::new(false));

            glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                let Some(web) = web_weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                let shared = web.shared();
                let (toast, dialog, flag_offline, find, completed, failed) = {
                    let mut s = shared.lock();
                    (
                        s.crash_toast.take(),
                        s.crash_dialog_request.take(),
                        s.offline,
                        s.find_result.take(),
                        std::mem::take(&mut s.downloads_completed),
                        std::mem::take(&mut s.downloads_failed),
                    )
                };

                if let Some(find) = find
                    && let Some(label) = counter_weak.upgrade()
                {
                    if find.count == 0 {
                        label.set_visible(false);
                    } else {
                        label.set_text(&format!("{} of {}", find.active, find.count));
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

                if !completed.is_empty() || !failed.is_empty() {
                    if let Some(win) = win_weak.upgrade() {
                        for dl in completed {
                            show_download_toast(&toast_overlay, &win, dl);
                        }
                        for fail in failed {
                            show_download_failed(&win, fail);
                        }
                    }
                }

                let net_down = !monitor.is_network_available();
                // Network just came back: reload so the cached page reconnects.
                if was_net_down.get() && !net_down {
                    web.reload();
                }
                was_net_down.set(net_down);

                if let Some(page) = offline_weak.upgrade() {
                    page.set_visible(flag_offline || net_down);
                }
                glib::ControlFlow::Continue
            });
        }

        /// Withdraw live notification banners when the window regains focus
        /// (M14 4.1). A short debounce coalesces rapid focus toggles so we don't
        /// stampede `execute_java_script` on every flicker (4.2).
        fn setup_focus_withdraw(&self) {
            let window = self.obj();
            let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

            window.connect_is_active_notify(move |win| {
                if !win.is_active() {
                    return;
                }
                // M15: focus clears the unread tray indicator.
                if let Some(app) = gio::Application::default() {
                    app.activate_action("set-unread", Some(&0u32.to_variant()));
                }
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
                        if let Some(web) = web {
                            crate::notifications::tracker()
                                .on_focus_gained(|js| web.run_js(js));
                        }
                    },
                );
                *pending.borrow_mut() = Some(id);
            });
        }

        // ---- M20 account switcher ------------------------------------------

        pub fn setup_account_switcher(&self) {
            // Instantiate the wide-variant popover from its own resource and
            // parent it to the header account button.
            let builder = gtk::Builder::from_resource(
                "/io/github/tobagin/karere/ui/account_switcher.ui",
            );
            if let Some(popover) = builder.object::<gtk::Popover>("account_switcher_popover") {
                popover.set_parent(&*self.account_button);
                *self.account_popover.borrow_mut() = Some(popover);
            }
            *self.accounts_list_popover.borrow_mut() =
                builder.object::<gtk::ListBox>("accounts_list_popover");

            // Header button opens the popover on wide windows, the bottom sheet
            // on narrow ones.
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

            // Disable "Add account" at the hard cap (matches the Alt+1..9 jumps).
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

            // Mirror the account list into the tray (§9): name + avatar pixmap.
            let summaries = accounts
                .iter()
                .map(|a| crate::tray::AccountSummary {
                    id: a.id.clone(),
                    name: row_title(a),
                    has_unread: a.has_unread,
                    icon_png: a.avatar_png.clone(),
                })
                .collect();
            crate::tray::set_accounts(summaries);

            // Sync the header avatar to the active (or MRU-first) account.
            let active = mgr.active().or_else(|| accounts.first().cloned());
            if let Some(a) = active {
                let label = row_title(&a);
                self.account_avatar.set_text(Some(&label));
                match a.avatar_png.as_deref().and_then(texture_from_png) {
                    Some(tex) => self.account_avatar.set_custom_image(Some(&tex)),
                    None => self.account_avatar.set_custom_image(None::<&gtk::gdk::Texture>),
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
            // Title = account name (pushname); subtitle = the user label. Before
            // pairing (no pushname yet) show the waiting state instead.
            if runtime.awaiting_pairing && account.pushname.is_none() {
                row.set_subtitle(&gettextrs::gettext("Waiting for QR scan…"));
            } else if let Some(label) = account.user_label.as_deref() {
                row.set_subtitle(&glib::markup_escape_text(label));
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

            // Persistent degraded-mode yellow badge.
            if runtime.degraded {
                let badge = gtk::Label::new(Some(&gettextrs::gettext("degraded")));
                badge.add_css_class("warning");
                badge.add_css_class("caption-heading");
                badge.set_valign(gtk::Align::Center);
                if let Some(reason) = runtime.degraded_reason.as_deref() {
                    badge.set_tooltip_text(Some(reason));
                }
                row.add_suffix(&badge);
            }

            // Edit + remove affordances (no reorder controls — MRU only).
            let edit = gtk::Button::from_icon_name("document-edit-symbolic");
            edit.add_css_class("flat");
            edit.set_valign(gtk::Align::Center);
            edit.set_tooltip_text(Some(&gettextrs::gettext("Edit account")));
            edit.connect_clicked(clone!(
                #[weak(rename_to = this)]
                self,
                #[strong]
                id,
                move |_| this.open_account_dialog(&id, false)
            ));
            row.add_suffix(&edit);

            let remove = gtk::Button::from_icon_name("user-trash-symbolic");
            remove.add_css_class("flat");
            remove.set_valign(gtk::Align::Center);
            // The last remaining account cannot be removed (there must always be
            // at least one).
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

        /// Switch to the account at `idx` in MRU order (no-op if out of range).
        fn switch_to_index(&self, idx: usize) {
            if let Some(a) = crate::accounts::manager().get_accounts_sorted().get(idx) {
                self.switch_account(&a.id);
            }
        }

        /// Cycle the active account by `delta` (+1 next, -1 previous) in MRU order.
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
                // M18 4.2: reflect the new account's zoom. An already-loaded
                // browser retains its CEF zoom level across hide/show; a
                // freshly-spawned one re-applies on its own first `on_load_end`.
                // Either way the headerbar label must track the active account.
                let z = Self::load_zoom_for_active_account().max(Self::zoom_floor());
                web.set_zoom_linear(z);
                self.update_zoom_label(z);
            }
        }

        /// `win.add-account`: create an account with a unique default label, give
        /// it the foreground browser so its QR is visible immediately, and close
        /// the switcher. No upfront prompt — the real name fills in from the
        /// Store hook on pairing; the label is editable later via the pencil.
        fn add_account(&self) {
            let mgr = crate::accounts::manager();
            let count = mgr.get_accounts_sorted().len();
            if count >= MAX_ACCOUNTS {
                self.close_switcher();
                self.toast_overlay.add_toast(adw::Toast::new(&gettextrs::gettext(
                    "Maximum of 9 accounts reached",
                )));
                return;
            }
            let n = count + 1;
            let account = mgr.add();
            mgr.update_user_label(&account.id, Some(format!("Account {n}")));
            if let Some(web) = self.web_view.borrow().as_ref() {
                web.spawn_account(&account.id, true);
            }
            mgr.activate(&account.id);
            self.close_switcher();
        }

        /// Add/edit dialog: only `user_label` is editable; identity fields are
        /// shown read-only. When `is_new`, the dialog auto-closes once the
        /// account's `pushname` is discovered (pairing complete).
        fn open_account_dialog(&self, id: &str, is_new: bool) {
            let mgr = crate::accounts::manager();
            let Some(account) = mgr.get(id) else { return };
            let id = account.id.clone();

            let page = adw::PreferencesPage::new();
            let group = adw::PreferencesGroup::new();

            let label_row = adw::EntryRow::builder()
                .title(gettextrs::gettext("Label"))
                .text(account.user_label.as_deref().unwrap_or(""))
                .build();
            group.add(&label_row);

            let dim = |title: &str, value: Option<&str>| -> adw::ActionRow {
                let r = adw::ActionRow::builder()
                    .title(title)
                    .subtitle(value.unwrap_or("—"))
                    .build();
                r.set_sensitive(false); // greyed / read-only
                r.add_css_class("property");
                r
            };
            group.add(&dim(&gettextrs::gettext("Name"), account.pushname.as_deref()));
            group.add(&dim(&gettextrs::gettext("WhatsApp ID"), account.wid.as_deref()));
            group.add(&dim(
                &gettextrs::gettext("Avatar URL"),
                account.avatar_url.as_deref(),
            ));
            page.add(&group);

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
                .title(if is_new {
                    gettextrs::gettext("Add Account")
                } else {
                    gettextrs::gettext("Edit Account")
                })
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
                    let label = (!text.trim().is_empty()).then(|| text.to_string());
                    crate::accounts::manager().update_user_label(&id, label);
                    dialog.close();
                }
            ));

            // For a new account, close automatically once it pairs (pushname set).
            if is_new {
                let mgr2 = crate::accounts::manager();
                let handler_id = std::rc::Rc::new(RefCell::new(None));
                let dialog_weak = dialog.downgrade();
                let id_owned = id.clone();
                let sig = mgr2.connect_local("accounts-changed", false, move |_| {
                    if let Some(a) = crate::accounts::manager().get(&id_owned)
                        && a.pushname.is_some()
                        && let Some(d) = dialog_weak.upgrade()
                    {
                        d.close();
                    }
                    None
                });
                *handler_id.borrow_mut() = Some(sig);
                // Disconnect the watcher when the dialog goes away.
                dialog.connect_closed(clone!(
                    #[strong]
                    handler_id,
                    move |_| {
                        if let Some(sig) = handler_id.borrow_mut().take() {
                            crate::accounts::manager().disconnect(sig);
                        }
                    }
                ));
            }

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
                Some(&format!(
                    "{} {}. {}",
                    gettextrs::gettext("This removes the local session for"),
                    name,
                    gettextrs::gettext(
                        "The device stays linked on your phone until you remove it there."
                    ),
                )),
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
                        // Was the removed account the visible one?
                        let was_active = mgr.active().map(|a| a.id) == Some(id.clone());
                        if let Some(web) = this.web_view.borrow().as_ref() {
                            web.close_account(&id);
                        }
                        mgr.remove(&id);
                        // Wipe the on-disk session shortly after the browser
                        // closes (give CEF a moment to release the profile).
                        let id_del = id.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(1500),
                            move || crate::accounts::delete_session_dir(&id_del),
                        );
                        // Removing the foreground account leaves no visible
                        // browser; promote the MRU-first survivor so the view
                        // doesn't go blank until restart.
                        if was_active
                            && let Some(next) = mgr.get_accounts_sorted().first()
                        {
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

            // Account-switch shortcuts: cycle next/prev + jump-to-Nth (MRU order).
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

            // M18: zoom actions. Each steps the active account's linear zoom,
            // applies it through the CEF boundary (respecting the accessibility
            // floor), persists it, and refreshes the headerbar label.
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
        }
    }

    impl KarereWindow {
        /// Effective accessibility floor (linear). (M18, delegates to the
        /// shared `web_view::zoom_floor`.)
        fn zoom_floor() -> f64 {
            crate::web_view::zoom_floor()
        }

        /// Persisted linear zoom for the active account (M20 `Account::zoom_level`),
        /// defaulting to `1.0`. (M18 2.2)
        fn load_zoom_for_active_account() -> f64 {
            crate::accounts::manager()
                .active()
                .map(|a| a.zoom_level)
                .unwrap_or(1.0)
        }

        /// Persist `linear` as the active account's zoom (M18 2.3). Does not
        /// emit `accounts-changed` (see `AccountManager::set_zoom`).
        fn persist_zoom(linear: f64) {
            if let Some(a) = crate::accounts::manager().active() {
                crate::accounts::manager().set_zoom(&a.id, linear);
            }
        }

        /// Apply `linear` (lifted to the floor) to the live browser, persist the
        /// effective value, and update the headerbar label. (M18 2.4 / 5.2)
        fn apply_and_persist_zoom(&self, linear: f64) {
            let effective = linear.max(Self::zoom_floor());
            if let Some(web) = self.web_view.borrow().as_ref() {
                web.set_zoom_linear(effective);
            }
            Self::persist_zoom(effective);
            self.update_zoom_label(effective);
        }

        /// Handle `win.zoom-in` / `win.zoom-out` / `win.zoom-reset`. (M18 3.x)
        fn zoom_step(&self, verb: &str) {
            let floor = Self::zoom_floor();
            let cur = Self::load_zoom_for_active_account().max(floor);
            let target = match verb {
                "zoom-in" => cur * 1.1,
                // Clamp the step-down up to the floor; never cross it. (5.3)
                "zoom-out" => (cur / 1.1).max(floor),
                _ => 1.0_f64.max(floor),
            };
            self.apply_and_persist_zoom(target);
        }

        /// Refresh the headerbar `<int>%` label from a linear factor. (M18 6.4)
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
        /// CEF 148 cannot render `ShowDevTools` windowless, so DevTools is the
        /// frontend page served by `--remote-debugging-port`, loaded like any
        /// other URL. The target URL is resolved off the main thread.
        fn open_devtools(&self) {
            if self.web_view.borrow().is_none() {
                log::warn!("open_devtools: no web view");
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
                        toast.add_toast(adw::Toast::new(&format!("DevTools unavailable: {e}")));
                        if let Some(win) = win_weak.upgrade() {
                            win.imp().close_devtools();
                        }
                    }
                }
                glib::ControlFlow::Break
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
            // render; force it to repaint so it doesn't stay black.
            if let Some(main) = self.web_view.borrow().as_ref() {
                let main = main.clone();
                glib::idle_add_local_once(move || main.queue_render());
            }
        }

        /// Wire the headerbar spellcheck-language dropdown (M16): a sorted view
        /// of `KNOWN_LANGUAGES` with star-pin favorites. Selecting a language
        /// persists `spell-checking-languages` and switches the live browser via
        /// `KarereWebView::set_spellcheck_languages` (no reload). Toggling a row
        /// star persists `favorite-spell-check-languages` and re-sorts.
        fn setup_spellcheck(&self) {
            use crate::spellcheck_ui::{self, SpellLang};
            use gtk::gio::prelude::SettingsExtManual;
            use std::rc::Rc;

            let settings = gio::Settings::new(APP_ID);
            let dropdown = self.dictionary_dropdown.get();

            // Visible only when spellcheck is enabled AND the headerbar control
            // is opted in; keep it in sync with both keys.
            let spell_dropdown_visible =
                |s: &gio::Settings| s.boolean("enable-spell-checking") && s.boolean("spellcheck-headerbar");
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
            // load handler also re-seeds it on every navigation).
            let win_ac = self.obj().downgrade();
            settings.connect_changed(
                Some("enable-auto-correct"),
                move |s, _| {
                    let on = s.boolean("enable-auto-correct");
                    if let Some(win) = win_ac.upgrade()
                        && let Some(web) = win.imp().web_view.borrow().as_ref()
                    {
                        web.run_js(&format!("window.__karereAutoCorrect = {on};"));
                    }
                },
            );

            let favorites = strv_vec(&settings, "favorite-spell-check-languages");
            let store = spellcheck_ui::build_store(&favorites);
            let sorter = spellcheck_ui::build_sorter();
            let sort_model = gtk::SortListModel::new(Some(store), Some(sorter.clone()));
            dropdown.set_model(Some(&sort_model));
            dropdown.set_factory(Some(&spellcheck_ui::build_button_factory()));

            // Star toggle → persist favorites and re-sort.
            let on_toggle: Rc<dyn Fn(&SpellLang, bool)> = Rc::new(clone!(
                #[strong]
                settings,
                #[strong]
                sorter,
                move |lang: &SpellLang, now: bool| {
                    lang.set_favorite(now);
                    let mut favs = strv_vec(&settings, "favorite-spell-check-languages");
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
            ));
            dropdown.set_list_factory(Some(&spellcheck_ui::build_list_factory(on_toggle)));

            // Resolve the effective startup language: explicit list first, else
            // a single auto-detected code (mirrors cef_runtime). The Chromium
            // command-line switch alone does NOT populate `spellcheck.dictionaries`
            // — only `set_preference` does — so the active list must be pushed to
            // the live browser at startup, otherwise nothing is checked until the
            // user changes the dropdown.
            let startup_langs = resolve_startup_languages(&settings);

            // Initialise the active row from the resolved language BEFORE wiring
            // the change handler, so restoring state doesn't write back.
            if let Some(want) = startup_langs.first() {
                let n = sort_model.n_items();
                for i in 0..n {
                    if let Some(lang) = sort_model.item(i).and_downcast::<SpellLang>()
                        && &lang.code() == want
                    {
                        dropdown.set_selected(i);
                        break;
                    }
                }
            }

            // NOTE: the startup language is applied to the browser by the load
            // handler on the first main-frame `on_load_end` (setting the
            // preference before the page/spellcheck service is ready is ignored).
            // Here we only seed the dropdown's visible selection.

            let win = self.obj().downgrade();
            dropdown.connect_selected_item_notify(clone!(
                #[strong]
                settings,
                move |dd| {
                    let Some(lang) = dd.selected_item().and_downcast::<SpellLang>() else {
                        return;
                    };
                    let code = lang.code();
                    let _ = settings.set_strv("spell-checking-languages", [code.as_str()]);
                    if let Some(win) = win.upgrade()
                        && let Some(web) = win.imp().web_view.borrow().as_ref()
                    {
                        web.set_spellcheck_languages(&[code], true);
                    }
                }
            ));
        }

        /// Wire the find-in-page search bar to Chromium's `BrowserHost::find`.
        fn setup_search(&self) {
            // Standard reveal/hide + Escape handling provided by GtkSearchBar.
            self.search_bar.connect_entry(&*self.search_entry);

            // Fresh search on every keystroke (find_next=false restarts the match set).
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

            // Next/Prev reuse the last query with find_next=true so Chromium cycles.
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

    /// Read a GSettings `as` key into an owned `Vec<String>`.
    fn strv_vec(settings: &gio::Settings, key: &str) -> Vec<String> {
        use gtk::gio::prelude::SettingsExtManual;
        settings.strv(key).iter().map(|s| s.to_string()).collect()
    }

    /// Effective spellcheck languages at startup (explicit selection, else
    /// closest auto-detected locale). Thin wrapper over
    /// `spellcheck::resolve_languages` reading the app's GSettings.
    fn resolve_startup_languages(settings: &gio::Settings) -> Vec<String> {
        use gtk::prelude::SettingsExt;
        let explicit = strv_vec(settings, "spell-checking-languages");
        crate::spellcheck::resolve_languages(&explicit, settings.boolean("auto-detect-language"))
    }

    fn show_crash_dialog(
        win: &super::KarereWindow,
        req: CrashDialog,
        dialog_open: Rc<Cell<bool>>,
    ) {
        let dialog = adw::AlertDialog::new(Some(&req.title), Some(&req.body));
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("logs", "Open logs");
        dialog.set_response_appearance("logs", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("logs"));
        dialog.set_close_response("cancel");
        dialog.connect_response(None, move |_dialog, response| {
            dialog_open.set(false);
            if response == "logs" {
                open_logs();
            }
        });
        dialog.present(Some(win));
    }

    /// Raise the completion toast for a finished download: `"<name> downloaded"`
    /// with "Open" (the file) and "Show in Folder" (its parent) buttons, both
    /// routed through the `app.open-download` action.
    fn show_download_toast(
        overlay: &adw::ToastOverlay,
        win: &super::KarereWindow,
        dl: DownloadCompleted,
    ) {
        let DownloadCompleted { path, name } = dl;
        let file_path = path.to_string_lossy().into_owned();
        let parent_path = path
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| file_path.clone());

        // AdwToast carries a single native action button; a custom title widget
        // lets us offer both "Open" and "Show in Folder".
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.set_valign(gtk::Align::Center);
        let label = gtk::Label::new(Some(&format!("{name} downloaded")));
        label.set_hexpand(true);
        label.set_xalign(0.0);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let open = gtk::Button::with_label("Open");
        open.add_css_class("flat");
        let show = gtk::Button::with_label("Show in Folder");
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
                invoke_open_download(&win, &parent_path);
                toast.dismiss();
            }
        ));

        overlay.add_toast(toast);
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
        let title = format!("Download failed: {}", fail.reason);
        let dialog = if fail.name.is_empty() {
            adw::AlertDialog::new(Some(&title), None)
        } else {
            let body = format!("“{}” could not be saved.", fail.name);
            adw::AlertDialog::new(Some(&title), Some(&body))
        };
        dialog.add_response("ok", "OK");
        dialog.set_default_response(Some("ok"));
        dialog.set_close_response("ok");
        dialog.present(Some(win));
    }

    /// Best-effort: open the directory where logs are written. A dedicated
    /// in-app log viewer is future work.
    fn open_logs() {
        let dir = glib::user_data_dir().join(APP_ID);
        let uri = format!("file://{}", dir.display());
        if let Err(err) =
            gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>)
        {
            log::warn!("open-logs: could not open {uri}: {err}");
        }
    }
}
