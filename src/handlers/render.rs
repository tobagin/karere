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

        fn screen_point(
            &self,
            _browser: Option<&mut Browser>,
            _view_x: ::std::os::raw::c_int,
            _view_y: ::std::os::raw::c_int,
            _screen_x: Option<&mut ::std::os::raw::c_int>,
            _screen_y: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            0
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
