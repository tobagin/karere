// 60-store-hook.js — first-class identity source (M20).
//
// WhatsApp's Store/webpack internals are unreachable: the app restores native
// Array.push on its webpack chunk array post-load, so the @wppconnect/wa-js
// "parasite chunk" technique can't capture __webpack_require__ (verified).
//
// Instead read the account's own identity from data the app persists (stable
// and present once paired):
//   • wid     — localStorage['last-wid-md']            (e.g. "15550001234:19@c.us")
//   • name    — IndexedDB model-storage → contact[self].name
//   • avatar  — localStorage['WACachedProfilePicEURL'] → fetch → base64 (CORS-OK,
//               served by pps.whatsapp.net with image/jpeg)
//
// Emits ProfileIdentity/ProfileAvatar (source:"store"); a store-sourced identity
// clears any prior degraded badge. While unpaired (no wid) emits AwaitingPairing
// (debounced) and keeps polling — never StoreUnavailable, so accounts never get
// stuck in degraded mode.
(function () {
  "use strict";

  if (window.__karereStoreHookInstalled) return;
  window.__karereStoreHookInstalled = true;

  function send(name, payload) {
    try {
      var body = payload === undefined ? "" : JSON.stringify(payload);
      if (window.karere && typeof window.karere.send === "function") {
        window.karere.send(name, body);
      } else if (typeof window.karere_send === "function") {
        window.karere_send(name, body);
      }
    } catch (e) {
      /* channel unavailable — drop silently */
    }
  }

  // Open an existing database only. A versionless open() of a missing DB would
  // create an empty one before WhatsApp does, so list first and abort any upgrade.
  function openDB(name) {
    function open() {
      return new Promise(function (resolve) {
        try {
          var req = indexedDB.open(name);
          req.onupgradeneeded = function () {
            try {
              req.transaction.abort();
            } catch (e) {}
          };
          req.onsuccess = function () {
            resolve(req.result);
          };
          req.onerror = function () {
            resolve(null);
          };
        } catch (e) {
          resolve(null);
        }
      });
    }
    try {
      if (typeof indexedDB.databases === "function") {
        return indexedDB.databases().then(
          function (list) {
            var found = (list || []).some(function (d) {
              return d && d.name === name;
            });
            return found ? open() : null;
          },
          function () {
            return open();
          }
        );
      }
    } catch (e) {}
    return open();
  }

  function getAll(db, store) {
    return new Promise(function (resolve) {
      try {
        var rq = db.transaction(store, "readonly").objectStore(store).getAll();
        rq.onsuccess = function () {
          resolve(rq.result || []);
        };
        rq.onerror = function () {
          resolve([]);
        };
      } catch (e) {
        resolve([]);
      }
    });
  }

  // last-wid-md is JSON like "15550001234:19@c.us". Return the bare user part
  // ("15550001234") and canonical wid ("…@c.us").
  function readWid() {
    var raw = localStorage.getItem("last-wid-md");
    if (!raw) return null;
    raw = raw.replace(/"/g, "");
    var user = raw.split(":")[0].split("@")[0];
    if (!user) return null;
    return { user: user, wid: user + "@c.us" };
  }

  function readEurl() {
    var e = localStorage.getItem("WACachedProfilePicEURL");
    return e ? e.replace(/"/g, "") : null;
  }

  // Does a contact row id denote this exact wid? Ids are "user@c.us" strings or
  // objects with _serialized / user+server; a ":device" part is ignored.
  function idMatches(id, w) {
    var ser = "";
    if (typeof id === "string") {
      ser = id;
    } else if (id && typeof id === "object") {
      ser = typeof id._serialized === "string" ? id._serialized : "";
      if (!ser && id.user && id.server) ser = id.user + "@" + id.server;
    }
    if (!ser) return false;
    return ser.replace(/:\d+@/, "@") === w.wid;
  }

  // Own display name from the model-storage contact table: the exact-wid row
  // that carries a name (other rows for the same wid may be empty).
  function readOwnName(w) {
    return openDB("model-storage").then(function (db) {
      if (!db) return null;
      return getAll(db, "contact").then(function (rows) {
        db.close();
        for (var i = 0; i < rows.length; i++) {
          var c = rows[i];
          if (!c || !idMatches(c.id, w)) continue;
          var n = c.name || c.pushname || c.notify || c.verifiedName;
          if (n) return n;
        }
        return null;
      });
    });
  }

  function fetchAvatarBase64(eurl) {
    return fetch(eurl)
      .then(function (resp) {
        var type = resp.headers && resp.headers.get ? resp.headers.get("content-type") || "" : "";
        if (!resp.ok || type.indexOf("image/") !== 0) {
          throw new Error("avatar response " + resp.status + " " + type);
        }
        return resp.blob();
      })
      .then(function (blob) {
        return new Promise(function (resolve, reject) {
          var fr = new FileReader();
          fr.onload = function () {
            var s = String(fr.result || "");
            var comma = s.indexOf(",");
            resolve(comma >= 0 ? s.slice(comma + 1) : s);
          };
          fr.onerror = function () {
            reject(fr.error || new Error("FileReader failed"));
          };
          fr.readAsDataURL(blob);
        });
      });
  }

  // De-dupe so we don't re-emit identical identity/avatar every poll.
  var lastIdentity = null;
  var lastEurl = null;
  var avatarBusy = false;
  var awaitingTimer = null;

  function emitAwaiting() {
    if (awaitingTimer) return;
    awaitingTimer = setTimeout(function () {
      awaitingTimer = null;
      if (!readWid()) send("AwaitingPairing");
    }, 500);
  }

  function tick() {
    var w = readWid();
    if (!w) {
      // Logged out: reset de-dup state so a later re-login re-emits identity
      // and avatar (otherwise the same wid/name would be suppressed).
      lastIdentity = null;
      lastEurl = null;
      emitAwaiting();
      return;
    }

    // Identity (wid + name).
    var identityDone = readOwnName(w)
      .then(function (name) {
        // No stored name: fall back to the phone number so pairing state clears.
        name = name || w.user;
        var key = w.wid + "|" + name;
        if (key !== lastIdentity) {
          lastIdentity = key;
          send("ProfileIdentity", { wid: w.wid, pushname: name, source: "store" });
        }
      })
      .catch(function () {});

    // Avatar (only when the cached EURL changes).
    var eurl = readEurl();
    if (eurl && eurl !== lastEurl && !avatarBusy) {
      avatarBusy = true;
      fetchAvatarBase64(eurl)
        .then(function (b64) {
          lastEurl = eurl;
          avatarBusy = false;
          send("ProfileAvatar", { base64_png: b64, source: "store" });
        })
        .catch(function (e) {
          // lastEurl stays unchanged so the next tick retries.
          avatarBusy = false;
          send("ConsoleLog", {
            level: "warn",
            msg: "karere store-hook: avatar fetch failed: " + (e && e.message ? e.message : e),
          });
        });
    }
    return identityDone;
  }

  // Poll to catch pairing and later name/avatar changes.
  // Fast while unpaired / no identity yet; once identity is out, back off.
  function loop() {
    function next() {
      setTimeout(loop, lastIdentity ? 60000 : 3000);
    }
    try {
      Promise.resolve(tick()).then(next, next);
    } catch (e) {
      next();
    }
  }
  loop();
})();
