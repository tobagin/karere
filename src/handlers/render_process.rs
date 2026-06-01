//! Render-process handler — runs **only in the renderer subprocess**.
//!
//! CEF invokes this via `App::render_process_handler` (see
//! [`crate::cef_runtime`]); it never fires in the browser process. Its jobs:
//!
//! - On `on_context_created` (main frame only), bind the native
//!   `karere_send(name, json)` function onto the context global and inject the
//!   build-time JS bundle. Binding happens *after* the context exists (not via
//!   `register_extension`, which runs before context creation and can break the
//!   page's own JS), so page rendering is never affected.
//! - On `on_process_message_received`, decode inbound [`BrowserMessage`]s and
//!   dispatch them (most variants are M13 stubs; debug `Ping` replies `Pong`).

use std::os::raw::c_int;

use cef::{
    Browser, CefString, Frame, ImplFrame, ImplRenderProcessHandler, ImplV8Context, ImplV8Handler,
    ImplV8Value, ProcessId, ProcessMessage, RenderProcessHandler, V8Context, V8Handler,
    V8Propertyattribute, V8Value, WrapRenderProcessHandler, WrapV8Handler, rc::Rc,
    v8_context_get_current_context, v8_value_create_function, wrap_render_process_handler,
    wrap_v8_handler,
};

use crate::ipc::{self, BrowserMessage, PasteBlob, RendererMessage};

/// The concatenated `data/js/*.js` bundle, embedded at build time so the shipped
/// binary needs no runtime `data/js/` lookup (works under flatpak/relocation).
const EMBED_BUNDLE: &str = include_str!(concat!(env!("OUT_DIR"), "/injected_bundle.js"));

#[derive(Clone, Default)]
pub struct ShellRenderProcessHandler;

wrap_render_process_handler! {
    pub struct ShellRenderProcessHandlerBuilder {
        handler: ShellRenderProcessHandler,
    }

    impl RenderProcessHandler {
        fn on_context_created(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            context: Option<&mut V8Context>,
        ) {
            let is_main = frame.as_ref().map(|f| f.is_main()).unwrap_or(-1);
            if is_main != 1 {
                return;
            }

            // Bind the native send function onto the context global so the
            // injected bundle can reach the host (`window.karere_send`).
            if let Some(context) = context
                && let Some(global) = context.global()
            {
                let mut handler = KarereV8HandlerBuilder::build();
                if let Some(mut func) = v8_value_create_function(
                    Some(&CefString::from("karere_send")),
                    Some(&mut handler),
                ) {
                    global.set_value_bykey(
                        Some(&CefString::from("karere_send")),
                        Some(&mut func),
                        V8Propertyattribute::default(),
                    );
                }
            }

            if let Some(frame) = frame {
                frame.execute_java_script(
                    Some(&CefString::from(EMBED_BUNDLE)),
                    Some(&CefString::from("karere://bootstrap")),
                    0,
                );
                log::debug!("renderer: injected bundle into main frame");
            }
        }

        fn on_process_message_received(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _source_process: ProcessId,
            message: Option<&mut ProcessMessage>,
        ) -> c_int {
            let Some(message) = message else { return 0 };
            match BrowserMessage::try_from_cef_message(message) {
                Ok(msg) => {
                    dispatch(msg, frame);
                    1
                }
                Err(ipc::IpcError::UnknownVariant(name)) => {
                    log::warn!("renderer: unknown message {name:?}");
                    0
                }
                Err(e) => {
                    log::warn!("renderer: failed to parse process message: {e}");
                    0
                }
            }
        }
    }
}

impl ShellRenderProcessHandlerBuilder {
    pub fn build() -> RenderProcessHandler {
        Self::new(ShellRenderProcessHandler)
    }
}

/// Route a decoded [`BrowserMessage`] to its handler. Most variants are M13
/// stubs that log; the real handling lands in M14/M17/M20. Debug `Ping` replies
/// with `Pong` on the originating frame's channel.
fn dispatch(msg: BrowserMessage, frame: Option<&mut Frame>) {
    match msg {
        BrowserMessage::DispatchPasteEvent {
            mime,
            kind,
            payload,
            name,
            x,
            y,
        } => {
            let Some(frame) = frame else { return };
            // Re-shape the payload into a JS-friendly discriminated object so
            // `paste_bridge.js` can branch on `payload.kind` without knowing
            // serde's externally-tagged encoding.
            let payload_json = match payload {
                PasteBlob::Base64(data) => serde_json::json!({ "kind": "Base64", "data": data }),
                PasteBlob::FilePath(path) => {
                    serde_json::json!({ "kind": "FilePath", "path": path.to_string_lossy() })
                }
            };
            let detail = serde_json::json!({
                "mime": mime,
                "kind": kind,
                "name": name,
                "x": x,
                "y": y,
                "payload": payload_json,
            });
            // JSON is a syntactic subset of a JS object literal, so inlining the
            // serialized detail is safe.
            let script = format!(
                "window.dispatchEvent(new CustomEvent('karere:dispatch-paste',{{detail:{detail}}}))"
            );
            frame.execute_java_script(
                Some(&CefString::from(script.as_str())),
                Some(&CefString::from("karere://paste")),
                0,
            );
        }
        BrowserMessage::DragHover { phase, x, y } => {
            if let Some(frame) = frame {
                let detail = serde_json::json!({ "phase": phase, "x": x, "y": y });
                let script = format!(
                    "window.dispatchEvent(new CustomEvent('karere:drag-hover',{{detail:{detail}}}))"
                );
                frame.execute_java_script(
                    Some(&CefString::from(script.as_str())),
                    Some(&CefString::from("karere://drag")),
                    0,
                );
            }
        }
        BrowserMessage::SetViewportSize { w, h } => {
            log::debug!("renderer: SetViewportSize {w}x{h} — stub (M14)");
        }
        BrowserMessage::CloseNotifByTag { tag } => {
            log::debug!("renderer: CloseNotifByTag {tag} — stub (M20)");
        }
        #[cfg(debug_assertions)]
        BrowserMessage::Ping => {
            if let Some(frame) = frame
                && let Some(mut pong) = RendererMessage::Pong.to_cef_message()
            {
                frame.send_process_message(ProcessId::BROWSER, Some(&mut pong));
            }
        }
    }
}

wrap_v8_handler! {
    pub struct KarereV8HandlerBuilder {
        inner: KarereV8Handler,
    }

    impl V8Handler {
        fn execute(
            &self,
            name: Option<&CefString>,
            _object: Option<&mut V8Value>,
            arguments: Option<&[Option<V8Value>]>,
            _retval: Option<&mut Option<V8Value>>,
            _exception: Option<&mut CefString>,
        ) -> c_int {
            let fname = name.map(|n| n.to_string()).unwrap_or_default();
            if fname != "karere_send" {
                return 0;
            }

            let args = arguments.unwrap_or(&[]);
            let read = |i: usize| -> String {
                args.get(i)
                    .and_then(|v| v.as_ref())
                    .filter(|v| v.is_string() == 1)
                    .map(|v| CefString::from(&v.string_value()).to_string())
                    .unwrap_or_default()
            };
            let variant = read(0);
            let inner_json = read(1);

            // Wrap the page-supplied inner fields into the externally-tagged
            // envelope serde expects: unit variant -> `"Tag"`, otherwise
            // `{"Tag": <inner>}`.
            let envelope = if inner_json.trim().is_empty() || inner_json == "null" {
                format!("\"{variant}\"")
            } else {
                format!("{{\"{variant}\":{inner_json}}}")
            };

            if let Some(mut msg) = ipc::to_cef_message(&variant, envelope)
                && let Some(ctx) = v8_context_get_current_context()
                && let Some(frame) = ctx.frame()
            {
                frame.send_process_message(ProcessId::BROWSER, Some(&mut msg));
            }
            1
        }
    }
}

impl KarereV8HandlerBuilder {
    pub fn build() -> V8Handler {
        Self::new(KarereV8Handler)
    }
}

#[derive(Clone, Default)]
pub struct KarereV8Handler;
