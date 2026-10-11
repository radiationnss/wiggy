# Wiggy engineering status

This file records project state, not user-machine state. The GitHub issues are
the source of truth for outstanding production work; see
[docs/roadmap.md](docs/roadmap.md).

## Implemented

- A native Tauri palette with Hyprland mouse-wiggle recall and settings.
- Chat through the user's existing Codex app-server login; no API-key field,
  credential scraping, or second sign-in flow.
- GPT model selection populated from the connected Codex account.
- Streaming text, stop, clear, and in-memory chat state.
- Explicit screen, area, and visible-window capture, preview, removal, and
  screenshot submission with a message.
- Private temporary screenshot files removed at the end of a turn.
- The manual command terminal, its arbitrary shell execution bridge, and its
  sudo helper have been removed.
- CI checks cover frontend, Rust, linting, and a Linux native build.

## Open release gates

- Clean desktop acceptance for the Arch package, including first launch and
  upgrade behavior. The package recipe and isolated CI install/uninstall check
  are implemented; package installation does not enable startup.
- Local speech recognition that starts only after mouse summon, can be disabled
  in Settings, and never waits for a wake phrase.
- Final review of app-server tool isolation, account/runtime boundaries, and
  data retention before describing the product as production-ready. A live
  app-server check returned a chat reply but also reported `unified_exec`
  enabled and 27 MCP tools in two server inventories; no callable connected
  apps were found. This needs human security review.
- Native verification of screenshot inference, stop/recovery behavior,
  installation, startup, and upgrade/uninstall.
- UI readability, keyboard accessibility, and multiple-monitor acceptance.

## Verification status

Local verification: 19 frontend tests passed; 35 Rust tests passed and one
screen-capture test remained ignored because it requires explicit live screen
capture approval. Frontend build, Rust formatting, Clippy, and the native
release build passed. The first GitHub push also passed the full Linux verify
workflow and the Arch package build/install/diagnose/uninstall workflow; the
private `.pkg.tar.zst` artifact was uploaded with 14-day retention. A
one-message app-server chat smoke test returned a completed reply through the
existing login.
