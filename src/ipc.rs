//! Typed IPC envelope between the CEF browser process and the renderer
//! subprocess.
//!
//! # Envelope contract
//!
//! CEF carries host↔page messages as [`cef::ProcessMessage`] values over the
//! bidirectional browser↔renderer channel. We layer a typed envelope on top:
//!
//! - [`BrowserMessage`] — browser process → renderer subprocess.
//! - [`RendererMessage`] — renderer subprocess → browser process.
//!
//! ## Encoding
//!
//! Each message maps to a `ProcessMessage` whose **name** is the enum variant
//! tag (e.g. `"SetViewportSize"`) and whose single string **argument** is
//! `base64(json(value))`, where `json(value)` is serde's externally-tagged
//! encoding of the whole enum value (so `{"SetViewportSize":{"w":..,"h":..}}`,
//! or a bare `"AwaitingPairing"` for a unit variant). Base64 sidesteps any
//! UTF-8 / null-byte issues with binary payloads such as [`PasteBlob::Base64`].
//!
//! Decoding rejects an unknown name and any payload that fails base64 decode or
//! JSON deserialization, returning [`IpcError`] rather than panicking.
//!
//! # Adding a new variant
//!
//! 1. Add the variant to [`BrowserMessage`] or [`RendererMessage`].
//! 2. Add its tag to that enum's `variant_tag` match and to `KNOWN_TAGS`.
//! 3. That's it — encoding/decoding is derived from serde + the tag tables.
//!    JS senders pass the variant's inner fields to `window.karere.send(tag,
//!    fieldsJson)`; the native handler wraps them into the envelope.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use cef::{CefString, ImplListValue, ImplProcessMessage, ProcessMessage, process_message_create};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Clipboard payload carried by [`BrowserMessage::DispatchPasteEvent`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PasteBlob {
    /// Base64-encoded raw bytes (e.g. a pasted image).
    Base64(String),
    /// Path to a file on disk.
    FilePath(PathBuf),
}

/// Messages sent from the browser process to the renderer subprocess.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BrowserMessage {
    /// Synthesize a paste (`kind == "paste"`) or drop (`kind == "drop"`) of
    /// `payload` (MIME `mime`) into the page. For drops, `x`/`y` are the widget
    /// coordinates of the release so the renderer can target the element under
    /// the cursor; `name` carries the original filename for file/drop payloads.
    DispatchPasteEvent {
        mime: String,
        kind: String,
        payload: PasteBlob,
        name: Option<String>,
        x: Option<f64>,
        y: Option<f64>,
    },
    /// A file drag is hovering over the embedding widget (no payload yet) so the
    /// page can mount its dropzone before the drop commits. `phase` is
    /// `"enter"` / `"over"` / `"leave"`; `x`/`y` are widget coordinates.
    DragHover { phase: String, x: f64, y: f64 },
    /// Inform the page of the host viewport size (drives responsive layout).
    SetViewportSize { w: i32, h: i32 },
    /// Ask the page to dismiss the notification tagged `tag`.
    CloseNotifByTag { tag: String },
    /// Debug-only channel probe; the renderer replies with [`RendererMessage::Pong`].
    #[cfg(debug_assertions)]
    Ping,
}

/// Messages sent from the renderer subprocess to the browser process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RendererMessage {
    /// The signed-in account identity. `wid` is `None` when reported by the DOM
    /// fallback (which cannot read the internal id). `source` is `Some("store")`
    /// for the first-class Store hook and `Some("dom-fallback")` for the degraded
    /// path, distinguishing the two so the UI can keep its degraded badge.
    ProfileIdentity {
        #[serde(default)]
        wid: Option<String>,
        pushname: String,
        #[serde(default)]
        source: Option<String>,
    },
    /// The account avatar as a base64-encoded PNG. `source` distinguishes the
    /// Store hook (`Some("store")`) from the DOM fallback (`Some("dom-fallback")`).
    ProfileAvatar {
        base64_png: String,
        #[serde(default)]
        source: Option<String>,
    },
    /// The page is showing the pairing / QR screen (not logged in).
    AwaitingPairing,
    /// The page store could not be reached; `reason` is a human-readable note.
    StoreUnavailable { reason: String },
    /// A notification became visible in the page. `icon`, when present, is a
    /// renderer-resolved avatar as a `data:`/base64 string (the browser process
    /// cannot re-fetch a blob/authed URL, so the renderer inlines the bytes).
    NotificationSeen {
        account_id: String,
        title: String,
        body: String,
        icon: Option<String>,
        tag: String,
    },
    /// A notification was dismissed in the page.
    NotificationClosed { tag: String },
    /// Forwarded `console.log/warn/error` output.
    ConsoleLog { level: String, msg: String },
    /// The renderer finished synthesizing a paste/drop; the host may unlink the
    /// backing tempfile (when the payload was a [`PasteBlob::FilePath`]).
    PasteConsumed { tempfile_path: Option<PathBuf> },
    /// Mirror a page text selection / copy to the GDK clipboard (outbound): CEF
    /// windowless mode never owns the system clipboard, so the page reports its
    /// selection and the host writes it. `primary` targets the PRIMARY selection
    /// (Linux middle-click) instead of the regular clipboard.
    SetClipboard { text: String, primary: bool },
    /// Debug-only reply to [`BrowserMessage::Ping`].
    #[cfg(debug_assertions)]
    Pong,
}

/// Failure decoding a [`ProcessMessage`] into a typed envelope value.
#[derive(Debug, Clone, PartialEq)]
pub enum IpcError {
    /// The message name matched no variant tag of the target enum.
    UnknownVariant(String),
    /// The message had no string argument carrying the payload.
    MissingPayload,
    /// The base64 layer failed to decode.
    Base64(String),
    /// The decoded bytes were not valid UTF-8 / JSON.
    Json(String),
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IpcError::UnknownVariant(name) => write!(f, "unknown message variant: {name}"),
            IpcError::MissingPayload => write!(f, "process message has no payload argument"),
            IpcError::Base64(e) => write!(f, "base64 decode failed: {e}"),
            IpcError::Json(e) => write!(f, "json decode failed: {e}"),
        }
    }
}

impl std::error::Error for IpcError {}

/// Build a `ProcessMessage` named `name` carrying `base64(args_json)` as its
/// single string argument. `args_json` is the full externally-tagged JSON of
/// the envelope value. Returns `None` if CEF declines to allocate the message.
pub fn to_cef_message(name: &str, args_json: String) -> Option<ProcessMessage> {
    let msg = process_message_create(Some(&CefString::from(name)))?;
    let payload = B64.encode(args_json.as_bytes());
    if let Some(args) = msg.argument_list() {
        args.set_size(1);
        args.set_string(0, Some(&CefString::from(payload.as_str())));
    }
    Some(msg)
}

/// Read `(name, base64_payload)` out of a `ProcessMessage`.
fn read_envelope(msg: &ProcessMessage) -> Result<(String, String), IpcError> {
    let name = CefString::from(&msg.name()).to_string();
    let args = msg.argument_list().ok_or(IpcError::MissingPayload)?;
    if args.size() < 1 {
        return Err(IpcError::MissingPayload);
    }
    let payload = CefString::from(&args.string(0)).to_string();
    Ok((name, payload))
}

/// Decode the base64-JSON `payload` into `T`, after verifying `name` is one of
/// `known_tags`. Pure (no CEF dependency) so it is unit-testable.
fn decode_payload<T: for<'de> Deserialize<'de>>(
    name: &str,
    payload: &str,
    known_tags: &[&str],
) -> Result<T, IpcError> {
    if !known_tags.contains(&name) {
        return Err(IpcError::UnknownVariant(name.to_owned()));
    }
    let bytes = B64
        .decode(payload.as_bytes())
        .map_err(|e| IpcError::Base64(e.to_string()))?;
    let json = String::from_utf8(bytes).map_err(|e| IpcError::Json(e.to_string()))?;
    serde_json::from_str(&json).map_err(|e| IpcError::Json(e.to_string()))
}

impl BrowserMessage {
    /// Variant tags carried in the `ProcessMessage` name field.
    const KNOWN_TAGS: &'static [&'static str] = &[
        "DispatchPasteEvent",
        "DragHover",
        "SetViewportSize",
        "CloseNotifByTag",
        #[cfg(debug_assertions)]
        "Ping",
    ];

    /// The variant tag for `self`.
    pub fn variant_tag(&self) -> &'static str {
        match self {
            BrowserMessage::DispatchPasteEvent { .. } => "DispatchPasteEvent",
            BrowserMessage::DragHover { .. } => "DragHover",
            BrowserMessage::SetViewportSize { .. } => "SetViewportSize",
            BrowserMessage::CloseNotifByTag { .. } => "CloseNotifByTag",
            #[cfg(debug_assertions)]
            BrowserMessage::Ping => "Ping",
        }
    }

    /// Encode into a `ProcessMessage` (tag + base64-JSON payload).
    pub fn to_cef_message(&self) -> Option<ProcessMessage> {
        let json = serde_json::to_string(self).ok()?;
        to_cef_message(self.variant_tag(), json)
    }

    /// Decode a received `ProcessMessage` into a `BrowserMessage`.
    pub fn try_from_cef_message(msg: &ProcessMessage) -> Result<Self, IpcError> {
        let (name, payload) = read_envelope(msg)?;
        decode_payload(&name, &payload, Self::KNOWN_TAGS)
    }
}

impl RendererMessage {
    /// Variant tags carried in the `ProcessMessage` name field.
    const KNOWN_TAGS: &'static [&'static str] = &[
        "ProfileIdentity",
        "ProfileAvatar",
        "AwaitingPairing",
        "StoreUnavailable",
        "NotificationSeen",
        "NotificationClosed",
        "ConsoleLog",
        "PasteConsumed",
        "SetClipboard",
        #[cfg(debug_assertions)]
        "Pong",
    ];

    /// The variant tag for `self`.
    pub fn variant_tag(&self) -> &'static str {
        match self {
            RendererMessage::ProfileIdentity { .. } => "ProfileIdentity",
            RendererMessage::ProfileAvatar { .. } => "ProfileAvatar",
            RendererMessage::AwaitingPairing => "AwaitingPairing",
            RendererMessage::StoreUnavailable { .. } => "StoreUnavailable",
            RendererMessage::NotificationSeen { .. } => "NotificationSeen",
            RendererMessage::NotificationClosed { .. } => "NotificationClosed",
            RendererMessage::ConsoleLog { .. } => "ConsoleLog",
            RendererMessage::PasteConsumed { .. } => "PasteConsumed",
            RendererMessage::SetClipboard { .. } => "SetClipboard",
            #[cfg(debug_assertions)]
            RendererMessage::Pong => "Pong",
        }
    }

    /// Encode into a `ProcessMessage` (tag + base64-JSON payload).
    pub fn to_cef_message(&self) -> Option<ProcessMessage> {
        let json = serde_json::to_string(self).ok()?;
        to_cef_message(self.variant_tag(), json)
    }

    /// Decode a received `ProcessMessage` into a `RendererMessage`.
    pub fn try_from_cef_message(msg: &ProcessMessage) -> Result<Self, IpcError> {
        let (name, payload) = read_envelope(msg)?;
        decode_payload(&name, &payload, Self::KNOWN_TAGS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode a value the same way `to_cef_message` does, but without touching
    /// CEF: `base64(externally-tagged json)`.
    fn encode<T: Serialize>(value: &T) -> String {
        B64.encode(serde_json::to_string(value).unwrap().as_bytes())
    }

    #[test]
    fn browser_roundtrip_every_variant() {
        let cases = vec![
            BrowserMessage::DispatchPasteEvent {
                mime: "image/png".into(),
                kind: "paste".into(),
                payload: PasteBlob::Base64("AAAA".into()),
                name: None,
                x: None,
                y: None,
            },
            BrowserMessage::DispatchPasteEvent {
                mime: "application/pdf".into(),
                kind: "drop".into(),
                payload: PasteBlob::FilePath("/tmp/x.png".into()),
                name: Some("x.pdf".into()),
                x: Some(12.0),
                y: Some(34.0),
            },
            BrowserMessage::DragHover {
                phase: "over".into(),
                x: 12.5,
                y: 34.0,
            },
            BrowserMessage::SetViewportSize { w: 800, h: 600 },
            BrowserMessage::CloseNotifByTag { tag: "chat-42".into() },
            #[cfg(debug_assertions)]
            BrowserMessage::Ping,
        ];
        for value in cases {
            let payload = encode(&value);
            let back =
                decode_payload::<BrowserMessage>(value.variant_tag(), &payload, BrowserMessage::KNOWN_TAGS)
                    .expect("roundtrip");
            assert_eq!(value, back);
        }
    }

    #[test]
    fn renderer_roundtrip_every_variant() {
        let cases = vec![
            RendererMessage::ProfileIdentity {
                wid: Some("123@c.us".into()),
                pushname: "Ada".into(),
                source: Some("store".into()),
            },
            RendererMessage::ProfileAvatar {
                base64_png: "iVBOR".into(),
                source: Some("store".into()),
            },
            RendererMessage::AwaitingPairing,
            RendererMessage::StoreUnavailable {
                reason: "boot".into(),
            },
            RendererMessage::NotificationSeen {
                account_id: "a".into(),
                title: "t".into(),
                body: "b".into(),
                icon: Some("data:image/png;base64,AAAA".into()),
                tag: "g".into(),
            },
            RendererMessage::NotificationClosed { tag: "g".into() },
            RendererMessage::ConsoleLog {
                level: "log".into(),
                msg: "hi".into(),
            },
            RendererMessage::PasteConsumed {
                tempfile_path: Some("/run/user/1000/karere/paste-abc".into()),
            },
            RendererMessage::PasteConsumed { tempfile_path: None },
            RendererMessage::SetClipboard {
                text: "hello".into(),
                primary: true,
            },
            #[cfg(debug_assertions)]
            RendererMessage::Pong,
        ];
        for value in cases {
            let payload = encode(&value);
            let back = decode_payload::<RendererMessage>(
                value.variant_tag(),
                &payload,
                RendererMessage::KNOWN_TAGS,
            )
            .expect("roundtrip");
            assert_eq!(value, back);
        }
    }

    #[test]
    fn rejects_unknown_name() {
        let payload = encode(&RendererMessage::AwaitingPairing);
        let err = decode_payload::<RendererMessage>("NoSuchVariant", &payload, RendererMessage::KNOWN_TAGS)
            .unwrap_err();
        assert_eq!(err, IpcError::UnknownVariant("NoSuchVariant".into()));
    }

    #[test]
    fn rejects_bad_base64() {
        let err = decode_payload::<RendererMessage>(
            "AwaitingPairing",
            "not valid base64!!!",
            RendererMessage::KNOWN_TAGS,
        )
        .unwrap_err();
        assert!(matches!(err, IpcError::Base64(_)));
    }

    #[test]
    fn rejects_bad_json() {
        // Valid base64, but the decoded bytes are not valid envelope JSON.
        let payload = B64.encode(b"{not json");
        let err = decode_payload::<RendererMessage>("ConsoleLog", &payload, RendererMessage::KNOWN_TAGS)
            .unwrap_err();
        assert!(matches!(err, IpcError::Json(_)));
    }
}
