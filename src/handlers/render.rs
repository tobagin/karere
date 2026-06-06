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
            rect.x = 0;
            rect.y = 0;
            rect.width = w.max(1);
            rect.height = h.max(1);
        }

        fn screen_info(
            &self,
            _browser: Option<&mut Browser>,
            screen_info: Option<&mut ScreenInfo>,
        ) -> ::std::os::raw::c_int {
            let Some(info) = screen_info else { return 0 };
            let s = self.handler.shared.lock();
            info.device_scale_factor = s.scale_factor.max(1.0);
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
            if s.frame.pixels.len() != len {
                s.frame.pixels.resize(len, 0);
            }
            s.frame.pixels.copy_from_slice(slice);
            s.frame.width = width;
            s.frame.height = height;
            s.frame.dirty = true;
            log::debug!("on_paint {}x{}", width, height);
        }

        // Fires when an editable field gains focus (input_mode != NONE) — a
        // reliable signal the spellcheck service is live. Apply dictionaries
        // once per load here, vs. guessing readiness with timers.
        fn on_virtual_keyboard_requested(
            &self,
            browser: Option<&mut Browser>,
            input_mode: TextInputMode,
        ) {
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
