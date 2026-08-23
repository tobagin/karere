use cef::{
    self, Browser, ImplBrowser, ImplRenderHandler, PaintElementType, Rect, RenderHandler,
    ScreenInfo, TextInputMode, WrapRenderHandler, rc::Rc, wrap_render_handler,
};

use super::SharedRef;

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

#[derive(Clone)]
pub struct ShellRenderHandler {
    shared: SharedRef,
}

impl ShellRenderHandler {
    pub fn new(shared: SharedRef) -> Self {
        Self { shared }
    }
}

wrap_render_handler! {
    pub struct ShellRenderHandlerBuilder {
        handler: ShellRenderHandler,
    }

    impl RenderHandler {
        fn view_rect(&self, browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
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
            log::info!(
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
            log::info!(
                "coord: J2 screen_info size={}x{} scale={:.3} device_scale_factor={:.1} rect={}x{}",
                w, h, scale, info.device_scale_factor, info.rect.width, info.rect.height
            );
            1
        }

        /// `view → screen` in physical pixels: `screen = view + window_origin`
        /// via saturating `i32` add. `view` coords are already physical
        /// (pin-`device_scale=1.0` model); no scale is re-applied.
        /// Wayland fallback is `origin = (0,0)` so `screen==view`; on X11
        /// origin is the cached physical window origin queried via `gdk4-x11`
        /// (see `window_origin_for` in `src/web_view.rs`; xlib path returns
        /// physical pixels and is not re-scaled). Returns `1` if at least one
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
            let origin = self.handler.shared.lock().window_origin;
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

        fn on_paint(
            &self,
            browser: Option<&mut Browser>,
            _type_: PaintElementType,
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
            {
                let fg = self.handler.shared.lock().foreground_browser_id;
                if fg != 0 {
                    let painting = browser.as_ref().map(|b| b.identifier()).unwrap_or(0);
                    if painting != fg {
                        return;
                    }
                }
            }
            let len = (width as usize) * (height as usize) * 4;
            let slice = unsafe { std::slice::from_raw_parts(buffer, len) };

            let mut s = self.handler.shared.lock();
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
            log::debug!(
                "coord: J2 on_paint delivered={}x{} expected_physical={}x{} scale={:.3}",
                width, height, expected.0, expected.1, scale
            );
        }

        // GPU path: when shared-texture OSR is enabled, CEF delivers a DMA-BUF
        // handle here instead of a CPU buffer via on_paint. Dup the plane fds and
        // stash the frame for `draw` to import via EGL. (gpu-osr)
        fn on_accelerated_paint(
            &self,
            browser: Option<&mut Browser>,
            _type_: PaintElementType,
            _dirty_rects: Option<&[Rect]>,
            info: Option<&cef::AcceleratedPaintInfo>,
        ) {
            let Some(info) = info else { return };
            // Same foreground gating as on_paint.
            {
                let fg = self.handler.shared.lock().foreground_browser_id;
                if fg != 0 {
                    let painting = browser.as_ref().map(|b| b.identifier()).unwrap_or(0);
                    if painting != fg {
                        return;
                    }
                }
            }
            let (w, h) = (info.extra.coded_size.width, info.extra.coded_size.height);
            if w <= 0 || h <= 0 {
                return;
            }
            let Some(fourcc) = crate::gl_dmabuf::cef_format_to_fourcc(info.format) else {
                log::warn!("on_accelerated_paint: unsupported color format");
                return;
            };
            // Dup each plane fd (CEF's are valid only for this call) into an OwnedFd.
            let mut planes = Vec::with_capacity(info.plane_count as usize);
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
                        return;
                    }
                }
            }
            if planes.is_empty() {
                return;
            }
            let mut s = self.handler.shared.lock();
            let expected = s.size;
            let scale = s.scale_factor;
            s.accel = Some(crate::gl_dmabuf::AccelFrame {
                width: w,
                height: h,
                fourcc,
                modifier: info.modifier,
                planes,
                dirty: true,
            });
            log::debug!(
                "coord: J2 on_accelerated_paint delivered={}x{} expected_physical={}x{} scale={:.3} fourcc={fourcc:#x} mod={:#x}",
                w, h, expected.0, expected.1, scale, info.modifier
            );
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
            self.handler.shared.lock().keyboard_request = Some(input_mode != TextInputMode::NONE);
            if input_mode == TextInputMode::NONE {
                return;
            }
            let Some(browser) = browser else { return };
            let (enabled, langs) = super::load::resolve_spellcheck_settings();
            let force_clear;
            {
                let mut s = self.handler.shared.lock();
                if s.spellcheck_last.as_ref() == Some(&(enabled, langs.clone())) {
                    return; // already applied this exact config — avoid re-check flash
                }
                // First apply of this load → force the [] transition; a live
                // switch later sets directly (no [] teardown).
                force_clear = s.spellcheck_last.is_none();
                s.spellcheck_last = Some((enabled, langs.clone()));
            }
            log::debug!("editable focused → applying spellcheck {langs:?} (force_clear={force_clear})");
            crate::web_view::apply_spellcheck_to_browser(browser, &langs, enabled, force_clear);
        }
    }
}

impl ShellRenderHandlerBuilder {
    pub fn build(handler: ShellRenderHandler) -> RenderHandler {
        Self::new(handler)
    }
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
    (view_x.saturating_add(origin_x), view_y.saturating_add(origin_y))
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
        Rect { x, y, width: w, height: h }
    }

    #[test]
    fn union_dirty_narrows_to_the_changed_region() {
        assert_eq!(union_dirty(Some(&[rect(10, 20, 5, 5)]), 100, 100), Some((10, 20, 5, 5)));
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
        assert_eq!(union_dirty(Some(&[rect(-10, -10, 30, 30)]), 100, 100), Some((0, 0, 20, 20)));
        assert_eq!(union_dirty(Some(&[rect(90, 90, 50, 50)]), 100, 100), Some((90, 90, 10, 10)));
        assert_eq!(union_dirty(Some(&[rect(200, 200, 10, 10)]), 100, 100), None);
        assert_eq!(union_dirty(Some(&[rect(0, 0, i32::MAX, i32::MAX)]), 100, 100), None);
    }

    #[test]
    fn merge_damage_accumulates_until_the_draw_consumes_it() {
        assert_eq!(merge_damage(Some((10, 10, 5, 5)), (20, 30, 5, 5)), Some((10, 10, 15, 25)));
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
