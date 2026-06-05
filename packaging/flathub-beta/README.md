# Flathub beta channel

This directory holds the manifest used to publish Karere to the **`flathub-beta`**
remote — a separate channel from stable. Stable users never see beta builds; testers
opt in explicitly.

It differs from the in-tree dev manifest (`packaging/io.github.tobagin.karere.yml`)
in exactly one way: the `karere` module pulls a **tagged git source** from GitHub
instead of the local `type: dir` checkout (Flathub cannot build from a local dir).

## How a beta gets published

Flathub builds whatever is on the **`beta` branch** of
`github.com/flathub/io.github.tobagin.karere` and pushes it to the `flathub-beta`
remote (flatpak branch `beta`). No PR is needed for an existing app — pushing the
branch triggers a buildbot run.

1. **Bump versions** in `Cargo.toml`, `Cargo.lock`, `meson.build` to `4.0.0-betaN`.
2. **Tag** the release commit:
   ```
   git tag -a v4.0.0-betaN -m "Karere 4.0.0 beta N"
   git rev-list -n1 v4.0.0-betaN     # paste into the manifest `commit:`
   git push origin v4.0.0-betaN
   ```
3. **Update this manifest** — set `tag:` and `commit:` on the `karere` module to the
   new tag.
4. **Push to the Flathub `beta` branch:**
   ```
   git clone git@github.com:flathub/io.github.tobagin.karere.git
   cd io.github.tobagin.karere
   git checkout -b beta            # first time; thereafter: git checkout beta
   cp .../packaging/flathub-beta/io.github.tobagin.karere.yml .
   cp .../packaging/cargo-sources.json .
   git add -A && git commit -m "Karere 4.0.0-betaN"
   git push -u origin beta
   ```
5. Watch the build at <https://buildbot.flathub.org>.

## Testers install

```
flatpak remote-add --if-not-exists flathub-beta https://flathub.org/beta-repo/flathub-beta.flatpakrepo
flatpak install flathub-beta io.github.tobagin.karere
```

## Promote beta to stable

The stable manifest on the `master` branch uses the **same git-source form** — when a
beta is ready, copy this manifest to `master` (or open the normal PR) with the `tag:`
bumped to the final `vX.Y.Z`. `master` publishes to the `flathub` (stable) remote.

> **Note:** stable and beta share the app-id `io.github.tobagin.karere`, hence the same
> data dir `~/.var/app/io.github.tobagin.karere`. A tester running both points one data
> dir at two builds; the v4 first-launch migration dialog handles the v3→v4 re-link.
