## ADDED Requirements

### Requirement: Developer script to fetch CEF binaries
The repository SHALL ship `download-cef.sh` which fetches the upstream CEF
148 minimal distribution into `cef-binaries/` and exposes it via the
`cef-binaries/current` symlink for `CEF_PATH` consumers.

#### Scenario: Script downloads and links the expected CEF version
- **WHEN** a contributor runs `./download-cef.sh` from the repository root
- **THEN** the CEF 148.0.8+g18e00ea+chromium-148.0.7778.96 minimal tarball is downloaded and extracted under `cef-binaries/`, and `cef-binaries/current` is updated to symlink the freshly extracted directory

### Requirement: Build consumes CEF via `CEF_PATH`
The build SHALL link against the CEF distribution pointed to by the
`CEF_PATH` environment variable.

#### Scenario: cargo build succeeds with CEF_PATH set
- **WHEN** a contributor runs `CEF_PATH=$(pwd)/cef-binaries/current/Release cargo build`
- **THEN** the build completes successfully and produces `target/debug/gtk-cef-shell`
