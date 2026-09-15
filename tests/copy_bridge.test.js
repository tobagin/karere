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
});
