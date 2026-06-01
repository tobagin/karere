## ADDED Requirements

### Requirement: Build-time JS bundling
The build script SHALL concatenate all `data/js/*.js` files into a single embedded bundle that the Rust binary loads via `include_str!`.

#### Scenario: Bundle is generated during cargo build
- **WHEN** `cargo build` runs
- **THEN** `build.rs` scans `data/js/` for files matching `*.js` in lexical filename order
- **AND** writes the concatenation to `$OUT_DIR/injected_bundle.js`
- **AND** emits `cargo:rerun-if-changed=data/js` so additions or edits trigger rebuilds

#### Scenario: Bundle is embedded into the binary
- **WHEN** the renderer subprocess starts
- **THEN** `EMBED_BUNDLE` resolves via `include_str!(concat!(env!("OUT_DIR"), "/injected_bundle.js"))`
- **AND** no runtime file access to `data/js/` is required

#### Scenario: Binary works outside its source tree
- **WHEN** the compiled binary is invoked from a directory other than the project root (including flatpak/installed locations)
- **THEN** JS injection still occurs with the same bundle contents

### Requirement: Bootstrap script
The bundle SHALL include a `bootstrap.js` entry that initializes the renderer-side bridge.

#### Scenario: Console forwarding hook installed
- **WHEN** the bootstrap script runs in a fresh V8 context
- **THEN** it replaces `console.log`, `console.warn`, and `console.error` with shims that forward each call to the browser process as a `RendererMessage::ConsoleLog { level, msg }`
- **AND** the original console behavior is preserved (the shim still calls through to the native console)

#### Scenario: Browser-to-page DOM event listeners installed
- **WHEN** the bootstrap script runs
- **THEN** it registers `document` event listeners for `karere:dispatch-paste` and `karere:close-notif`
- **AND** these listeners are invoked when the host process dispatches the corresponding `BrowserMessage` (which is converted to a DOM event by the renderer dispatcher)

#### Scenario: Bootstrap errors do not crash the renderer
- **WHEN** any statement in the bootstrap throws
- **THEN** the surrounding `try/catch` reports the error via `RendererMessage::ConsoleLog { level: "error", ... }` and execution of the remainder of the bundle continues
