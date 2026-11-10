use gtk::prelude::*;

use super::KarereWebView;

/// Physical screen origin of the window showing `area` (X11; `(0, 0)` on
/// Wayland), queried now rather than at the last allocation so a moved window
/// reports its new position. `None` when `area` is not a web view. Main thread
/// only.
pub(crate) fn current_window_origin(area: &gtk::GLArea) -> Option<(i32, i32)> {
    use gtk::prelude::Cast;
    area.downcast_ref::<KarereWebView>()
        .map(window_origin_for)
}

/// Widget screen origin in physical pixels for `RenderHandler::screen_point`.
///
/// - On Wayland returns `(0,0)` — global position is compositor-private and
///   must stay `(0,0)` so `screen==view` (correct Wayland fallback).
/// - On X11 downcasts the native `GdkSurface` to `gdk4_x11::X11Surface` and
///   queries the root-relative position via `XTranslateCoordinates` (xlib).
///   `XTranslateCoordinates` returns X-server **physical** pixels already
///   (GDK creates X11 windows at physical size under `GDK_SCALE=2`,
///   `scale_factor()==2`), so the result is passed through clamped without
///   re-scaling — do NOT multiply by `widget.scale_factor()` (would double-
///   count and break `GetScreenPoint`/popup anchoring at 2×). (KARE-019)
/// - Before realize or after unrealize (`native` or `surface` is `None`)
///   returns `(0,0)` without panicking; any query failure also falls back
///   to `(0,0)` at `debug` level. Main-thread only (callers are
///   `sync_viewport` and, per query so window moves are seen,
///   `RenderHandler::screen_point` via `current_window_origin`).
///
/// Verified: no root `GdkSurface` exists in gdk4 0.11 to translate against,
/// so the `translate_coordinates`/`compute_bounds` logical path would always
/// return `false` — the shipped path is the xlib physical path.
pub(super) fn window_origin_for(widget: &super::KarereWebView) -> (i32, i32) {
    // Post-unrealize guard: no native/surface → (0,0), never panic.
    let Some(native) = widget.native() else {
        return (0, 0);
    };
    let Some(surface) = native.surface() else {
        return (0, 0);
    };
    // Try X11 downcast; Wayland/Broadway/missing X11 backend → (0,0)
    // Wayland fallback must stay (0,0) by design, not a partial value.
    let x11_surface = match surface.downcast_ref::<gdk4_x11::X11Surface>() {
        Some(s) => s,
        None => return (0, 0),
    };
    // X11 path — XTranslateCoordinates returns physical pixels — do not scale; already in screen physical space.
    let display = surface.display();
    let Some(x11_display) = display.downcast_ref::<gdk4_x11::X11Display>() else {
        log::debug!("window_origin_for: display is not X11");
        return (0, 0);
    };
    let xid = x11_surface.xid();
    // SAFETY: xdisplay is valid on the GTK main thread while the widget
    // is realized; we guard null and the Xlib call is main-thread-only.
    let xdisplay = unsafe { x11_display.xdisplay() };
    if xdisplay.is_null() {
        log::debug!("window_origin_for: xdisplay is null");
        return (0, 0);
    }
    let root = x11_display.xrootwindow();
    let mut rx: ::std::os::raw::c_int = 0;
    let mut ry: ::std::os::raw::c_int = 0;
    let mut child: ::std::os::raw::c_ulong = 0;
    // x11-dl loads libX11 dynamically; opening can fail without libX11.
    // Resolved once: `open()` rebuilds the whole symbol table per call.
    static XLIB: std::sync::OnceLock<Option<x11::xlib::Xlib>> = std::sync::OnceLock::new();
    let Some(xlib) = XLIB
        .get_or_init(|| {
            x11::xlib::Xlib::open()
                .map_err(|e| log::debug!("window_origin_for: Xlib::open failed: {e}"))
                .ok()
        })
        .as_ref()
    else {
        return (0, 0);
    };
    let ok = unsafe {
        (xlib.XTranslateCoordinates)(
            xdisplay as *mut x11::xlib::Display,
            xid,
            root,
            0,
            0,
            &mut rx,
            &mut ry,
            &mut child,
        )
    };
    if ok == 0 {
        log::debug!("window_origin_for: XTranslateCoordinates failed");
        return (0, 0);
    }
    // XTranslateCoordinates returns physical pixels — do not scale; already in screen physical space.
    (rx, ry)
}
