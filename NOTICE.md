# NOTICE

## scmCode is a derivative work

scmCode is a fork of **SideX** (<https://github.com/Sidenai/sidex>), which is itself a port
of **Visual Studio Code — Code - OSS** (<https://github.com/microsoft/vscode>) from Electron
to Tauri. Both upstream projects are distributed under the MIT License.

| Project | Copyright | License |
|---|---|---|
| Code - OSS | Copyright (c) 2015 - present Microsoft Corporation | MIT |
| SideX | Copyright (c) 2025 - present Siden Technologies Inc. | MIT |
| scmCode modifications | Copyright (c) 2026 - present scmCode contributors | MIT |

As required by the MIT License, the original copyright notices and permission notice are
kept in [LICENSE](./LICENSE) and in the headers of the source files that carry them. Those
headers must not be removed. New scmCode files may carry their own notice.

Third-party packages (npm, Cargo and Go dependencies, bundled grammars, fonts and icons)
keep their own licenses, as declared in their respective package metadata.

### Third-party files kept in this repository

These files are committed outside the dependency folders and ship with the app, so their
licenses are listed here:

| File | Origin | License |
|---|---|---|
| `public/codicon.ttf` | Codicons icon font, [`@vscode/codicons`](https://github.com/microsoft/vscode-codicons), Copyright (c) Microsoft Corporation | [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) |
| `public/onig.wasm` | [`vscode-oniguruma`](https://github.com/microsoft/vscode-oniguruma), Copyright (c) Microsoft Corporation, which compiles [Oniguruma](https://github.com/kkos/oniguruma), Copyright (c) K.Kosako | MIT (vscode-oniguruma), BSD 2-Clause (Oniguruma) |
| `public/webview-pre/` | Webview host pages from Code - OSS, Copyright (c) Microsoft Corporation | MIT |

## No warranty, no liability

scmCode, like SideX and Code - OSS, is provided **"AS IS", without warranty of any kind**,
and its authors, contributors and copyright holders are **not liable for any claim, damages
or other liability** arising from the software or its use, as set out in the
[LICENSE](./LICENSE). The modifications listed below describe intended behavior; they are
not a guarantee. See the
[Disclaimer and limitation of liability](./README.md#disclaimer-and-limitation-of-liability).

## No affiliation, no endorsement

scmCode is an independent project. It is not affiliated with, sponsored by, or endorsed by
Siden Technologies Inc. or Microsoft Corporation.

"SideX" is a name of Siden Technologies Inc. "Visual Studio Code" and "VS Code" are
trademarks of Microsoft Corporation. These names are used only to identify the origin of
the code (nominative use). scmCode does not use their logos: the SideX application icon and
logo have been replaced with scmCode's own artwork.

scmCode does not use the Visual Studio Marketplace, whose terms of use restrict it to
Microsoft products; extensions are obtained from [Open VSX](https://open-vsx.org/).

## Summary of changes from SideX

- Renamed the product to scmCode: product name, window titles, application identifier
  (`dev.scmcode.app`), URL protocol, data folders (`~/.scmcode`, app-data `scmCode`),
  package metadata, user-visible strings, icons and logo. Internal identifiers
  (`sidex-*` crates, `SideX*` classes, `SIDEX_*` environment variables) are unchanged to
  keep merging from upstream practical.
- Extension gallery switched from the SideX marketplace proxy to Open VSX directly; the
  marketplace-proxy Cloudflare Worker and the Microsoft Marketplace hosts were removed.
- Agent server: PostHog analytics replaced by a no-op (SDK dependency removed); live
  OpenRouter pricing made opt-in; implicit OpenRouter fallback endpoint removed;
  `HTTP-Referer` / `X-Title` attribution headers removed.
- Claude Code / Codex CLI login reuse removed (reading those CLIs' credential files and
  Keychain entries, presenting requests as the official CLIs, and subscription quota
  checks). Credentials are API keys or keyless local model servers only.
- Updater: SideX signing key and CDN release job removed; background checks honor
  `update.mode`; updates without a configured public key are refused.
- Telemetry: unused Microsoft 1DS appenders and dependencies, and the dormant completion
  telemetry service, deleted. The misleading "Data Sharing" toggle was replaced by a
  statement that no data is collected.
- Workspace extension recommendations disabled by default; locally installed language
  servers preferred over downloads; commit/PR attribution toggles default to off.
- SideX community links (Discord, contact e-mail, social accounts) removed; documentation
  rewritten (README, CONTRIBUTING, PRIVACY, this NOTICE).
