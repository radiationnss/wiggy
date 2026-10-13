# Production roadmap

The open work is tracked in the public GitHub repository. The parent issue is
[Production readiness roadmap](https://github.com/radiationnss/wiggy/issues/1).

| Issue | Work | Current state |
| --- | --- | --- |
| [#2](https://github.com/radiationnss/wiggy/issues/2) | Remove manual terminal and arbitrary shell execution | Closed after tests and push |
| [#3](https://github.com/radiationnss/wiggy/issues/3) | Build and accept the Arch package | CI build/install/uninstall passed; clean desktop first-launch and upgrade acceptance remain |
| [#4](https://github.com/radiationnss/wiggy/issues/4) | User-controlled startup at login | Planned |
| [#5](https://github.com/radiationnss/wiggy/issues/5) | Local voice input after mouse summon | Planned; local recognizer evaluation required |
| [#6](https://github.com/radiationnss/wiggy/issues/6) | Existing-login, app-server safety, and data review | Open release gate |
| [#7](https://github.com/radiationnss/wiggy/issues/7) | Settings readability and keyboard accessibility | Open; native visual review required |
| [#8](https://github.com/radiationnss/wiggy/issues/8) | Screenshot and chat native acceptance | Live text chat smoke passed; image inference and native capture acceptance remain open |
| [#9](https://github.com/radiationnss/wiggy/issues/9) | Optional reusable personas, separate from GPT model | Planned |
| [#10](https://github.com/radiationnss/wiggy/issues/10) | Tracked docs and production release checks | Closed after docs, workflows, and CI checks passed |

## Decisions already made

- Keep the existing Codex app-server ChatGPT login; no API key and no separate
  sign-in flow.
- Summon Wiggy by mouse wiggle. If the user enables listening, local speech
  recognition begins only after summon. There is no assistant wake phrase and
  no passive listening.
- The Settings model selector chooses a GPT model. Personas, if implemented,
  are a separate feature.
- Manual shell execution is removed from the app.

## Decisions and remaining release gates

The current public download is a temporary GitHub Actions artifact containing
the native Arch `.pkg.tar.zst` package and its checksum. A version tag builds a
draft GitHub Release for maintainer review; no release has been published yet.
The package registers a desktop launcher and icon without enabling login
startup. There is no AUR recipe.

Broader distribution still needs the maintainer to choose a project license
and complete the app-server/account-policy review before publishing a release.
