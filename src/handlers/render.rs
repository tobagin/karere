use cef::{
    self, Browser, ImplRenderHandler, PaintElementType, Rect, RenderHandler, ScreenInfo,
    WrapRenderHandler, rc::Rc, wrap_render_handler,
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
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            let Some(rect) = rect else { return };
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
            _browser: Option<&mut Browser>,
            _type_: PaintElementType,
            _dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: ::std::os::raw::c_int,
            height: ::std::os::raw::c_int,
        ) {
            if buffer.is_null() || width <= 0 || height <= 0 {
                return;
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
    }
}

impl ShellRenderHandlerBuilder {
    pub fn build(handler: ShellRenderHandler) -> RenderHandler {
        Self::new(handler)
    }
}
