// 60-store-hook.js — first-class identity source (M20).
//
// WhatsApp's Store/webpack internals are unreachable: the app restores native
// Array.push on its webpack chunk array post-load, so the @wppconnect/wa-js
// "parasite chunk" technique can't capture __webpack_require__ (verified).
//
// Instead read the account's own identity from data the app persists (stable
// and present once paired):
//   • wid     — localStorage['last-wid-md']            (e.g. "353830357840:19@c.us")
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

  function send(name, payload) {
    try {
      if (window.karere && typeof window.karere.send === "function") {
        window.karere.send(name, payload === undefined ? "" : JSON.stringify(payload));
      }
    } catch (e) {
      /* channel unavailable — drop silently */
    }
  }

  function openDB(name) {
    return new Promise(function (resolve) {
      try {
        var req = indexedDB.open(name);
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

  // last-wid-md is JSON like "353830357840:19@c.us". Return the bare user part
  // ("353830357840") and canonical wid ("…@c.us").
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

  // Own display name from the model-storage contact table.
  function readOwnName(user) {
    return openDB("model-storage").then(function (db) {
      if (!db) return null;
      return getAll(db, "contact").then(function (rows) {
        db.close();
        for (var i = 0; i < rows.length; i++) {
          var c = rows[i];
          var idStr = "";
          try {
            idStr = JSON.stringify(c.id || "");
          } catch (e) {
            idStr = String(c.id || "");
          }
          if (idStr.indexOf(user) !== -1) {
            return c.name || c.pushname || c.notify || c.verifiedName || null;
          }
        }
        return null;
      });
    });
  }

  function fetchAvatarBase64(eurl) {
    return fetch(eurl)
      .then(function (resp) {
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
    readOwnName(w.user)
      .then(function (name) {
        var key = w.wid + "|" + (name || "");
        if (key !== lastIdentity && name) {
          lastIdentity = key;
          send("ProfileIdentity", { wid: w.wid, pushname: name, source: "store" });
        }
      })
      .catch(function () {});

    // Avatar (only when the cached EURL changes).
    var eurl = readEurl();
    if (eurl && eurl !== lastEurl) {
      lastEurl = eurl;
      fetchAvatarBase64(eurl)
        .then(function (b64) {
          send("ProfileAvatar", { base64_png: b64, source: "store" });
        })
        .catch(function (e) {
          // Allow a retry next tick.
          lastEurl = null;
          send("ConsoleLog", {
            level: "warn",
            msg: "karere store-hook: avatar fetch failed: " + (e && e.message ? e.message : e),
          });
        });
    }
  }

  // Poll to catch pairing and later name/avatar changes.
  tick();
  setInterval(tick, 3000);
})();
