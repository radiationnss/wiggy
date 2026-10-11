# Architecture

## Scope

Wiggy is a local Linux desktop assistant built with Tauri 2, Rust, React, and
TypeScript. The current compositor adapter targets Hyprland. A small hidden
window is recalled by the mouse-wiggle detector and also hosts Settings.

The palette supports chat and explicit screenshot capture. Arbitrary typed shell
execution is not part of the product: the former terminal UI, Rust runner,
command IPC, process-management code, and sudo askpass helper were removed.
Screenshot selection still calls the fixed `grim` and `slurp` programs with
argument arrays; user text is never interpreted as a shell command.

## Existing account and chat path

The Rust bridge connects to the local Codex app-server endpoint and reads the
account/model state through app-server methods. Authentication stays owned by
Codex. Wiggy does not read credential files, copy tokens, ask for an API key,
or start another sign-in flow. Each user message is an explicit Send. Chat state
is held in memory, and Clear starts a fresh conversation context.

The bridge requests an ephemeral thread, an empty workspace, no dynamic tools,
and disabled tool-related runtime features; it also rejects unexpected
app-server action requests. Local adversarial fixtures verified a chat-only
response path, and a live one-message smoke test returned a completed reply.
In that live session, however, app-server reported `unified_exec` enabled and
27 MCP tools across two server inventories, while reporting zero callable
connected apps. This inventory does not establish that those tools were
callable, but it also does not prove the requested isolation. The exact
configuration and policy response remain a release gate in GitHub; no shared
Codex daemon settings were changed.

## Screenshot handling

Capture, preview, and removal do not send data. Pressing Send submits attached
images with the text. PNG files are written only for that turn inside a private
temporary directory (0700 directory, 0600 files) and removed when the turn ends.
This only describes Wiggy's local temporary-file lifecycle; it does not make a
claim about Codex or model-service retention.

## Settings and local state

Non-secret preferences are stored in the app configuration directory. They
include mouse trigger, sensitivity, popup placement, and the selected GPT model.
The model list comes from the connected Codex account. The selector is not a
persona/agent selector.

## Production constraints

- Local speech recognition has not been implemented. The approved interaction
  is mouse summon followed by optional listening; listening must be off unless
  the user enables it, and no wake phrase or passive listening is intended.
- A private Arch package recipe and CI artifact workflow are implemented.
  Clean desktop acceptance, update behavior, and app-controlled startup remain
  open in the roadmap; package installation does not enable startup.
- Native screenshot inference, app-server error recovery, runtime isolation,
  install/upgrade behavior, and hardware/compositor variations need acceptance.
- App-server and ChatGPT account integration requirements must be checked again
  before broader distribution. The official documentation describes supported
  local/open-source usage and newer Sign in with ChatGPT options; retain the
  user's requested existing-login experience unless the maintainer approves a
  change.

## References

- [Codex app-server](https://developers.openai.com/codex/app-server)
- [Codex app-server and ChatGPT plan authentication](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server)
- [Open-source Sign in with ChatGPT overview](https://developers.openai.com/siwc/token-sharing-open-source)
- [Tauri Linux distribution formats](https://v2.tauri.app/distribute/)
