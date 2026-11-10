use cef::{
    self, Browser, ImplBrowser, ImplRenderHandler, PaintElementType, Rect, RenderHandler,
    ScreenInfo, TextInputMode, WrapRenderHandler, rc::Rc, wrap_render_handler,
};

use super::{SharedRef, SharedState};
use gtk::prelude::GLAreaExt;

#[derive(Default)]
pub struct FrameBuffer {
    pub pixels: Vec<u8>, // BGRA8
    pub width: i32,
    pub height: i32,
    pub dirty: bool,
    /// Region changed since the last GL upload, as `(x, y, w, h)` in frame
    /// pixels — the union of CEF's dirty rects across every `on_paint` the
    /// draw hasn't consumed yet. `None` means "assume the whole frame"
    /// (first paint, resize, or damage CEF didn't describe). Uploading only
    /// this region keeps a keystroke from re-sending the entire window to the
    /// GPU every frame, which is most of the CPU-OSR cost on integrated
    /// graphics (#179/#180).
    pub damage: Option<(i32, i32, i32, i32)>,
}

/// A CEF popup widget (`PaintElementType::POPUP`: native `<select>` lists,
/// datalist suggestions) drawn on top of the view. CEF paints it separately
/// from the view and positions it via `on_popup_size`.
#[derive(Default)]
pub struct PopupLayer {
    /// CEF id of the browser that owns the popup (0 = pool not wired).
    pub browser_id: i32,
    /// `(x, y, w, h)` in view (physical) pixels from `on_popup_size`; `None`
    /// while hidden. Not clamped to the view: CEF routes input to the popup
    /// by its own rect, so drawing it anywhere else would misplace clicks.
    /// GL/snapshot clipping keeps any overhang off screen.
    pub rect: Option<(i32, i32, i32, i32)>,
    /// Latest CPU popup paint (BGRA). `damage` is unused: popups are small and
    /// are always uploaded whole.
    pub frame: FrameBuffer,
    /// Latest accelerated popup paint, when shared-texture OSR is on.
    pub accel: Option<crate::gl_dmabuf::AccelFrame>,
}

impl PopupLayer {
    /// Rect to composite while `foreground` (0 = no pool) is on screen, if the
    /// popup is shown and belongs to that browser.
    pub fn visible_rect(&self, foreground: i32) -> Option<(i32, i32, i32, i32)> {
        let rect = self.rect?;
        if foreground != 0 && self.browser_id != foreground {
            return None;
        }
        (rect.2 > 0 && rect.3 > 0).then_some(rect)
    }
}

#[derive(Clone)]
pub struct ShellRenderHandler {
    shared: SharedRef,
    a11y: cef::AccessibilityHandler,
    a11y_inner: super::accessibility::ShellAccessibilityHandler,
}

impl ShellRenderHandler {
    pub fn new(shared: SharedRef) -> Self {
        let a11y_inner = super::accessibility::ShellAccessibilityHandler::new(shared.clone());
        let a11y = super::accessibility::ShellAccessibilityHandlerBuilder::build(a11y_inner.clone());
        Self {
            shared,
            a11y,
            a11y_inner,
        }
    }
}

wrap_render_handler! {
    pub struct ShellRenderHandlerBuilder {
        handler: ShellRenderHandler,
    }

    impl RenderHandler {
        fn accessibility_handler(&self) -> Option<cef::AccessibilityHandler> {
            Some(self.handler.a11y.clone())
        }

        fn view_rect(&self, browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            if let Some(b) = browser.as_ref() {
                self.handler.a11y_inner.bind_browser(b.identifier());
            }
            let Some(rect) = rect else { return };
            // Background browsers (M20 pool) report the cached foreground
            // viewport: the OSR surface is shared and only sized to the
            // foreground, so the same rect keeps a hidden browser's layout
            // consistent for when it switches in.
            let s = self.handler.shared.lock();
            let (w, h) = s.size;
            let scale = s.scale_factor;
            rect.x = 0;
            rect.y = 0;
            rect.width = w.max(1);
            rect.height = h.max(1);
            log::trace!(
                "coord: J2 view_rect browser={} rect={}x{} scale={:.3} expected_physical={}x{}",
                browser.as_ref().map(|b| b.identifier()).unwrap_or(0),
                rect.width,
                rect.height,
                scale,
                w,
                h
            );
        }

        fn screen_info(
            &self,
            _browser: Option<&mut Browser>,
            screen_info: Option<&mut ScreenInfo>,
        ) -> ::std::os::raw::c_int {
            let Some(info) = screen_info else { return 0 };
            let s = self.handler.shared.lock();
            let (w, h) = s.size;
            let scale = s.scale_factor;
            // device_scale_factor is pinned to 1: this CEF build ignores it for
            // OSR sizing anyway, and the HiDPI buffer is produced by a physical
            // view rect + a compensating page zoom instead (#158). Pinning it
            // makes the CSS viewport == view_rect deterministically.
            info.device_scale_factor = 1.0;
            info.depth = 24;
            info.depth_per_component = 8;
            info.is_monochrome = 0;
            let rect = Rect { x: 0, y: 0, width: w.max(1), height: h.max(1) };
            info.rect = rect.clone();
            info.available_rect = rect;
            log::trace!(
                "coord: J2 screen_info size={}x{} scale={:.3} device_scale_factor={:.1} rect={}x{}",
                w, h, scale, info.device_scale_factor, info.rect.width, info.rect.height
            );
            1
        }

        /// `view → screen` in physical pixels: `screen = view + window_origin`
        /// via saturating `i32` add. `view` coords are already physical
        /// (pin-`device_scale=1.0` model); no scale is re-applied.
        /// Wayland fallback is `origin = (0,0)` so `screen==view`; on X11
        /// origin is the physical window origin queried via `gdk4-x11` (see
        /// `window_origin_for` in `src/web_view/x11_origin.rs`; xlib path returns
        /// physical pixels and is not re-scaled), re-queried here so a moved
        /// window is not reported at its old position; the cached value is the
        /// fallback when the widget is gone. Returns `1` if at least one
        /// out-param was written, `0` only when both are `None` (CEF treats
        /// `0` as failure).
        fn screen_point(
            &self,
            _browser: Option<&mut Browser>,
            view_x: ::std::os::raw::c_int,
            view_y: ::std::os::raw::c_int,
            screen_x: Option<&mut ::std::os::raw::c_int>,
            screen_y: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            if screen_x.is_none() && screen_y.is_none() {
                return 0;
            }
            let (redraw, cached) = {
                let s = self.handler.shared.lock();
                (s.redraw.clone(), s.window_origin)
            };
            // Lock released before GTK runs. Main thread (external pump), so
            // upgrading the SendWeakRef is legal.
            let origin = match redraw
                .and_then(|w| w.upgrade())
                .and_then(|area| crate::web_view::current_window_origin(&area))
            {
                Some(o) => {
                    self.handler.shared.lock().window_origin = o;
                    o
                }
                None => cached,
            };
            let (sx, sy) = view_to_screen(view_x, view_y, origin.0, origin.1);
            if let Some(out) = screen_x {
                *out = sx;
            }
            if let Some(out) = screen_y {
                *out = sy;
            }
            log::debug!(
                "screen_point view=({},{}) origin=({},{}) screen=({},{})",
                view_x, view_y, origin.0, origin.1, sx, sy
            );
            1
        }

        fn on_popup_show(&self, browser: Option<&mut Browser>, show: ::std::os::raw::c_int) {
            let id = browser_id(&browser);
            let mut s = self.handler.shared.lock();
            if show != 0 {
                if !is_foreground(&s, id) {
                    return;
                }
                s.popup.browser_id = id;
                return; // nothing to draw until on_popup_size + a POPUP paint
            }
            // Hide is honoured from the owning browser even after it left the
            // foreground (CEF hides popups when a browser is hidden), so a
            // switched-away account cannot leave its dropdown on screen.
            if id != s.popup.browser_id && !is_foreground(&s, id) {
                return;
            }
            s.popup = PopupLayer::default();
            request_redraw(s);
        }

        fn on_popup_size(&self, browser: Option<&mut Browser>, rect: Option<&Rect>) {
            let Some(rect) = rect else { return };
            let id = browser_id(&browser);
            let mut s = self.handler.shared.lock();
            if !is_foreground(&s, id) {
                return;
            }
            s.popup.browser_id = id;
            s.popup.rect = Some((rect.x, rect.y, rect.width, rect.height));
            request_redraw(s);
        }

        fn on_paint(
            &self,
            browser: Option<&mut Browser>,
            type_: PaintElementType,
            dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: ::std::os::raw::c_int,
            height: ::std::os::raw::c_int,
        ) {
            if buffer.is_null() || width <= 0 || height <= 0 {
                return;
            }
            // M20 paint gating: discard non-foreground frames so a background
            // account never overwrites the visible GL texture. Foreground id 0 =
            // pool not wired (startup) → every paint passes.
            let id = browser_id(&browser);
            if !is_foreground(&self.handler.shared.lock(), id) {
                return;
            }
            let len = (width as usize) * (height as usize) * 4;
            let slice = unsafe { std::slice::from_raw_parts(buffer, len) };

            let mut s = self.handler.shared.lock();
            if type_ == PaintElementType::POPUP {
                // A popup paint is the popup's own pixels, never the view's.
                let popup = &mut s.popup;
                popup.browser_id = id;
                popup.accel = None; // CPU frames now; drop any stale DMA-BUF
                popup.frame.pixels.clear();
                popup.frame.pixels.extend_from_slice(slice);
                popup.frame.width = width;
                popup.frame.height = height;
                popup.frame.damage = None;
                popup.frame.dirty = true;
                request_redraw(s);
                return;
            }
            // CPU frames after accelerated ones (CEF fell back mid-session):
            // the last DMA-BUF must not keep shadowing them. `draw` releases
            // the imported EGLImage once it sees `accel` gone.
            s.accel = None;
            let expected = s.size;
            let scale = s.scale_factor;
            // A resized frame invalidates every cached pixel: take the whole
            // buffer and let draw() reallocate the texture.
            let resized =
                s.frame.pixels.len() != len || (s.frame.width, s.frame.height) != (width, height);
            if resized {
                s.frame.pixels.resize(len, 0);
                s.frame.pixels.copy_from_slice(slice);
                s.frame.damage = None;
            } else {
                match union_dirty(dirty_rects, width, height) {
                    Some(r) => {
                        copy_region(&mut s.frame.pixels, slice, width, r);
                        // `dirty` still set = the draw hasn't consumed the last
                        // damage yet, so keep accumulating into it.
                        s.frame.damage =
                            if s.frame.dirty { merge_damage(s.frame.damage, r) } else { Some(r) };
                    }
                    // CEF didn't describe the damage (or it covers everything):
                    // fall back to the full copy this path always did.
                    None => {
                        s.frame.pixels.copy_from_slice(slice);
                        s.frame.damage = None;
                    }
                }
            }
            s.frame.width = width;
            s.frame.height = height;
            s.frame.dirty = true;
            log::trace!(
                "coord: J2 on_paint delivered={}x{} expected_physical={}x{} scale={:.3}",
                width, height, expected.0, expected.1, scale
            );
            request_redraw(s);
        }

        // GPU path: when shared-texture OSR is enabled, CEF delivers a DMA-BUF
        // handle here instead of a CPU buffer via on_paint. Dup the plane fds and
        // stash the frame for `draw` to import via EGL. (gpu-osr)
        fn on_accelerated_paint(
            &self,
            browser: Option<&mut Browser>,
            type_: PaintElementType,
            _dirty_rects: Option<&[Rect]>,
            info: Option<&cef::AcceleratedPaintInfo>,
        ) {
            let Some(info) = info else { return };
            // Same foreground gating as on_paint.
            let id = browser_id(&browser);
            if !is_foreground(&self.handler.shared.lock(), id) {
                return;
            }
            let Some(frame) = accel_frame_from(info) else { return };
            let mut s = self.handler.shared.lock();
            log::trace!(
                "coord: J2 on_accelerated_paint popup={} delivered={}x{} visible={:?} expected_physical={}x{} scale={:.3} fourcc={:#x} mod={:#x}",
                type_ == PaintElementType::POPUP,
                frame.width,
                frame.height,
                frame.visible,
                s.size.0,
                s.size.1,
                s.scale_factor,
                frame.fourcc,
                frame.modifier
            );
            if type_ == PaintElementType::POPUP {
                s.popup.browser_id = id;
                s.popup.accel = Some(frame);
            } else {
                s.accel = Some(frame);
            }
            request_redraw(s);
        }

        // The page's IME composition moved. Keep the caret rect so the input
        // method's candidate window can be placed at the text, not the pointer.
        fn on_ime_composition_range_changed(
            &self,
            browser: Option<&mut Browser>,
            _selected_range: Option<&cef::Range>,
            character_bounds: Option<&[Rect]>,
        ) {
            let id = browser_id(&browser);
            let mut s = self.handler.shared.lock();
            if !is_foreground(&s, id) {
                return;
            }
            if let Some(r) = character_bounds.and_then(|b| b.last()) {
                s.ime_caret_rect = Some((r.x, r.y, r.width, r.height));
            }
        }

        // Fires when a page editable gains/loses focus. Drives two things: the IM
        // focus (so Phosh's on-screen keyboard tracks the text field, not the
        // always-focused GLArea), and — when an editable gains focus — a reliable
        // signal the spellcheck service is live, to apply dictionaries once per load.
        fn on_virtual_keyboard_requested(
            &self,
            browser: Option<&mut Browser>,
            input_mode: TextInputMode,
        ) {
            let id = browser_id(&browser);
            {
                // Only the visible account drives the shared IM: a background
                // page's focus change must not pop the on-screen keyboard.
                let mut s = self.handler.shared.lock();
                if is_foreground(&s, id) {
                    s.keyboard_request = Some(input_mode != TextInputMode::NONE);
                    super::wake_view(s);
                }
            }
            if input_mode == TextInputMode::NONE {
                return;
            }
            // Spellcheck is per browser (its own RequestContext), so a
            // background account still gets its dictionaries applied.
            let Some(browser) = browser else { return };
            let (enabled, langs) = super::load::resolve_spellcheck_settings();
            {
                let mut s = self.handler.shared.lock();
                let applied = s.spellcheck_last.get_or_insert_with(Default::default);
                if applied.get(&id) == Some(&(enabled, langs.clone())) {
                    return; // already applied this exact config — avoid re-check flash
                }
                applied.insert(id, (enabled, langs.clone()));
            }
            log::debug!("editable focused → applying spellcheck {langs:?}");
            crate::web_view::apply_spellcheck_to_browser(browser, &langs, enabled);
        }
    }
}

/// CEF id of the calling browser; `0` when CEF passes none (tests).
fn browser_id(browser: &Option<&mut Browser>) -> i32 {
    browser.as_ref().map(|b| b.identifier()).unwrap_or(0)
}

/// M20 gating: only the foreground browser may touch the shared view state.
/// Foreground id `0` = pool not wired (startup, DevTools) → everything passes.
fn is_foreground(s: &SharedState, browser_id: i32) -> bool {
    s.foreground_browser_id == 0 || s.foreground_browser_id == browser_id
}

/// Take a DMA-BUF frame out of CEF's paint info: dup each plane fd (CEF's are
/// valid only for this call) into an `OwnedFd`. `None` when the frame is
/// unusable (empty, unsupported format, dup failure). (gpu-osr)
fn accel_frame_from(info: &cef::AcceleratedPaintInfo) -> Option<crate::gl_dmabuf::AccelFrame> {
    let (w, h) = (info.extra.coded_size.width, info.extra.coded_size.height);
    if w <= 0 || h <= 0 {
        return None;
    }
    let Some(fourcc) = crate::gl_dmabuf::cef_format_to_fourcc(info.format) else {
        log::warn!("on_accelerated_paint: unsupported color format");
        return None;
    };
    let mut planes = Vec::with_capacity(info.plane_count.max(0) as usize);
    for p in info.planes.iter().take(info.plane_count.max(0) as usize) {
        let owned = unsafe { std::os::fd::BorrowedFd::borrow_raw(p.fd) }.try_clone_to_owned();
        match owned {
            Ok(fd) => planes.push(crate::gl_dmabuf::Plane {
                fd,
                offset: p.offset,
                stride: p.stride,
            }),
            Err(e) => {
                log::warn!("on_accelerated_paint: dup plane fd failed: {e}");
                return None;
            }
        }
    }
    if planes.is_empty() {
        return None;
    }
    let v = &info.extra.visible_rect;
    Some(crate::gl_dmabuf::AccelFrame {
        width: w,
        height: h,
        visible: visible_or_coded((v.x, v.y, v.width, v.height), w, h),
        fourcc,
        modifier: info.modifier,
        planes,
        dirty: true,
    })
}

/// CEF's `visible_rect` when it is a non-empty rect inside the coded buffer,
/// else the whole buffer (older producers leave it zeroed).
pub(crate) fn visible_or_coded(
    visible: (i32, i32, i32, i32),
    coded_w: i32,
    coded_h: i32,
) -> (i32, i32, i32, i32) {
    let (x, y, w, h) = visible;
    let inside = x >= 0
        && y >= 0
        && w > 0
        && h > 0
        && x.saturating_add(w) <= coded_w
        && y.saturating_add(h) <= coded_h;
    if inside { visible } else { (0, 0, coded_w, coded_h) }
}

impl ShellRenderHandlerBuilder {
    pub fn build(handler: ShellRenderHandler) -> RenderHandler {
        Self::new(handler)
    }
}

/// Ask the GLArea to draw the frame just stored in `shared`. Takes the guard so
/// the lock is released before GTK runs (queue_render is cheap and coalesces
/// within a frame, so a paint burst never over-schedules).
fn request_redraw(s: parking_lot::MutexGuard<'_, SharedState>) {
    let redraw = s.redraw.clone();
    drop(s);
    if let Some(area) = redraw.and_then(|w| w.upgrade()) {
        area.queue_render();
    }
    // Frame delivery can consume CEF's external pump time slice without another
    // scheduling callback. Resume after GTK can draw, so remaining frame/IPC
    // work does not wait 100 ms. Both CPU and accelerated frames need this.
    crate::cef_pump::schedule(0);
}

/// Union of CEF's dirty rects, clamped to the frame, as `(x, y, w, h)`.
/// `None` when there is nothing usable to narrow the upload with — no rects, an
/// empty list, a degenerate rect, or damage that already covers the whole frame
/// — in which case callers take the full-frame path. (#179/#180)
pub(crate) fn union_dirty(
    rects: Option<&[Rect]>,
    width: i32,
    height: i32,
) -> Option<(i32, i32, i32, i32)> {
    let rects = rects?;
    if rects.is_empty() {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for r in rects {
        // Clamp: a rect outside the frame would index past the buffer.
        let rx0 = r.x.clamp(0, width);
        let ry0 = r.y.clamp(0, height);
        let rx1 = r.x.saturating_add(r.width).clamp(0, width);
        let ry1 = r.y.saturating_add(r.height).clamp(0, height);
        if rx1 <= rx0 || ry1 <= ry0 {
            continue;
        }
        x0 = x0.min(rx0);
        y0 = y0.min(ry0);
        x1 = x1.max(rx1);
        y1 = y1.max(ry1);
    }
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    if (x0, y0, x1, y1) == (0, 0, width, height) {
        return None; // whole frame — the full path is cheaper than row slicing
    }
    Some((x0, y0, x1 - x0, y1 - y0))
}

/// Merge new damage into what the draw hasn't uploaded yet. `None` on either
/// side means "whole frame" and stays that way.
pub(crate) fn merge_damage(
    existing: Option<(i32, i32, i32, i32)>,
    new: (i32, i32, i32, i32),
) -> Option<(i32, i32, i32, i32)> {
    let (ex, ey, ew, eh) = existing?;
    let (nx, ny, nw, nh) = new;
    let x0 = ex.min(nx);
    let y0 = ey.min(ny);
    let x1 = (ex + ew).max(nx + nw);
    let y1 = (ey + eh).max(ny + nh);
    Some((x0, y0, x1 - x0, y1 - y0))
}

/// Copy just `(x, y, w, h)` of `src` into the cached full-frame `dst`, row by
/// row. Both buffers are BGRA8 with the same stride (`width * 4`).
pub(crate) fn copy_region(dst: &mut [u8], src: &[u8], width: i32, rect: (i32, i32, i32, i32)) {
    let (x, y, w, h) = rect;
    let stride = width as usize * 4;
    let x0 = x as usize * 4;
    let run = w as usize * 4;
    for row in y as usize..(y + h) as usize {
        let start = row * stride + x0;
        let end = start + run;
        if end > dst.len() || end > src.len() {
            return;
        }
        dst[start..end].copy_from_slice(&src[start..end]);
    }
}

/// Pure `view → screen` in physical pixels. Saturating `i32` add so
/// `i32::MAX + 1 == MAX` rather than wrapping. No `scale_factor` is
/// applied — view coords are already physical via `size_allocate` and
/// `physical_mouse_coordinates`. Wayland fallback is `origin=(0,0)` so
/// `screen==view`. (KARE-017)
pub(crate) fn view_to_screen(view_x: i32, view_y: i32, origin_x: i32, origin_y: i32) -> (i32, i32) {
    (
        view_x.saturating_add(origin_x),
        view_y.saturating_add(origin_y),
    )
}

/// Exercise the exact CEF `RenderHandler::on_paint` callback implementation in
/// production-shaped renderer tests without manufacturing a CEF `Browser`.
#[cfg(test)]
pub(crate) fn dispatch_cpu_paint_for_test(
    shared: &SharedRef,
    pixels: &[u8],
    width: i32,
    height: i32,
) {
    let handler = ShellRenderHandlerBuilder {
        handler: ShellRenderHandler::new(shared.clone()),
        cef_object: std::ptr::null_mut(),
    };
    ImplRenderHandler::on_paint(
        &handler,
        None,
        PaintElementType::VIEW,
        None,
        pixels.as_ptr(),
        width,
        height,
    );
}

/// Show a popup at `rect` and paint it through the exact CEF callbacks
/// (`on_popup_show`, `on_popup_size`, `on_paint(POPUP)`), as for a `<select>`.
#[cfg(test)]
pub(crate) fn dispatch_popup_for_test(
    shared: &SharedRef,
    rect: (i32, i32, i32, i32),
    pixels: &[u8],
    width: i32,
    height: i32,
) {
    let handler = ShellRenderHandlerBuilder {
        handler: ShellRenderHandler::new(shared.clone()),
        cef_object: std::ptr::null_mut(),
    };
    let r = Rect {
        x: rect.0,
        y: rect.1,
        width: rect.2,
        height: rect.3,
    };
    ImplRenderHandler::on_popup_show(&handler, None, 1);
    ImplRenderHandler::on_popup_size(&handler, None, Some(&r));
    ImplRenderHandler::on_paint(
        &handler,
        None,
        PaintElementType::POPUP,
        None,
        pixels.as_ptr(),
        width,
        height,
    );
}

/// Exercise `ImplRenderHandler::screen_point` without a real `Browser`,
/// mirroring `dispatch_cpu_paint_for_test`. Returns CEF's `1`/`0` and writes
/// through present out-params.
#[cfg(test)]
pub(crate) fn dispatch_screen_point_for_test(
    shared: &SharedRef,
    view_x: i32,
    view_y: i32,
    screen_x: Option<&mut i32>,
    screen_y: Option<&mut i32>,
) -> i32 {
    let handler = ShellRenderHandlerBuilder {
        handler: ShellRenderHandler::new(shared.clone()),
        cef_object: std::ptr::null_mut(),
    };
    ImplRenderHandler::screen_point(&handler, None, view_x, view_y, screen_x, screen_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handlers::new_shared;

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn union_dirty_narrows_to_the_changed_region() {
        assert_eq!(
            union_dirty(Some(&[rect(10, 20, 5, 5)]), 100, 100),
            Some((10, 20, 5, 5))
        );
        assert_eq!(
            union_dirty(Some(&[rect(10, 20, 5, 5), rect(50, 10, 10, 40)]), 100, 100),
            Some((10, 10, 50, 40))
        );
    }

    #[test]
    fn union_dirty_falls_back_to_full_frame() {
        assert_eq!(union_dirty(None, 100, 100), None);
        assert_eq!(union_dirty(Some(&[]), 100, 100), None);
        assert_eq!(union_dirty(Some(&[rect(0, 0, 100, 100)]), 100, 100), None);
        assert_eq!(union_dirty(Some(&[rect(0, 0, 0, 0)]), 100, 100), None);
    }

    #[test]
    fn union_dirty_clamps_out_of_frame_rects() {
        // Would otherwise index past the buffer.
        assert_eq!(
            union_dirty(Some(&[rect(-10, -10, 30, 30)]), 100, 100),
            Some((0, 0, 20, 20))
        );
        assert_eq!(
            union_dirty(Some(&[rect(90, 90, 50, 50)]), 100, 100),
            Some((90, 90, 10, 10))
        );
        assert_eq!(union_dirty(Some(&[rect(200, 200, 10, 10)]), 100, 100), None);
        assert_eq!(
            union_dirty(Some(&[rect(0, 0, i32::MAX, i32::MAX)]), 100, 100),
            None
        );
    }

    #[test]
    fn merge_damage_accumulates_until_the_draw_consumes_it() {
        assert_eq!(
            merge_damage(Some((10, 10, 5, 5)), (20, 30, 5, 5)),
            Some((10, 10, 15, 25))
        );
        // "whole frame" is absorbing.
        assert_eq!(merge_damage(None, (20, 30, 5, 5)), None);
    }

    #[test]
    fn copy_region_touches_only_the_damaged_rows() {
        let width = 4;
        let src = vec![9u8; width * 3 * 4];
        let mut dst = vec![0u8; width * 3 * 4];
        copy_region(&mut dst, &src, width as i32, (1, 1, 2, 1));
        // Row 1, pixels 1..3 only.
        let stride = width * 4;
        assert_eq!(&dst[..stride], &[0u8; 16]);
        assert_eq!(&dst[stride..stride + 4], &[0u8; 4]);
        assert_eq!(&dst[stride + 4..stride + 12], &[9u8; 8]);
        assert_eq!(&dst[stride + 12..stride + 16], &[0u8; 4]);
        assert_eq!(&dst[stride * 2..], &[0u8; 16]);
    }

    #[test]
    fn copy_region_refuses_to_run_past_either_buffer() {
        let mut dst = vec![0u8; 16];
        let src = vec![9u8; 16];
        copy_region(&mut dst, &src, 4, (0, 0, 4, 99));
        assert_eq!(dst, vec![9u8; 16]); // first row copied, then stopped
    }

    #[test]
    fn view_to_screen_basic() {
        assert_eq!(view_to_screen(10, 20, 100, 200), (110, 220));
    }

    #[test]
    fn view_to_screen_zero() {
        assert_eq!(view_to_screen(0, 0, 0, 0), (0, 0));
    }

    #[test]
    fn view_to_screen_negative() {
        assert_eq!(view_to_screen(-5, -5, 10, 10), (5, 5));
    }

    #[test]
    fn view_to_screen_saturating() {
        assert_eq!(view_to_screen(i32::MAX, 0, 1, 0), (i32::MAX, 0));
        assert_eq!(view_to_screen(i32::MIN, 0, -1, 0), (i32::MIN, 0));
        assert_eq!(view_to_screen(0, i32::MAX, 0, 1), (0, i32::MAX));
    }

    #[test]
    fn view_to_screen_wayland_fallback() {
        assert_eq!(view_to_screen(10, 20, 0, 0), (10, 20));
    }

    #[test]
    fn screen_point_both_present() {
        let shared = new_shared((800, 600), 1.0);
        shared.lock().window_origin = (50, 80);
        let mut sx = 0;
        let mut sy = 0;
        let rc = dispatch_screen_point_for_test(&shared, 10, 20, Some(&mut sx), Some(&mut sy));
        assert_eq!(rc, 1);
        assert_eq!((sx, sy), (60, 100));
    }

    #[test]
    fn screen_point_only_x() {
        let shared = new_shared((800, 600), 1.0);
        shared.lock().window_origin = (50, 80);
        let mut sx = 0;
        let rc = dispatch_screen_point_for_test(&shared, 10, 20, Some(&mut sx), None);
        assert_eq!(rc, 1);
        assert_eq!(sx, 60);
    }

    #[test]
    fn screen_point_only_y() {
        let shared = new_shared((800, 600), 1.0);
        shared.lock().window_origin = (50, 80);
        let mut sy = 0;
        let rc = dispatch_screen_point_for_test(&shared, 10, 20, None, Some(&mut sy));
        assert_eq!(rc, 1);
        assert_eq!(sy, 100);
    }

    #[test]
    fn screen_point_both_null() {
        let shared = new_shared((800, 600), 1.0);
        let rc = dispatch_screen_point_for_test(&shared, 10, 20, None, None);
        assert_eq!(rc, 0);
    }

    #[test]
    fn screen_point_wayland_fallback() {
        let shared = new_shared((800, 600), 1.0);
        shared.lock().window_origin = (0, 0);
        let mut sx = 0;
        let mut sy = 0;
        let rc = dispatch_screen_point_for_test(&shared, 10, 20, Some(&mut sx), Some(&mut sy));
        assert_eq!(rc, 1);
        assert_eq!((sx, sy), (10, 20));
    }

    fn handler_for_test(shared: &SharedRef) -> ShellRenderHandlerBuilder {
        ShellRenderHandlerBuilder {
            handler: ShellRenderHandler::new(shared.clone()),
            cef_object: std::ptr::null_mut(),
        }
    }

    fn paint(shared: &SharedRef, type_: PaintElementType, pixels: &[u8], w: i32, h: i32) {
        let handler = handler_for_test(shared);
        ImplRenderHandler::on_paint(&handler, None, type_, None, pixels.as_ptr(), w, h);
    }

    fn null_accel_frame() -> crate::gl_dmabuf::AccelFrame {
        let fd: std::os::fd::OwnedFd = std::fs::File::open("/dev/null").unwrap().into();
        crate::gl_dmabuf::AccelFrame {
            width: 1,
            height: 1,
            visible: (0, 0, 1, 1),
            fourcc: 0,
            modifier: 0,
            planes: vec![crate::gl_dmabuf::Plane {
                fd,
                offset: 0,
                stride: 4,
            }],
            dirty: false,
        }
    }

    #[test]
    fn popup_paint_never_replaces_the_view_frame() {
        let shared = new_shared((4, 4), 1.0);
        let view = [7u8; 2 * 2 * 4];
        paint(&shared, PaintElementType::VIEW, &view, 2, 2);
        shared.lock().frame.dirty = false; // as if drawn

        let handler = handler_for_test(&shared);
        ImplRenderHandler::on_popup_show(&handler, None, 1);
        ImplRenderHandler::on_popup_size(&handler, None, Some(&rect(1, 1, 1, 1)));
        let popup = [9u8; 4];
        paint(&shared, PaintElementType::POPUP, &popup, 1, 1);

        let s = shared.lock();
        assert_eq!(s.frame.pixels, view, "view pixels untouched by the popup");
        assert_eq!((s.frame.width, s.frame.height), (2, 2));
        assert!(!s.frame.dirty, "a popup paint must not mark the view dirty");
        assert_eq!(s.popup.frame.pixels, popup);
        assert!(s.popup.frame.dirty);
        assert_eq!(s.popup.visible_rect(0), Some((1, 1, 1, 1)));
    }

    #[test]
    fn popup_hide_clears_the_layer() {
        let shared = new_shared((4, 4), 1.0);
        let handler = handler_for_test(&shared);
        ImplRenderHandler::on_popup_size(&handler, None, Some(&rect(0, 0, 1, 1)));
        paint(&shared, PaintElementType::POPUP, &[1u8; 4], 1, 1);
        assert!(shared.lock().popup.visible_rect(0).is_some());

        ImplRenderHandler::on_popup_show(&handler, None, 0);
        let s = shared.lock();
        assert_eq!(s.popup.visible_rect(0), None);
        assert!(s.popup.frame.pixels.is_empty());
    }

    #[test]
    fn popup_of_another_browser_is_not_composited() {
        let mut layer = PopupLayer {
            browser_id: 5,
            rect: Some((0, 0, 10, 10)),
            ..Default::default()
        };
        assert_eq!(layer.visible_rect(5), Some((0, 0, 10, 10)));
        assert_eq!(layer.visible_rect(6), None, "switched-away account");
        assert_eq!(layer.visible_rect(0), Some((0, 0, 10, 10)), "no pool wired");
        layer.rect = Some((0, 0, 0, 10));
        assert_eq!(layer.visible_rect(5), None, "degenerate rect");
    }

    #[test]
    fn cpu_paint_drops_a_stale_accelerated_frame() {
        let shared = new_shared((1, 1), 1.0);
        shared.lock().accel = Some(null_accel_frame());
        shared.lock().popup.accel = Some(null_accel_frame());

        paint(&shared, PaintElementType::VIEW, &[1, 2, 3, 255], 1, 1);
        let s = shared.lock();
        assert!(s.accel.is_none(), "CPU frames must not stay behind the last DMA-BUF");
        assert!(s.frame.dirty);
        assert!(
            s.popup.accel.is_some(),
            "a view paint says nothing about the popup"
        );
        drop(s);

        paint(&shared, PaintElementType::POPUP, &[1, 2, 3, 255], 1, 1);
        assert!(shared.lock().popup.accel.is_none());
    }

    #[test]
    fn background_browser_does_not_drive_the_im() {
        let shared = new_shared((1, 1), 1.0);
        shared.lock().foreground_browser_id = 5;
        let handler = handler_for_test(&shared);
        // `None` browser = id 0, i.e. not the foreground account.
        ImplRenderHandler::on_virtual_keyboard_requested(&handler, None, TextInputMode::TEXT);
        assert_eq!(shared.lock().keyboard_request, None);

        shared.lock().foreground_browser_id = 0; // pool not wired: all pass
        ImplRenderHandler::on_virtual_keyboard_requested(&handler, None, TextInputMode::NONE);
        assert_eq!(shared.lock().keyboard_request, Some(false));
    }

    #[test]
    fn visible_rect_falls_back_to_the_coded_buffer() {
        assert_eq!(visible_or_coded((0, 0, 800, 600), 816, 608), (0, 0, 800, 600));
        assert_eq!(visible_or_coded((0, 0, 0, 0), 816, 608), (0, 0, 816, 608));
        assert_eq!(visible_or_coded((10, 0, 816, 608), 816, 608), (0, 0, 816, 608));
        assert_eq!(visible_or_coded((-1, 0, 10, 10), 816, 608), (0, 0, 816, 608));
    }

    #[test]
    fn screen_point_ignores_scale() {
        let shared = new_shared((800, 600), 2.0);
        shared.lock().window_origin = (10, 10);
        let mut sx = 0;
        let mut sy = 0;
        let rc = dispatch_screen_point_for_test(&shared, 10, 10, Some(&mut sx), Some(&mut sy));
        assert_eq!(rc, 1);
        assert_eq!((sx, sy), (20, 20)); // +origin only, not ×scale
    }
}
