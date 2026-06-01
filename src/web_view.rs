use gtk::glib;
use gtk::subclass::prelude::*;

glib::wrapper! {
    pub struct KarereWebView(ObjectSubclass<imp::KarereWebView>)
        @extends gtk::GLArea, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for KarereWebView {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl KarereWebView {
    pub fn new() -> Self {
        Self::default()
    }

    /// A view for the embedded DevTools frontend: uses a permissive client that
    /// keeps every navigation in-view instead of routing it to the browser.
    pub fn new_devtools() -> Self {
        let obj: Self = glib::Object::new();
        obj.imp().devtools.store(true, std::sync::atomic::Ordering::Relaxed);
        obj
    }

    pub fn load_url(&self, url: &str) {
        self.imp().load_url(url);
    }

    pub fn close_browser(&self) {
        self.imp().close_browser();
    }

    pub fn reload(&self) {
        self.imp().reload();
    }

    /// Reload bypassing the cache (hard refresh).
    pub fn reload_hard(&self) {
        self.imp().reload_hard();
    }

    pub fn find(&self, text: &str, forward: bool, find_next: bool) {
        self.imp().find(text, forward, find_next);
    }

    pub fn stop_finding(&self) {
        self.imp().stop_finding();
    }

    pub fn shared(&self) -> crate::handlers::SharedRef {
        self.imp().shared.lock().as_ref().unwrap().clone()
    }

    /// Run `script` in the page's main frame, if a browser is live. Used by the
    /// notification tracker to drive `__karereCloseNotif` / `__karereActivateNotif`.
    pub fn run_js(&self, script: &str) {
        self.imp().run_js(script);
    }

    /// Switch the active spellcheck language list on the live browser, without
    /// recreating it. Writes the Chromium `spellcheck.dictionaries` and
    /// `browser.enable_spellchecking` preferences on the browser's request
    /// context; Chromium downloads any missing `.bdic` dictionaries on demand.
    ///
    /// `langs` are BCP-47 codes (e.g. `["pt-BR"]`). An empty list with
    /// `enabled = true` lets Chromium keep its current/auto behaviour.
    pub fn set_spellcheck_languages(&self, langs: &[String], enabled: bool) {
        self.imp().set_spellcheck_languages(langs, enabled);
    }

    pub fn is_browser_closed(&self) -> bool {
        match self.imp().life_span.lock().as_ref() {
            Some(life) => life.state.lock().closed,
            // No life-span handler yet → no browser to wait on.
            None => true,
        }
    }
}

/// Apply the spellcheck language list to a live CEF browser by writing the
/// `browser.enable_spellchecking` and `spellcheck.dictionaries` preferences on
/// its request context. Shared by the headerbar dropdown (via
/// `KarereWebView::set_spellcheck_languages`) and the load handler (which
/// re-applies on every main-frame `on_load_end`, since the command-line switch
/// alone does not populate the dictionaries and the preference must be set after
/// the page — and its spellcheck service — is up).
///
/// Must run on the CEF UI thread (the glib main thread here).
/// Force Chromium to re-spellcheck the currently-focused editable. Changing
/// `spellcheck.dictionaries` only affects text typed AFTER the change — existing
/// text keeps its old-language markers — so toggling the focused element's
/// `spellcheck` property off→on makes Chromium drop and recompute the markers
/// with the new dictionary. Must run on the CEF UI thread.
fn force_spellcheck_recheck(browser: &cef::Browser) {
    use cef::{CefString, ImplBrowser, ImplFrame};
    let Some(frame) = browser.main_frame() else { return };
    let js = "(function(){var e=document.activeElement;if(!e)return;\
try{var t=e.closest&&e.closest('[contenteditable=\"true\"],input,textarea');var n=t||e;\
n.spellcheck=false;void n.offsetHeight;n.spellcheck=true;}catch(_){}})();";
    frame.execute_java_script(
        Some(&CefString::from(js)),
        Some(&CefString::from("karere://spellrecheck")),
        0,
    );
}

/// Write the `spellcheck.dictionaries` list preference on the browser's request
/// context. Returns false on failure. Must run on the CEF UI thread.
fn write_spellcheck_dictionaries(browser: &cef::Browser, langs: &[String]) -> bool {
    use cef::{
        CefString, ImplBrowser, ImplBrowserHost, ImplListValue, ImplPreferenceManager, ImplValue,
        list_value_create, value_create,
    };

    let Some(host) = browser.host() else { return false };
    let Some(ctx) = host.request_context() else { return false };
    let Some(mut list) = list_value_create() else {
        return false;
    };
    list.set_size(langs.len());
    for (i, lang) in langs.iter().enumerate() {
        let s = CefString::from(lang.as_str());
        list.set_string(i, Some(&s));
    }
    let Some(mut v) = value_create() else {
        return false;
    };
    v.set_list(Some(&mut list));
    let name = CefString::from("spellcheck.dictionaries");
    let mut err = CefString::from("");
    ctx.set_preference(Some(&name), Some(&mut v), Some(&mut err)) != 0
}

/// Apply the spellcheck enable flag + dictionary list to a live browser.
///
/// `force_clear` selects how the dictionary list is written:
/// - `true` (first apply of a page load): Chromium ignores a `Set` whose value
///   equals the persisted pref, so to make the initial language actually take
///   effect we force a real `[]`→`[lang]` transition (decoupled across a loop
///   tick so the renderer sees two distinct updates).
/// - `false` (a live language switch): the value genuinely differs, so set
///   `[lang]` DIRECTLY. The `[]` clear must NOT be used here — clearing to empty
///   after spellcheck is already running tears the service down in OSR and the
///   re-set does not revive it (spellcheck dies until restart).
pub(crate) fn apply_spellcheck_to_browser(
    browser: &cef::Browser,
    langs: &[String],
    enabled: bool,
    force_clear: bool,
) {
    use cef::ImplBrowser;
    use cef::{CefString, ImplBrowserHost, ImplPreferenceManager, ImplValue, value_create};

    let Some(host) = browser.host() else { return };
    let Some(ctx) = host.request_context() else {
        log::warn!("apply_spellcheck: no request context");
        return;
    };

    // browser.enable_spellchecking (boolean)
    if let Some(mut v) = value_create() {
        v.set_bool(enabled as i32);
        let name = CefString::from("browser.enable_spellchecking");
        let mut err = CefString::from("");
        if ctx.set_preference(Some(&name), Some(&mut v), Some(&mut err)) == 0 {
            log::warn!("set browser.enable_spellchecking failed: {}", err);
        }
    }

    if force_clear && !langs.is_empty() {
        // First apply: force the [] → [lang] transition across a loop tick.
        write_spellcheck_dictionaries(browser, &[]);
        log::info!("spellcheck.dictionaries = [] (force re-check)");
        let browser = browser.clone();
        let langs = langs.to_vec();
        glib::idle_add_local_once(move || {
            if write_spellcheck_dictionaries(&browser, &langs) {
                log::info!("spellcheck.dictionaries = {langs:?}");
            } else {
                log::warn!("set spellcheck.dictionaries failed");
            }
        });
        return;
    }

    // Live switch (or disable): write directly, no [] teardown.
    if write_spellcheck_dictionaries(browser, langs) {
        log::info!("spellcheck.dictionaries = {langs:?} (direct)");
    } else {
        log::warn!("set spellcheck.dictionaries failed");
    }
    if !langs.is_empty() {
        // Best-effort: nudge Chromium to re-scan existing text in the new
        // language (a pref change alone only affects newly-typed text). Deferred
        // so the new dictionary has propagated to the renderer first.
        let browser = browser.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
            force_spellcheck_recheck(&browser);
        });
    }
}

// ---- context-menu widget registry (main-thread only) ----------------------
//
// `ContextMenuHandler::run_context_menu` runs on the CEF UI thread, which under
// the external message pump IS the glib main thread, so this registry and the
// `RunContextMenuCallback` it carries never cross threads (deliberately NOT
// routed through the `Arc<Mutex>` `SharedRef`, since the callback is not `Send`).
// The map links a CEF browser id to the widget rendering it.
thread_local! {
    static CTX_MENU_WIDGETS: std::cell::RefCell<
        std::collections::HashMap<i32, glib::WeakRef<KarereWebView>>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

pub(crate) fn register_context_menu_widget(id: i32, view: &KarereWebView) {
    use glib::prelude::ObjectExt;
    CTX_MENU_WIDGETS.with(|m| {
        m.borrow_mut().insert(id, view.downgrade());
    });
}

pub(crate) fn unregister_context_menu_widget(id: i32) {
    CTX_MENU_WIDGETS.with(|m| {
        m.borrow_mut().remove(&id);
    });
}

/// Route a snapshotted context menu (from `ContextMenuHandler::run_context_menu`)
/// to the widget that owns `browser_id`. If the widget is gone, the callback is
/// cancelled so CEF never leaks the pending-menu state. Main-thread only.
pub fn dispatch_context_menu(
    browser_id: i32,
    items: Vec<crate::handlers::context_menu::MenuEntry>,
    x: i32,
    y: i32,
    callback: cef::RunContextMenuCallback,
) {
    let view = CTX_MENU_WIDGETS.with(|m| {
        m.borrow()
            .get(&browser_id)
            .and_then(|w| w.upgrade())
    });
    match view {
        Some(v) => v.imp().show_context_menu(items, x, y, callback),
        None => {
            use cef::ImplRunContextMenuCallback;
            callback.cancel();
        }
    }
}

mod imp {
    use cef::{
        self, Browser, BrowserSettings, CefString, EventFlags, ImplBrowser, ImplBrowserHost,
        ImplFrame, ImplRunContextMenuCallback, KeyEvent, KeyEventType, MouseButtonType, MouseEvent,
        RunContextMenuCallback, WindowInfo, browser_host_create_browser_sync, sys,
    };
    use std::cell::RefCell;
    use gl::types::{GLenum, GLint, GLuint};
    use glib::subclass::Signal;
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use once_cell::sync::Lazy;
    use parking_lot::Mutex;
    use std::ffi::{CString, c_void};
    
    use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

    use crate::handlers::{ClientBuilder, SharedRef, life_span::ShellLifeSpanHandler, new_shared};

    #[derive(Default)]
    pub struct KarereWebView {
        pub shared: Mutex<Option<SharedRef>>,
        pub browser: Mutex<Option<Browser>>,
        pub life_span: Mutex<Option<ShellLifeSpanHandler>>,
        pub pending_url: Mutex<Option<String>>,
        /// Set for the embedded DevTools view; selects the permissive client.
        pub devtools: AtomicBool,
        program: AtomicU32,
        vao: AtomicU32,
        vbo: AtomicU32,
        texture: AtomicU32,
        tex_w: AtomicI32,
        tex_h: AtomicI32,
        /// Last pointer position (logical px) so wheel events hit the element
        /// under the cursor instead of the top-left corner.
        last_mouse_x: AtomicI32,
        last_mouse_y: AtomicI32,
        /// CEF browser id, used to (de)register this widget in the context-menu
        /// registry. Set once the browser spawns; `0` before then.
        browser_id: AtomicI32,
        /// Pending OSR context-menu callback (non-`Send`, main-thread only). Held
        /// for the lifetime of the open menu and resolved exactly once in the
        /// popover `closed` handler: `cont(command)` if an item was activated,
        /// else `cancel()`. Dispatching from `closed` (after popdown) means the
        /// webview is re-focused before the command runs — required for
        /// spellcheck-replace and the edit commands to actually apply.
        pub pending_menu_callback: RefCell<Option<RunContextMenuCallback>>,
        /// Command id selected by item activation, consumed by the `closed`
        /// handler. `None` → the menu was dismissed without a selection.
        pub pending_menu_command: std::cell::Cell<Option<i32>>,
        /// The currently-displayed context popover, kept alive while open.
        pub context_popover: RefCell<Option<gtk::Popover>>,
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
            widget.set_has_depth_buffer(false);
            widget.set_has_stencil_buffer(false);
            widget.set_auto_render(false);

            let scale = widget.scale_factor() as f32;
            let shared = new_shared((1, 1), scale);
            *self.shared.lock() = Some(shared.clone());

            // CEF on_paint runs on glib main thread (external_message_pump
            // shoehorns CEF UI work onto our thread). Drive redraws via a
            // GtkWidget tick callback that polls the dirty flag.
            widget.add_tick_callback(move |w, _clock| {
                let imp = w.imp();
                let Some(shared) = imp.shared.lock().clone() else {
                    return glib::ControlFlow::Continue;
                };
                let cursor_name = {
                    let mut s = shared.lock();
                    if s.frame.dirty {
                        w.queue_render();
                    }
                    // Apply a pending CEF cursor change on the main thread.
                    if s.cursor_dirty {
                        s.cursor_dirty = false;
                        Some(s.cursor_name)
                    } else {
                        None
                    }
                };
                if let Some(name) = cursor_name {
                    w.set_cursor_from_name(Some(name));
                }
                glib::ControlFlow::Continue
            });

            widget.set_focusable(true);
            widget.set_can_focus(true);
            install_input_controllers(&widget);
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| vec![
                Signal::builder("title-changed")
                    .param_types([String::static_type()])
                    .build(),
            ]);
            SIGNALS.as_ref()
        }
    }

    impl WidgetImpl for KarereWebView {
        fn realize(&self) {
            self.parent_realize();
            let widget = self.obj();
            widget.make_current();
            if let Some(err) = widget.error() {
                log::error!("GLArea realize error: {err}");
                return;
            }
            unsafe {
                self.init_gl();
            }
            self.create_browser();
        }

        fn unrealize(&self) {
            self.close_browser();
            unsafe {
                self.teardown_gl();
            }
            self.parent_unrealize();
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
            let scale = self.obj().scale_factor();
            let phys_w = width * scale;
            let phys_h = height * scale;

            if let Some(shared) = self.shared.lock().as_ref() {
                let mut s = shared.lock();
                s.size = (phys_w, phys_h);
                s.scale_factor = scale as f32;
            }

            if let Some(browser) = resolved_browser(self)
                && let Some(host) = browser.host()
            {
                host.notify_screen_info_changed();
                host.was_resized();
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

        pub fn close_browser(&self) {
            // Cancel any in-flight OSR context menu so CEF never leaks the
            // pending-menu state when the widget goes away mid-menu, and drop the
            // popover.
            if let Some(cb) = self.pending_menu_callback.borrow_mut().take() {
                cb.cancel();
            }
            if let Some(pop) = self.context_popover.borrow_mut().take() {
                pop.unparent();
            }
            let id = self.browser_id.swap(0, Ordering::Relaxed);
            if id != 0 {
                super::unregister_context_menu_widget(id);
            }

            if let Some(browser) = self.browser.lock().as_ref()
                && let Some(host) = browser.host()
            {
                host.close_browser(0);
            }
        }

        /// Present the snapshotted CEF context menu as a GTK `Popover` of buttons
        /// anchored at the cursor over the `GLArea`. Main-thread only. Stores the
        /// callback in `pending_menu_callback`; a button click records the command
        /// and pops down, and the `closed` handler dispatches `cont`/`cancel` —
        /// exactly one fires. (A manual button popover is used instead of
        /// `PopoverMenu`+`gio::Menu`: the model menu's actions did not activate
        /// when parented to the OSR `GLArea`.)
        pub fn show_context_menu(
            &self,
            items: Vec<crate::handlers::context_menu::MenuEntry>,
            x_dev: i32,
            y_dev: i32,
            callback: RunContextMenuCallback,
        ) {
            // Replace any stale menu cleanly (shouldn't normally overlap).
            if let Some(old) = self.context_popover.borrow_mut().take() {
                old.unparent();
            }
            if let Some(cb) = self.pending_menu_callback.borrow_mut().take() {
                cb.cancel();
            }
            *self.pending_menu_callback.borrow_mut() = Some(callback);
            self.pending_menu_command.set(None);

            let obj = self.obj();
            let popover = gtk::Popover::new();
            popover.set_parent(&*obj);
            popover.set_has_arrow(false);
            popover.set_autohide(true);
            popover.set_position(gtk::PositionType::Bottom);
            popover.add_css_class("menu");

            let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
            build_menu_box(&items, &container, &obj, &popover);
            popover.set_child(Some(&container));

            // `run_context_menu` reports the cursor in view (device-pixel)
            // coordinates; GTK widget coordinates are logical, so divide by the
            // scale factor (inverse of the input-forwarding multiply).
            let s = scale(&obj);
            let rect = gtk::gdk::Rectangle::new(x_dev / s, y_dev / s, 1, 1);
            popover.set_pointing_to(Some(&rect));

            // Resolve the callback once, here, AFTER the popover is down so the
            // webview is focused again: dispatch the activated command, or cancel
            // if dismissed without a selection.
            popover.connect_closed(glib::clone!(
                #[weak] obj,
                move |pop| {
                    let imp = obj.imp();
                    let cmd = imp.pending_menu_command.take();
                    if let Some(cb) = imp.pending_menu_callback.borrow_mut().take() {
                        // Re-assert CEF focus on the view before running the
                        // command (spellcheck-replace / cut / paste act on the
                        // focused frame's selection).
                        set_focus(&obj, true);
                        match cmd {
                            Some(id) => {
                                log::debug!("context menu: cont(id={id})");
                                cb.cont(id, EventFlags::default());
                            }
                            None => cb.cancel(),
                        }
                    }
                    pop.unparent();
                    *imp.context_popover.borrow_mut() = None;
                }
            ));

            *self.context_popover.borrow_mut() = Some(popover.clone());
            popover.popup();
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

        /// Live spellcheck-language switch via request-context preferences.
        /// Must run on the CEF UI thread — the GTK callbacks that drive this are
        /// already on the glib main thread, which is where the external message
        /// pump runs CEF UI work, so direct calls are safe.
        pub fn set_spellcheck_languages(&self, langs: &[String], enabled: bool) {
            let Some(browser) = self.browser.lock().as_ref().cloned() else {
                log::warn!("set_spellcheck_languages: no live browser");
                return;
            };
            // Dropdown-driven live switch: never use the [] force-clear (it
            // tears down spellcheck in OSR); set the new language directly.
            super::apply_spellcheck_to_browser(&browser, langs, enabled, false);
        }

        pub fn reload(&self) {
            if let Some(browser) = self.browser.lock().as_ref() {
                browser.reload();
            }
        }

        pub fn reload_hard(&self) {
            if let Some(browser) = self.browser.lock().as_ref() {
                browser.reload_ignore_cache();
            }
        }

        fn create_browser(&self) {
            if self.browser.lock().is_some() {
                return;
            }
            let shared = self.shared.lock().as_ref().unwrap().clone();
            let (client, life) = if self.devtools.load(Ordering::Relaxed) {
                ClientBuilder::build_devtools_for(shared.clone())
            } else {
                ClientBuilder::build_for(shared.clone())
            };
            *self.life_span.lock() = Some(life);

            let window_info = WindowInfo {
                windowless_rendering_enabled: 1,
                ..Default::default()
            };
            let settings = BrowserSettings {
                windowless_frame_rate: 60,
                ..Default::default()
            };

            let url_string = self
                .pending_url
                .lock()
                .take()
                .unwrap_or_else(|| "about:blank".to_owned());
            let url = CefString::from(url_string.as_str());

            let mut client = client;
            let browser = browser_host_create_browser_sync(
                Some(&window_info),
                Some(&mut client),
                Some(&url),
                Some(&settings),
                None,
                None,
            );
            match browser {
                Some(b) => {
                    log::info!("browser spawned");
                    // Register in the context-menu widget registry so the OSR
                    // `run_context_menu` handler can reach this widget by id.
                    let id = b.identifier();
                    self.browser_id.store(id, Ordering::Relaxed);
                    super::register_context_menu_widget(id, &self.obj());
                    // Debug IPC verification: once the renderer has had time to
                    // build its V8 context, send a Ping and expect the browser
                    // log to show the matching Pong (handled in ClientBuilder).
                    #[cfg(debug_assertions)]
                    {
                        let browser = b.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_secs(2),
                            move || {
                                if let Some(frame) = browser.main_frame()
                                    && let Some(mut msg) =
                                        crate::ipc::BrowserMessage::Ping.to_cef_message()
                                {
                                    frame.send_process_message(
                                        cef::ProcessId::RENDERER,
                                        Some(&mut msg),
                                    );
                                    log::info!("IPC verify: Ping sent to renderer");
                                }
                            },
                        );
                    }
                    *self.browser.lock() = Some(b);
                }
                None => log::error!("browser_host_create_browser_sync returned None"),
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

        // ---- GL ------------------------------------------------------------

        unsafe fn init_gl(&self) {
            let vsrc = CString::new(VS).unwrap();
            let fsrc = CString::new(FS).unwrap();
            let program = unsafe { compile_program(&vsrc, &fsrc) };
            self.program.store(program, Ordering::Relaxed);

            // fullscreen quad (pos.xy, uv.xy) — y-flipped so BGRA top-left
            // origin from CEF appears correctly.
            let verts: [f32; 16] = [
                -1.0, -1.0, 0.0, 1.0, //
                1.0, -1.0, 1.0, 1.0, //
                -1.0, 1.0, 0.0, 0.0, //
                1.0, 1.0, 1.0, 0.0, //
            ];

            let mut vao = 0;
            let mut vbo = 0;
            unsafe {
                gl::GenVertexArrays(1, &mut vao);
                gl::GenBuffers(1, &mut vbo);
                gl::BindVertexArray(vao);
                gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
                gl::BufferData(
                    gl::ARRAY_BUFFER,
                    (verts.len() * std::mem::size_of::<f32>()) as isize,
                    verts.as_ptr() as *const c_void,
                    gl::STATIC_DRAW,
                );
                let stride = (4 * std::mem::size_of::<f32>()) as i32;
                gl::EnableVertexAttribArray(0);
                gl::VertexAttribPointer(0, 2, gl::FLOAT, gl::FALSE, stride, std::ptr::null());
                gl::EnableVertexAttribArray(1);
                gl::VertexAttribPointer(
                    1,
                    2,
                    gl::FLOAT,
                    gl::FALSE,
                    stride,
                    (2 * std::mem::size_of::<f32>()) as *const c_void,
                );
                gl::BindBuffer(gl::ARRAY_BUFFER, 0);
                gl::BindVertexArray(0);
            }
            self.vao.store(vao, Ordering::Relaxed);
            self.vbo.store(vbo, Ordering::Relaxed);

            let mut tex = 0;
            unsafe {
                gl::GenTextures(1, &mut tex);
                gl::BindTexture(gl::TEXTURE_2D, tex);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
                gl::TexParameteri(
                    gl::TEXTURE_2D,
                    gl::TEXTURE_WRAP_S,
                    gl::CLAMP_TO_EDGE as GLint,
                );
                gl::TexParameteri(
                    gl::TEXTURE_2D,
                    gl::TEXTURE_WRAP_T,
                    gl::CLAMP_TO_EDGE as GLint,
                );
                gl::BindTexture(gl::TEXTURE_2D, 0);
            }
            self.texture.store(tex, Ordering::Relaxed);
            self.tex_w.store(0, Ordering::Relaxed);
            self.tex_h.store(0, Ordering::Relaxed);
        }

        unsafe fn teardown_gl(&self) {
            unsafe {
                let tex = self.texture.load(Ordering::Relaxed);
                if tex != 0 {
                    gl::DeleteTextures(1, &tex);
                    self.texture.store(0, Ordering::Relaxed);
                }
                let vbo = self.vbo.load(Ordering::Relaxed);
                if vbo != 0 {
                    gl::DeleteBuffers(1, &vbo);
                    self.vbo.store(0, Ordering::Relaxed);
                }
                let vao = self.vao.load(Ordering::Relaxed);
                if vao != 0 {
                    gl::DeleteVertexArrays(1, &vao);
                    self.vao.store(0, Ordering::Relaxed);
                }
                let p = self.program.load(Ordering::Relaxed);
                if p != 0 {
                    gl::DeleteProgram(p);
                    self.program.store(0, Ordering::Relaxed);
                }
            }
        }

        unsafe fn draw(&self) {
            let shared = match self.shared.lock().as_ref() {
                Some(s) => s.clone(),
                None => return,
            };
            let mut s = shared.lock();

            unsafe {
                gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);

                if s.frame.width == 0 || s.frame.height == 0 || s.frame.pixels.is_empty() {
                    return;
                }

                let tex = self.texture.load(Ordering::Relaxed);
                gl::BindTexture(gl::TEXTURE_2D, tex);
                let tw = self.tex_w.load(Ordering::Relaxed);
                let th = self.tex_h.load(Ordering::Relaxed);
                if (tw, th) != (s.frame.width, s.frame.height) {
                    gl::TexImage2D(
                        gl::TEXTURE_2D,
                        0,
                        gl::RGBA8 as GLint,
                        s.frame.width,
                        s.frame.height,
                        0,
                        gl::RGBA,
                        gl::UNSIGNED_BYTE,
                        s.frame.pixels.as_ptr() as *const c_void,
                    );
                    self.tex_w.store(s.frame.width, Ordering::Relaxed);
                    self.tex_h.store(s.frame.height, Ordering::Relaxed);
                    s.frame.dirty = false;
                } else if s.frame.dirty {
                    gl::TexSubImage2D(
                        gl::TEXTURE_2D,
                        0,
                        0,
                        0,
                        s.frame.width,
                        s.frame.height,
                        gl::RGBA,
                        gl::UNSIGNED_BYTE,
                        s.frame.pixels.as_ptr() as *const c_void,
                    );
                    s.frame.dirty = false;
                }
                drop(s);

                gl::UseProgram(self.program.load(Ordering::Relaxed));
                gl::BindVertexArray(self.vao.load(Ordering::Relaxed));
                gl::ActiveTexture(gl::TEXTURE0);
                gl::BindTexture(gl::TEXTURE_2D, tex);
                gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);
                gl::BindVertexArray(0);
                gl::UseProgram(0);
            }
        }
    }

    fn install_input_controllers(widget: &super::KarereWebView) {
        use gtk::gdk;

        // Mouse motion -------------------------------------------------------
        let motion = gtk::EventControllerMotion::new();
        motion.connect_motion(glib::clone!(
            #[weak] widget,
            move |ctrl, x, y| {
                let modifiers = modifiers_from_state(ctrl.current_event_state());
                send_move(&widget, x, y, modifiers, false);
            }
        ));
        motion.connect_leave(glib::clone!(
            #[weak] widget,
            move |ctrl| {
                let modifiers = modifiers_from_state(ctrl.current_event_state());
                send_move(&widget, 0.0, 0.0, modifiers, true);
            }
        ));
        widget.add_controller(motion);

        // Mouse buttons ------------------------------------------------------
        for button in 1..=3 {
            let click = gtk::GestureClick::builder().button(button).build();
            click.connect_pressed(glib::clone!(
                #[weak] widget,
                move |gesture, n_press, x, y| {
                    widget.grab_focus();
                    // Tell CEF it is focused on every click, not only via the
                    // EventControllerFocus enter signal. On launch the GLArea may
                    // already hold GTK focus, so `grab_focus` is a no-op and the
                    // enter signal never fires — leaving CEF unfocused, so the
                    // input caret/indicator never shows until the window is
                    // de-focused and re-focused.
                    set_focus(&widget, true);
                    let modifiers = modifiers_from_state(gesture.current_event_state());
                    send_click(&widget, x, y, button, true, n_press, modifiers);
                    // M17: middle-click also pastes the primary selection. The
                    // click is NOT claimed, so CEF still receives the middle
                    // button (preserving middle-click-to-open-link).
                    if button == 2 {
                        read_primary_clipboard_paste(&widget);
                    }
                }
            ));
            click.connect_released(glib::clone!(
                #[weak] widget,
                move |gesture, n_press, x, y| {
                    let modifiers = modifiers_from_state(gesture.current_event_state());
                    send_click(&widget, x, y, button, false, n_press, modifiers);
                }
            ));
            widget.add_controller(click);
        }

        // Scroll -------------------------------------------------------------
        let scroll = gtk::EventControllerScroll::new(
            gtk::EventControllerScrollFlags::BOTH_AXES,
        );
        scroll.connect_scroll(glib::clone!(
            #[weak] widget,
            #[upgrade_or] glib::Propagation::Proceed,
            move |ctrl, dx, dy| {
                let modifiers = modifiers_from_state(ctrl.current_event_state());
                send_wheel(&widget, dx, dy, modifiers);
                glib::Propagation::Stop
            }
        ));
        widget.add_controller(scroll);

        // Keyboard -----------------------------------------------------------
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak] widget,
            #[upgrade_or] glib::Propagation::Proceed,
            move |_ctrl, keyval, keycode, state| {
                use gtk::gdk::{Key, ModifierType};
                // M17: intercept Ctrl+V before CEF. If the GDK clipboard holds an
                // image or files, synthesize the paste ourselves and swallow the
                // key so CEF's GDK-blind native paste does not also fire. A
                // text-only / empty clipboard falls through to CEF's handler.
                if state.contains(ModifierType::CONTROL_MASK)
                    && !state.intersects(ModifierType::ALT_MASK | ModifierType::SUPER_MASK)
                    && matches!(keyval, Key::v | Key::V)
                    && try_intercept_paste(&widget)
                {
                    return glib::Propagation::Stop;
                }
                send_key(&widget, keyval, keycode, state, true);
                // Let accelerator-style combos bubble to window/app shortcuts
                // (Ctrl/Alt/Super + key, or F5/F11); consume everything else so
                // plain typing/navigation stays inside the webview.
                if is_accelerator_key(keyval, state) {
                    glib::Propagation::Proceed
                } else {
                    glib::Propagation::Stop
                }
            }
        ));
        keys.connect_key_released(glib::clone!(
            #[weak] widget,
            move |_ctrl, keyval, keycode, state| {
                send_key(&widget, keyval, keycode, state, false);
            }
        ));
        widget.add_controller(keys);

        // Focus --------------------------------------------------------------
        let focus = gtk::EventControllerFocus::new();
        focus.connect_enter(glib::clone!(
            #[weak] widget,
            move |_| set_focus(&widget, true)
        ));
        focus.connect_leave(glib::clone!(
            #[weak] widget,
            move |_| set_focus(&widget, false)
        ));
        widget.add_controller(focus);

        // Drag-drop ----------------------------------------------------------
        // M17: accept file drops and surface each as a synthetic `drop` event on
        // the element under the cursor (paste_bridge.js targets it via the drop
        // coordinates carried in the envelope).
        let drop_target = gtk::DropTarget::new(
            gdk::FileList::static_type(),
            gdk::DragAction::COPY,
        );
        drop_target.connect_drop(glib::clone!(
            #[weak] widget,
            #[upgrade_or] false,
            move |_target, value, x, y| {
                match value.get::<gdk::FileList>() {
                    Ok(list) => {
                        for file in list.files() {
                            load_and_send_file(&widget, file, "drop", Some((x, y)));
                        }
                        true
                    }
                    Err(_) => false,
                }
            }
        ));
        widget.add_controller(drop_target);

        let _ = gdk::ModifierType::SHIFT_MASK;  // suppress unused import warning shape
    }

    fn is_accelerator_key(keyval: gtk::gdk::Key, state: gtk::gdk::ModifierType) -> bool {
        use gtk::gdk::{Key, ModifierType};
        if state.intersects(
            ModifierType::CONTROL_MASK | ModifierType::ALT_MASK | ModifierType::SUPER_MASK,
        ) {
            return true;
        }
        matches!(keyval, Key::F5 | Key::F11)
    }

    fn modifiers_from_state(state: gtk::gdk::ModifierType) -> u32 {
        use gtk::gdk::ModifierType;
        use sys::cef_event_flags_t as F;
        let mut m = 0u32;
        if state.contains(ModifierType::SHIFT_MASK) { m |= F::EVENTFLAG_SHIFT_DOWN.0; }
        if state.contains(ModifierType::CONTROL_MASK) { m |= F::EVENTFLAG_CONTROL_DOWN.0; }
        if state.contains(ModifierType::ALT_MASK) { m |= F::EVENTFLAG_ALT_DOWN.0; }
        if state.contains(ModifierType::SUPER_MASK) { m |= F::EVENTFLAG_COMMAND_DOWN.0; }
        if state.contains(ModifierType::BUTTON1_MASK) { m |= F::EVENTFLAG_LEFT_MOUSE_BUTTON.0; }
        if state.contains(ModifierType::BUTTON2_MASK) { m |= F::EVENTFLAG_MIDDLE_MOUSE_BUTTON.0; }
        if state.contains(ModifierType::BUTTON3_MASK) { m |= F::EVENTFLAG_RIGHT_MOUSE_BUTTON.0; }
        m
    }

    /// Populate `container` with one widget per [`MenuEntry`] snapshot entry:
    /// flat `gtk::Button`s for commands (click → record command id + popdown),
    /// `gtk::Separator`s between sections, and dim heading labels for submenus
    /// (flattened inline — the real menus are flat). Disabled items are
    /// insensitive.
    fn build_menu_box(
        entries: &[crate::handlers::context_menu::MenuEntry],
        container: &gtk::Box,
        obj: &super::KarereWebView,
        popover: &gtk::Popover,
    ) {
        use crate::handlers::context_menu::MenuEntry;
        use gtk::prelude::*;

        for e in entries {
            match e {
                MenuEntry::Separator => {
                    container.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
                }
                MenuEntry::Submenu { label, items } => {
                    let heading = gtk::Label::new(Some(label));
                    heading.set_xalign(0.0);
                    heading.add_css_class("dim-label");
                    container.append(&heading);
                    build_menu_box(items, container, obj, popover);
                }
                MenuEntry::Item {
                    label,
                    command_id,
                    enabled,
                } => {
                    let btn = gtk::Button::with_label(label);
                    btn.set_has_frame(false);
                    btn.add_css_class("flat");
                    btn.set_sensitive(*enabled);
                    if let Some(lbl) = btn.child().and_downcast::<gtk::Label>() {
                        lbl.set_xalign(0.0);
                    }
                    let cmd = *command_id;
                    btn.connect_clicked(glib::clone!(
                        #[weak] obj,
                        #[weak] popover,
                        move |_| {
                            log::debug!("context menu: item activated id={cmd}");
                            // Record the choice; the popover `closed` handler runs
                            // `cont` once it is down and the view is re-focused.
                            obj.imp().pending_menu_command.set(Some(cmd));
                            popover.popdown();
                        }
                    ));
                    container.append(&btn);
                }
            }
        }
    }

    /// The browser driving this view: a `Page` view stores it directly, a
    /// `DevTools` view receives it asynchronously via its life-span handler.
    fn resolved_browser(imp: &KarereWebView) -> Option<Browser> {
        if let Some(b) = imp.browser.lock().as_ref() {
            return Some(b.clone());
        }
        if let Some(life) = imp.life_span.lock().as_ref() {
            return life.state.lock().browser.clone();
        }
        None
    }

    fn with_host<F: FnOnce(&cef::BrowserHost)>(widget: &super::KarereWebView, f: F) {
        if let Some(b) = resolved_browser(widget.imp())
            && let Some(host) = b.host()
        {
            f(&host);
        }
    }

    fn scale(widget: &super::KarereWebView) -> i32 {
        widget.scale_factor().max(1)
    }

    fn send_move(widget: &super::KarereWebView, x: f64, y: f64, modifiers: u32, leave: bool) {
        let s = scale(widget);
        if !leave {
            // Remember the cursor so wheel events scroll the element under it.
            widget.imp().last_mouse_x.store(x as i32, Ordering::Relaxed);
            widget.imp().last_mouse_y.store(y as i32, Ordering::Relaxed);
        }
        let event = MouseEvent {
            x: (x as i32) * s,
            y: (y as i32) * s,
            modifiers,
        };
        with_host(widget, |host| {
            host.send_mouse_move_event(Some(&event), leave as i32);
        });
    }

    fn send_click(
        widget: &super::KarereWebView,
        x: f64,
        y: f64,
        button: u32,
        down: bool,
        n_press: i32,
        modifiers: u32,
    ) {
        let s = scale(widget);
        let event = MouseEvent {
            x: (x as i32) * s,
            y: (y as i32) * s,
            modifiers,
        };
        let btn = match button {
            1 => MouseButtonType::LEFT,
            2 => MouseButtonType::MIDDLE,
            3 => MouseButtonType::RIGHT,
            _ => return,
        };
        with_host(widget, |host| {
            host.send_mouse_click_event(Some(&event), btn, (!down) as i32, n_press.max(1));
        });
    }

    fn send_wheel(widget: &super::KarereWebView, dx: f64, dy: f64, modifiers: u32) {
        let s = scale(widget);
        // GTK scroll deltas are in "wheel ticks" (1.0 = one notch). CEF expects
        // pixel deltas; multiply by a standard step.
        const STEP: f64 = 40.0;
        // CEF needs the cursor position to hit-test which element scrolls;
        // (0,0) targets the top-left corner and usually scrolls nothing.
        let imp = widget.imp();
        let event = MouseEvent {
            x: imp.last_mouse_x.load(Ordering::Relaxed) * s,
            y: imp.last_mouse_y.load(Ordering::Relaxed) * s,
            modifiers,
        };
        with_host(widget, |host| {
            host.send_mouse_wheel_event(
                Some(&event),
                (-dx * STEP) as i32 * s,
                (-dy * STEP) as i32 * s,
            );
        });
    }

    fn send_key(
        widget: &super::KarereWebView,
        keyval: gtk::gdk::Key,
        keycode: u32,
        state: gtk::gdk::ModifierType,
        down: bool,
    ) {
        let modifiers = modifiers_from_state(state);
        let windows_key_code = gdk_key_to_vk(keyval);
        let character = keyval.to_unicode().map(|c| c as u16).unwrap_or(0);

        let base = KeyEvent {
            size: std::mem::size_of::<sys::_cef_key_event_t>(),
            type_: if down { KeyEventType::RAWKEYDOWN } else { KeyEventType::KEYUP },
            modifiers,
            windows_key_code,
            native_key_code: keycode as i32,
            is_system_key: 0,
            character,
            unmodified_character: character,
            focus_on_editable_field: 0,
        };
        with_host(widget, |host| {
            host.send_key_event(Some(&base));
            if down && character != 0 {
                let char_evt = KeyEvent {
                    type_: KeyEventType::CHAR,
                    windows_key_code: character as i32,
                    ..base.clone()
                };
                host.send_key_event(Some(&char_evt));
            }
        });
    }

    fn set_focus(widget: &super::KarereWebView, focused: bool) {
        with_host(widget, |host| host.set_focus(focused as i32));
    }

    // ---- M17 paste / drop bridge ---------------------------------------

    /// Send a [`crate::ipc::BrowserMessage`] to the renderer's main frame.
    fn send_browser_message(widget: &super::KarereWebView, msg: crate::ipc::BrowserMessage) {
        if let Some(browser) = resolved_browser(widget.imp())
            && let Some(frame) = browser.main_frame()
            && let Some(mut m) = msg.to_cef_message()
        {
            frame.send_process_message(cef::ProcessId::RENDERER, Some(&mut m));
        }
    }

    /// Marshal a binary clipboard/drop payload into a `DispatchPasteEvent`
    /// (`kind` is `"paste"` or `"drop"`). Large payloads round-trip via a
    /// scoped tempfile (see [`crate::paste::make_blob`]).
    fn send_blob_paste(
        widget: &super::KarereWebView,
        mime: String,
        kind: &str,
        bytes: &[u8],
        name: Option<String>,
        coords: Option<(f64, f64)>,
    ) {
        let payload = crate::paste::make_blob(bytes);
        let (x, y) = match coords {
            Some((x, y)) => (Some(x), Some(y)),
            None => (None, None),
        };
        send_browser_message(
            widget,
            crate::ipc::BrowserMessage::DispatchPasteEvent {
                mime,
                kind: kind.to_string(),
                payload,
                name,
                x,
                y,
            },
        );
    }

    /// Marshal primary-clipboard text (middle-click) as a `text/plain` paste.
    fn send_text_paste(widget: &super::KarereWebView, text: &str) {
        send_browser_message(
            widget,
            crate::ipc::BrowserMessage::DispatchPasteEvent {
                mime: "text/plain".to_string(),
                kind: "paste".to_string(),
                payload: crate::ipc::PasteBlob::Base64(crate::paste::b64(text.as_bytes())),
                name: None,
                x: None,
                y: None,
            },
        );
    }

    /// On Ctrl+V, inspect the GDK clipboard. If it holds an image or files, kick
    /// off an async read that dispatches a synthetic paste and return `true` so
    /// the caller swallows the keystroke (CEF's GDK-blind native paste must not
    /// also fire). Text-only / empty clipboards return `false` to fall through.
    fn try_intercept_paste(widget: &super::KarereWebView) -> bool {
        let Some(display) = gtk::gdk::Display::default() else {
            return false;
        };
        let clipboard = display.clipboard();
        let formats = clipboard.formats();

        let has_image = formats.contains_type(gtk::gdk::Texture::static_type())
            || formats.contain_mime_type("image/png")
            || formats.contain_mime_type("image/jpeg")
            || formats.contain_mime_type("image/gif")
            || formats.contain_mime_type("image/webp");
        if has_image {
            read_clipboard_image(widget, &clipboard);
            return true;
        }

        let has_files = formats.contains_type(gtk::gdk::FileList::static_type())
            || formats.contain_mime_type("text/uri-list");
        if has_files {
            read_clipboard_files(widget, &clipboard);
            return true;
        }

        // CEF's offscreen (windowless) clipboard does not consult GDK, so its
        // native Ctrl+V pastes nothing. Read GDK clipboard text ourselves and
        // synthesize the paste, same as the image/file paths.
        let has_text = formats.contains_type(glib::Type::STRING)
            || formats.contain_mime_type("text/plain")
            || formats.contain_mime_type("text/plain;charset=utf-8")
            || formats.contain_mime_type("UTF8_STRING");
        if has_text {
            read_clipboard_text(widget, &clipboard);
            return true;
        }

        false
    }

    fn read_clipboard_text(widget: &super::KarereWebView, clipboard: &gtk::gdk::Clipboard) {
        clipboard.read_text_async(
            gtk::gio::Cancellable::NONE,
            glib::clone!(
                #[weak] widget,
                move |res| match res {
                    Ok(Some(text)) if !text.is_empty() => send_text_paste(&widget, text.as_str()),
                    Ok(_) => {}
                    Err(err) => log::warn!("clipboard text read failed: {err}"),
                }
            ),
        );
    }

    fn read_clipboard_image(widget: &super::KarereWebView, clipboard: &gtk::gdk::Clipboard) {
        clipboard.read_texture_async(
            gtk::gio::Cancellable::NONE,
            glib::clone!(
                #[weak] widget,
                move |res| match res {
                    Ok(Some(texture)) => {
                        let bytes = texture.save_to_png_bytes();
                        send_blob_paste(
                            &widget,
                            "image/png".to_string(),
                            "paste",
                            &bytes,
                            None,
                            None,
                        );
                    }
                    Ok(None) => log::debug!("clipboard image read: empty"),
                    Err(err) => log::warn!("clipboard image read failed: {err}"),
                }
            ),
        );
    }

    fn read_clipboard_files(widget: &super::KarereWebView, clipboard: &gtk::gdk::Clipboard) {
        clipboard.read_value_async(
            gtk::gdk::FileList::static_type(),
            glib::Priority::DEFAULT,
            gtk::gio::Cancellable::NONE,
            glib::clone!(
                #[weak] widget,
                move |res| match res {
                    Ok(value) => match value.get::<gtk::gdk::FileList>() {
                        Ok(list) => {
                            for file in list.files() {
                                load_and_send_file(&widget, file, "paste", None);
                            }
                        }
                        Err(err) => log::warn!("clipboard file list decode failed: {err}"),
                    },
                    Err(err) => log::warn!("clipboard file read failed: {err}"),
                }
            ),
        );
    }

    /// Load one file's contents asynchronously and dispatch it as a paste/drop.
    fn load_and_send_file(
        widget: &super::KarereWebView,
        file: gtk::gio::File,
        kind: &'static str,
        coords: Option<(f64, f64)>,
    ) {
        let name = file.basename().map(|p| p.to_string_lossy().into_owned());
        file.load_contents_async(
            gtk::gio::Cancellable::NONE,
            glib::clone!(
                #[weak] widget,
                move |res| match res {
                    Ok((bytes, _etag)) => {
                        let mime = guess_mime(name.as_deref(), &bytes);
                        send_blob_paste(&widget, mime, kind, &bytes, name, coords);
                    }
                    Err(err) => log::warn!("read dropped/pasted file failed: {err}"),
                }
            ),
        );
    }

    /// Best-effort MIME from filename + leading bytes; defaults to octet-stream.
    fn guess_mime(name: Option<&str>, bytes: &[u8]) -> String {
        let (content_type, _certain) = gtk::gio::content_type_guess(name, bytes);
        gtk::gio::content_type_get_mime_type(content_type.as_str())
            .map(|m| m.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string())
    }

    /// Middle-click: paste primary-clipboard text. Empty selection sends nothing.
    fn read_primary_clipboard_paste(widget: &super::KarereWebView) {
        let Some(display) = gtk::gdk::Display::default() else {
            return;
        };
        display.primary_clipboard().read_text_async(
            gtk::gio::Cancellable::NONE,
            glib::clone!(
                #[weak] widget,
                move |res| match res {
                    Ok(Some(text)) if !text.is_empty() => send_text_paste(&widget, text.as_str()),
                    Ok(_) => {}
                    Err(err) => log::debug!("primary clipboard read failed: {err}"),
                }
            ),
        );
    }

    fn gdk_key_to_vk(keyval: gtk::gdk::Key) -> i32 {
        use gtk::gdk::Key;
        // Just the common keys. Anything else falls back to the unicode value
        // CEF receives via CHAR events.
        match keyval {
            Key::BackSpace => 0x08,
            Key::Tab => 0x09,
            Key::Return | Key::KP_Enter => 0x0D,
            Key::Escape => 0x1B,
            Key::Page_Up => 0x21,
            Key::Page_Down => 0x22,
            Key::End => 0x23,
            Key::Home => 0x24,
            Key::Left => 0x25,
            Key::Up => 0x26,
            Key::Right => 0x27,
            Key::Down => 0x28,
            Key::Insert => 0x2D,
            Key::Delete => 0x2E,
            _ => keyval.to_unicode().map(|c| c.to_ascii_uppercase() as i32).unwrap_or(0),
        }
    }

    const VS: &str = r#"#version 300 es
precision highp float;
layout(location=0) in vec2 a_pos;
layout(location=1) in vec2 a_uv;
out vec2 v_uv;
void main() {
    v_uv = a_uv;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}
"#;

    const FS: &str = r#"#version 300 es
precision highp float;
in vec2 v_uv;
out vec4 frag;
uniform sampler2D u_tex;
void main() {
    frag = texture(u_tex, v_uv).bgra;
}
"#;

    unsafe fn compile_program(vsrc: &CString, fsrc: &CString) -> GLuint {
        unsafe {
            let vs = compile_shader(gl::VERTEX_SHADER, vsrc);
            let fs = compile_shader(gl::FRAGMENT_SHADER, fsrc);
            let p = gl::CreateProgram();
            gl::AttachShader(p, vs);
            gl::AttachShader(p, fs);
            gl::LinkProgram(p);
            let mut ok = 0;
            gl::GetProgramiv(p, gl::LINK_STATUS, &mut ok);
            if ok == 0 {
                let mut log_len = 0;
                gl::GetProgramiv(p, gl::INFO_LOG_LENGTH, &mut log_len);
                let mut buf = vec![0u8; log_len as usize];
                gl::GetProgramInfoLog(
                    p,
                    log_len,
                    std::ptr::null_mut(),
                    buf.as_mut_ptr() as *mut i8,
                );
                log::error!("program link: {}", String::from_utf8_lossy(&buf));
            }
            gl::DeleteShader(vs);
            gl::DeleteShader(fs);
            p
        }
    }

    unsafe fn compile_shader(kind: GLenum, src: &CString) -> GLuint {
        unsafe {
            let s = gl::CreateShader(kind);
            let p = src.as_ptr();
            gl::ShaderSource(s, 1, &p, std::ptr::null());
            gl::CompileShader(s);
            let mut ok = 0;
            gl::GetShaderiv(s, gl::COMPILE_STATUS, &mut ok);
            if ok == 0 {
                let mut log_len = 0;
                gl::GetShaderiv(s, gl::INFO_LOG_LENGTH, &mut log_len);
                let mut buf = vec![0u8; log_len as usize];
                gl::GetShaderInfoLog(s, log_len, std::ptr::null_mut(), buf.as_mut_ptr() as *mut i8);
                log::error!("shader compile: {}", String::from_utf8_lossy(&buf));
            }
            s
        }
    }
}
