//! Chrome DevTools Protocol bridge for WhatsApp notifications. CEF exposes no
//! notification API, so a background thread attaches over `--remote-debugging-port`,
//! patches `Notification`/`showNotification`, and forwards payloads to branded
//! `gio::Notification`s. WebSocket client is hand-rolled to avoid a new dependency.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;

// Transport is console.log (not Runtime.addBinding): bindingCalled only reaches the registering session, but consoleAPICalled reaches any session with Runtime.enable.
const NOTIF_PREFIX: &str = "__KARERE_NOTIF__:";

const SW_PATCH: &str = r#"
(function () {
  var PREFIX = "__KARERE_NOTIF__:";

  function override(title, opts) {
    opts = opts || {};
    var payload = {
      title: title == null ? "" : String(title),
      body: opts.body == null ? "" : String(opts.body),
      tag: opts.tag == null ? "" : String(opts.tag),
      icon: "",
    };
    function emit() {
      try { console.log(PREFIX + JSON.stringify(payload)); } catch (e) {}
    }
    var iconUrl = opts.icon;
    if (iconUrl) {
      fetch(iconUrl)
        .then(function (r) { return r.ok ? r.blob() : null; })
        .then(function (b) {
          if (!b) { emit(); return; }
          return new Promise(function (res) {
            var fr = new FileReader();
            fr.onloadend = function () {
              payload.icon = typeof fr.result === "string" ? fr.result : "";
              res();
            };
            fr.onerror = function () { res(); };
            fr.readAsDataURL(b);
          }).then(emit);
        })
        .catch(emit);
    } else {
      emit();
    }
    return Promise.resolve();
  }
  override.__karere = true;

  var did = [];

  // Patch the prototype: showNotification resolves via it, so an own-property patch alone is bypassed.
  try {
    var P = (self.ServiceWorkerRegistration && self.ServiceWorkerRegistration.prototype) || null;
    if (P && typeof P.showNotification === "function" && !P.showNotification.__karere) {
      P.showNotification = override;
      did.push("proto");
    }
  } catch (e) {}

  try {
    var reg = self.registration;
    if (reg && typeof reg.showNotification === "function" && !reg.showNotification.__karere) {
      reg.showNotification = override;
      did.push("instance");
    }
  } catch (e) {}

  return "patched:[" + did.join(",") + "]";
})();
"#;

/// Page-realm patch: WhatsApp raises message notifications via `new Notification()` in the page (not the SW). Replaces `window.Notification` with a Proxy whose `construct` trap suppresses the banner, forwards via `console.log`, and returns a Notification-shaped stub.
const PAGE_PATCH: &str = r#"
(function () {
  var PREFIX = "__KARERE_NOTIF__:";
  try {
    var Orig = window.Notification;
    if (typeof Orig !== "function") return "no-notification-api";
    if (window.__karereNotifPatched) return "already";
    window.__karereNotifPatched = true;

    var liveByTag = (window.__karereLive = window.__karereLive || new Map());
    var seq = 0;

    function Stub(title, opts) {
      opts = opts || {};
      this.title = title == null ? "" : String(title);
      this.body = opts.body == null ? "" : String(opts.body);
      this.icon = opts.icon == null ? "" : String(opts.icon);
      this.tag = opts.tag == null ? ("__k" + (++seq)) : String(opts.tag);
      this.data = opts.data;
      this.onclick = null; this.onclose = null; this.onshow = null; this.onerror = null;
      this._l = { click: [], close: [], show: [], error: [] };
    }
    Stub.prototype.addEventListener = function (t, c) { if (this._l[t] && typeof c === "function") this._l[t].push(c); };
    Stub.prototype.removeEventListener = function (t, c) { var a = this._l[t]; if (!a) return; var i = a.indexOf(c); if (i >= 0) a.splice(i, 1); };
    Stub.prototype.dispatchEvent = function (e) {
      var t = e && e.type, h = this["on" + t];
      try { if (typeof h === "function") h.call(this, e); } catch (x) {}
      var a = this._l[t] || []; for (var i = 0; i < a.length; i++) { try { a[i].call(this, e); } catch (x) {} }
      return true;
    };
    Stub.prototype._fire = function (t) { var e; try { e = new Event(t); } catch (x) { e = { type: t, target: this }; } this.dispatchEvent(e); };
    Stub.prototype.close = function () { liveByTag.delete(this.tag); this._fire("close"); };

    function resolveIcon(url) {
      if (!url) return Promise.resolve("");
      if (/^data:/i.test(url)) return Promise.resolve(url);
      return fetch(url).then(function (r) { return r.ok ? r.blob() : null; }).then(function (b) {
        if (!b) return "";
        return new Promise(function (res) {
          var fr = new FileReader();
          fr.onloadend = function () { res(typeof fr.result === "string" ? fr.result : ""); };
          fr.onerror = function () { res(""); };
          fr.readAsDataURL(b);
        });
      }).catch(function () { return ""; });
    }

    var bodyTags = (window.__karereBodyTags = window.__karereBodyTags || {});
    function construct(title, opts) {
      var stub = new Stub(title, opts);
      liveByTag.set(stub.tag, stub);
      // WhatsApp fires twice per tag (real body, then empty); skip an empty body only if a real one already fired.
      if (!stub.body && bodyTags[stub.tag]) return stub;
      if (stub.body) bodyTags[stub.tag] = true;
      resolveIcon(stub.icon).then(function (icon) {
        try {
          console.log(PREFIX + JSON.stringify({
            title: stub.title, body: stub.body, tag: stub.tag, icon: icon,
          }));
        } catch (e) {}
      });
      return stub;
    }

    var P = new Proxy(Orig, {
      construct: function (_t, a) { return construct(a[0], a[1]); },
      get: function (t, p, r) { return Reflect.get(t, p, r); },
      set: function (t, p, v, r) { return Reflect.set(t, p, v, r); },
      has: function (t, p) { return Reflect.has(t, p); },
    });
    try {
      Object.defineProperty(window, "Notification", { configurable: true, writable: true, value: P });
    } catch (e) { window.Notification = P; }

    window.__karereCloseNotif = function (tag) { var s = liveByTag.get(String(tag)); if (s) s.close(); };
    window.__karereActivateNotif = function (tag) { var s = liveByTag.get(String(tag)); if (s) s._fire("click"); };

    return "patched-page";
  } catch (e) {
    return "error:" + (e && e.message ? e.message : e);
  }
})();
"#;

/// Start the CDP bridge on a detached background thread; call once after browser init.
pub fn start(port: u16) {
    std::thread::Builder::new()
        .name("karere-cdp".into())
        .spawn(move || supervise(port))
        .expect("spawn cdp thread");
}

// Browser-level + auto-attach (not poll-the-SW-target): an idle SW is stopped and a push wakes it to call showNotification immediately, so polling loses the race; auto-attach hooks every worker the instant it starts.
fn supervise(port: u16) {
    loop {
        match browser_ws(port) {
            Some(url) => {
                if let Err(e) = run_session(&url) {
                    log::debug!("cdp: session ended: {e}");
                }
            }
            None => log::debug!("cdp: browser endpoint not ready"),
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// Browser-level session: arm auto-attach + target discovery, then pump events.
fn run_session(ws_url: &str) -> Result<(), String> {
    let mut ws = WsClient::connect(ws_url)?;
    let mut next_id: u32 = 100;

    // Do NOT pause the SW at startup: ServiceWorkerRegistration isn't populated yet, so the patch would find nothing to wrap. Patch on each executionContextCreated instead.
    ws.send_text(&json_msg(
        1,
        "Target.setAutoAttach",
        "{\"autoAttach\":true,\"waitForDebuggerOnStart\":false,\"flatten\":true}",
    ))?;
    log::info!("cdp: auto-attach armed for service workers");

    // Auto-attach covers workers but not page targets, so discover/attach pages explicitly (the page realm is where new Notification() fires).
    ws.send_text(&json_msg(2, "Target.setDiscoverTargets", "{\"discover\":true}"))?;
    ws.send_text(&json_msg(3, "Target.getTargets", "{}"))?;
    log::info!("cdp: target discovery armed for page realms");

    let mut patched: std::collections::HashMap<String, Patch> =
        std::collections::HashMap::new();
    let mut attached_pages: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    loop {
        let msg = ws.recv_text()?;
        let v: serde_json::Value = match serde_json::from_str(&msg) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");

        match method {
            // Re-snapshot on any churn: targetCreated isn't reliably typed "page" and pages created after arm aren't auto-attached.
            "Target.targetCreated" => {
                ws.send_text(&json_msg(3, "Target.getTargets", "{}"))?;
            }
            _ if v.get("id").and_then(|i| i.as_u64()) == Some(3) => {
                if let Some(infos) = v
                    .get("result")
                    .and_then(|r| r.get("targetInfos"))
                    .and_then(|t| t.as_array())
                {
                    for t in infos {
                        let ttype = t.get("type").and_then(|x| x.as_str()).unwrap_or("");
                        let tid = t.get("targetId").and_then(|x| x.as_str()).unwrap_or("");
                        if ttype == "page" && !tid.is_empty() && !attached_pages.contains(tid) {
                            attached_pages.insert(tid.to_owned());
                            let params =
                                format!("{{\"targetId\":{},\"flatten\":true}}", json_string(tid));
                            log::info!("cdp: attaching page target {}", &tid[..tid.len().min(8)]);
                            ws.send_text(&json_msg(next_id, "Target.attachToTarget", &params))?;
                            next_id += 1;
                        }
                    }
                }
            }
            "Target.attachedToTarget" => {
                let params = v.get("params");
                let ttype = params
                    .and_then(|p| p.get("targetInfo"))
                    .and_then(|t| t.get("type"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                let session = params
                    .and_then(|p| p.get("sessionId"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_owned();
                if session.is_empty() {
                    continue;
                }
                let which = match ttype {
                    "service_worker" => Some(Patch::Sw),
                    "page" => Some(Patch::Page),
                    _ => None,
                };
                if let Some(which) = which {
                    log::info!("cdp: {ttype} attached; arming {which:?} patch");
                    // Record before enabling Runtime: Runtime.enable emits executionContextCreated synchronously, and that handler only patches known sessions.
                    patched.insert(session.clone(), which);
                    setup_session(&mut ws, &session, which, &mut next_id)?;
                }
            }
            "Runtime.executionContextCreated" => {
                let session = v.get("sessionId").and_then(|s| s.as_str()).unwrap_or("");
                if let Some(&which) = patched.get(session) {
                    log::info!("cdp: executionContextCreated session={session} patch={which:?}");
                    evaluate_patch(&mut ws, session, which, &mut next_id)?;
                }
            }
            "Runtime.consoleAPICalled" => {
                if let Some(payload) = console_payload(&v) {
                    dispatch(payload);
                }
            }
            _ => {
                if v.get("id").is_some() {
                    if let Some(val) = v
                        .get("result")
                        .and_then(|r| r.get("result"))
                        .and_then(|r| r.get("value"))
                    {
                        log::info!("cdp: eval result -> {val}");
                    } else if let Some(exc) =
                        v.get("result").and_then(|r| r.get("exceptionDetails"))
                    {
                        log::warn!("cdp: eval exception -> {exc}");
                    } else if let Some(err) = v.get("error") {
                        log::warn!("cdp: cmd error -> {err}");
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Patch {
    Sw,
    Page,
}

impl Patch {
    fn js(self) -> &'static str {
        match self {
            Patch::Sw => SW_PATCH,
            Patch::Page => PAGE_PATCH,
        }
    }
}

// Enable Runtime, then apply the patch once now (covering an already-running context that emits no later event).
fn setup_session(
    ws: &mut WsClient,
    session: &str,
    which: Patch,
    next_id: &mut u32,
) -> Result<(), String> {
    ws.send_text(&json_msg_sess(*next_id, "Runtime.enable", "{}", session))?;
    *next_id += 1;
    evaluate_patch(ws, session, which, next_id)
}

// Idempotent: each patch guards against double-wrapping via its own marker.
fn evaluate_patch(
    ws: &mut WsClient,
    session: &str,
    which: Patch,
    next_id: &mut u32,
) -> Result<(), String> {
    let patch_params = format!(
        "{{\"expression\":{},\"returnByValue\":true}}",
        json_string(which.js())
    );
    ws.send_text(&json_msg_sess(*next_id, "Runtime.evaluate", &patch_params, session))?;
    *next_id += 1;
    Ok(())
}

fn dispatch(payload_json: String) {
    glib::MainContext::default().invoke(move || {
        let p: NotifPayload = match serde_json::from_str(&payload_json) {
            Ok(p) => p,
            Err(e) => {
                log::warn!("cdp: bad notif payload: {e}");
                return;
            }
        };
        let icon = if p.icon.is_empty() {
            None
        } else {
            Some(p.icon.as_str())
        };
        log::info!("cdp: SW notification tag={:?} title={:?}", p.tag, p.title);
        crate::notifications::tracker().on_seen(&p.tag, &p.title, &p.body, icon, "");
    });
}

#[derive(serde::Deserialize)]
struct NotifPayload {
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    tag: String,
    #[serde(default)]
    icon: String,
}

/// Query `/json/version` for the browser-level `webSocketDebuggerUrl`.
fn browser_ws(port: u16) -> Option<String> {
    let body = http_get(port, "/json/version").ok()?;
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    v.get("webSocketDebuggerUrl")
        .and_then(|u| u.as_str())
        .map(|s| s.to_owned())
}

/// Extract our notification payload (a `NOTIF_PREFIX`-tagged string arg) from a `consoleAPICalled` event.
fn console_payload(v: &serde_json::Value) -> Option<String> {
    let args = v.get("params")?.get("args")?.as_array()?;
    for a in args {
        if a.get("type").and_then(|t| t.as_str()) == Some("string") {
            if let Some(s) = a.get("value").and_then(|s| s.as_str()) {
                if let Some(rest) = s.strip_prefix(NOTIF_PREFIX) {
                    return Some(rest.to_owned());
                }
            }
        }
    }
    None
}

/// Build a CDP request frame; `params` is a raw JSON object string.
fn json_msg(id: u32, method: &str, params: &str) -> String {
    format!("{{\"id\":{id},\"method\":\"{method}\",\"params\":{params}}}")
}

/// Like [`json_msg`] but addressed to a flattened child session.
fn json_msg_sess(id: u32, method: &str, params: &str, session: &str) -> String {
    format!(
        "{{\"id\":{id},\"method\":\"{method}\",\"params\":{params},\"sessionId\":{}}}",
        json_string(session)
    )
}

fn json_string(s: &str) -> String {
    serde_json::Value::String(s.to_owned()).to_string()
}

const TIMEOUT: Duration = Duration::from_secs(2);

fn http_get(port: u16, path: &str) -> Result<String, String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream =
        TcpStream::connect_timeout(&addr, TIMEOUT).map_err(|e| format!("connect: {e}"))?;
    stream.set_read_timeout(Some(TIMEOUT)).ok();
    let req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n");
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("write: {e}"))?;
    let mut buf = Vec::with_capacity(8192);
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        if let Some(pos) = find_sub(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
        let n = stream.read(&mut chunk).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            return Err("closed before headers".into());
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let headers = String::from_utf8_lossy(&buf[..header_end]).to_ascii_lowercase();
    let len = headers
        .split("content-length:")
        .nth(1)
        .and_then(|s| s.split("\r\n").next())
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let total = header_end + len;
    while buf.len() < total {
        let n = stream.read(&mut chunk).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    Ok(String::from_utf8_lossy(&buf[header_end..total]).into_owned())
}

fn find_sub(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

struct WsClient {
    stream: TcpStream,
}

impl WsClient {
    /// Connect and perform the opening handshake (`ws://host:port/path`).
    fn connect(url: &str) -> Result<Self, String> {
        let rest = url.strip_prefix("ws://").ok_or("not a ws:// url")?;
        let (hostport, path) = rest.split_once('/').unwrap_or((rest, ""));
        let (host, port) = hostport.split_once(':').ok_or("no port in ws url")?;
        let port: u16 = port.parse().map_err(|_| "bad port")?;
        let addr = SocketAddr::from((
            host.parse::<std::net::Ipv4Addr>().map_err(|e| e.to_string())?,
            port,
        ));
        let mut stream =
            TcpStream::connect_timeout(&addr, TIMEOUT).map_err(|e| format!("connect: {e}"))?;
        stream.set_nodelay(true).ok();

        let key = B64.encode(rand16());
        let req = format!(
            "GET /{path} HTTP/1.1\r\nHost: {hostport}\r\nUpgrade: websocket\r\n\
             Connection: Upgrade\r\nSec-WebSocket-Key: {key}\r\n\
             Sec-WebSocket-Version: 13\r\n\r\n"
        );
        stream
            .write_all(req.as_bytes())
            .map_err(|e| format!("handshake write: {e}"))?;

        let mut buf = Vec::with_capacity(1024);
        let mut one = [0u8; 1];
        loop {
            let n = stream.read(&mut one).map_err(|e| format!("handshake read: {e}"))?;
            if n == 0 {
                return Err("handshake closed".into());
            }
            buf.push(one[0]);
            if buf.ends_with(b"\r\n\r\n") {
                break;
            }
            if buf.len() > 8192 {
                return Err("handshake too large".into());
            }
        }
        let resp = String::from_utf8_lossy(&buf);
        if !resp.starts_with("HTTP/1.1 101") {
            return Err(format!("handshake not 101: {}", &resp[..resp.len().min(40)]));
        }
        Ok(Self { stream })
    }

    /// Send a masked text frame.
    fn send_text(&mut self, text: &str) -> Result<(), String> {
        let payload = text.as_bytes();
        let mut frame = Vec::with_capacity(payload.len() + 14);
        frame.push(0x81);
        let mask = rand4();
        let n = payload.len();
        if n < 126 {
            frame.push(0x80 | n as u8);
        } else if n < 65536 {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(n as u16).to_be_bytes());
        } else {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&(n as u64).to_be_bytes());
        }
        frame.extend_from_slice(&mask);
        frame.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        self.stream
            .write_all(&frame)
            .map_err(|e| format!("send: {e}"))
    }

    /// Receive the next text message, reassembling continuation frames and answering control frames inline.
    fn recv_text(&mut self) -> Result<String, String> {
        let mut message: Vec<u8> = Vec::new();
        loop {
            let (fin, opcode, payload) = self.read_frame()?;
            match opcode {
                0x1 | 0x2 | 0x0 => {
                    message.extend_from_slice(&payload);
                    if fin {
                        return String::from_utf8(message).map_err(|e| e.to_string());
                    }
                }
                0x8 => return Err("ws closed by peer".into()),
                0x9 => self.send_pong(&payload)?,
                0xA => {}
                other => return Err(format!("bad opcode {other}")),
            }
        }
    }

    fn read_frame(&mut self) -> Result<(bool, u8, Vec<u8>), String> {
        let mut h = [0u8; 2];
        self.read_exact(&mut h)?;
        let fin = h[0] & 0x80 != 0;
        let opcode = h[0] & 0x0f;
        let masked = h[1] & 0x80 != 0;
        let mut len = (h[1] & 0x7f) as usize;
        if len == 126 {
            let mut e = [0u8; 2];
            self.read_exact(&mut e)?;
            len = u16::from_be_bytes(e) as usize;
        } else if len == 127 {
            let mut e = [0u8; 8];
            self.read_exact(&mut e)?;
            len = u64::from_be_bytes(e) as usize;
        }
        let mut mask = [0u8; 4];
        if masked {
            self.read_exact(&mut mask)?;
        }
        let mut payload = vec![0u8; len];
        self.read_exact(&mut payload)?;
        if masked {
            for (i, b) in payload.iter_mut().enumerate() {
                *b ^= mask[i % 4];
            }
        }
        Ok((fin, opcode, payload))
    }

    fn send_pong(&mut self, data: &[u8]) -> Result<(), String> {
        let mut frame = Vec::with_capacity(data.len() + 6);
        frame.push(0x8A);
        let mask = rand4();
        frame.push(0x80 | data.len().min(125) as u8);
        frame.extend_from_slice(&mask);
        frame.extend(
            data.iter()
                .take(125)
                .enumerate()
                .map(|(i, b)| b ^ mask[i % 4]),
        );
        self.stream
            .write_all(&frame)
            .map_err(|e| format!("pong: {e}"))
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), String> {
        self.stream.read_exact(buf).map_err(|e| format!("read: {e}"))
    }
}

// Cheap per-handshake entropy; need only be unique, not crypto-strong (loopback).
fn rand16() -> [u8; 16] {
    let mut out = [0u8; 16];
    let seed = seed();
    for (i, b) in out.iter_mut().enumerate() {
        out_byte(b, seed, i);
    }
    out
}

fn rand4() -> [u8; 4] {
    let mut out = [0u8; 4];
    let seed = seed();
    for (i, b) in out.iter_mut().enumerate() {
        out_byte(b, seed, i);
    }
    out
}

fn out_byte(b: &mut u8, seed: u64, i: usize) {
    let x = seed.wrapping_mul(6364136223846793005).wrapping_add(i as u64 + 1);
    *b = (x >> ((i % 8) * 8)) as u8;
}

fn seed() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static CTR: AtomicU64 = AtomicU64::new(0x9e3779b97f4a7c15);
    let local = 0u8;
    let addr = std::ptr::addr_of!(local) as u64;
    CTR.fetch_add(0x2545f4914f6cdd1d, Ordering::Relaxed) ^ addr
}
