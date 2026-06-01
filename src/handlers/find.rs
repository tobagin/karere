use cef::{
    self, Browser, FindHandler, ImplFindHandler, Rect, WrapFindHandler, rc::Rc, wrap_find_handler,
};

use super::{FindResult, SharedRef};

#[derive(Clone)]
pub struct ShellFindHandler {
    shared: SharedRef,
}

impl ShellFindHandler {
    pub fn new(shared: SharedRef) -> Self {
        Self { shared }
    }
}

wrap_find_handler! {
    pub struct ShellFindHandlerBuilder {
        handler: ShellFindHandler,
    }

    impl FindHandler {
        fn on_find_result(
            &self,
            _browser: Option<&mut Browser>,
            _identifier: ::std::os::raw::c_int,
            count: ::std::os::raw::c_int,
            _selection_rect: Option<&Rect>,
            active_match_ordinal: ::std::os::raw::c_int,
            _final_update: ::std::os::raw::c_int,
        ) {
            // Single active browser per window: ignore the identifier and key the
            // result by the active browser implicitly. Recorded for the poll loop.
            self.handler.shared.lock().find_result = Some(FindResult {
                count,
                active: active_match_ordinal,
            });
        }
    }
}

impl ShellFindHandlerBuilder {
    pub fn build(handler: ShellFindHandler) -> FindHandler {
        Self::new(handler)
    }
}
