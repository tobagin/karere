//! Typed IPC envelope over the CEF ProcessMessage channel.
//! Payload is base64'd to avoid UTF-8/null-byte issues with binary blobs.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use cef::{CefString, ImplListValue, ImplProcessMessage, ProcessMessage, process_message_create};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Clipboard payload carried by a paste/drop event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PasteBlob {
    Base64(String),
    FilePath(PathBuf),
}

/// Messages from the browser process to the renderer subprocess.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BrowserMessage {
    /// Read the main frame's live selection and report it through SetClipboard.
    /// This is user-triggered (Ctrl+C or CEF's Copy menu command).
    CopySelection,
    /// Synthesize a paste/drop; for drops x/y are release widget coords.
    DispatchPasteEvent {
        mime: String,
        kind: String,
        payload: PasteBlob,
        name: Option<String>,
        x: Option<f64>,
        y: Option<f64>,
    },
    DragHover {
        phase: String,
        x: f64,
        y: f64,
    },
    #[cfg(debug_assertions)]
    Ping,
}

/// Messages from the renderer subprocess to the browser process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RendererMessage {
    /// `wid` is `None` from the DOM fallback.
    ProfileIdentity {
        #[serde(default)]
        wid: Option<String>,
        pushname: String,
        #[serde(default)]
        source: Option<String>,
    },
    ProfileAvatar {
        base64_png: String,
        #[serde(default)]
        source: Option<String>,
    },
    AwaitingPairing,
    StoreUnavailable {
        reason: String,
    },
    /// `icon` is a renderer-inlined data URL.
    NotificationSeen {
        account_id: String,
        title: String,
        body: String,
        icon: Option<String>,
        tag: String,
    },
    NotificationClosed {
        tag: String,
    },
    ConsoleLog {
        level: String,
        msg: String,
    },
    PasteConsumed {
        tempfile_path: Option<PathBuf>,
    },
    /// `primary` targets the PRIMARY selection (middle-click) not the clipboard.
    SetClipboard {
        text: String,
        primary: bool,
    },
    #[cfg(debug_assertions)]
    Pong,
}

/// Failure decoding a `ProcessMessage` into a typed value.
#[derive(Debug, Clone, PartialEq)]
pub enum IpcError {
    UnknownVariant(String),
    MissingPayload,
    Base64(String),
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

/// Build a `ProcessMessage` named `name` carrying `base64(args_json)`.
pub fn to_cef_message(name: &str, args_json: String) -> Option<ProcessMessage> {
    let msg = process_message_create(Some(&CefString::from(name)))?;
    let payload = B64.encode(args_json.as_bytes());
    if let Some(args) = msg.argument_list() {
        args.set_size(1);
        args.set_string(0, Some(&CefString::from(payload.as_str())));
    }
    Some(msg)
}

fn read_envelope(msg: &ProcessMessage) -> Result<(String, String), IpcError> {
    let name = CefString::from(&msg.name()).to_string();
    let args = msg.argument_list().ok_or(IpcError::MissingPayload)?;
    if args.size() < 1 {
        return Err(IpcError::MissingPayload);
    }
    let payload = CefString::from(&args.string(0)).to_string();
    Ok((name, payload))
}

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
    const KNOWN_TAGS: &'static [&'static str] = &[
        "CopySelection",
        "DispatchPasteEvent",
        "DragHover",
        #[cfg(debug_assertions)]
        "Ping",
    ];

    pub fn variant_tag(&self) -> &'static str {
        match self {
            BrowserMessage::CopySelection => "CopySelection",
            BrowserMessage::DispatchPasteEvent { .. } => "DispatchPasteEvent",
            BrowserMessage::DragHover { .. } => "DragHover",
            #[cfg(debug_assertions)]
            BrowserMessage::Ping => "Ping",
        }
    }

    pub fn to_cef_message(&self) -> Option<ProcessMessage> {
        let json = serde_json::to_string(self).ok()?;
        to_cef_message(self.variant_tag(), json)
    }

    pub fn try_from_cef_message(msg: &ProcessMessage) -> Result<Self, IpcError> {
        let (name, payload) = read_envelope(msg)?;
        decode_payload(&name, &payload, Self::KNOWN_TAGS)
    }
}

impl RendererMessage {
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

    pub fn to_cef_message(&self) -> Option<ProcessMessage> {
        let json = serde_json::to_string(self).ok()?;
        to_cef_message(self.variant_tag(), json)
    }

    pub fn try_from_cef_message(msg: &ProcessMessage) -> Result<Self, IpcError> {
        let (name, payload) = read_envelope(msg)?;
        decode_payload(&name, &payload, Self::KNOWN_TAGS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode<T: Serialize>(value: &T) -> String {
        B64.encode(serde_json::to_string(value).unwrap().as_bytes())
    }

    #[test]
    fn browser_roundtrip_every_variant() {
        let cases = vec![
            BrowserMessage::CopySelection,
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
            #[cfg(debug_assertions)]
            BrowserMessage::Ping,
        ];
        for value in cases {
            let payload = encode(&value);
            let back = decode_payload::<BrowserMessage>(
                value.variant_tag(),
                &payload,
                BrowserMessage::KNOWN_TAGS,
            )
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
            RendererMessage::PasteConsumed {
                tempfile_path: None,
            },
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
        let err = decode_payload::<RendererMessage>(
            "NoSuchVariant",
            &payload,
            RendererMessage::KNOWN_TAGS,
        )
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
        let payload = B64.encode(b"{not json");
        let err =
            decode_payload::<RendererMessage>("ConsoleLog", &payload, RendererMessage::KNOWN_TAGS)
                .unwrap_err();
        assert!(matches!(err, IpcError::Json(_)));
    }
}
