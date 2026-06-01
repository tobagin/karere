// 40-paste-bridge.js — M17 GDK→page paste/drop bridge (renderer side).
//
// The browser process intercepts Ctrl+V (image/file), file drag-drop, and
// middle-click primary-clipboard text, then sends a `DispatchPasteEvent` over
// IPC. `render_process.rs` turns that into a `window` CustomEvent named
// `karere:dispatch-paste` whose `detail` is:
//
//   { mime, kind: "paste"|"drop", name?, x?, y?,
//     payload: { kind: "Base64", data } | { kind: "FilePath", path } }
//
// This script reconstructs a `DataTransfer` (a `File` for binary payloads, or a
// `text/plain` string for middle-click text) and dispatches a synthetic `paste`
// (on `document.activeElement`) or `drop` (on the element under the cursor),
// then acks via `PasteConsumed` so the host can unlink any tempfile.
//
// Wrapped in try/catch so a throw never takes down the renderer.
(function () {
  "use strict";

  function ack(tempfilePath) {
    try {
      if (typeof window.karere_send === "function") {
        window.karere_send(
          "PasteConsumed",
          JSON.stringify({ tempfile_path: tempfilePath || null })
        );
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

  // An element that can receive a text paste: a form field or anything inside a
  // contenteditable (the WhatsApp composer). `isContentEditable` is inherited,
  // so it's true for descendants of the composer too.
  function isEditable(el) {
    if (!el) return false;
    if (el.isContentEditable) return true;
    var tag = el.tagName;
    return tag === "INPUT" || tag === "TEXTAREA";
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
  // CEF only delivers the actual drop on release, but GTK reports the drag hover
  // (enter/motion/leave) in real time. We replay those as synthetic
  // dragenter/dragover carrying a Files-typed DataTransfer so WhatsApp mounts its
  // dropzone overlay DURING the physical hover — then the real drop lands on it.
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

  // A "leave" fired on release would dismiss the overlay just before the drop;
  // delay it so a drop arriving within the window can cancel it.
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

  // Resolve a payload object into a Blob.
  async function payloadToBlob(payload, mime) {
    if (payload.kind === "FilePath") {
      var resp = await fetch("file://" + payload.path);
      return await resp.blob();
    }
    // Base64
    return new Blob([base64ToBytes(payload.data)], { type: mime });
  }

  async function handle(detail) {
    var kind = detail.kind === "drop" ? "drop" : "paste";

    // Target element: focused element for paste, element-under-cursor for drop.
    var target;
    if (kind === "drop") {
      // Drop listeners commonly sit on a container/document; if the exact point
      // misses, fall back to body so the (bubbling) sequence still reaches them.
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
      // Middle-click (text/plain with coords) targets the element UNDER the
      // cursor; Ctrl+V (image/file/text) targets the focused element. Either
      // way the target must be editable — a middle-click on a link or message
      // text pastes nothing (the click just opens the link).
      if (detail.mime === "text/plain" && typeof detail.x === "number") {
        target = document.elementFromPoint(detail.x, detail.y);
      } else {
        target = document.activeElement;
      }
      if (!isEditable(target)) {
        if (detail.mime !== "text/plain") {
          warn("karere paste bridge: no editable target for paste");
        }
        ack(detail.payload && detail.payload.path);
        return;
      }
    }

    var dt = new DataTransfer();

    // Middle-click text paste: no File, just a text/plain entry.
    if (kind === "paste" && detail.mime === "text/plain") {
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
      ack(null);
      return;
    }

    // Binary payload → File.
    var blob = await payloadToBlob(detail.payload, detail.mime);
    var filename =
      detail.name || "paste." + extFromMime(detail.mime || "application/octet-stream");
    var file = new File([blob], filename, {
      type: detail.mime || blob.type || "application/octet-stream",
    });
    dt.items.add(file);

    if (kind === "drop") {
      // Dispatch the full DnD sequence — React/WhatsApp dropzones gate the drop
      // on prior dragenter/dragover (and expect preventDefault on dragover).
      var dndOpts = {
        dataTransfer: dt,
        clientX: detail.x,
        clientY: detail.y,
        bubbles: true,
        cancelable: true,
      };
      // The overlay was mounted during the hover; keep a release-time leave from
      // dismissing it, re-assert the drag, then drop on the topmost element
      // (the overlay, which elementFromPoint now returns since it's on top).
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

    ack(detail.payload && detail.payload.kind === "FilePath" ? detail.payload.path : null);
  }

  try {
    window.addEventListener("karere:dispatch-paste", function (ev) {
      try {
        handle(ev.detail).catch(function (err) {
          warn("karere paste bridge failed: " + (err && err.stack ? err.stack : err));
          // Best-effort tempfile reclaim even on failure.
          var d = ev.detail || {};
          ack(d.payload && d.payload.kind === "FilePath" ? d.payload.path : null);
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
