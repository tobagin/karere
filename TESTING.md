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
- [ ] Open/close 50 times: no leaked memory (`heaptrack ./gtk-cef-shell`).
- [ ] After quit: `ps -ef | grep gtk-cef-shell` shows no orphan processes.

## Automated

- [ ] `cargo test` — headless integration test launches `--url about:blank` and confirms clean exit on SIGTERM.
