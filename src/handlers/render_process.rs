//! Render-process handler — runs only in the renderer subprocess (via
//! `App::render_process_handler`; never in the browser process). Jobs:
//!
//! - `on_context_created` (main frame): bind native `karere_send(name, json)`
//!   onto the global and inject the build-time JS bundle. Binding happens AFTER
//!   the context exists (not `register_extension`, which runs before context
//!   creation and breaks the page's JS), so rendering is unaffected.
//! - `on_process_message_received`: decode + dispatch [`BrowserMessage`]s
//!   (paste/drop re-emission; debug `Ping` replies `Pong`).

use std::os::raw::c_int;

use cef::{
    Browser, CefString, Frame, ImplFrame, ImplRenderProcessHandler, ImplV8Context, ImplV8Handler,
    ImplV8Value, ProcessId, ProcessMessage, RenderProcessHandler, V8Context, V8Handler,
    V8Propertyattribute, V8Value, WrapRenderProcessHandler, WrapV8Handler, rc::Rc,
    v8_context_get_current_context, v8_value_create_function, wrap_render_process_handler,
    wrap_v8_handler,
};

use crate::ipc::{self, BrowserMessage, PasteBlob, RendererMessage};

/// The concatenated `data/js/*.js` bundle, embedded at build time so the binary
/// needs no runtime lookup (works under flatpak/relocation).
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

            // Bind native send onto the global so the bundle can reach the host
            // (`window.karere_send`).
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
                Ok(message) => {
                    let mut sink = frame.map(FrameDispatchSink);
                    dispatch_browser_message_with(message, &mut sink);
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

/// Renderer operations used by the production CEF frame and by the headless
/// pipeline regression. The seam sits at the actual `Frame` calls, after typed
/// IPC decoding, so tests cannot bypass browser-message dispatch.
pub(crate) trait RendererDispatchSink {
    fn execute_java_script(&mut self, script: &str, source_url: &str);
    fn send_to_browser(&mut self, message: RendererMessage);
}

struct FrameDispatchSink<'a>(&'a mut Frame);

impl RendererDispatchSink for FrameDispatchSink<'_> {
    fn execute_java_script(&mut self, script: &str, source_url: &str) {
        self.0.execute_java_script(
            Some(&CefString::from(script)),
            Some(&CefString::from(source_url)),
            0,
        );
    }

    fn send_to_browser(&mut self, message: RendererMessage) {
        if let Some(mut message) = message.to_cef_message() {
            self.0
                .send_process_message(ProcessId::BROWSER, Some(&mut message));
        }
    }
}

impl<S: RendererDispatchSink> RendererDispatchSink for Option<S> {
    fn execute_java_script(&mut self, script: &str, source_url: &str) {
        if let Some(sink) = self {
            sink.execute_java_script(script, source_url);
        }
    }

    fn send_to_browser(&mut self, message: RendererMessage) {
        if let Some(sink) = self {
            sink.send_to_browser(message);
        }
    }
}

const COPY_SELECTION_SCRIPT: &str =
    "window.dispatchEvent(new CustomEvent('karere:copy-selection'))";

/// Dispatch the typed message produced by the real CEF process-message decoder.
/// Tests inject only the final frame sink because constructing CEF ref-counted
/// process messages without a running CEF context is unsupported.
pub(crate) fn dispatch_browser_message_with(
    msg: BrowserMessage,
    sink: &mut impl RendererDispatchSink,
) {
    match msg {
        BrowserMessage::CopySelection => {
            sink.execute_java_script(COPY_SELECTION_SCRIPT, "karere://copy-selection");
        }
        BrowserMessage::DispatchPasteEvent {
            mime,
            kind,
            payload,
            name,
            x,
            y,
        } => {
            // Re-shape into a discriminated object so `paste_bridge.js` can
            // branch on `payload.kind` without serde's externally-tagged form.
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
            // JSON is a JS object-literal subset, so inlining is safe.
            let script = format!(
                "window.dispatchEvent(new CustomEvent('karere:dispatch-paste',{{detail:{detail}}}))"
            );
            sink.execute_java_script(&script, "karere://paste");
        }
        BrowserMessage::DragHover { phase, x, y } => {
            let detail = serde_json::json!({ "phase": phase, "x": x, "y": y });
            let script = format!(
                "window.dispatchEvent(new CustomEvent('karere:drag-hover',{{detail:{detail}}}))"
            );
            sink.execute_java_script(&script, "karere://drag");
        }
        #[cfg(debug_assertions)]
        BrowserMessage::Ping => sink.send_to_browser(RendererMessage::Pong),
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

            if let Some(message) = renderer_message_from_v8_args(&variant, &inner_json)
                && let Some(mut msg) = message.to_cef_message()
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

/// Build the exact renderer→browser envelope emitted by `karere_send`. This is
/// shared with the pipeline regression so the JavaScript result traverses the
/// production V8 bridge rather than being reconstructed as RendererMessage.
pub(crate) fn renderer_message_from_v8_args(
    variant: &str,
    inner_json: &str,
) -> Option<RendererMessage> {
    // Wrap inner fields into serde's externally-tagged envelope: unit variant
    // -> `"Tag"`, else `{"Tag": <inner>}`.
    let envelope = if inner_json.trim().is_empty() || inner_json == "null" {
        format!("\"{variant}\"")
    } else {
        format!("{{\"{variant}\":{inner_json}}}")
    };
    serde_json::from_str(&envelope).ok()
}

#[derive(Clone, Default)]
pub struct KarereV8Handler;
