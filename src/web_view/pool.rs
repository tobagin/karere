use cef::{
    self, Browser, BrowserSettings, CefString, ImplBrowser, ImplBrowserHost, ImplFrame,
    ImplRequestContextHandler, RequestContext, RequestContextHandler, RequestContextSettings,
    WindowInfo, WrapRequestContextHandler, browser_host_create_browser_sync, rc::Rc,
    request_context_create_context, wrap_request_context_handler,
};
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::sync::atomic::Ordering;
use crate::handlers::{ClientBuilder, life_span::ShellLifeSpanHandler};

use super::gl::accel_osr_enabled;
use super::imp::KarereWebView;
use super::input::keyboard::reset_ime;

/// How long a visible accelerated browser may go without a first frame.
const FIRST_ACCEL_FRAME_DEADLINE_SECS: u64 = 5;

impl KarereWebView {
    pub fn close_browser(&self) {
        #[cfg(test)]
        if self.suppress_browser_creation.load(Ordering::Acquire) {
            self.fallback_test_events.borrow_mut().push(("close", None));
        }

        // Cancel any in-flight OSR menu so CEF doesn't leak pending-menu state
        // on mid-menu teardown, and drop the popover.
        self.cancel_context_menu();
        let id = self.browser_id.swap(0, Ordering::Relaxed);
        if id != 0 {
            super::unregister_context_menu_widget(id);
        }

        // Close every pooled account browser, then the legacy/DevTools single
        // browser if it's outside the pool.
        let pooled: Vec<Browser> = self.browsers.lock().drain().map(|(_, b)| b).collect();
        {
            // Keep every handler until it reports closed (`is_browser_closed`).
            let mut closing = self.closing_life_spans.lock();
            closing.extend(self.life_spans.lock().drain().map(|(_, l)| l));
            if let Some(l) = self.life_span.lock().take() {
                closing.push(l);
            }
        }
        self.cdp_registrations.borrow_mut().clear();
        self.pending_contexts.lock().clear();
        *self.foreground.lock() = None;
        *self.wanted_foreground.lock() = None;
        if let Some(shared) = self.shared.lock().as_ref() {
            shared.lock().foreground_browser_id = 0;
        }
        for browser in &pooled {
            crate::accounts::unregister_browser(browser.identifier());
            if let Some(host) = browser.host() {
                host.close_browser(0);
            }
        }
        if pooled.is_empty()
            && let Some(browser) = self.browser.lock().as_ref()
            && let Some(host) = browser.host()
        {
            host.close_browser(0);
        }
        *self.browser.lock() = None;
        *self.life_span.lock() = None;
    }

    /// Recreate this widget's browser pool with CEF shared textures disabled.
    /// `shared_texture_enabled` is immutable after browser creation, so dropping
    /// only the failed accelerated frame cannot restore CPU `on_paint` delivery.
    fn restart_with_software_osr(&self) {
        log::warn!("accelerated OSR disabled for this view; recreating browsers for CPU paint");
        let is_devtools = self.devtools.load(Ordering::Relaxed);
        // Keep account contexts that were already initializing. Their callbacks
        // consult the new software-only flag, and retaining them avoids racing a
        // duplicate context/browser creation during a first-frame fallback.
        let pending = std::mem::take(&mut *self.pending_contexts.lock());
        self.close_browser();
        *self.pending_contexts.lock() = pending;
        if is_devtools {
            self.spawn_browser(None, true);
        } else {
            self.spawn_all_accounts(true);
        }
    }

    /// Make the accelerated-to-software transition exactly once and defer the
    /// CEF pool restart until after the current GLArea render callback returns.
    pub(super) fn force_software_osr(&self) -> bool {
        !self.software_osr_forced.swap(true, Ordering::AcqRel)
    }

    pub(super) fn schedule_software_osr_fallback(&self) {
        if !self.force_software_osr() {
            return;
        }
        let weak = self.obj().downgrade();
        glib::idle_add_local_once(move || {
            if let Some(widget) = weak.upgrade() {
                widget.imp().restart_with_software_osr();
            }
        });
    }

    /// Accelerated OSR can deliver no frame at all (CEF 156 on radeonsi: the
    /// capturer's buffer fails to allocate), which no import error reports.
    /// Arm a deadline whenever a visible foreground still has no accelerated
    /// frame; when it passes, recreate the browsers for CPU paint.
    pub(super) fn watch_first_accel_frame(&self) {
        if let Some(watch) = self.first_accel_watch.borrow_mut().take() {
            watch.remove();
        }
        if !self.no_accel_frame_yet() {
            return;
        }
        let weak = self.obj().downgrade();
        *self.first_accel_watch.borrow_mut() = Some(glib::timeout_add_local_once(
            std::time::Duration::from_secs(FIRST_ACCEL_FRAME_DEADLINE_SECS),
            move || {
                let Some(view) = weak.upgrade() else { return };
                let imp = view.imp();
                imp.first_accel_watch.borrow_mut().take();
                if !imp.no_accel_frame_yet() {
                    return;
                }
                log::warn!(
                    "accelerated OSR delivered no frame in {FIRST_ACCEL_FRAME_DEADLINE_SECS}s; falling back to CPU paint"
                );
                if let Some(window) = view.root().and_downcast::<crate::window::KarereWindow>() {
                    window.show_toast(&gettextrs::gettext(
                        "GPU rendering is not working on this system. Using software rendering.",
                    ));
                }
                imp.schedule_software_osr_fallback();
            },
        ));
    }

    /// A visible, accelerated foreground browser that has painted nothing.
    fn no_accel_frame_yet(&self) -> bool {
        self.shared_texture_enabled_for_browser()
            && self.window_visible.load(Ordering::Relaxed)
            && self.browser_id.load(Ordering::Relaxed) != 0
            && self.shared.lock().as_ref().is_some_and(|s| s.lock().accel.is_none())
    }

    pub(super) fn shared_texture_enabled_for_browser(&self) -> bool {
        !self.software_osr_forced.load(Ordering::Acquire) && accel_osr_enabled()
    }

    /// WhatsApp Web entry point; every account browser starts here.
    const WHATSAPP_URL: &'static str = "https://web.whatsapp.com/";

    /// Bring up the account pool on first realize: spawn every account and adopt
    /// the active one as the visible foreground.
    pub(super) fn bootstrap_pool(&self) {
        self.spawn_all_accounts(true);
    }

    /// Spawn a CEF browser for EVERY account so each loads WhatsApp and delivers
    /// notifications, not just the visible one. `adopt_active` makes the active
    /// account the foreground; otherwise all stay paused (`was_hidden`) — the
    /// background-start prewarm path (no window yet). Idempotent: [`spawn_browser`]
    /// skips already-spawned accounts, so realize-after-prewarm only adopts the foreground.
    pub fn spawn_all_accounts(&self, adopt_active: bool) {
        if self.devtools.load(Ordering::Relaxed) {
            // DevTools view is not an account: single legacy browser, default context.
            self.spawn_browser(None, true);
            return;
        }

        let mgr = crate::accounts::manager();
        if mgr.get_accounts_sorted().is_empty() {
            mgr.add();
        }
        // Default account is created label-less; backfill it (and migrate
        // older installs) to "Account 1" so it matches added accounts.
        mgr.backfill_labels();
        let accounts = mgr.get_accounts_sorted();
        // Boot into the last-used account, not list-first (#166).
        let Some(first) = mgr.mru_first() else {
            log::error!("spawn_all_accounts: no account after add()");
            return;
        };
        mgr.activate(&first.id);
        for acc in &accounts {
            let foreground = adopt_active && acc.id == first.id;
            self.spawn_browser(Some(acc.id.clone()), foreground);
        }
    }

    /// Spawn a CEF browser for `account_id` (or the legacy default context when
    /// `None`, DevTools only). `make_foreground` shows it; else it starts paused.
    ///
    /// A per-account isolated `RequestContext` must finish init before
    /// `CreateBrowserSync` succeeds (CEF Chrome runtime), so creation is deferred
    /// to its init callback ([`on_account_context_ready`]). The legacy/DevTools
    /// path uses the global context and is created synchronously.
    pub fn spawn_browser(&self, account_id: Option<String>, make_foreground: bool) {
        #[cfg(test)]
        if self.suppress_browser_creation.load(Ordering::Acquire) {
            // Before fallback the exact capability probe is irrelevant to this
            // test seam (and may require an installed GSettings schema). After
            // fallback, call the production selector and observe the immutable
            // WindowInfo choice that the recreated browser would receive.
            let shared_textures = if self.software_osr_forced.load(Ordering::Acquire) {
                self.shared_texture_enabled_for_browser()
            } else {
                true
            };
            self.fallback_test_events
                .borrow_mut()
                .push(("create", Some(shared_textures)));
            return;
        }

        if self.shutting_down.load(Ordering::Acquire) {
            return; // quitting: a late spawn would never be closed
        }

        if let Some(id) = account_id.as_ref() {
            if make_foreground {
                // Remembered until the browser exists (switch_to/adopt clears
                // it), so a still-initialising context still takes over.
                *self.wanted_foreground.lock() = Some(id.clone());
            }
            if self.browsers.lock().contains_key(id) {
                if make_foreground {
                    self.switch_to(id);
                }
                return;
            }
            if self.pending_contexts.lock().contains_key(id) {
                return;
            }
        }

        let Some(id) = account_id else {
            // Legacy / DevTools view: global context, synchronous create.
            self.create_browser_now(None, None, make_foreground);
            return;
        };

        // Per-account: build the isolated context now (held in `pending_contexts`),
        // create the browser once it signals initialized.
        let cache = crate::accounts::session_cache_path(&id);
        // 0700: per-account profile holds auth cookies/session — keep it
        // unreadable by other local users (see cef_runtime root_cache).
        {
            use std::os::unix::fs::DirBuilderExt;
            let _ = std::fs::DirBuilder::new()
                .mode(0o700)
                .recursive(true)
                .create(&cache);
        }
        // Inherit the UI-language override so WhatsApp Web localizes too;
        // empty (no override) leaves the global CefSettings default.
        let accept_lang = crate::i18n::override_locale()
            .as_deref()
            .map(crate::i18n::accept_language_for)
            .unwrap_or_default();
        let rc_settings = RequestContextSettings {
            cache_path: CefString::from(cache.to_string_lossy().as_ref()),
            persist_session_cookies: 1,
            accept_language_list: CefString::from(accept_lang.as_str()),
            ..Default::default()
        };
        let mut handler =
            ContextReadyHandler::new(self.obj().downgrade(), id.clone(), make_foreground);
        match request_context_create_context(Some(&rc_settings), Some(&mut handler)) {
            Some(ctx) => {
                self.pending_contexts.lock().insert(id, ctx);
            }
            None => log::error!("request_context_create_context returned None for {id}"),
        }
    }

    /// Continuation from the per-account `RequestContext` init callback: context
    /// ready, so create the browser against it.
    pub fn on_account_context_ready(
        &self,
        request_context: Option<&mut RequestContext>,
        account_id: &str,
        make_foreground: bool,
    ) {
        if self.shutting_down.load(Ordering::Acquire) {
            log::info!("context ready for {account_id} during quit; not creating a browser");
            self.pending_contexts.lock().remove(account_id);
            return;
        }
        // The latest foreground request wins over the flag captured at spawn
        // time: a switch to this account while it was initialising must show
        // it, and a switch away from it must not be undone now.
        let make_foreground = {
            let mut wanted = self.wanted_foreground.lock();
            if wanted.as_deref() == Some(account_id) {
                *wanted = None;
                true
            } else {
                make_foreground && wanted.is_none() && self.foreground.lock().is_none()
            }
        };
        self.create_browser_now(
            request_context,
            Some(account_id.to_owned()),
            make_foreground,
        );
        // Browser now holds its own ref; release the pending hold.
        self.pending_contexts.lock().remove(account_id);
    }

    /// Create the browser (sync) against `request_context` (or global if `None`)
    /// and wire it into the pool.
    fn create_browser_now(
        &self,
        request_context: Option<&mut RequestContext>,
        account_id: Option<String>,
        make_foreground: bool,
    ) {
        let shared = self
            .shared
            .lock()
            .as_ref()
            .expect("create_browser_now called before shared was initialized")
            .clone();
        let (client, life) = if self.devtools.load(Ordering::Relaxed) {
            ClientBuilder::build_devtools_for(shared.clone())
        } else {
            ClientBuilder::build_for(shared.clone())
        };

        let window_info = WindowInfo {
            windowless_rendering_enabled: 1,
            // GPU-accelerated OSR: hand us a DMA-BUF via on_accelerated_paint
            // instead of a CPU buffer, when supported + opted in. (gpu-osr)
            shared_texture_enabled: self.shared_texture_enabled_for_browser() as i32,
            ..Default::default()
        };
        let settings = BrowserSettings {
            // 60fps OSR: the old 30 cap read as scroll/typing lag next to a
            // real browser (#173) — measured 29 vs 58fps under wheel load.
            // Idle cost is nil (an idle chat paints ~0 frames; hidden windows
            // are paused via was_hidden, #151), so this only spends CPU while
            // content actually animates. Runtime set_windowless_frame_rate is
            // a no-op in this CEF build, so the rate is fixed at creation.
            windowless_frame_rate: 60,
            ..Default::default()
        };

        // A launch URL (deep link, --url) goes to the foreground browser only.
        let url_string = if make_foreground || account_id.is_none() {
            self.pending_url.lock().take()
        } else {
            None
        }
        .unwrap_or_else(|| Self::WHATSAPP_URL.to_owned());
        let url = CefString::from(url_string.as_str());

        let mut client = client;
        let browser = browser_host_create_browser_sync(
            Some(&window_info),
            Some(&mut client),
            Some(&url),
            Some(&settings),
            None,
            request_context,
        );
        let Some(b) = browser else {
            log::error!("browser_host_create_browser_sync returned None");
            return;
        };
        log::info!("browser spawned (account={account_id:?}, foreground={make_foreground})");

        if let Some(id) = account_id.clone() {
            crate::accounts::register_browser(b.identifier(), &id);
            self.browsers.lock().insert(id.clone(), b.clone());
            self.life_spans.lock().insert(id.clone(), life.clone());
            // In-process notification bridge (no CDP port). Attributes
            // notifications to this account via the trusted browser id.
            if let Some(reg) = crate::cdp::attach(&b, &id) {
                self.cdp_registrations.borrow_mut().insert(id, reg);
            }
        }

        if make_foreground {
            self.adopt_foreground(account_id, b, life);
        } else if let Some(host) = b.host() {
            host.was_hidden(1); // background: pause until switched in
        }
        self.apply_audio_mute();
    }

    /// Make `browser` the foreground: cache it for input/render, publish its CEF
    /// id for paint gating, register it in the context-menu registry, and show it.
    fn adopt_foreground(
        &self,
        account_id: Option<String>,
        browser: Browser,
        life: ShellLifeSpanHandler,
    ) {
        // A menu belongs to the browser that opened it. Cancel it before an
        // account switch so a delayed Copy activation cannot target the newly
        // foregrounded account's selection.
        if self
            .browser
            .lock()
            .as_ref()
            .is_some_and(|previous| previous.identifier() != browser.identifier())
        {
            self.cancel_context_menu();
            // An open composition belongs to the outgoing page too. (I1)
            reset_ime(&self.obj());
        }

        // A launch URL (deep link) that arrived while no foreground browser
        // existed and was not consumed at creation.
        if account_id.is_some()
            && let Some(url) = self.pending_url.lock().take()
            && let Some(frame) = browser.main_frame()
        {
            frame.load_url(Some(&CefString::from(url.as_str())));
        }

        // Pause the OUTGOING foreground. Required so a later switch back wakes
        // it with a real visibility change: `was_hidden(0)` is a no-op on an
        // already-visible browser and emits no paint, which would leave a
        // stale frame on screen (the add-account path reaches here without
        // having paused the previous account — e.g. deleting the foreground
        // account then left its QR frame stuck).
        if let Some(prev) = self.browser.lock().as_ref()
            && prev.identifier() != browser.identifier()
            && let Some(host) = prev.host()
        {
            host.was_hidden(1);
        }

        // Unregister the previous foreground from the context-menu registry.
        let prev_id = self.browser_id.swap(0, Ordering::Relaxed);
        if prev_id != 0 {
            super::unregister_context_menu_widget(prev_id);
        }

        let id = browser.identifier();
        self.browser_id.store(id, Ordering::Relaxed);
        super::register_context_menu_widget(id, &self.obj());
        if let Some(shared) = self.shared.lock().as_ref() {
            let mut s = shared.lock();
            s.foreground_browser_id = id;
            // Screen reader in use: mirror the new foreground's page (#204).
            if self.ax_export.borrow().is_some() {
                s.a11y_tree_dirty = true;
                crate::handlers::wake_view(s);
            }
        }
        {
            let mut wanted = self.wanted_foreground.lock();
            if wanted.is_some() && *wanted == account_id {
                *wanted = None;
            }
        }
        *self.foreground.lock() = account_id;
        *self.life_span.lock() = Some(life);
        *self.browser.lock() = Some(browser.clone());

        if let Some(host) = browser.host() {
            // Respect window visibility: when the window is hidden (e.g.
            // start-in-background) the new foreground must stay paused, or
            // CEF composites an invisible page at full rate (#151).
            if self.window_visible.load(Ordering::Relaxed) {
                host.was_hidden(0);
                host.notify_screen_info_changed();
                host.was_resized();
                // Re-sync the HiDPI zoom for the newly-visible browser so a
                // scale change or stale prewarm seed does not leave its
                // layout/paint offset from input (#158/#176).
                super::apply_zoom_from_account(&browser, self.display_scale());
                // Force a fresh paint even if CEF thinks visibility is
                // unchanged, so the new foreground immediately overwrites any
                // stale texture.
                host.invalidate(cef::PaintElementType::VIEW);
            } else {
                host.was_hidden(1);
            }
        }
        self.watch_first_accel_frame();
        self.obj().queue_render();
        self.apply_audio_mute();
        // A background account may have loaded at another width; bring
        // its layout in line now that it is the one on screen. (D5)
        self.schedule_mobile_layout_check();
    }

    /// Switch the foreground to the account `new_id`, pausing the previous
    /// foreground and resuming the target. No-op if already foreground or the
    /// target has no spawned browser.
    pub fn switch_to(&self, new_id: &str) {
        if self.foreground.lock().as_deref() == Some(new_id) {
            let mut wanted = self.wanted_foreground.lock();
            if wanted.as_deref() == Some(new_id) {
                *wanted = None;
            }
            return;
        }
        // Resolve the target before touching the current foreground, so a
        // missing target leaves the visible account showing.
        let Some(browser) = self.browsers.lock().get(new_id).cloned() else {
            log::warn!("switch_to: no spawned browser for account {new_id}");
            return;
        };
        let life = self.life_spans.lock().get(new_id).cloned();
        let Some(life) = life else {
            log::warn!("switch_to: no life-span handler for account {new_id}");
            return;
        };
        if let Some(prev) = self.browser.lock().as_ref()
            && let Some(host) = prev.host()
        {
            host.was_hidden(1);
        }
        self.adopt_foreground(Some(new_id.to_owned()), browser, life);
    }

    /// Close and drop the browser for `account_id` (the `remove(id)` path).
    pub fn close_account_browser(&self, account_id: &str) {
        let was_foreground = self.foreground.lock().as_deref() == Some(account_id);
        {
            let mut wanted = self.wanted_foreground.lock();
            if wanted.as_deref() == Some(account_id) {
                *wanted = None;
            }
        }
        // Quit still waits on this browser if it has not finished closing.
        let life = self.life_spans.lock().remove(account_id);
        if let Some(life) = life {
            self.closing_life_spans.lock().push(life);
        }
        // Detach the notification bridge (drop = remove observer).
        self.cdp_registrations.borrow_mut().remove(account_id);
        if let Some(browser) = self.browsers.lock().remove(account_id) {
            crate::accounts::unregister_browser(browser.identifier());
            if let Some(host) = browser.host() {
                host.close_browser(1);
            }
        }
        if was_foreground {
            reset_ime(&self.obj());
            let id = self.browser_id.swap(0, Ordering::Relaxed);
            if id != 0 {
                super::unregister_context_menu_widget(id);
            }
            *self.browser.lock() = None;
            *self.life_span.lock() = None;
            *self.foreground.lock() = None;
            if let Some(shared) = self.shared.lock().as_ref() {
                let mut s = shared.lock();
                s.foreground_browser_id = 0;
                s.popup = Default::default();
            }
        }
    }
}

// Bridges a per-account `RequestContext`'s init callback to the widget so the
// browser is created only once the context is ready.
wrap_request_context_handler! {
    pub struct ContextReadyHandler {
        widget: glib::object::WeakRef<super::KarereWebView>,
        account_id: String,
        make_foreground: bool,
    }

    impl RequestContextHandler {
        fn on_request_context_initialized(
            &self,
            request_context: Option<&mut RequestContext>,
        ) {
            if let Some(widget) = self.widget.upgrade() {
                widget.imp().on_account_context_ready(
                    request_context,
                    &self.account_id,
                    self.make_foreground,
                );
            }
        }
    }
}
