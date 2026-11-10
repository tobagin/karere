pub mod accessibility;
pub mod client;
pub mod context_menu;
pub mod display;
pub mod download;
pub mod find;
pub mod life_span;
pub mod load;
pub mod permission;
pub mod render;
pub mod render_process;
pub mod request;

use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

pub use client::ClientBuilder;
pub use context_menu::ShellContextMenuHandlerBuilder;
pub use render::FrameBuffer;
pub use request::{ShellRequestHandler, ShellRequestHandlerBuilder};

/// Request to surface the "web view keeps crashing" dialog; drained by the
/// crash poll loop.
#[derive(Clone)]
pub struct CrashDialog {
    pub title: String,
    pub body: String,
}

/// Latest `FindHandler::on_find_result` payload; drained by the poll loop for
/// the "active of count" search counter.
#[derive(Clone, Copy, Default)]
pub struct FindResult {
    pub count: i32,
    pub active: i32,
}

/// A finished download, drained by the poll loop to raise a completion toast.
#[derive(Clone)]
pub struct DownloadCompleted {
    pub path: PathBuf,
    pub name: String,
}

/// A failed (canceled/errored) download; drained by the poll loop for a failure
/// dialog.
#[derive(Clone)]
pub struct DownloadFailed {
    pub name: String,
    pub reason: String,
}

/// State shared between the CEF handlers (CEF UI thread, which is the glib
/// main thread under external_message_pump) and the GTK widget.
#[derive(Default)]
pub struct SharedState {
    pub frame: FrameBuffer,
    /// Latest GPU-accelerated frame (DMA-BUF) from `on_accelerated_paint`, when
    /// shared-texture OSR is enabled; imported to a GL texture in `draw` and
    /// kept (with `dirty` cleared) while it is the frame on screen. Cleared by a
    /// CPU `on_paint`, so a mid-session fallback to CPU frames is not hidden
    /// behind the last DMA-BUF. (gpu-osr)
    pub accel: Option<crate::gl_dmabuf::AccelFrame>,
    /// Logical (DIP) viewport size — CEF's GetViewRect and GetScreenInfo rect.
    /// With non-empty screen rects Chromium honours device_scale_factor, so the
    /// physical paint buffer = this × `scale_factor` = the GLArea framebuffer
    /// (1:1, crisp). (#155, #158)
    pub size: (i32, i32),
    /// Integer paint scale (DIP→physical) matching the `GtkGLArea` framebuffer,
    /// e.g. 2 at 150 % fractional scaling. Fed to CEF as device_scale_factor so the
    /// paint buffer maps 1:1 onto the framebuffer (a fractional value would be
    /// upscaled by the blit and blur the web view). (#155, #158)
    pub scale_factor: f32,
    pub title: String,
    pub is_loading: bool,
    /// Pending toast text published by the crash handler; drained by the poll loop.
    pub crash_toast: Option<String>,
    /// Renderer-termination timestamps inside the active 60 s window.
    pub crash_history: Vec<Instant>,
    /// Set once the crash storm threshold is crossed; drained by the poll loop.
    pub crash_dialog_request: Option<CrashDialog>,
    /// In-flight load-error retry timer, cancelled on successful load.
    pub pending_reload: Option<glib::SourceId>,
    /// Consecutive load failures driving the retry backoff.
    pub load_error_count: u32,
    /// Main-frame load currently failing — drives the offline overlay.
    pub offline: bool,
    /// Latest find-in-page result; drained by the poll loop to update the counter.
    pub find_result: Option<FindResult>,
    /// Completed downloads awaiting a completion toast; drained by the poll loop.
    pub downloads_completed: Vec<DownloadCompleted>,
    /// Failed downloads awaiting a failure dialog; drained by the poll loop.
    pub downloads_failed: Vec<DownloadFailed>,
    /// Last (enabled, languages) applied via the editable-focus trigger, keyed
    /// by CEF browser id: spellcheck prefs live on each account's own
    /// RequestContext, so one browser's apply says nothing about another's.
    /// `None` (reset on any main-frame load start) makes every browser apply
    /// again on its next editable focus. Focus re-applies only when resolved
    /// settings differ, so a live language switch re-checks on returning focus
    /// to the composer.
    pub spellcheck_last: Option<std::collections::HashMap<i32, (bool, Vec<String>)>>,
    /// Native popup widget (`<select>` dropdown etc.) of the foreground
    /// browser, composited over the view by `draw`/`snapshot`.
    pub popup: render::PopupLayer,
    /// Caret/composition rect (x, y, w, h) in view (physical) coordinates
    /// from `on_ime_composition_range_changed`, for placing the IM candidate
    /// window. `None` until the page reports a composition range.
    pub ime_caret_rect: Option<(i32, i32, i32, i32)>,
    /// Pending IM focus state from CEF's virtual-keyboard request: `Some(true)`
    /// when a page editable gains focus, `Some(false)` when it blurs. Drained by
    /// the view flush (`wake_view`) to focus/unfocus the IM context — so Phosh's on-screen
    /// keyboard tracks the actual text field, not the always-focused GLArea.
    pub keyboard_request: Option<bool>,
    /// Pending screen-reader announcement for page focus (#204); drained by
    /// the view flush (`wake_view`) into `gtk::Accessible::announce`.
    pub a11y_announce: Option<String>,
    /// Latest CEF cursor as a GTK/CSS name, plus dirty flag. OSR never touches
    /// the GTK cursor; the view flush (`wake_view`) applies this.
    pub cursor_name: &'static str,
    pub cursor_dirty: bool,
    /// Pending JS fullscreen request (M21): `Some(true)` = enter, `Some(false)`
    /// = exit. Drained by the window poll loop (CEF callback must not touch GTK).
    pub fullscreen_request: Option<bool>,
    /// CEF `identifier()` of the foreground browser (M20 pool). Account browsers
    /// share this `SharedState`/widget; only the foreground's paint is uploaded
    /// to the GL texture. `0` = no pool wired yet (startup) → all paints pass.
    pub foreground_browser_id: i32,
    /// Widget screen origin in physical pixels for `RenderHandler::screen_point`
    /// (`screen = view + origin` via saturating add). On Wayland the value is
    /// always `(0,0)` because global position is compositor-private and must
    /// stay `(0,0)`. On X11 the value is the cached root-relative origin
    /// queried via `gdk4_x11::X11Surface` in `window_origin_for` (see
    /// `src/web_view/x11_origin.rs`), converted to physical pixels (xlib physical results
    /// passed through clamped; only logical/DIP results are scaled via
    /// `widget.scale_factor()`). Before realize or after unrealize the value
    /// is `(0,0)`. Updated on the GTK main thread when the viewport changes
    /// and re-queried by `screen_point` (so a moved window is seen), which
    /// runs on the CEF UI thread = main thread under `external_message_pump`.
    /// (KARE-019)
    pub window_origin: (i32, i32),
    /// The presenting `GtkGLArea`, so a paint can queue its own render instead
    /// of waiting for a poll timer. On_paint runs on the main
    /// thread (external_message_pump), so the upgrade is always legal; the
    /// `SendWeakRef` only satisfies the `Arc<Mutex<_>>` bounds. Going through
    /// the timer added 0–16 ms per frame and beat against the 60 Hz paint
    /// stream, dropping and doubling frames — the "stutter" in #173/#179.
    pub redraw: Option<gtk::glib::SendWeakRef<gtk::GLArea>>,
    /// A `wake_view` flush is queued and has not drained the requests yet, so
    /// a burst of cursor/IM/announce changes costs one main-loop callback.
    pub view_wake_queued: bool,
}

pub type SharedRef = Arc<Mutex<SharedState>>;

/// Have the owning view apply the cursor / IM focus / announcement requests
/// just stored in `s`, from a main-loop idle callback rather than from inside
/// the CEF callback. Takes the guard so the lock is released first; at most
/// one flush is queued until the view drains (`view_wake_queued`).
pub fn wake_view(mut s: parking_lot::MutexGuard<'_, SharedState>) {
    if s.view_wake_queued {
        return;
    }
    let Some(area) = s.redraw.clone() else { return };
    s.view_wake_queued = true;
    drop(s);
    gtk::glib::idle_add_full(gtk::glib::Priority::DEFAULT, move || {
        if let Some(area) = area.upgrade() {
            crate::web_view::flush_view_requests(&area);
        }
        gtk::glib::ControlFlow::Break
    });
}

pub fn new_shared(size: (i32, i32), scale: f32) -> SharedRef {
    Arc::new(Mutex::new(SharedState {
        size,
        scale_factor: scale,
        ..Default::default()
    }))
}
