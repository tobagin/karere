// Regression harness for data/js/40-paste-bridge.js target selection (Ctrl+V
// fallback to the composer). Uses only Node built-ins.
"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");

const source = fs.readFileSync("data/js/40-paste-bridge.js", "utf8");

function load(state) {
  const listeners = new Map();
  const acks = [];
  const context = {
    window: {
      karere_send(name, json) {
        acks.push({ name, payload: JSON.parse(json) });
      },
      addEventListener(name, cb) {
        listeners.set(name, cb);
      },
    },
    document: {
      get activeElement() {
        return state.active;
      },
      querySelector(sel) {
        return sel.indexOf("footer") !== -1 ? state.composer : null;
      },
      elementFromPoint: () => null,
    },
    DataTransfer: class {
      constructor() {
        this.items = { add: (f) => (this.file = f) };
        this.data = {};
      }
      setData(k, v) {
        this.data[k] = v;
      }
    },
    File: class {
      constructor(parts, name, opts) {
        this.name = name;
        this.type = opts && opts.type;
      }
    },
    Blob: class {},
    ClipboardEvent: class {
      constructor(type, init) {
        this.type = type;
        this.clipboardData = init.clipboardData;
      }
    },
    atob: (s) => Buffer.from(s, "base64").toString("binary"),
    Uint8Array,
    TextDecoder,
    console: { log() {}, warn() {}, error() {} },
    setTimeout,
    clearTimeout,
  };
  vm.createContext(context);
  vm.runInContext(source, context, { filename: "40-paste-bridge.js" });
  return { acks, fire: (detail) => listeners.get("karere:dispatch-paste")({ detail }) };
}

const tick = () => new Promise((r) => setImmediate(r));
const el = (editable) => ({
  isContentEditable: editable,
  tagName: "DIV",
  events: [],
  dispatchEvent(e) {
    this.events.push(e);
  },
});
const image = { mime: "image/png", kind: "paste", payload: { kind: "Base64", data: "iVBORw0KGgo=" } };

(async () => {
  // Focus on a non-editable element: image paste goes to the composer.
  {
    const state = { active: el(false), composer: el(true) };
    const { fire, acks } = load(state);
    fire(image);
    await tick();
    assert.equal(state.composer.events.length, 1);
    assert.equal(state.composer.events[0].type, "paste");
    assert.equal(state.active.events.length, 0);
    assert.equal(acks[0].name, "PasteConsumed");
  }

  // Focus already in an editable: it keeps the paste.
  {
    const state = { active: el(true), composer: el(true) };
    const { fire } = load(state);
    fire(image);
    await tick();
    assert.equal(state.active.events.length, 1);
    assert.equal(state.composer.events.length, 0);
  }

  // No composer on screen: nothing is dispatched, tempfile still acked.
  {
    const state = { active: el(false), composer: null };
    const { fire, acks } = load(state);
    fire(image);
    await tick();
    assert.equal(state.active.events.length, 0);
    assert.equal(acks[0].name, "PasteConsumed");
  }

  // Plain text without a name is not redirected to the composer.
  {
    const state = { active: el(false), composer: el(true) };
    const { fire } = load(state);
    fire({ mime: "text/plain", kind: "paste", payload: { kind: "Base64", data: "aGk=" } });
    await tick();
    assert.equal(state.composer.events.length, 0);
  }

  console.log("paste bridge: ok");
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
