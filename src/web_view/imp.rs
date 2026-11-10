use cef::{
    self, Browser, CefString, ImplBrowser, ImplBrowserHost, ImplFrame, RequestContext,
    RunContextMenuCallback,
};
use glib::subclass::Signal;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::sync::LazyLock;
use parking_lot::Mutex;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use crate::handlers::{SharedRef, life_span::ShellLifeSpanHandler, new_shared};

use super::gl::{NO_GL, cached_frame_texture, probe_accel_osr};
use super::input::keyboard::{IM_FOCUS_TICK, ImFocusAction, ImeState, im_focus_step};
use super::input::install_input_controllers;
use super::x11_origin::window_origin_for;

#[derive(Default)]
pub struct KarereWebView {
    pub shared: Mutex<Option<SharedRef>>,
    /// The foreground browser (clone of the `browsers` entry for `foreground`).
    /// Input/render/spellcheck paths resolve through this → the visible account.
    pub browser: Mutex<Option<Browser>>,
    /// M20 browser pool: every account's CEF browser, keyed by account id.
    /// Background browsers stay alive (paused via `was_hidden(true)`) so a
    /// switch never logs the account out.
    pub browsers: Mutex<HashMap<String, Browser>>,
    /// Per-account life-span handlers, kept alive alongside their browsers.
    pub life_spans: Mutex<HashMap<String, ShellLifeSpanHandler>>,
    /// Per-account in-process CDP notification-bridge registrations. Held
    /// alive for the browser's lifetime; dropping one detaches the observer.
    pub cdp_registrations: RefCell<HashMap<String, cef::Registration>>,
    /// RequestContexts kept alive until the browser is created against them
    /// (in their init callback). Keyed by account id.
    pub pending_contexts: Mutex<HashMap<String, RequestContext>>,
    /// The account id whose browser is currently foreground.
    pub foreground: Mutex<Option<String>>,
    /// Account the user last asked to show while its context was still
    /// initialising; consumed by `on_account_context_ready`.
    pub wanted_foreground: Mutex<Option<String>>,
    /// Life-span handlers of browsers asked to close, held until each reports
    /// `closed` so quit can wait for every one of them.
    pub closing_life_spans: Mutex<Vec<ShellLifeSpanHandler>>,
    /// Set by quit: a context that finishes initialising afterwards must not
    /// create a browser nobody will close.
    pub shutting_down: AtomicBool,
    pub life_span: Mutex<Option<ShellLifeSpanHandler>>,
    pub pending_url: Mutex<Option<String>>,
    /// Set for the embedded DevTools view; selects the permissive client.
    pub devtools: AtomicBool,
    /// One-way runtime kill switch for this widget's accelerated OSR browsers.
    /// A rejected DMA-BUF flips it before the browser pool is recreated, so CEF
    /// resumes delivering CPU `on_paint` frames instead of repeatedly exporting
    /// accelerated frames that this GL context cannot import.
    pub software_osr_forced: AtomicBool,
    /// Test-only observations at the real close/create boundaries. This lets the
    /// DMA-BUF regression execute the production deferred restart without
    /// requiring a live CEF subprocess inside the unit-test process.
    #[cfg(test)]
    pub fallback_test_events: RefCell<Vec<(&'static str, Option<bool>)>>,
    #[cfg(test)]
    pub suppress_browser_creation: AtomicBool,
    /// Last chrome-window visibility. `adopt_foreground` reads it to keep a
    /// new foreground paused while the window is hidden (#151).
    pub window_visible: AtomicBool,
    pub(super) program: AtomicU32,
    /// No usable GL context, so frames are presented through GTK's normal
    /// (software) render path instead of the GLArea framebuffer. See
    /// `snapshot`. (#177)
    pub(super) software_present: AtomicBool,
    pub(super) vao: AtomicU32,
    pub(super) vbo: AtomicU32,
    pub(super) texture: AtomicU32,
    pub(super) tex_w: AtomicI32,
    pub(super) tex_h: AtomicI32,
    /// GPU-OSR: dedicated texture the imported DMA-BUF EGLImage binds to, and
    /// the live EGLImage kept alive while that texture is sampled. (gpu-osr)
    pub(super) accel_tex: AtomicU32,
    pub(super) imported: RefCell<Option<crate::gl_dmabuf::ImportedImage>>,
    /// Popup layer (`<select>` dropdowns) textures: CPU upload target with
    /// its current size, and the accelerated import target + live
    /// EGLImage. Composited over the view in `draw`.
    pub(super) popup_tex: AtomicU32,
    pub(super) popup_tex_w: AtomicI32,
    pub(super) popup_tex_h: AtomicI32,
    pub(super) accel_popup_tex: AtomicU32,
    pub(super) imported_popup: RefCell<Option<crate::gl_dmabuf::ImportedImage>>,
    /// Software present path: textures built from the last view / popup
    /// frame, reused while CEF has not painted again so an unrelated
    /// redraw does not copy the whole frame into a new texture. (P6)
    sw_texture: RefCell<Option<gtk::gdk::MemoryTexture>>,
    sw_popup_texture: RefCell<Option<gtk::gdk::MemoryTexture>>,
    /// Surface whose `scale` we follow and the handler id, so a realize
    /// connects once per surface and unrealize disconnects. (I11)
    scale_notify: RefCell<Option<(gtk::gdk::Surface, glib::SignalHandlerId)>>,
    /// Last pointer position (logical px) so wheel events hit the element
    /// under the cursor, not the top-left corner.
    pub(super) last_mouse_x: AtomicI32,
    pub(super) last_mouse_y: AtomicI32,
    /// Sub-pixel scroll remainder per axis, carried between scroll events so
    /// fine touchpad deltas aren't truncated away. Main-thread only. (#161)
    pub(super) scroll_accum_x: std::cell::Cell<f64>,
    pub(super) scroll_accum_y: std::cell::Cell<f64>,
    /// Clears the mouse controllers' held-button state without dispatching
    /// releases, for a page drag-out whose release went to the drop target.
    /// Set once in `install_input_controllers`.
    pub(super) forget_mouse_buttons: RefCell<Option<Box<dyn Fn()>>>,
    /// The page drag-out in progress, kept alive until it ends; taking it is
    /// what reports the end to CEF exactly once.
    pub(super) drag_out: RefCell<Option<(gtk::gdk::Drag, Browser)>>,
    /// CEF browser id for context-menu registry (de)registration; `0` until spawn.
    pub(super) browser_id: AtomicI32,
    /// Pending OSR context-menu callback (non-`Send`, main-thread only). Resolved
    /// exactly once in the popover `closed` handler: `cont(command)` or `cancel()`.
    /// Dispatching from `closed` (after popdown) re-focuses the webview first —
    /// required for spellcheck-replace and edit commands to apply.
    pub pending_menu_callback: RefCell<Option<RunContextMenuCallback>>,
    /// Command id selected by item activation, consumed by `closed`. `None` →
    /// dismissed without a selection.
    pub pending_menu_command: std::cell::Cell<Option<i32>>,
    /// The currently-displayed context popover, kept alive while open.
    pub context_popover: RefCell<Option<gtk::Popover>>,
    /// M21 mobile-layout gate: pending debounced re-check after a resize or
    /// foreground switch. When it fires, a foreground page loaded with the
    /// other layout is reloaded so `on_load_end` re-evaluates. (D5)
    pub(super) mobile_check: RefCell<Option<glib::SourceId>>,
    /// The GLArea's IM context, stored so `flush_requests` can focus it in/out
    /// in response to CEF's editable-focus signal (keyboard_request). Set once
    /// in `install_input_controllers`.
    pub im_context: RefCell<Option<gtk::IMMulticontext>>,
    /// Mirror of the IM context's focus state. The page can leave the IM
    /// focused-out (a send re-renders the input → virtual-keyboard NONE with
    /// no follow-up), which kills dead-key composition; the key handler uses
    /// this to re-focus the IM on the next physical keypress. (#154)
    pub im_focused: std::cell::Cell<bool>,
    /// Set while the attach file chooser is up, so a drop of several
    /// unreadable files opens it once rather than once per file. (#192)
    pub attach_chooser_open: std::cell::Cell<bool>,
    /// Ticks left before a pending focus-out is actually applied, or 0 when
    /// none is queued. A composer re-render emits editable-focus false then
    /// true; applying the false immediately unfocused the IM mid-typing and
    /// dropped the next key (#180). Deferring it lets the paired true cancel
    /// it, while a genuine blur still settles. See `im_focus_step`.
    pub im_focus_out_pending: std::cell::Cell<u8>,
    /// The focus-out debounce timer is running (`arm_im_focus_timer`).
    im_focus_timer_armed: std::cell::Cell<bool>,
    /// Input-method composition (preedit) state; decides whether a key press
    /// goes to the IM before CEF and how committed text is delivered. (I1)
    pub ime_state: std::cell::Cell<ImeState>,
}

#[glib::object_subclass]
impl ObjectSubclass for KarereWebView {
    const NAME: &'static str = "KarereWebView";
    type Type = super::KarereWebView;
    type ParentType = gtk::GLArea;
}

impl ObjectImpl for KarereWebView {
    fn constructed(&self) {
        self.parent_constructed();
        let widget = self.obj();
        // The production shaders are GLSL ES 3.00. Negotiate that exact API
        // before realization instead of letting GtkGLArea prefer desktop GL;
        // GLES-only Mesa stacks (including PinePhone) otherwise fail context
        // creation before either the CPU or DMA-BUF OSR path can present.
        #[cfg(debug_assertions)]
        let allowed_api = if std::env::var_os("KARERE_TEST_FORCE_DESKTOP_GL").is_some() {
            // Integration-test-only reproduction of the pre-fix contract. The
            // Mesa fixture rejects desktop GL while retaining GLES 3.x.
            gtk::gdk::GLAPI::GL
        } else {
            gtk::gdk::GLAPI::GLES
        };
        #[cfg(not(debug_assertions))]
        let allowed_api = gtk::gdk::GLAPI::GLES;
        widget.set_allowed_apis(allowed_api);
        widget.set_required_version(3, 0);
        widget.set_has_depth_buffer(false);
        widget.set_has_stencil_buffer(false);
        widget.set_auto_render(false);

        let scale = widget.scale_factor() as f32;
        // Default viewport so prewarm browsers (created before sizing) lay out
        // usably; the real allocation replaces it on first show. Stored
        // PHYSICAL (logical 1280×800 × scale) like size_allocate writes it —
        // a logical seed with the pre-realize scale guess of 2 (multi-monitor
        // mixed scale) read back as 640 logical in on_load_end and wrongly
        // injected the mobile layout (#176).
        let shared = new_shared(((1280.0 * scale) as i32, (800.0 * scale) as i32), scale);
        *self.shared.lock() = Some(shared.clone());
        shared.lock().redraw = Some(widget.upcast_ref::<gtk::GLArea>().downgrade().into());

        // No periodic timer here. Paints queue their own render
        // (`request_redraw` in the render handler), and the cursor / IM /
        // announcement handlers queue one coalesced flush through
        // `handlers::wake_view` (`flush_requests`). Only a pending IM
        // focus-out runs a short-lived timer, for its debounce. A hidden
        // window therefore costs no wakeups from this view.
        // (Never add_tick_callback: the frame clock free-runs when vsync is
        // unreliable, pinning a core — issue #151.)

        widget.set_focusable(true);
        widget.set_can_focus(true);
        install_input_controllers(&widget);
    }

    fn signals() -> &'static [Signal] {
        static SIGNALS: LazyLock<Vec<Signal>> = LazyLock::new(|| {
            vec![
                Signal::builder("title-changed")
                    .param_types([String::static_type()])
                    .build(),
            ]
        });
        SIGNALS.as_ref()
    }
}

impl WidgetImpl for KarereWebView {
    /// Present the CEF frame without GL when `realize` could not get a
    /// context (#177). GTK has already fallen back to its software renderer
    /// in that situation, so handing it a memory texture is all that is
    /// left to do; on every normal machine this defers to `GtkGLArea`,
    /// which drives `render` -> `draw` as before.
    fn snapshot(&self, snapshot: &gtk::Snapshot) {
        if !self.software_present.load(Ordering::Relaxed) {
            self.parent_snapshot(snapshot);
            return;
        }
        let widget = self.obj();
        let (w, h) = (widget.width() as f32, widget.height() as f32);
        let bounds = gtk::graphene::Rect::new(0.0, 0.0, w, h);
        let [r, g, b] = super::page_background(&*widget);
        snapshot.append_color(&gtk::gdk::RGBA::new(r, g, b, 1.0), &bounds);
        let Some(shared) = self.shared.lock().as_ref().cloned() else {
            return;
        };
        let mut s = shared.lock();
        let view = cached_frame_texture(&self.sw_texture, &mut s.frame);
        let popup_rect = s.popup.visible_rect(s.foreground_browser_id);
        let popup = match popup_rect {
            Some(_) => cached_frame_texture(&self.sw_popup_texture, &mut s.popup.frame),
            None => {
                *self.sw_popup_texture.borrow_mut() = None;
                None
            }
        };
        drop(s);
        let Some(view) = view else { return };
        // Frame and popup rect are physical pixels, widget bounds logical.
        // Draw the frame at its own size, anchored top-left: during a
        // resize the previous-size frame stays 1:1 instead of stretching.
        let scale = paint_scale(&widget) as f32;
        snapshot.push_clip(&bounds);
        snapshot.append_texture(
            &view,
            &gtk::graphene::Rect::new(
                0.0,
                0.0,
                view.width() as f32 / scale,
                view.height() as f32 / scale,
            ),
        );
        if let (Some(popup), Some((px, py, pw, ph))) = (popup, popup_rect) {
            snapshot.append_texture(
                &popup,
                &gtk::graphene::Rect::new(
                    px as f32 / scale,
                    py as f32 / scale,
                    pw as f32 / scale,
                    ph as f32 / scale,
                ),
            );
        }
        snapshot.pop();
    }

    fn realize(&self) {
        self.parent_realize();
        // Both present paths (GL and software) must follow scale changes.
        self.watch_surface_scale();
        let widget = self.obj();
        widget.make_current();
        if let Some(err) = widget.error() {
            // No GL context. GDK cannot produce one below GLES 3.0, which is
            // all a Mali-400 (PinePhone, lima) offers, so this is terminal for
            // the GL path -- but not for the app: CEF's software OSR still
            // hands us BGRA frames, and `snapshot` can present them through
            // GTK's own renderer. Clear GTK's error so the GLArea shows our
            // frames rather than its built-in error label, and carry on to
            // the browser bootstrap. (#177)
            log::warn!("no GL context ({err}) — presenting frames in software");
            widget.set_error(None);
            self.software_present.store(true, Ordering::Relaxed);
            NO_GL.store(true, Ordering::Relaxed);
            self.bootstrap_pool();
            return;
        }
        if let Some(context) = widget.context() {
            let (major, minor) = context.version();
            log::info!(
                "GLArea context ready: api={:?} version={major}.{minor}",
                widget.api()
            );
        } else {
            // A successful make_current() should always expose its context.
            // Do not continue into raw GL or CEF bootstrap if GTK does not.
            log::error!("GLArea realize error: no context after make_current");
            return;
        }
        unsafe {
            self.init_gl();
        }
        // The one point where this GLArea's context is known current and
        // before any browser of a visible start is created. (I6)
        probe_accel_osr();
        self.bootstrap_pool();
    }

    fn unrealize(&self) {
        self.close_browser();
        if let Some((surface, id)) = self.scale_notify.take() {
            surface.disconnect(id);
        }
        if let Some(id) = self.mobile_check.take() {
            id.remove();
        }
        *self.sw_texture.borrow_mut() = None;
        *self.sw_popup_texture.borrow_mut() = None;
        // glDelete* act on whatever context is current; deleting by name
        // with another view's context current (DevTools closing next to the
        // main view) would free that view's objects. Only tear down with
        // our own context current. (I4)
        let widget = self.obj();
        widget.make_current();
        if widget.error().is_none() && widget.context().is_some() {
            unsafe {
                self.teardown_gl();
            }
        } else {
            // Our names die with the context GTK is about to destroy.
            self.forget_gl_names();
        }
        self.parent_unrealize();
    }

    fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
        self.parent_size_allocate(width, height, baseline);
        // This CEF build ignores device_scale_factor for OSR: on_paint always
        // matches GetViewRect exactly. So the view rect must be PHYSICAL
        // (logical × integer GLArea scale) to fill the framebuffer 1:1 and stay
        // crisp; the resulting too-many-CSS-px (content too small) is corrected
        // by an equal page zoom applied to the browser (see host_zoom_level).
        // Mouse/wheel/menu coords are therefore physical too. (#158)
        self.sync_viewport(width, height, "size_allocate");

        // M21: gate the mobile single-pane layout on logical width. The v3
        // script can't un-apply its DOM/CSS, so a threshold crossing reloads
        // and `on_load_end` re-injects (or not) — debounced, so dragging
        // across the threshold reloads once at the end, not per frame.
        if width > 0
            && let Some(browser) = resolved_browser(self)
            && super::mobile_layout_mismatch(browser.identifier(), width)
        {
            self.schedule_mobile_layout_check();
        }
    }
}

impl GLAreaImpl for KarereWebView {
    fn render(&self, _ctx: &gtk::gdk::GLContext) -> glib::Propagation {
        unsafe {
            self.draw();
        }
        glib::Propagation::Stop
    }
}

// ---- public-from-parent helpers ------------------------------------

impl KarereWebView {
    pub fn load_url(&self, url: &str) {
        if let Some(browser) = self.browser.lock().as_ref()
            && let Some(frame) = browser.main_frame()
        {
            let s = CefString::from(url);
            frame.load_url(Some(&s));
            return;
        }
        *self.pending_url.lock() = Some(url.to_owned());
    }

    /// Recompute the physical paint size + scale for a live scale change
    /// (e.g. dragging between monitors) and tell CEF, then re-apply zoom so
    /// the display-scale term tracks the new scale. (#155, #158)
    fn refresh_screen_scale(&self) {
        let widget = self.obj();
        self.sync_viewport(widget.width(), widget.height(), "refresh_scale");
        widget.queue_render();
    }

    /// Follow the native surface's scale (e.g. dragging between monitors of
    /// different scale) so the physical paint buffer tracks it. The `scale`
    /// notify fires across the integer boundaries the OSR buffer cares
    /// about; a pure fractional change leaves the integer paint scale
    /// unchanged, so `refresh_screen_scale` is then a no-op. Connected once
    /// per surface — a re-realize on the same surface must not stack a
    /// second handler — and disconnected on unrealize. (#155, #158, I11)
    fn watch_surface_scale(&self) {
        let widget = self.obj();
        let Some(surface) = widget.native().and_then(|n| n.surface()) else {
            return;
        };
        let mut slot = self.scale_notify.borrow_mut();
        if slot.as_ref().is_some_and(|(watched, _)| *watched == surface) {
            return;
        }
        if let Some((old, id)) = slot.take() {
            old.disconnect(id);
        }
        let id = surface.connect_scale_notify(glib::clone!(
            #[weak]
            widget,
            move |_s| widget.imp().refresh_screen_scale()
        ));
        *slot = Some((surface, id));
    }

    /// Publish the physical viewport for a logical `width`×`height` at the
    /// current integer paint scale (a non-positive size keeps the old one)
    /// and tell CEF. CEF is left alone when neither the physical size nor
    /// the scale changed — GTK re-allocates far more often than either
    /// moves (P3). A scale change re-applies zoom so the display-scale term
    /// tracks it, keeping buffer and input transform coupled (#158/#176).
    fn sync_viewport(&self, width: i32, height: i32, why: &str) {
        let widget = self.obj();
        let scale = paint_scale(&widget);
        let Some(shared) = self.shared.lock().clone() else {
            return;
        };
        let (old_size, old_scale, new_size) = {
            let mut s = shared.lock();
            let old = (s.size, s.scale_factor as f64);
            if width > 0 && height > 0 {
                s.size = (
                    (width as f64 * scale).round() as i32,
                    (height as f64 * scale).round() as i32,
                );
            }
            s.scale_factor = scale as f32;
            (old.0, old.1, s.size)
        };
        let scale_changed = (scale - old_scale).abs() > f64::EPSILON;
        log::trace!(
            "coord: {why} logical={width}x{height} scale={scale:.3} physical={}x{} old_scale={old_scale:.3} old_physical={}x{}",
            new_size.0,
            new_size.1,
            old_size.0,
            old_size.1
        );
        if new_size == old_size && !scale_changed {
            return;
        }
        // Lock released: the X11 query talks to the server.
        let origin = window_origin_for(&widget);
        shared.lock().window_origin = origin;
        if let Some(browser) = resolved_browser(self)
            && let Some(host) = browser.host()
        {
            host.notify_screen_info_changed();
            host.was_resized();
            if scale_changed {
                super::apply_zoom_from_account(&browser, scale);
            }
        }
    }

    pub fn run_js(&self, script: &str) {
        if let Some(browser) = self.browser.lock().as_ref()
            && let Some(frame) = browser.main_frame()
        {
            let code = CefString::from(script);
            let url = CefString::from("karere://notify");
            frame.execute_java_script(Some(&code), Some(&url), 0);
        }
    }

    pub fn reload(&self) {
        if let Some(browser) = self.browser.lock().as_ref() {
            browser.reload();
        }
    }

    /// Reload every account's browser (foreground + background) so each
    /// re-evaluates a changed load-time setting (e.g. `mobile-layout`) via `on_load_end`.
    pub fn reload_all(&self) {
        let browsers = self.browsers.lock();
        if browsers.is_empty() {
            if let Some(browser) = self.browser.lock().as_ref() {
                browser.reload();
            }
            return;
        }
        for browser in browsers.values() {
            browser.reload();
        }
    }

    /// Record window visibility (kept for callers; sound gating uses the JS hook now).
    pub fn set_window_visible(&self, visible: bool) {
        self.window_visible.store(visible, Ordering::Relaxed);
        // Gate the foreground browser's compositing on real window
        // visibility. Without this CEF keeps compositing WhatsApp's
        // animations while the window is hidden (tray/minimized/
        // start-in-background), pinning a core in software-GL OSR (#151).
        if let Some(b) = self.browser.lock().as_ref()
            && let Some(host) = b.host()
        {
            if visible {
                host.was_hidden(0);
                host.was_resized();
                // Window re-shown after prewarm/tray: stale seed may have
                // sized the buffer wrong (#176); ensure zoom tracks scale.
                super::apply_zoom_from_account(b, self.display_scale());
                host.invalidate(cef::PaintElementType::VIEW);
            } else {
                host.was_hidden(1);
            }
        }
        if visible {
            self.obj().queue_render();
        }
    }

    /// Apply the cursor, screen-reader and IM-focus requests the CEF
    /// handlers left in the shared state. Runs from the idle callback
    /// `handlers::wake_view` queues (`debounce_tick` false), and from the
    /// IM focus-out debounce timer (`debounce_tick` true), never inside a
    /// CEF callback or with the shared lock held across GTK calls.
    pub(super) fn flush_requests(&self, debounce_tick: bool) {
        let w = self.obj();
        let Some(shared) = self.shared.lock().clone() else {
            return;
        };
        let (dirty, cursor_name, keyboard_request, announce) = {
            let mut s = shared.lock();
            if !debounce_tick {
                s.view_wake_queued = false;
            }
            let dirty = s.frame.dirty || s.accel.as_ref().is_some_and(|a| a.dirty);
            let cursor = if s.cursor_dirty {
                s.cursor_dirty = false;
                Some(s.cursor_name)
            } else {
                None
            };
            (dirty, cursor, s.keyboard_request.take(), s.a11y_announce.take())
        };
        // Paints queue their own render; this only covers a frame still
        // waiting for one.
        if dirty {
            w.queue_render();
        }
        if let Some(name) = cursor_name {
            w.set_cursor_from_name(Some(name));
        }
        // Page focus moved under a screen reader (#204).
        if let Some(text) = announce {
            gtk::prelude::AccessibleExt::announce(
                &*w,
                &text,
                gtk::AccessibleAnnouncementPriority::Medium,
            );
        }
        // Editable focus changed in the page → focus the IM context so the
        // on-screen keyboard shows only for real text fields. Focus-out is
        // debounced so a composer re-render's false/true pair never leaves
        // the IM unfocused across the next keystroke (#180). The countdown
        // only advances on debounce ticks, so an unrelated flush (cursor,
        // announcement) cannot shorten it.
        if keyboard_request.is_none() && !debounce_tick {
            return;
        }
        let Some(im) = self.im_context.borrow().clone() else {
            return;
        };
        let mut pending = self.im_focus_out_pending.get();
        let action = im_focus_step(keyboard_request, self.im_focused.get(), &mut pending);
        self.im_focus_out_pending.set(pending);
        match action {
            ImFocusAction::FocusIn => {
                im.focus_in();
                self.im_focused.set(true);
            }
            ImFocusAction::FocusOut => {
                im.focus_out();
                self.im_focused.set(false);
            }
            ImFocusAction::Idle => {}
        }
        if pending > 0 {
            self.arm_im_focus_timer();
        }
    }

    /// Tick the IM focus-out debounce every `IM_FOCUS_TICK` until it has
    /// settled (applied or cancelled), then stop. At most one runs.
    fn arm_im_focus_timer(&self) {
        if self.im_focus_timer_armed.replace(true) {
            return;
        }
        let weak = self.obj().downgrade();
        glib::timeout_add_local(IM_FOCUS_TICK, move || {
            let Some(w) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let imp = w.imp();
            imp.flush_requests(true);
            if imp.im_focus_out_pending.get() > 0 {
                glib::ControlFlow::Continue
            } else {
                // Cleared before returning Break: the source is then gone,
                // so nothing may hold or remove its id.
                imp.im_focus_timer_armed.set(false);
                glib::ControlFlow::Break
            }
        });
    }

    pub fn reload_hard(&self) {
        if let Some(browser) = self.browser.lock().as_ref() {
            browser.reload_ignore_cache();
        }
    }

    // ---- find-in-page -------------------------------------------------

    pub fn find(&self, text: &str, forward: bool, find_next: bool) {
        let search = CefString::from(text);
        with_host(&self.obj(), |host| {
            host.find(Some(&search), forward as i32, 0, find_next as i32);
        });
    }

    pub fn stop_finding(&self) {
        with_host(&self.obj(), |host| host.stop_finding(1));
    }
}

pub(super) fn resolved_browser(imp: &KarereWebView) -> Option<Browser> {
    if let Some(b) = imp.browser.lock().as_ref() {
        return Some(b.clone());
    }
    if let Some(life) = imp.life_span.lock().as_ref() {
        return life.state.lock().browser.clone();
    }
    None
}

pub(super) fn with_resolved<T>(resolve: impl FnOnce() -> Option<T>, send: impl FnOnce(T)) {
    if let Some(target) = resolve() {
        send(target);
    }
}

pub(super) fn with_host<F: FnOnce(&cef::BrowserHost)>(widget: &super::KarereWebView, f: F) {
    with_resolved(
        || resolved_browser(widget.imp()).and_then(|browser| browser.host()),
        |host| f(&host),
    );
}

/// Scale (DIP→physical) the OSR paint buffer must use so it maps 1:1 onto the
/// `GtkGLArea` framebuffer. `GtkGLArea` always allocates that framebuffer at the
/// *integer* `gtk_widget_get_scale_factor()` (`width * scale`, `height * scale`)
/// — it cannot render at a fractional scale — so CEF MUST paint at the same
/// integer scale. Painting at the fractional surface scale (e.g. 1.5) leaves the
/// texture smaller than the framebuffer, and the fullscreen-quad blit upscales it
/// (GL_LINEAR), which is what blurred the web view under fractional scaling (#158).
///
/// At 150 % this paints at 2× into the 2× framebuffer (crisp), and the compositor
/// downscales the result to the 1.5× surface — a single high-quality downscale,
/// the same path every GTK GLArea takes on a fractional display. View rect and
/// mouse/wheel coords stay in DIP (CEF maps them via device_scale_factor). (#155, #158)
fn paint_scale(widget: &super::KarereWebView) -> f64 {
    widget.scale_factor().max(1) as f64
}
