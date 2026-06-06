//! Embedded DevTools via the Chrome DevTools Protocol (CDP).
//!
//! `ShowDevTools` always opens a native top-level window under CEF 148's Chrome
//! runtime, so we instead load the DevTools frontend page (served by
//! `--remote-debugging-port`) into an OSR `KarereWebView`, like
//! `chrome://inspect`.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

/// Loopback port passed to `--remote-debugging-port`; fixed (single-instance).
pub const DEVTOOLS_PORT: u16 = 9333;

const TIMEOUT: Duration = Duration::from_secs(2);

/// Query `/json/list` and return a fully-qualified DevTools frontend URL for the
/// active page. Blocking — run off the GTK main thread.
pub fn fetch_frontend_url(port: u16) -> Result<String, String> {
    let body = http_get(port, "/json/list")?;
    log::debug!("cdp /json/list -> {body}");
    pick_frontend_url(&body, port)
        .ok_or_else(|| "no inspectable page target found".to_owned())
}

fn http_get(port: u16, path: &str) -> Result<String, String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream =
        TcpStream::connect_timeout(&addr, TIMEOUT).map_err(|e| format!("connect: {e}"))?;
    stream.set_read_timeout(Some(TIMEOUT)).ok();
    stream.set_write_timeout(Some(TIMEOUT)).ok();

    // Chromium's debug server needs HTTP/1.1 and keeps the connection alive, so
    // don't wait for EOF — read headers, then exactly `Content-Length` bytes.
    let req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n");
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("write: {e}"))?;

    let mut buf: Vec<u8> = Vec::with_capacity(8192);
    let mut chunk = [0u8; 4096];

    // 1. Read until end of headers.
    let header_end = loop {
        if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
        let n = stream.read(&mut chunk).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            return Err("connection closed before headers".to_owned());
        }
        buf.extend_from_slice(&chunk[..n]);
    };

    let headers = String::from_utf8_lossy(&buf[..header_end]).to_ascii_lowercase();
    let content_len: usize = headers
        .split("content-length:")
        .nth(1)
        .and_then(|s| s.split("\r\n").next())
        .and_then(|s| s.trim().parse().ok())
        .ok_or("missing content-length")?;

    // 2. Read the body until we have `content_len` bytes.
    while buf.len() - header_end < content_len {
        let n = stream.read(&mut chunk).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }

    Ok(String::from_utf8_lossy(&buf[header_end..header_end + content_len.min(buf.len() - header_end)]).into_owned())
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
}

/// Pick a page target from `/json/list` and return its absolute
/// `devtoolsFrontendUrl`. Prefers the WhatsApp page; skips `about:blank` and any
/// DevTools frontend page.
fn pick_frontend_url(body: &str, port: u16) -> Option<String> {
    let mut fallback: Option<String> = None;
    let mut preferred: Option<String> = None;

    for obj in split_objects(body) {
        let obj = obj.as_str();
        if field(obj, "type").as_deref() != Some("page") {
            continue;
        }
        let url = field(obj, "url").unwrap_or_default();
        let Some(front) = field(obj, "devtoolsFrontendUrl") else {
            continue;
        };
        // Skip blank pages and lingering DevTools frontend pages.
        if !url.starts_with("http")
            || url.contains("devtools")
            || url.contains("inspector.html")
            || url.contains("chrome-devtools-frontend")
        {
            continue;
        }
        if url.contains("whatsapp") {
            preferred = Some(front);
            break;
        }
        if fallback.is_none() {
            fallback = Some(front);
        }
    }

    let front = unescape(&preferred.or(fallback)?);
    log::info!("devtools target frontend url resolved");
    if front.starts_with("http") {
        Some(front)
    } else {
        Some(format!("http://127.0.0.1:{port}{front}"))
    }
}

/// Split a JSON array body into top-level object fragments by brace depth.
fn split_objects(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = None;
    for (i, c) in body.char_indices() {
        match c {
            '{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 && let Some(s) = start.take() {
                    out.push(body[s..=i].to_owned());
                }
            }
            _ => {}
        }
    }
    out
}

/// Extract a `"key": "value"` string field from a flat JSON object fragment.
fn field(obj: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let after_key = &obj[obj.find(&needle)? + needle.len()..];
    let after_colon = &after_key[after_key.find(':')? + 1..];
    let start = after_colon.find('"')? + 1;
    let rest = &after_colon[start..];
    // Values never contain escaped quotes, so the next quote ends it.
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

/// Undo the handful of JSON/unicode escapes Chromium may emit in a URL.
fn unescape(s: &str) -> String {
    s.replace("\\/", "/")
        .replace("\\u003d", "=")
        .replace("\\u0026", "&")
        .replace("\\u003f", "?")
}
