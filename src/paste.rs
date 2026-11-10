//! Paste/drop bridge payloads. Everything travels inline as base64 inside the
//! IPC message; https pages cannot read `file://`, so there is no tempfile path.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;

use crate::ipc::PasteBlob;

/// Largest payload sent inline (~64 MB once base64'd). Beyond that the IPC
/// message gets too large, so the paste is refused instead.
const MAX_PASTE_BYTES: usize = 48 * 1024 * 1024;

/// A paste/drop payload over [`MAX_PASTE_BYTES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasteTooLarge(pub usize);

impl std::fmt::Display for PasteTooLarge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "payload of {} bytes exceeds the {} byte paste limit",
            self.0, MAX_PASTE_BYTES
        )
    }
}

impl std::error::Error for PasteTooLarge {}

pub fn b64(bytes: &[u8]) -> String {
    B64.encode(bytes)
}

/// IPC envelope for a binary paste/drop: inline base64, or an error the
/// caller must surface when the payload is too large to send.
pub fn make_blob(bytes: &[u8]) -> Result<PasteBlob, PasteTooLarge> {
    if bytes.len() > MAX_PASTE_BYTES {
        return Err(PasteTooLarge(bytes.len()));
    }
    Ok(PasteBlob::Base64(B64.encode(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_payload_inlines_base64() {
        assert_eq!(make_blob(b"hello"), Ok(PasteBlob::Base64("aGVsbG8=".into())));
    }

    #[test]
    fn oversized_payload_is_an_error() {
        let big = vec![0u8; MAX_PASTE_BYTES + 1];
        assert_eq!(make_blob(&big), Err(PasteTooLarge(MAX_PASTE_BYTES + 1)));
    }
}
