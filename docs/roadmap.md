# Production roadmap

The open work is tracked in the private GitHub project. The parent issue is
[Production readiness roadmap](https://github.com/radiationnss/wiggy/issues/1).

| Issue | Work | Current state |
| --- | --- | --- |
| [#2](https://github.com/radiationnss/wiggy/issues/2) | Remove manual terminal and arbitrary shell execution | Implemented in this change; close after checks and push |
| [#3](https://github.com/radiationnss/wiggy/issues/3) | Choose and build a seamless Linux installer | Arch `.pkg.tar.zst` selected; recipe and CI install smoke check added; clean desktop acceptance remains |
| [#4](https://github.com/radiationnss/wiggy/issues/4) | User-controlled startup at login | Planned |
| [#5](https://github.com/radiationnss/wiggy/issues/5) | Local voice input after mouse summon | Planned; local recognizer evaluation required |
| [#6](https://github.com/radiationnss/wiggy/issues/6) | Existing-login, app-server safety, and data review | Open release gate |
| [#7](https://github.com/radiationnss/wiggy/issues/7) | Settings readability and keyboard accessibility | Open; native visual review required |
| [#8](https://github.com/radiationnss/wiggy/issues/8) | Screenshot and chat native acceptance | Live text chat smoke passed; image inference and native capture acceptance remain open |
| [#9](https://github.com/radiationnss/wiggy/issues/9) | Optional reusable personas, separate from GPT model | Planned |
| [#10](https://github.com/radiationnss/wiggy/issues/10) | Tracked docs and production release checks | Docs updated here; close after checks and push |

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

The first release path is a native Arch `.pkg.tar.zst` built by private GitHub
Actions. This keeps the source and CI artifact within the private repository;
it is not a public AUR package. Local installation is available with
`makepkg -si` from a private checkout. The package registers a desktop launcher
and icon without enabling login startup.

Broader distribution also needs the maintainer to choose a project license and
complete the app-server/account-policy review before publishing outside this
private repository.
