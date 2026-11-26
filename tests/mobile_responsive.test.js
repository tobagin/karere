// Regression harness for data/js-deferred/mobile_responsive.js (narrow-window
// layout): no global font inflation, theme-aware navigation icons, and the
// sidebar toggle button following the sidebar state. Uses only Node built-ins.
"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");

const source = fs.readFileSync("data/js-deferred/mobile_responsive.js", "utf8");

function body(name) {
  const start = source.indexOf(`function ${name}(`);
  assert.notEqual(start, -1, `function ${name} not found`);
  const open = source.indexOf("{", start);
  let depth = 0;
  for (let i = open; i < source.length; i++) {
    if (source[i] === "{") depth++;
    if (source[i] === "}" && --depth === 0) return source.slice(open, i + 1);
  }
  throw new Error(`unbalanced braces in ${name}`);
}

// The script is injected as a plain classic script: it must at least parse.
assert.doesNotThrow(() => new vm.Script(source), "script has a syntax error");

// Text must keep WhatsApp Web's own size. Percentage font-size on `span` and
// friends compounds through nested spans and made the chat text far larger
// than in the browser.
assert.ok(
  !/addCss\([^)]*(?:^|[\s"'])(?:span|\.copyable-text|\.html-span)\s*\{[^}]*font-size/.test(source),
  "mobile layout must not override font-size on span/.copyable-text/.html-span",
);

// `--panel-header-icon` no longer exists in WhatsApp Web; relying on it left
// the icons transparent (menu) or hard-coded black (back button).
assert.ok(!source.includes("--panel-header-icon"), "stale --panel-header-icon variable");

// Back button: follows the text colour, so it is visible on the dark theme.
const back = body("addBackButtonToChatView");
assert.ok(!/#000000|#000\b|black/i.test(back), "back button icon has a hard-coded colour");
assert.match(back, /fill='currentColor'/);
assert.match(back, /\.back_button path \{ fill:currentColor/);

// Sidebar toggle button: one icon per state, both drawn with currentColor.
const menu = body("addLeftMenuButtonToChatList");
assert.match(menu, /karere-ico-expand/);
assert.match(menu, /karere-ico-collapse/);
assert.equal((menu.match(/stroke="currentColor"/g) || []).length, 2);
assert.match(menu, /\.added_menu_button\.karere-menu-open \.karere-ico-expand \{ display:none \}/);
assert.match(menu, /\.added_menu_button\.karere-menu-open \.karere-ico-collapse \{ display:block \}/);
assert.match(menu, /\.added_menu_button \.karere-ico-collapse \{ display:none \}/);

// The state class follows toggleLeftMenu(): open branch -> true, close -> false.
const toggle = body("toggleLeftMenu");
const openAt = toggle.indexOf("setMenuButtonState(true)");
const closeAt = toggle.indexOf("setMenuButtonState(false)");
assert.ok(openAt !== -1 && closeAt !== -1 && openAt < closeAt);
assert.ok(
  toggle.indexOf("display = 'block'") > openAt && toggle.indexOf("display = 'none'") > closeAt,
  "state must be set in the matching branch",
);

// setMenuButtonState toggles the class on the button and tolerates its absence.
const classes = new Set();
const button = {
  classList: { toggle: (c, on) => (on ? classes.add(c) : classes.delete(c)) },
};
let present = true;
const sandbox = { document: { querySelector: (q) => (q === ".added_menu_button" && present ? button : null) } };
vm.runInNewContext(`${source.match(/function setMenuButtonState[\s\S]*?\n}\n/)[0]}; this.fn = setMenuButtonState;`, sandbox);
sandbox.fn(true);
assert.ok(classes.has("karere-menu-open"));
sandbox.fn(false);
assert.ok(!classes.has("karere-menu-open"));
present = false;
assert.doesNotThrow(() => sandbox.fn(true));

console.log("mobile_responsive: ok");
