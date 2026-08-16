// Execute the production copy bridge as the renderer would after Rust dispatches
// its fixed JavaScript operation. Prints the exact karere_send(name, json) args.
"use strict";

const fs = require("node:fs");
const vm = require("node:vm");

const selectionText = Buffer.from(process.argv[2] || "", "base64").toString("utf8");
const collapsed = process.argv[3] === "true";
const executeScript = Buffer.from(process.argv[4] || "", "base64").toString("utf8");
const windowListeners = new Map();
const documentListeners = new Map();
let sent = null;

const context = {
  CustomEvent: function CustomEvent(type) { this.type = type; },
  window: {
    karere_send(name, innerJson) { sent = { name, innerJson }; },
    getSelection() {
      return { isCollapsed: collapsed, toString: () => selectionText };
    },
    addEventListener(name, callback) { windowListeners.set(name, callback); },
    dispatchEvent(event) {
      const callback = windowListeners.get(event.type);
      if (callback) callback(event);
    },
  },
  document: {
    addEventListener(name, callback) { documentListeners.set(name, callback); },
  },
  console: { log() {}, error() {} },
  setTimeout() { return 1; },
  clearTimeout() {},
};
vm.createContext(context);
vm.runInContext(fs.readFileSync("data/js/50-copy-bridge.js", "utf8"), context, {
  filename: "50-copy-bridge.js",
});
vm.runInContext(executeScript, context, { filename: "karere://copy-selection" });
process.stdout.write(JSON.stringify(sent));
