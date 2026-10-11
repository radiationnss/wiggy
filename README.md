# Wiggy

Wiggy is a small Linux desktop assistant for Hyprland. A deliberate mouse
wiggle summons a floating chat palette; you can also capture a screen region,
display, or visible window and attach it yourself. The app uses the existing
Codex app-server ChatGPT login. It does not ask for an API key or a second
account sign-in.

## Project status

The native chat, model picker, screenshot capture/send, settings, and mouse
trigger are implemented. The command terminal has been removed. A native Arch
package recipe and private GitHub Actions build are included. Local speech
recognition and user-controlled login startup remain tracked release work.

The current target is Arch Linux with Hyprland 0.55/0.56. Other compositors and
operating systems are not verified. See [architecture](docs/architecture.md)
and the [production roadmap](docs/roadmap.md).

## Requirements

For a source build, install Node.js/npm, Rust, and the Tauri Linux development
libraries. Runtime use also requires the Codex CLI with its app-server and an
active ChatGPT login, plus Hyprland, `grim`, and `slurp` for screenshot capture.
The tray integration uses AppIndicator.

On Arch Linux, the build uses `webkit2gtk-4.1`, `gtk3`, `libappindicator`,
`librsvg`, Node.js/npm, and Rust. Runtime screenshot capture also uses `grim`
and `slurp`.

## Install on Arch Linux

From a checkout with access to the private repository, install the native
package and its dependencies with:

```sh
makepkg -si
```

This builds Wiggy locally, installs its launcher and icon, and does not enable
login startup. The private GitHub Actions workflow also creates a
`.pkg.tar.zst` artifact on pushes to `main`; collaborators can download it from
the workflow run's **Artifacts** section. The artifact is retained for 14 days.
There is no public AUR recipe or signed release yet.

## Build and run from source

```sh
npm ci
npm run tauri -- dev
```

The native app starts hidden. Open it from the tray or invoke the app with
`--toggle`. `--settings` opens Settings and `--diagnose` prints local
integration status. A production binary can be built with:

```sh
npm run tauri -- build --no-bundle
```

Login startup remains a separate, user-controlled roadmap item.

## Chat and screenshots

Text and screenshots are sent only when you press Send. Captured images remain
in the current chat's memory until removed or the app exits. On Send, Wiggy
temporarily materializes PNG files in a private directory for app-server and
removes them when the turn ends. This local cleanup does not determine any
Codex-side or model-service retention.

For screenshots, the selected GPT model must accept images. Region and window
selection use `slurp`; image capture uses `grim`. Window capture records the
visible screen rectangle, so overlapping content can appear in the image.

## Verification

```sh
npm run build
npm test
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
npm run tauri -- build --no-bundle
```

The same checks run in [GitHub Actions](.github/workflows/verify.yml). A second
workflow builds, installs, validates, and removes the Arch package in an
isolated Arch Linux container: [Arch package workflow](.github/workflows/package-arch.yml).
