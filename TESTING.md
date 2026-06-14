# Manual smoke tests

Per PRD §11.

- [ ] Window opens, page renders.
- [ ] Scroll wheel works.
- [ ] Click + type in search box.
- [ ] Resize window: no stretch or tearing.
- [ ] HiDPI on 2x monitor: crisp text.
- [ ] `webrtc.github.io/samples/src/content/peerconnection/pc1/` completes a peer connection.
  - [ ] `getUserMedia` triggers AdwAlertDialog prompt.
  - [ ] Allow → camera + mic active.
- [ ] Open/close 50 times: no leaked memory (`heaptrack ./karere`).
- [ ] After quit: `ps -ef | grep karere` shows no orphan processes.

## Automated

- [ ] `cargo test` — headless integration test launches `--url about:blank` and confirms clean exit on SIGTERM.

## Beta testing (flathub-beta)

Karere 4.0 betas ship on the **`flathub-beta`** channel — separate from stable, opt-in.
Stable users are unaffected.

### Install

```sh
flatpak remote-add --if-not-exists flathub-beta https://flathub.org/beta-repo/flathub-beta.flatpakrepo
flatpak install flathub-beta io.github.tobagin.karere
flatpak run io.github.tobagin.karere   # add --branch=beta if stable is also installed
```

> **Note:** beta and stable share the app-id, so they share the data dir
> `~/.var/app/io.github.tobagin.karere`. Coming from v3, you'll re-link by QR scan
> (v4 is a hard fork — no session migration).

### What to check

- [ ] First launch shows the v3→v4 migration dialog; QR pairing links an account.
- [ ] Send/receive text and media; **video attachments play in-app** (new in v4).
- [ ] Voice/video call connects (mic/cam permission prompt appears, then audio+video).
- [ ] Notifications fire with preview; clicking one opens the chat.
- [ ] Multi-account: add a second account, switch, confirm isolated sessions.
- [ ] Tray icon shows unread; close-to-tray + background run work.
- [ ] Downloads land in `~/Downloads`; spell-check, zoom (Ctrl +/-/0), paste (Ctrl+V) work.

### Report bugs — open an issue

File anything that breaks at **<https://github.com/tobagin/karere/issues>**.

Include:

- **Version**: `flatpak info io.github.tobagin.karere` (confirm it says `4.0.0`)
- **Desktop**: GNOME/KDE/other, Wayland or X11, distro
- **Steps to reproduce** + what you expected vs. saw
- **Logs**: run `flatpak run io.github.tobagin.karere` from a terminal, paste relevant output
- For crashes: install the Debug extension and attach a backtrace —
  `flatpak install flathub-beta io.github.tobagin.karere.Debug` then
  `flatpak-coredumpctl io.github.tobagin.karere`

Prefix the issue title with `[beta]` so it's easy to triage.
