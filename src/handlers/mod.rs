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
    /// Pending GPU-accelerated frame (DMA-BUF) from `on_accelerated_paint`, when
    /// shared-texture OSR is enabled; consumed + imported to a GL texture in
    /// `draw`. `None` on the software (`on_paint`) path. (gpu-osr)
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
    /// Last (enabled, languages) applied via the editable-focus trigger. `None`
    /// until the first apply of the current load (reset on each main-frame load
    /// start). Focus re-applies only when resolved settings differ, so a live
    /// language switch re-checks on returning focus to the composer.
    pub spellcheck_last: Option<(bool, Vec<String>)>,
    /// Pending IM focus state from CEF's virtual-keyboard request: `Some(true)`
    /// when a page editable gains focus, `Some(false)` when it blurs. Drained by
    /// the widget timer to focus/unfocus the IM context — so Phosh's on-screen
    /// keyboard tracks the actual text field, not the always-focused GLArea.
    pub keyboard_request: Option<bool>,
    /// Latest CEF cursor as a GTK/CSS name, plus dirty flag. OSR never touches
    /// the GTK cursor; the widget tick callback applies this.
    pub cursor_name: &'static str,
    pub cursor_dirty: bool,
    /// Pending JS fullscreen request (M21): `Some(true)` = enter, `Some(false)`
    /// = exit. Drained by the window poll loop (CEF callback must not touch GTK).
    pub fullscreen_request: Option<bool>,
    /// CEF `identifier()` of the foreground browser (M20 pool). Account browsers
    /// share this `SharedState`/widget; only the foreground's paint is uploaded
    /// to the GL texture. `0` = no pool wired yet (startup) → all paints pass.
    pub foreground_browser_id: i32,
    /// Widget origin in physical screen pixels for `RenderHandler::screen_point`
    /// (`screen = view + origin`). Currently always `(0, 0)` on every backend
    /// so `screen==view` (degenerate, correct fallback). On Wayland global
    /// position is compositor-private and must stay `(0,0)`; on X11 a real
    /// origin needs a `gdk4-x11` query not yet wired — see follow-up task.
    /// Before the widget is realized the value is also `(0,0)`. Updated by
    /// `KarereWebView::size_allocate` / `refresh_screen_scale` on the main
    /// thread (currently storing the fallback `(0,0)`); read by `screen_point`
    /// on the CEF UI thread (= main thread under external_message_pump). (KARE-017)
    pub window_origin: (i32, i32),
}

pub type SharedRef = Arc<Mutex<SharedState>>;

pub fn new_shared(size: (i32, i32), scale: f32) -> SharedRef {
    Arc::new(Mutex::new(SharedState {
        size,
        scale_factor: scale,
        ..Default::default()
    }))
}
