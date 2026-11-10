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
      get body() {
        return state.body || null;
      },
      elementFromPoint: () => state.point || null,
      dispatchEvent(e) {
        (state.docEvents = state.docEvents || []).push(e);
      },
    },
    DataTransfer: class {
      constructor() {
        this.files = [];
        this.items = {
          add: (f) => {
            this.files.push(f);
            this.file = f;
          },
        };
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
    DragEvent: class {
      constructor(type, init) {
        this.type = type;
        this.dataTransfer = init.dataTransfer;
        this.clientX = init.clientX;
        this.clientY = init.clientY;
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

  // A batch is ONE paste carrying every file, not one paste per file. (J4)
  {
    const state = { active: el(false), composer: el(true) };
    const { fire, acks } = load(state);
    fire({
      kind: "paste",
      files: [
        { mime: "image/png", name: "a.png", payload: { kind: "Base64", data: "iVBORw0KGgo=" } },
        { mime: "application/pdf", name: "b.pdf", payload: { kind: "Base64", data: "JVBERg==" } },
        { mime: "image/jpeg", name: null, payload: { kind: "Base64", data: "/9j/" } },
      ],
    });
    await tick();
    assert.equal(state.composer.events.length, 1);
    const files = state.composer.events[0].clipboardData.files;
    assert.deepEqual(
      files.map((f) => [f.name, f.type]),
      [
        ["a.png", "image/png"],
        ["b.pdf", "application/pdf"],
        ["paste.jpg", "image/jpeg"],
      ]
    );
    assert.equal(acks.length, 1);
  }

  // A multi-file drop delivers exactly one drop event holding every file.
  {
    const state = { active: el(false), composer: el(true), body: el(false) };
    const { fire, acks } = load(state);
    fire({
      kind: "drop",
      x: 10,
      y: 20,
      files: [
        { mime: "image/png", name: "a.png", payload: { kind: "Base64", data: "iVBORw0KGgo=" } },
        { mime: "image/png", name: "b.png", payload: { kind: "Base64", data: "iVBORw0KGgo=" } },
      ],
    });
    await new Promise((r) => setTimeout(r, 150));
    const drops = state.body.events.filter((e) => e.type === "drop");
    assert.equal(drops.length, 1);
    assert.deepEqual(
      drops[0].dataTransfer.files.map((f) => f.name),
      ["a.png", "b.png"]
    );
    assert.equal(drops[0].clientX, 10);
    assert.equal(acks.length, 1);
  }

  // The legacy single-file drop shape still works.
  {
    const state = { active: el(false), composer: el(true), body: el(false) };
    const { fire } = load(state);
    fire({ ...image, kind: "drop", name: "x.png", x: 1, y: 2 });
    await new Promise((r) => setTimeout(r, 150));
    const drops = state.body.events.filter((e) => e.type === "drop");
    assert.equal(drops.length, 1);
    assert.equal(drops[0].dataTransfer.files.length, 1);
  }

  // A dropped text lands in the editable under the drop point as a paste,
  // after any primed file dropzone was dismissed.
  {
    const state = { active: el(false), composer: el(true), point: el(true) };
    const { fire, acks } = load(state);
    fire({ mime: "text/plain", kind: "drop", x: 5, y: 6, payload: { kind: "Base64", data: "aGk=" } });
    await new Promise((r) => setTimeout(r, 150));
    assert.equal(state.point.events.length, 1);
    assert.equal(state.point.events[0].type, "paste");
    assert.equal(state.point.events[0].clipboardData.data["text/plain"], "hi");
    assert.equal(state.docEvents[0].type, "dragleave");
    assert.equal(state.composer.events.length, 0);
    assert.equal(acks.length, 1);
  }

  // A text drop outside any editable inserts nothing, and is not a file drop.
  {
    const state = { active: el(false), composer: el(true), point: el(false), body: el(false) };
    const { fire, acks } = load(state);
    fire({ mime: "text/plain", kind: "drop", x: 5, y: 6, payload: { kind: "Base64", data: "aGk=" } });
    await new Promise((r) => setTimeout(r, 150));
    assert.equal(state.point.events.length, 0);
    assert.equal(state.body.events.length, 0);
    assert.equal(state.composer.events.length, 0);
    assert.equal(acks.length, 1);
  }

  console.log("paste bridge: ok");
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
