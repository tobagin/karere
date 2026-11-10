// 40-paste-bridge.js — M17 GDK→page paste/drop bridge (renderer side).
//
// Browser intercepts Ctrl+V (image/file), drag-drop, and middle-click PRIMARY
// text, sends DispatchPasteEvent / DispatchPasteFiles over IPC; render_process.rs
// re-emits it as a window CustomEvent `karere:dispatch-paste` with detail:
//
//   single: { mime, kind: "paste"|"drop", name?, x?, y?,
//             payload: { kind: "Base64", data } }
//   batch:  { kind, x?, y?, files: [{ mime, name?, payload: { kind: "Base64", data } }] }
//
// x/y are page CSS px. We rebuild ONE DataTransfer (every File of a batch, or
// text/plain for middle-click) and dispatch a synthetic paste (on activeElement)
// or drop (on element under cursor), then ack via PasteConsumed.
//
// Wrapped in try/catch so a throw never takes down the renderer.
(function () {
  "use strict";

  // CEF may recreate/inject the bundle more than once for the same Window.
  // Keep exactly one paste listener, otherwise one Ctrl+V inserts text twice.
  if (window.__karerePasteBridgeInstalled) return;
  window.__karerePasteBridgeInstalled = true;

  // `tempfile_path` stays in the ack for the host's message shape; payloads are
  // always inline now, so it is always null.
  function ack() {
    try {
      if (typeof window.karere_send === "function") {
        window.karere_send("PasteConsumed", JSON.stringify({ tempfile_path: null }));
      }
    } catch (e) {
      /* channel unavailable — drop silently */
    }
  }

  function warn(msg) {
    try {
      console.warn(msg);
    } catch (e) {}
  }

  // Can receive a text paste: a form field or contenteditable descendant (the
  // WhatsApp composer). isContentEditable is inherited, so descendants match.
  function isEditable(el) {
    if (!el) return false;
    if (el.isContentEditable) return true;
    var tag = el.tagName;
    return tag === "INPUT" || tag === "TEXTAREA";
  }

  // The message composer, for file/image pastes when focus is elsewhere (e.g. on
  // the chat pane after a click). Null when it is not on screen.
  function findComposer() {
    try {
      return (
        document.querySelector('footer [contenteditable="true"]') ||
        document.querySelector('[contenteditable="true"][role="textbox"]')
      );
    } catch (e) {
      return null;
    }
  }

  // Map an image MIME to a file extension for the synthesized `paste.<ext>`.
  function extFromMime(mime) {
    var map = {
      "image/png": "png",
      "image/jpeg": "jpg",
      "image/gif": "gif",
      "image/webp": "webp",
      "image/bmp": "bmp",
    };
    if (map[mime]) return map[mime];
    var slash = mime.indexOf("/");
    var sub = slash >= 0 ? mime.slice(slash + 1) : "";
    // Strip any parameters (e.g. "svg+xml" -> "svg").
    sub = sub.split(/[+;]/)[0];
    return sub || "bin";
  }

  function base64ToBytes(b64) {
    var binary = atob(b64);
    var len = binary.length;
    var bytes = new Uint8Array(len);
    for (var i = 0; i < len; i++) bytes[i] = binary.charCodeAt(i);
    return bytes;
  }

  function sleep(ms) {
    return new Promise(function (r) {
      setTimeout(r, ms);
    });
  }

  // ---- Hover-driven dropzone priming ----------------------------------
  // CEF delivers the drop only on release, but GTK reports drag hover
  // (enter/motion/leave) live. Replay those as synthetic dragenter/dragover with
  // a Files-typed DataTransfer so WhatsApp mounts its dropzone overlay DURING the
  // hover — then the real drop lands on it.
  var hoverDT = null;
  function getHoverDT() {
    if (!hoverDT) {
      hoverDT = new DataTransfer();
      try {
        hoverDT.items.add(new File([new Uint8Array(0)], "drag"));
      } catch (e) {}
    }
    return hoverDT;
  }

  // A release-time "leave" would dismiss the overlay just before the drop; delay
  // it so a drop arriving within the window can cancel it.
  var pendingLeave = 0;
  function cancelPendingLeave() {
    if (pendingLeave) {
      clearTimeout(pendingLeave);
      pendingLeave = 0;
    }
  }

  function handleHover(d) {
    var opts = {
      dataTransfer: getHoverDT(),
      clientX: d.x,
      clientY: d.y,
      bubbles: true,
      cancelable: true,
    };
    if (d.phase === "leave") {
      cancelPendingLeave();
      pendingLeave = setTimeout(function () {
        pendingLeave = 0;
        document.dispatchEvent(new DragEvent("dragleave", opts));
      }, 250);
      return;
    }
    cancelPendingLeave();
    var el = document.elementFromPoint(d.x, d.y) || document.body;
    if (d.phase === "enter") el.dispatchEvent(new DragEvent("dragenter", opts));
    el.dispatchEvent(new DragEvent("dragover", opts));
    // React's delegated drag listener sits at the document root.
    document.dispatchEvent(new DragEvent("dragover", opts));
  }

  // One {mime, name?, payload} entry → File.
  function entryToFile(entry) {
    var mime = entry.mime || "application/octet-stream";
    var blob = new Blob([base64ToBytes(entry.payload.data)], { type: mime });
    return new File([blob], entry.name || "paste." + extFromMime(mime), { type: mime });
  }

  async function handle(detail) {
    var kind = detail.kind === "drop" ? "drop" : "paste";
    var batch = Array.isArray(detail.files) ? detail.files : null;
    // Middle-click / Ctrl+V text: no File, just a text/plain entry. A named
    // text/plain payload is a pasted text *file*, which attaches like any other.
    var isText = !batch && kind === "paste" && detail.mime === "text/plain" && !detail.name;

    // Target: focused element for paste, element-under-cursor for drop.
    var target;
    if (kind === "drop") {
      // Drop listeners often sit on a container/document; if the point misses,
      // fall back to body so the bubbling sequence still reaches them.
      target =
        document.elementFromPoint(detail.x, detail.y) || document.body || document.documentElement;
      try {
        console.log(
          "karere drop: target=" +
            (target && target.tagName) +
            " @(" + detail.x + "," + detail.y + ")"
        );
      } catch (e) {}
    } else {
      // Middle-click (text/plain with coords) targets element-under-cursor;
      // Ctrl+V targets the focused element. Target must be editable — a
      // middle-click on a link/message text pastes nothing.
      if (isText && typeof detail.x === "number") {
        target = document.elementFromPoint(detail.x, detail.y);
      } else {
        target = document.activeElement;
      }
      if (!isEditable(target) && !isText) {
        // Files/images are not text: send them to the composer instead of dropping.
        target = findComposer() || target;
      }
      if (!isEditable(target)) {
        if (!isText) {
          warn("karere paste bridge: no editable target for paste");
        }
        ack();
        return;
      }
    }

    var dt = new DataTransfer();

    if (isText) {
      var text = "";
      try {
        text = new TextDecoder().decode(base64ToBytes(detail.payload.data));
      } catch (e) {
        text = "";
      }
      dt.setData("text/plain", text);
      target.dispatchEvent(
        new ClipboardEvent("paste", {
          clipboardData: dt,
          bubbles: true,
          cancelable: true,
        })
      );
      ack();
      return;
    }

    // Binary payload(s) → File(s), all in this one DataTransfer so the page
    // sees a single multi-file paste/drop.
    (batch || [detail]).forEach(function (entry) {
      dt.items.add(entryToFile(entry));
    });

    if (kind === "drop") {
      // Full DnD sequence — WhatsApp dropzones gate the drop on prior
      // dragenter/dragover (and expect preventDefault on dragover).
      var dndOpts = {
        dataTransfer: dt,
        clientX: detail.x,
        clientY: detail.y,
        bubbles: true,
        cancelable: true,
      };
      // Overlay was mounted during hover; cancel any release-time leave,
      // re-assert the drag, then drop on the topmost element (the overlay,
      // which elementFromPoint now returns).
      cancelPendingLeave();
      target.dispatchEvent(new DragEvent("dragenter", dndOpts));
      target.dispatchEvent(new DragEvent("dragover", dndOpts));
      await sleep(60);
      var dropTarget = document.elementFromPoint(detail.x, detail.y) || target;
      try {
        console.log("karere drop: final target=" + (dropTarget && dropTarget.tagName));
      } catch (e) {}
      dropTarget.dispatchEvent(new DragEvent("dragover", dndOpts));
      dropTarget.dispatchEvent(new DragEvent("drop", dndOpts));
    } else {
      target.dispatchEvent(
        new ClipboardEvent("paste", {
          clipboardData: dt,
          bubbles: true,
          cancelable: true,
        })
      );
    }

    ack();
  }

  try {
    window.addEventListener("karere:dispatch-paste", function (ev) {
      try {
        handle(ev.detail).catch(function (err) {
          warn("karere paste bridge failed: " + (err && err.stack ? err.stack : err));
          ack();
        });
      } catch (err) {
        warn("karere paste bridge threw: " + (err && err.stack ? err.stack : err));
      }
    });

    // Real-time drag hover (enter/over/leave) → prime WhatsApp's dropzone.
    window.addEventListener("karere:drag-hover", function (ev) {
      try {
        handleHover(ev.detail || {});
      } catch (e) {}
    });
  } catch (err) {
    warn("karere paste bridge install failed: " + (err && err.stack ? err.stack : err));
  }
})();
