use cef::{
    self, Browser, BrowserSettings, CefString, Client, DictionaryValue, Frame, ImplLifeSpanHandler,
    LifeSpanHandler, PopupFeatures, WindowInfo, WindowOpenDisposition, WrapLifeSpanHandler, rc::Rc,
    wrap_life_span_handler,
};
use parking_lot::Mutex;
use std::sync::Arc;

use super::request::route_target;

#[derive(Default)]
pub struct LifeSpanState {
    pub browser: Option<Browser>,
    pub closed: bool,
}

#[derive(Clone)]
pub struct ShellLifeSpanHandler {
    pub state: Arc<Mutex<LifeSpanState>>,
}

impl ShellLifeSpanHandler {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(LifeSpanState::default())),
        }
    }
}

wrap_life_span_handler! {
    pub struct ShellLifeSpanHandlerBuilder {
        handler: ShellLifeSpanHandler,
    }

    impl LifeSpanHandler {
        #[allow(clippy::too_many_arguments)]
        fn on_before_popup(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: ::std::os::raw::c_int,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: ::std::os::raw::c_int,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            // Suppress every popup: route the target ourselves and cancel the
            // new CEF window so the shell stays a single window.
            let url = target_url.map(CefString::to_string).unwrap_or_default();
            route_target(browser, &url);
            1
        }

        fn on_after_created(&self, browser: Option<&mut Browser>) {
            let Some(b) = browser else { return };
            self.handler.state.lock().browser = Some(b.clone());
            log::info!("browser created");
        }

        fn do_close(&self, _browser: Option<&mut Browser>) -> i32 {
            // Allow the close; CEF will follow up with on_before_close.
            0
        }

        fn on_before_close(&self, _browser: Option<&mut Browser>) {
            let mut s = self.handler.state.lock();
            s.closed = true;
            s.browser = None;
            log::info!("browser closed");
        }
    }
}

impl ShellLifeSpanHandlerBuilder {
    pub fn build(handler: ShellLifeSpanHandler) -> LifeSpanHandler {
        Self::new(handler)
    }
}
