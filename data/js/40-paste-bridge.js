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

  // WhatsApp mounts its "Drag file here" dropzone overlay asynchronously (React)
  // on the first dragenter/dragover. Keep nudging dragover at the point until a
  // new topmost element appears (the overlay) or we time out (~600 ms), then
  // return that element so the caller drops on it — giving single-drag drops.
  async function waitForDropzone(x, y, original, opts) {
    var last = original;
    for (var i = 0; i < 15; i++) {
      var el = document.elementFromPoint(x, y) || original;
      if (el && el !== original && el !== document.body && el !== document.documentElement) {
        el.dispatchEvent(new DragEvent("dragenter", opts));
        el.dispatchEvent(new DragEvent("dragover", opts));
        return el;
      }
      el.dispatchEvent(new DragEvent("dragover", opts));
      last = el;
      await sleep(40);
    }
    return last;
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
      target = document.activeElement;
      if (!target || target === document.body || target === document.documentElement) {
        warn("karere paste bridge: no focused element for paste");
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
      target.dispatchEvent(new DragEvent("dragenter", dndOpts));
      target.dispatchEvent(new DragEvent("dragover", dndOpts));
      // Wait for WhatsApp's async dropzone overlay to mount, then drop on it.
      var dropTarget = await waitForDropzone(detail.x, detail.y, target, dndOpts);
      try {
        console.log("karere drop: final target=" + (dropTarget && dropTarget.tagName));
      } catch (e) {}
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
  } catch (err) {
    warn("karere paste bridge install failed: " + (err && err.stack ? err.stack : err));
  }
})();
