// profile_dom_fallback.js — degraded identity source (M20).
//
// NOT part of the default injected bundle. The browser process executes this
// script in an account's main frame only after the Store hook (60-store-hook.js)
// reports StoreUnavailable. It scrapes identity and avatar from the documented
// DOM landmarks — the v3 source of breakage — which is why the switcher keeps a
// persistent yellow "degraded mode" badge for any account running this path.
//
// It deliberately emits NO "store restored" signal: the degraded flag is
// cleared only when a later page load lets the Store hook attach successfully.
// Every IPC message it sends carries source: "dom-fallback" so the host can
// tell scraped data from Store data.
(function () {
  "use strict";

  function send(name, payload) {
    try {
      if (window.karere && typeof window.karere.send === "function") {
        window.karere.send(name, payload === undefined ? "" : JSON.stringify(payload));
      }
    } catch (e) {
      /* channel unavailable — drop silently */
    }
  }

  // Guard against double-injection (a second StoreUnavailable on the same frame).
  if (window.__karereDomFallback) return;
  window.__karereDomFallback = true;

  var lastPushname = null;
  var lastAvatarSrc = null;

  function readPushname() {
    // 6.3: the chat-list header carries the signed-in user's display name.
    var el = document.querySelector('#side header span[dir="auto"][title]');
    var name = el && (el.getAttribute("title") || el.textContent);
    if (name) name = name.trim();
    if (name && name !== lastPushname) {
      lastPushname = name;
      // wid is unknowable from the DOM — report null.
      send("ProfileIdentity", { wid: null, pushname: name, source: "dom-fallback" });
    }
  }

  function blobToBase64Png(blob) {
    return new Promise(function (resolve, reject) {
      var reader = new FileReader();
      reader.onerror = function () {
        reject(reader.error || new Error("FileReader failed"));
      };
      reader.onload = function () {
        var result = String(reader.result || "");
        var comma = result.indexOf(",");
        resolve(comma >= 0 ? result.slice(comma + 1) : result);
      };
      reader.readAsDataURL(blob);
    });
  }

  function readAvatar() {
    // 6.2: the chat-list header avatar is a blob: image once loaded.
    var img = document.querySelector("#side header img");
    var src = img && img.getAttribute("src");
    if (!src || src.indexOf("blob:") !== 0) return;
    if (src === lastAvatarSrc) return;
    lastAvatarSrc = src;
    fetch(src)
      .then(function (resp) {
        return resp.blob();
      })
      .then(blobToBase64Png)
      .then(function (b64) {
        send("ProfileAvatar", { base64_png: b64, source: "dom-fallback" });
      })
      .catch(function (e) {
        // Allow a retry on the next tick by clearing the de-dupe marker.
        lastAvatarSrc = null;
        send("ConsoleLog", {
          level: "warn",
          msg: "karere dom-fallback: avatar read failed: " + (e && e.message ? e.message : e),
        });
      });
  }

  // Poll at 1 Hz (max), per the spec. Stops once both name and avatar are known,
  // but keeps watching the name in case the header mounts late.
  var ticks = 0;
  var MAX_TICKS = 120; // give up after ~2 min of no header
  var timer = setInterval(function () {
    ticks++;
    try {
      readPushname();
      readAvatar();
    } catch (e) {
      /* DOM not ready yet — try again next tick */
    }
    if ((lastPushname && lastAvatarSrc) || ticks >= MAX_TICKS) {
      clearInterval(timer);
    }
  }, 1000);
})();
