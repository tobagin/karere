## Context

M6 produced a working Flatpak that runs karere v4 locally, but it deferred everything Flathub reviewers actually look at: a valid `metainfo` file, a populated `desktop` entry with proper categories and MIME handler, registered icons, and `appstream-compose: true`. Crash triage was deliberately deferred to a separate Debug extension rather than relying on `debuginfod` on user systems, because the project's locked decision is that Flathub-distributed apps must produce symbolicated stack traces without requiring users to configure external symbol servers. Between M6 and now, M7-M22 added dependencies, IPC code, tray support, paste bridge, accessibility, multi-account, and a preferences dialog, all of which mutated `Cargo.toml` and therefore invalidated the vendored `cargo-sources.json`. M23 consolidates these loose ends into a single, reviewable submission package.

## Goals / Non-Goals

**Goals:**
- Produce a Flatpak manifest that passes `flathub-quality-check` (icons, metainfo, desktop lints) and builds offline (no `--share=network` at karere build step).
- Ship a separately installable `io.github.tobagin.karere.Debug` extension carrying detached symbols at `lib/debug`.
- Port karere v3's metainfo, desktop, screenshots, and content-rating verbatim, with one new description paragraph and a `4.0.0` release entry.
- Refresh `cargo-sources.json` to match the post-M22 dependency closure.
- Update README and CHANGELOG to reflect the v4 rewrite and locked decisions (hard-fork, no migration from v3).

**Non-Goals:**
- Submitting to Flathub (the submission PR is out of scope; M23 ends when the local build + Debug extension install succeed and quality-check passes).
- Restructuring the CEF runtime layout under `/app/lib/cef` (kept as-is from M6).
- Introducing additional finish-args beyond StatusNotifierWatcher and the autostart filesystem.
- Building a `debuginfod` integration as an alternative to the Debug extension.
- Localizing metainfo/desktop strings (English only for the initial 4.0.0 ship, matching v3's initial release).

## Decisions

- **appstream-compose enabled**: flipping the flag is gated on a populated `metainfo.xml.in.in`; both land in the same change so the build cannot regress to "compose enabled but template empty." The metainfo template uses the `.in.in` pipeline because meson's `i18n` module rewrites desktop-id and other build-time substitutions before xgettext extracts strings.
- **Debug extension layout**: `directory: lib/debug` mirrors the freedesktop SDK Debug runtime convention so that `/app/lib/debug/<binary>.debug` is automatically picked up by `coredumpctl debug`. `autodelete: 'true'` ensures the extension is removed when the base app is uninstalled. `no-autodownload: 'true'` keeps download size low for users who don't need symbols.
- **Network access during build**: the karere module must build offline (`--share=network` removed from its build-args). Vendored crates produced by `cargo-sources.json` cover the full closure, so any remaining network reference is an M6 leftover. `cef-binaries` retains its `extra-data` source which is fetched by flatpak-builder itself, not by network access during build.
- **Cleanup array**: removing `*.la`, `*.a`, and `/app/include` (except `/app/lib/cef/include`) shrinks the bundle and removes dev-only artifacts that Flathub reviewers flag. CEF's headers stay because they're consumed by the build at runtime via the embedded loader path, not because end users need them, and excluding only that subtree is the smallest correct cleanup spec.
- **finish-args additions**: `--talk-name=org.kde.StatusNotifierWatcher` is the documented DBus name that KDE / Plasma's tray daemons expose for SNI clients; adding it is required for the M15 tray to register without portal escalation. `--filesystem=xdg-config/autostart:create` is the path the XDG autostart spec dictates for `.desktop` entries created by the autostart portal; without `:create` the portal call fails on first use.
- **README and CHANGELOG**: rebased on v3 README to preserve user-facing project framing (screenshots, install instructions), with two appended sections — "What changed in 4.0.0" (CEF/Chromium 148, hard fork) and "Migration from v3" (explicitly: no automatic migration, users start fresh). CHANGELOG `4.0.0` entry is prepended above the v3 entries (kept for history).
- **`cargo-sources.json` regeneration**: a single regeneration at the end of M23 captures the cumulative M7-M22 closure; intermediate milestones intentionally did not regenerate to avoid merge churn.

## Risks / Trade-offs

- `appstream-compose: true` will fail the build if any metainfo field is malformed (e.g., missing `<launchable>` or invalid OARS rating value). Mitigated by porting v3's already-Flathub-accepted metainfo verbatim and only adding the 4.0.0 release entry.
- The Debug extension doubles the build time and roughly doubles the cold storage footprint on Flathub's CDN; accepted as the cost of symbolicated crashes without forcing users to install `debuginfod`.
- Removing `--share=network` will surface any accidental online dependency added during M7-M22 (e.g., a `build.rs` that downloads files). Surfacing such a bug at M23 is acceptable because fixing it before Flathub submission is mandatory anyway.
- `gtk_update_icon_cache: true` runs at install time on the user's system; a malformed icon directory will fail the install with a confusing message. Mitigated by reusing v3's exact `data/icons/hicolor/**` tree which is already known-good.
- `MimeType=x-scheme-handler/whatsapp;` claims the `whatsapp://` URL scheme; if another installed app also claims it, the user's default-handler choice arbitrates, which matches v3 behavior.
- Manual smoke-test matrix (KDE Plasma 6, GNOME 50, XFCE; Wayland and X11) is large but cannot be automated within this milestone; documented as a release-gate checklist rather than a CI job.
