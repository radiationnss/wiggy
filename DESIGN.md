# Wiggy interface notes

Wiggy is a compact desktop chat palette for Hyprland. The main view presents a
focused message input, short conversation history, a screenshot action, and a
small header for settings, clear, and hide. Settings use the same window and a
single scrollable form.

The visual style uses a near-black surface, restrained blue actions, Geist
typography, simple line icons, and compact spacing. It should remain legible at
480 px wide and keep the message composer available while chat expands.

Screen capture is an explicit action. The user chooses an area, display, or
visible window; the image appears as a removable preview and is sent only with
the message. No always-on capture or voice wake word is part of the design.

The command terminal is removed. Voice input is planned as local speech
recognition started only after the user summons Wiggy, with a Settings control
to disable listening. It must not listen continuously or use an assistant wake
phrase.

Settings descriptions are provided as hover/focus tooltips so the compact form
does not become a wall of text. The model selector chooses a GPT model from the
existing Codex account; it is not an agent/persona selector.

## References

- [AI chat workspace](https://designlounge.vercel.app/pieces/ai-chat-workspace)
- [Settings with sticky navigation](https://designlounge.vercel.app/pieces/settings-page-sticky-nav)
- [Inline form validation](https://designlounge.vercel.app/pieces/inline-form-validation)
