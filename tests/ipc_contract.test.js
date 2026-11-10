// Contract test: every message tag the injected JS sends must be a known
// RendererMessage tag in src/ipc.rs. Parses the Rust file as text.
"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const rust = fs.readFileSync("src/ipc.rs", "utf8");
const m = /impl RendererMessage \{\s*const KNOWN_TAGS[^=]*=\s*&\[([\s\S]*?)\];/.exec(rust);
assert.ok(m, "RendererMessage::KNOWN_TAGS not found in src/ipc.rs");
const known = new Set([...m[1].matchAll(/"([A-Za-z]+)"/g)].map((x) => x[1]));
assert.ok(known.has("ConsoleLog"), "KNOWN_TAGS parse looks wrong");

const used = new Map();
for (const dir of ["data/js", "data/js-deferred"]) {
  for (const f of fs.readdirSync(dir).filter((n) => n.endsWith(".js"))) {
    const text = fs.readFileSync(path.join(dir, f), "utf8");
    for (const x of text.matchAll(/(?:\bsend|karere_send)\(\s*["']([A-Za-z]+)["']/g)) {
      if (!used.has(x[1])) used.set(x[1], f);
    }
  }
}
assert.ok(used.size >= 5, "found suspiciously few send(...) tags: " + [...used.keys()]);

for (const [tag, file] of used) {
  assert.ok(known.has(tag), `${file} sends "${tag}" which is not in RendererMessage::KNOWN_TAGS`);
}
console.log(`ipc contract: ok (${[...used.keys()].sort().join(", ")})`);
