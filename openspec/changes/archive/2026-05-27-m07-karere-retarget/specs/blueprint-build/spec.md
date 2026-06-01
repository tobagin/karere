## ADDED Requirements

### Requirement: Build script compiles blueprints to UI

The Cargo build script SHALL compile every `data/ui/*.blp` blueprint file to a corresponding `.ui` XML file under `$OUT_DIR/ui/` before the gresource bundle is built.

#### Scenario: blp files compile to ui files

- **WHEN** `cargo build` runs and `data/ui/foo.blp` exists
- **THEN** `$OUT_DIR/ui/foo.ui` is produced with the compiled XML output of `blueprint-compiler compile-file data/ui/foo.blp`

#### Scenario: Build script reruns when blueprint changes

- **WHEN** a `.blp` file in `data/ui/` is modified
- **THEN** the next `cargo build` recompiles that blueprint
- **AND** `build.rs` emits a `cargo:rerun-if-changed=data/ui/<file>.blp` directive for each blueprint

### Requirement: Gresource prefers compiled output, falls back to checked-in ui

The gresource bundle SHALL source each UI file from `$OUT_DIR/ui/<name>.ui` when present, otherwise from `data/ui/<name>.ui` when present.

#### Scenario: Compiled output preferred

- **WHEN** building with `blueprint-compiler` available
- **AND** the gresource registration runs
- **THEN** the bundled UI bytes come from `$OUT_DIR/ui/<name>.ui`

#### Scenario: Fallback when no compiler

- **WHEN** the build path that does not invoke blueprint-compiler runs (e.g., a doc-only generation step that skips ui compilation)
- **AND** a checked-in `data/ui/<name>.ui` exists
- **THEN** the bundled UI bytes come from `data/ui/<name>.ui`

### Requirement: Host blueprint-compiler detection

The build script SHALL detect `blueprint-compiler` on the host PATH before invoking it. If absent, the build SHALL fail with an actionable error message listing install commands for common package managers.

#### Scenario: blueprint-compiler missing produces actionable panic

- **WHEN** `cargo build` runs on a host where `which blueprint-compiler` returns nonzero
- **THEN** `build.rs` panics with a message that contains the string `blueprint-compiler`
- **AND** the message lists at least one install command (e.g., `dnf install blueprint-compiler` or `apt install blueprint-compiler` or the flatpak SDK path)
- **AND** the build does not silently produce a binary missing UI resources

#### Scenario: blueprint-compiler present succeeds

- **WHEN** `cargo build` runs on a host where `which blueprint-compiler` returns zero
- **THEN** the build proceeds without panic
- **AND** each `.blp` file is compiled exactly once per build

### Requirement: Blueprint compile errors surface to cargo

If `blueprint-compiler` exits nonzero for any `.blp`, the build SHALL fail with that compiler's stderr included in the cargo error output.

#### Scenario: Syntactically invalid blueprint fails build

- **WHEN** a `.blp` file contains a syntax error
- **AND** `cargo build` runs
- **THEN** the build fails
- **AND** the stderr output from `blueprint-compiler` is visible in cargo's error stream
