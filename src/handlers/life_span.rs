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
            target_disposition: WindowOpenDisposition,
            user_gesture: ::std::os::raw::c_int,
            _popup_features: Option<&PopupFeatures>,
            window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            let url = target_url.map(CefString::to_string).unwrap_or_default();
            log::info!(
                "on_before_popup url={url:?} disposition={target_disposition:?} gesture={user_gesture}"
            );

            // Deliberate popups (call pop-out → NEW_POPUP, plus blank/blob
            // `window.open()` surfaces) must be SUPPRESSED, not hosted: under OSR
            // a native popup renders blank and freezes the UI, and navigating the
            // main frame drops the session. The call keeps running in WhatsApp's
            // in-page floating window, so cancel and leave the main frame alone.
            let _ = window_info;
            let scheme = url.split_once(':').map(|(s, _)| s.to_ascii_lowercase());
            let suppress = url.is_empty()
                || target_disposition == WindowOpenDisposition::NEW_POPUP
                || matches!(scheme.as_deref(), Some("about") | Some("blob"));
            if suppress {
                return 1;
            }

            // Everything else stays single-window (WhatsApp links → main frame,
            // external → system browser). Cancel the CEF popup.
            route_target(browser, &url);
            1
        }

        fn on_after_created(&self, browser: Option<&mut Browser>) {
            let Some(b) = browser else { return };
            self.handler.state.lock().browser = Some(b.clone());
            log::info!("browser created");
        }

        fn do_close(&self, _browser: Option<&mut Browser>) -> i32 {
            // Allow the close; CEF follows up with on_before_close.
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
