use cef::{
    self, CefString, Client, ContextMenuHandler, DisplayHandler, DownloadHandler, FindHandler,
    ImplBrowser, ImplClient, ImplFrame, LifeSpanHandler, LoadHandler, PermissionHandler,
    RenderHandler, RequestHandler, WrapClient, rc::Rc, wrap_client,
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
use base64::Engine;

/// Degraded-mode DOM scraper (M20 §6). Not in the injected bundle; run in an
/// account's main frame only on `StoreUnavailable`.
const DOM_FALLBACK_JS: &str = include_str!("../../data/js-deferred/profile_dom_fallback.js");

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

        // Browser-process receiver for renderer -> browser messages.
        fn on_process_message_received(
            &self,
            browser: Option<&mut cef::Browser>,
            frame: Option<&mut cef::Frame>,
            _source_process: cef::ProcessId,
            message: Option<&mut cef::ProcessMessage>,
        ) -> ::std::os::raw::c_int {
            use crate::ipc::{IpcError, RendererMessage};
            let Some(message) = message else { return 0 };
            // Attribute message to its account via the sending browser's id.
            let cef_id = browser.as_ref().map(|b| b.identifier()).unwrap_or(0);
            let account_id = crate::accounts::account_for_browser(cef_id);
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
                Ok(RendererMessage::PasteConsumed { tempfile_path }) => {
                    if let Some(path) = tempfile_path {
                        crate::paste::consume(&path);
                    }
                    1
                }
                Ok(RendererMessage::SetClipboard { text, primary }) => {
                    write_host_clipboard(&text, primary);
                    1
                }
                Ok(RendererMessage::ProfileIdentity { wid, pushname, source }) => {
                    if let Some(id) = account_id.as_deref() {
                        // Identity = page connected → clear awaiting-pairing. A
                        // Store-sourced identity means the hook attached, so it
                        // also clears the degraded badge (the only thing that does).
                        crate::accounts::set_awaiting_pairing(id, false);
                        if source.as_deref() == Some("store") {
                            crate::accounts::clear_degraded(id);
                        }
                        crate::accounts::manager().update_identity(id, wid, Some(pushname));
                    } else {
                        log::warn!("ProfileIdentity for unknown browser id {cef_id}");
                    }
                    1
                }
                Ok(RendererMessage::ProfileAvatar { base64_png, source: _ }) => {
                    if let Some(id) = account_id.as_deref() {
                        match base64::engine::general_purpose::STANDARD.decode(base64_png.as_bytes())
                        {
                            Ok(bytes) => crate::accounts::manager().update_avatar(id, bytes),
                            Err(e) => log::warn!("ProfileAvatar: bad base64: {e}"),
                        }
                    }
                    1
                }
                Ok(RendererMessage::AwaitingPairing) => {
                    if let Some(id) = account_id.as_deref() {
                        crate::accounts::set_awaiting_pairing(id, true);
                    }
                    1
                }
                Ok(RendererMessage::StoreUnavailable { reason }) => {
                    if let Some(id) = account_id.as_deref() {
                        // Act only on the first transition into degraded; the
                        // hook may report this many times per second.
                        let newly_degraded = crate::accounts::set_degraded(id, reason.clone());
                        if newly_degraded {
                            log::warn!("account {id}: Store hook unavailable: {reason}");
                            // §6.1: inject the degraded DOM fallback here.
                            if let Some(frame) = frame.as_ref() {
                                let code = CefString::from(DOM_FALLBACK_JS);
                                let url = CefString::from("karere://profile-dom-fallback");
                                frame.execute_java_script(Some(&code), Some(&url), 0);
                            }
                        }
                    }
                    1
                }
                #[cfg(debug_assertions)]
                Ok(RendererMessage::Pong) => {
                    log::info!("IPC verify: Pong received from renderer");
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

/// Mirror page-reported selection/copy text onto the host GDK clipboard. Runs on
/// the CEF UI thread (= glib main thread under the external pump), so GDK access
/// is safe here.
fn write_host_clipboard(text: &str, primary: bool) {
    use gtk::prelude::*;
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let clipboard = if primary {
        display.primary_clipboard()
    } else {
        display.clipboard()
    };
    clipboard.set_text(text);
}

impl ClientBuilder {
    pub fn build_for(shared: SharedRef) -> (Client, ShellLifeSpanHandler) {
        Self::build_inner(shared, false)
    }

    /// Client for the embedded DevTools view: identical, except the request
    /// handler keeps every navigation in-view (DevTools frontend, not routed out).
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
