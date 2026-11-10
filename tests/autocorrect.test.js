// Regression harness for data/js/30-autocorrect.js. Uses only Node built-ins.
"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");

const source = fs.readFileSync("data/js/30-autocorrect.js", "utf8");

function load(lang) {
  const listeners = new Map();
  const context = {
    window: { __karereAutoCorrect: true },
    document: {
      documentElement: { lang },
      addEventListener(name, cb) {
        listeners.set(name, cb);
      },
    },
    navigator: { language: "xx" },
    Event: class {},
  };
  vm.createContext(context);
  vm.runInContext(source, context, { filename: "30-autocorrect.js" });
  return { context, fire: (ev) => listeners.get("input")(ev) };
}

function field(value, type) {
  return {
    tagName: "INPUT",
    type: type || "text",
    value,
    selectionStart: value.length,
    selectionEnd: value.length,
    dispatchEvent() {},
  };
}

const typed = (t, extra) => Object.assign({ target: t, inputType: "insertText", data: " ", isComposing: false }, extra);

// Correction on a boundary character.
{
  const { fire } = load("en");
  const t = field("say teh ");
  fire(typed(t));
  assert.equal(t.value, "say the ");
  assert.equal(t.selectionStart, 8);
}

// English regional variants and navigator fallback are accepted.
{
  const t = field("teh ");
  load("en-GB").fire(typed(t));
  assert.equal(t.value, "the ");
}

// No re-correction after undo / paste / delete input types.
for (const inputType of ["historyUndo", "historyRedo", "insertFromPaste", "deleteContentBackward"]) {
  const { fire } = load("en");
  const t = field("teh ");
  fire(typed(t, { inputType, data: null }));
  assert.equal(t.value, "teh ", inputType);
}

// Composition is ignored.
{
  const { fire } = load("en");
  const t = field("teh ");
  fire(typed(t, { isComposing: true }));
  assert.equal(t.value, "teh ");
}

// Non-English page language: "adn" (a real word in es/pt/fr/it) is untouched.
{
  const { fire } = load("es");
  const t = field("el adn ");
  fire(typed(t));
  assert.equal(t.value, "el adn ");
}

// Host hook can switch the language gate in both directions.
{
  const { context, fire } = load("es");
  assert.equal(typeof context.window.__karereAutocorrectSetEnabled, "function");
  context.window.__karereAutocorrectSetEnabled(true);
  const t = field("teh ");
  fire(typed(t));
  assert.equal(t.value, "the ");
  context.window.__karereAutocorrectSetEnabled(false);
  const u = field("teh ");
  fire(typed(u));
  assert.equal(u.value, "teh ");
}

// Disabled by the user toggle.
{
  const { context, fire } = load("en");
  context.window.__karereAutoCorrect = false;
  const t = field("teh ");
  fire(typed(t));
  assert.equal(t.value, "teh ");
}

// Password / email / number fields are never touched.
for (const type of ["password", "email", "number", "url", "tel"]) {
  const { fire } = load("en");
  const t = field("teh ", type);
  fire(typed(t));
  assert.equal(t.value, "teh ", type);
}

// Search inputs and textareas are corrected.
{
  const { fire } = load("en");
  const t = field("teh ", "search");
  fire(typed(t));
  assert.equal(t.value, "the ");
  const a = field("teh ");
  a.tagName = "TEXTAREA";
  delete a.type;
  fire(typed(a));
  assert.equal(a.value, "the ");
}

console.log("autocorrect: ok");
