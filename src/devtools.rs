//! Embedded DevTools via the Chrome DevTools Protocol (CDP).
//!
//! `ShowDevTools` always opens a native top-level window under the CEF Chrome
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
    pick_frontend_url(&body, port).ok_or_else(|| "no inspectable page target found".to_owned())
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

    Ok(String::from_utf8_lossy(
        &buf[header_end..header_end + content_len.min(buf.len() - header_end)],
    )
    .into_owned())
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Pick a page target from `/json/list` and return its absolute
/// `devtoolsFrontendUrl`. Prefers the WhatsApp page; skips `about:blank` and any
/// DevTools frontend page. Only URLs that resolve to loopback on `port` are
/// accepted, so a stray listener cannot steer the inspector elsewhere.
fn pick_frontend_url(body: &str, port: u16) -> Option<String> {
    let targets: Vec<serde_json::Value> = serde_json::from_str(body).ok()?;
    let mut fallback: Option<String> = None;
    let mut preferred: Option<String> = None;

    for t in &targets {
        if t.get("type").and_then(|v| v.as_str()) != Some("page") {
            continue;
        }
        let url = t.get("url").and_then(|v| v.as_str()).unwrap_or_default();
        let Some(front) = t.get("devtoolsFrontendUrl").and_then(|v| v.as_str()) else {
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
        let Some(front) = loopback_frontend_url(front, port) else {
            continue;
        };
        if url.contains("whatsapp") {
            preferred = Some(front);
            break;
        }
        if fallback.is_none() {
            fallback = Some(front);
        }
    }

    let front = preferred.or(fallback)?;
    log::info!("devtools target frontend url resolved");
    Some(front)
}

/// Make `front` absolute and require it (and its `ws=` target) to be
/// 127.0.0.1/localhost on `port`.
fn loopback_frontend_url(front: &str, port: u16) -> Option<String> {
    let full = if front.starts_with('/') {
        format!("http://127.0.0.1:{port}{front}")
    } else {
        front.to_owned()
    };
    let is_local = |s: &str| {
        s.strip_prefix(&format!("127.0.0.1:{port}"))
            .or_else(|| s.strip_prefix(&format!("localhost:{port}")))
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/') || rest.starts_with('?'))
    };
    let rest = full.strip_prefix("http://")?;
    if !is_local(rest) {
        return None;
    }
    if let Some((_, query)) = full.split_once('?') {
        for kv in query.split('&') {
            if let Some(ws) = kv.strip_prefix("ws=").or_else(|| kv.strip_prefix("wss=")) {
                if !is_local(ws) {
                    return None;
                }
            }
        }
    }
    Some(full)
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: u16 = 9333;

    fn entry(ty: &str, url: &str, front: &str) -> String {
        format!(
            r#"{{"type":"{ty}","url":"{url}","devtoolsFrontendUrl":"{front}"}}"#
        )
    }

    #[test]
    fn relative_frontend_is_made_absolute() {
        let body = format!(
            "[{}]",
            entry("page", "https://web.whatsapp.com/", "/devtools/inspector.html?ws=127.0.0.1:9333/devtools/page/A")
        );
        assert_eq!(
            pick_frontend_url(&body, P).as_deref(),
            Some("http://127.0.0.1:9333/devtools/inspector.html?ws=127.0.0.1:9333/devtools/page/A")
        );
    }

    #[test]
    fn prefers_whatsapp_and_skips_blank() {
        let body = format!(
            "[{},{},{}]",
            entry("page", "about:blank", "/devtools/inspector.html?ws=127.0.0.1:9333/devtools/page/B"),
            entry("page", "https://example.com/", "/devtools/inspector.html?ws=127.0.0.1:9333/devtools/page/C"),
            entry("page", "https://web.whatsapp.com/", "/devtools/inspector.html?ws=127.0.0.1:9333/devtools/page/D"),
        );
        assert!(pick_frontend_url(&body, P).unwrap().ends_with("/page/D"));
    }

    #[test]
    fn rejects_foreign_hosts_and_ports() {
        let evil = entry("page", "https://web.whatsapp.com/", "http://evil.example:9333/devtools/inspector.html");
        assert!(pick_frontend_url(&format!("[{evil}]"), P).is_none());
        let wrong_port = entry("page", "https://web.whatsapp.com/", "http://127.0.0.1:80/x");
        assert!(pick_frontend_url(&format!("[{wrong_port}]"), P).is_none());
        let bad_ws = entry("page", "https://web.whatsapp.com/", "/devtools/inspector.html?ws=evil.example:9333/x");
        assert!(pick_frontend_url(&format!("[{bad_ws}]"), P).is_none());
        let prefix_trick = entry("page", "https://web.whatsapp.com/", "http://127.0.0.1:93330/x");
        assert!(pick_frontend_url(&format!("[{prefix_trick}]"), P).is_none());
    }

    #[test]
    fn invalid_json_yields_none() {
        assert!(pick_frontend_url("not json", P).is_none());
    }
}
