use cef::{
    self, Client, ContextMenuHandler, DisplayHandler, DownloadHandler, FindHandler, ImplClient,
    LifeSpanHandler, LoadHandler, PermissionHandler, RenderHandler, RequestHandler, WrapClient,
    rc::Rc, wrap_client,
};

use super::display::{ShellDisplayHandler, ShellDisplayHandlerBuilder};
use super::download::{ShellDownloadHandler, ShellDownloadHandlerBuilder};
use super::find::{ShellFindHandler, ShellFindHandlerBuilder};
use super::life_span::{ShellLifeSpanHandler, ShellLifeSpanHandlerBuilder};
use super::load::{ShellLoadHandler, ShellLoadHandlerBuilder};
use super::permission::ShellPermissionHandlerBuilder;
use super::render::{ShellRenderHandler, ShellRenderHandlerBuilder};
use super::{ShellContextMenuHandlerBuilder, ShellRequestHandler, ShellRequestHandlerBuilder};
use super::SharedRef;

wrap_client! {
    pub struct ClientBuilder {
        render_handler: RenderHandler,
        life_span_handler: LifeSpanHandler,
        display_handler: DisplayHandler,
        load_handler: LoadHandler,
        permission_handler: PermissionHandler,
        request_handler: RequestHandler,
        context_menu_handler: ContextMenuHandler,
        find_handler: FindHandler,
        download_handler: DownloadHandler,
    }

    impl Client {
        fn render_handler(&self) -> Option<cef::RenderHandler> {
            Some(self.render_handler.clone())
        }
        fn life_span_handler(&self) -> Option<cef::LifeSpanHandler> {
            Some(self.life_span_handler.clone())
        }
        fn display_handler(&self) -> Option<cef::DisplayHandler> {
            Some(self.display_handler.clone())
        }
        fn load_handler(&self) -> Option<cef::LoadHandler> {
            Some(self.load_handler.clone())
        }
        fn permission_handler(&self) -> Option<cef::PermissionHandler> {
            Some(self.permission_handler.clone())
        }
        fn request_handler(&self) -> Option<cef::RequestHandler> {
            Some(self.request_handler.clone())
        }
        fn context_menu_handler(&self) -> Option<cef::ContextMenuHandler> {
            Some(self.context_menu_handler.clone())
        }
        fn find_handler(&self) -> Option<cef::FindHandler> {
            Some(self.find_handler.clone())
        }
        fn download_handler(&self) -> Option<cef::DownloadHandler> {
            Some(self.download_handler.clone())
        }

        // Browser-process receiver for renderer -> browser messages. Routes
        // forwarded console output into the host log facade; logs the debug
        // Pong probe; logs unknown / malformed messages without crashing.
        fn on_process_message_received(
            &self,
            _browser: Option<&mut cef::Browser>,
            _frame: Option<&mut cef::Frame>,
            _source_process: cef::ProcessId,
            message: Option<&mut cef::ProcessMessage>,
        ) -> ::std::os::raw::c_int {
            use crate::ipc::{IpcError, RendererMessage};
            let Some(message) = message else { return 0 };
            match RendererMessage::try_from_cef_message(message) {
                Ok(RendererMessage::ConsoleLog { level, msg }) => {
                    match level.as_str() {
                        "error" => log::error!("[page] {msg}"),
                        "warn" => log::warn!("[page] {msg}"),
                        _ => log::info!("[page] {msg}"),
                    }
                    1
                }
                Ok(RendererMessage::NotificationSeen {
                    account_id,
                    title,
                    body,
                    icon,
                    tag,
                }) => {
                    crate::notifications::tracker().on_seen(
                        &tag,
                        &title,
                        &body,
                        icon.as_deref(),
                        &account_id,
                    );
                    1
                }
                Ok(RendererMessage::NotificationClosed { tag }) => {
                    crate::notifications::tracker().on_closed(&tag);
                    1
                }
                #[cfg(debug_assertions)]
                Ok(RendererMessage::Pong) => {
                    log::info!("IPC verify: Pong received from renderer");
                    1
                }
                Ok(other) => {
                    log::debug!("browser: renderer message {:?} — stub", other.variant_tag());
                    1
                }
                Err(IpcError::UnknownVariant(name)) => {
                    log::warn!("browser: unknown renderer message {name:?}");
                    0
                }
                Err(e) => {
                    log::warn!("browser: failed to parse renderer message: {e}");
                    0
                }
            }
        }
    }
}

impl ClientBuilder {
    pub fn build_for(shared: SharedRef) -> (Client, ShellLifeSpanHandler) {
        Self::build_inner(shared, false)
    }

    /// Client for the embedded DevTools view: identical, except the request
    /// handler keeps every navigation in-view instead of routing the DevTools
    /// frontend to the external browser.
    pub fn build_devtools_for(shared: SharedRef) -> (Client, ShellLifeSpanHandler) {
        Self::build_inner(shared, true)
    }

    fn build_inner(shared: SharedRef, permissive: bool) -> (Client, ShellLifeSpanHandler) {
        let life = ShellLifeSpanHandler::new();
        let request = if permissive {
            ShellRequestHandler::new_permissive(shared.clone())
        } else {
            ShellRequestHandler::new(shared.clone())
        };
        let client = Self::new(
            ShellRenderHandlerBuilder::build(ShellRenderHandler::new(shared.clone())),
            ShellLifeSpanHandlerBuilder::build(life.clone()),
            ShellDisplayHandlerBuilder::build(ShellDisplayHandler::new(shared.clone())),
            ShellLoadHandlerBuilder::build(ShellLoadHandler::new(shared.clone())),
            ShellPermissionHandlerBuilder::build(),
            ShellRequestHandlerBuilder::build(request),
            ShellContextMenuHandlerBuilder::build(),
            ShellFindHandlerBuilder::build(ShellFindHandler::new(shared.clone())),
            ShellDownloadHandlerBuilder::build(ShellDownloadHandler::new(shared)),
        );
        (client, life)
    }
}
