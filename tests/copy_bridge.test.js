// Regression harness for data/js/50-copy-bridge.js. Uses only Node built-ins.
"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");

const source = fs.readFileSync("data/js/50-copy-bridge.js", "utf8");
const fixture = fs.readFileSync("tests/fixtures/text-selection.html", "utf8");
const listeners = new Map();
const hostListeners = new Map();
const sent = [];
const nativeWrites = [];
let selection = { text: "", collapsed: true };
let pendingTimer = null;

const context = {
  window: {
    karere_send(name, json) {
      sent.push({ name, payload: JSON.parse(json) });
    },
    getSelection() {
      return {
        isCollapsed: selection.collapsed,
        toString: () => selection.text,
      };
    },
    addEventListener(name, callback) {
      hostListeners.set(name, callback);
    },
  },
  document: {
    addEventListener(name, callback) {
      listeners.set(name, callback);
    },
  },
  // Stand-in for the browser FileReader: blobs here are { dataUrl }.
  FileReader: class {
    readAsDataURL(blob) {
      this.result = blob.dataUrl;
      Promise.resolve().then(() => this.onload());
    }
  },
  navigator: {
    clipboard: {
      writeText(text) {
        nativeWrites.push(text);
        return Promise.resolve();
      },
      write(items) {
        nativeWrites.push(items);
        return Promise.resolve();
      },
    },
  },
  console: { log() {}, error() {} },
  setTimeout(callback) {
    pendingTimer = callback;
    return 1;
  },
  clearTimeout() {
    pendingTimer = null;
  },
};
vm.createContext(context);
vm.runInContext(source, context, { filename: "50-copy-bridge.js" });

// Selection alone targets PRIMARY after its debounce.
assert.match(fixture, /Select and copy this multi-word Unicode message 🙂/);
selection = { text: "first line\nZażółć 🙂", collapsed: false };
listeners.get("selectionchange")();
assert.equal(sent.length, 0);
pendingTimer();
assert.deepEqual(sent.pop(), {
  name: "SetClipboard",
  payload: { text: selection.text, primary: true },
});

// Immediate explicit copy does not wait for that debounce and targets CLIPBOARD.
sent.length = 0;
pendingTimer = null;
listeners.get("selectionchange")();
hostListeners.get("karere:copy-selection")();
assert.deepEqual(sent, [{
  name: "SetClipboard",
  payload: { text: selection.text, primary: false },
}]);

// Duplicate host requests are harmless and preserve the exact Unicode text.
hostListeners.get("karere:copy-selection")();
assert.equal(sent.length, 2);
assert.equal(sent[1].payload.text, selection.text);

// Empty and collapsed selections never request a regular clipboard write.
sent.length = 0;
selection = { text: "", collapsed: true };
hostListeners.get("karere:copy-selection")();
selection = { text: "", collapsed: false };
hostListeners.get("karere:copy-selection")();
assert.deepEqual(sent, []);

// Context-menu/DOM Copy fallback uses current selection without localized labels.
selection = { text: "menu selection", collapsed: false };
let prevented = false;
listeners.get("copy")({
  clipboardData: { getData: () => "" },
  preventDefault() { prevented = true; },
});
assert.equal(prevented, true);
assert.deepEqual(sent.pop(), {
  name: "SetClipboard",
  payload: { text: "menu selection", primary: false },
});

// A copy event whose clipboardData carries text/html sends both flavors.
sent.length = 0;
listeners.get("copy")({
  clipboardData: { getData: (t) => (t === "text/html" ? "<b>rich</b>" : "rich") },
  preventDefault() {},
});
assert.deepEqual(sent.pop(), {
  name: "SetClipboard",
  payload: { text: "rich", primary: false, html: "<b>rich</b>" },
});

// Cut mirrors the selection to CLIPBOARD from the capture phase, before the
// page can stop propagation or delete it, and never cancels the deletion.
sent.length = 0;
selection = { text: "cut me 🙂", collapsed: false };
prevented = false;
const cutEvent = {
  clipboardData: { getData: () => "" },
  preventDefault() { prevented = true; },
};
hostListeners.get("cut")(cutEvent);
listeners.get("cut")(cutEvent);
assert.equal(prevented, false);
assert.deepEqual(sent, [{
  name: "SetClipboard",
  payload: { text: "cut me 🙂", primary: false },
}]);

// Page-set clipboardData that differs from the raw selection wins.
sent.length = 0;
selection = { text: "hi ", collapsed: false };
const pageCut = { clipboardData: { getData: () => "hi 😀" }, preventDefault() {} };
hostListeners.get("cut")(pageCut);
listeners.get("cut")(pageCut);
assert.deepEqual(sent.map((m) => m.payload.text), ["hi ", "hi 😀"]);

// Text selected inside a plain input/textarea is read from its value.
sent.length = 0;
selection = { text: "", collapsed: true };
context.document.activeElement = {
  tagName: "TEXTAREA",
  value: "keep CUT keep",
  selectionStart: 5,
  selectionEnd: 8,
};
hostListeners.get("cut")(cutEvent);
listeners.get("cut")(cutEvent);
assert.deepEqual(sent, [{
  name: "SetClipboard",
  payload: { text: "CUT", primary: false },
}]);
context.document.activeElement = undefined;

// An empty cut mirrors nothing (existing host clipboard preserved).
sent.length = 0;
hostListeners.get("cut")(cutEvent);
listeners.get("cut")(cutEvent);
assert.deepEqual(sent, []);

console.log("copy bridge regression: ok");

// WhatsApp's own message-menu "Copy" goes through the async Clipboard API;
// it must mirror to the host CLIPBOARD and still reach the native writer.
sent.length = 0;
nativeWrites.length = 0;
context.navigator.clipboard.writeText("menu copy 🙂");
assert.deepEqual(sent.pop(), {
  name: "SetClipboard",
  payload: { text: "menu copy 🙂", primary: false },
});
assert.deepEqual(nativeWrites, ["menu copy 🙂"]);

// Empty writeText mirrors nothing (existing host clipboard preserved).
sent.length = 0;
context.navigator.clipboard.writeText("");
assert.equal(sent.length, 0);

// ClipboardItem write: text/plain items mirror asynchronously; the promise
// chain resolves on the microtask queue, so await it.
const item = {
  types: ["text/plain"],
  getType: () => Promise.resolve({ text: () => Promise.resolve("item copy") }),
};
context.navigator.clipboard.write([item]);
setImmediate(() => {
  assert.deepEqual(sent.pop(), {
    name: "SetClipboard",
    payload: { text: "item copy", primary: false },
  });
  console.log("copy bridge clipboard api: ok");

  // A text/plain + text/html ClipboardItem sends both in one message.
  sent.length = 0;
  context.navigator.clipboard.write([{
    types: ["text/plain", "text/html"],
    getType: (t) =>
      Promise.resolve({ text: () => Promise.resolve(t === "text/html" ? "<i>x</i>" : "x") }),
  }]);
  setTimeout(() => {
    assert.deepEqual(sent.filter((m) => m.name === "SetClipboard"), [{
      name: "SetClipboard",
      payload: { text: "x", primary: false, html: "<i>x</i>" },
    }]);
    console.log("copy bridge clipboard html: ok");
  }, 0);

  // An image/png ClipboardItem ("Copy sticker image", #196) mirrors as base64.
  sent.length = 0;
  context.navigator.clipboard.write([{
    types: ["image/png"],
    getType: () => Promise.resolve({ dataUrl: "data:image/png;base64,iVBORw0KGgo=" }),
  }]);
  setTimeout(() => {
    assert.deepEqual(sent.filter((m) => m.name === "SetClipboardImage"), [{
      name: "SetClipboardImage",
      payload: { base64_png: "iVBORw0KGgo=" },
    }]);
    console.log("copy bridge clipboard image: ok");
  }, 0);
});
