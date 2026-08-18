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
        /// `window_origin` is `SharedState::window_origin`, currently always
        /// `(0,0)` on every backend so `screen==view` (degenerate transform).
        /// On Wayland global position is compositor-private and must stay
        /// `(0,0)`; on X11 a real origin requires a `gdk4-x11` query not yet
        /// wired (follow-up task). Returns `1` if at least one out-param was
        /// written, `0` only when both are `None` (CEF treats `0` as failure).
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
            _dirty_rects: Option<&[Rect]>,
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
            if s.frame.pixels.len() != len {
                s.frame.pixels.resize(len, 0);
            }
            s.frame.pixels.copy_from_slice(slice);
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
