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

Third-party components (npm, Cargo and Go dependencies, bundled grammars, fonts and icons)
keep their own licenses. [THIRD_PARTY_NOTICES.txt](./THIRD_PARTY_NOTICES.txt) reproduces
the notices and license texts of everything the installers include besides scmCode's own
code, and every installer ships it together with LICENSE and this NOTICE.

### Third-party files kept in this repository

These files are committed outside the dependency folders. Each one has its license text
next to it:

| File | Origin | License | License text |
|---|---|---|---|
| `public/codicon.ttf`, `src/vs/base/browser/ui/codicons/codicon/codicon.ttf` | Codicons icon font, [`@vscode/codicons`](https://github.com/microsoft/vscode-codicons) 0.0.45, Copyright (c) Microsoft Corporation | [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) | `public/codicon.ttf.license.txt` |
| `public/onig.wasm` | [`vscode-oniguruma`](https://github.com/microsoft/vscode-oniguruma) 2.0.1, Copyright (c) Microsoft Corporation, which compiles [Oniguruma](https://github.com/kkos/oniguruma) 6.9.5_rev1, Copyright (c) K.Kosako | MIT (vscode-oniguruma), BSD 2-Clause (Oniguruma) | `public/onig.wasm.license.txt` |
| `public/webview-pre/` | Webview host pages from Code - OSS, Copyright (c) Microsoft Corporation | MIT | `LICENSE` |
| `src/vs/base/browser/dompurify/` | [DOMPurify](https://github.com/cure53/DOMPurify) 3.2.7, Copyright (c) Cure53 and other contributors | Apache-2.0 (offered under Apache-2.0 or MPL-2.0; used under Apache-2.0) | `dompurify.license.txt` |
| `src/vs/base/common/marked/` | [marked](https://github.com/markedjs/marked) 14.0.0, Copyright (c) Christopher Jeffrey | MIT | `marked.license.txt` |
| `src/vs/base/common/semver/` | [node-semver](https://github.com/npm/node-semver) 5.5.0, Copyright (c) Isaac Z. Schlueter and Contributors | ISC | `semver.license.txt` |
| `src/vs/workbench/contrib/terminal/common/scripts/psreadline/` | [PSReadLine](https://github.com/PowerShell/PSReadLine) 2.4.4-beta4, Copyright (c) 2013, Jason Shirk | BSD 2-Clause | `PSReadLine.license.txt` |
| `shellIntegration.ps1` in `src/vs/workbench/contrib/terminal/common/scripts/` and `src-tauri/shell-integration/` | Includes code from [posh-git](https://github.com/dahlbyk/posh-git), Copyright (c) 2010-2018 Keith Dahlby, Keith Hill, and contributors | MIT | `posh-git.license.txt` in both folders |
| `src/vs/editor/browser/gpu/taskQueue.ts`, `src/vs/workbench/contrib/terminal/browser/media/xterm.css` | Code adapted from [xterm.js](https://github.com/xtermjs/xterm.js), Copyright (c) The xterm.js authors | MIT | File headers; the xterm.js license in THIRD_PARTY_NOTICES.txt |

The Code - OSS `cgmanifest.json` for posh-git carried the Oniguruma license by mistake; scmCode's
copy has the posh-git license.

### Built-in extensions

The installers include built-in extensions (language grammars, snippets, themes) from
Code - OSS 1.115.0, Copyright (c) Microsoft Corporation, MIT. They are not stored in this
repository: [`scripts/setup-extensions.sh`](./scripts/setup-extensions.sh) fetches them from
the Code - OSS repository at a pinned commit, and only the folders listed and reviewed in
[`scripts/builtin-extensions.txt`](./scripts/builtin-extensions.txt) are included. Nothing is
copied from an installed Visual Studio Code, Cursor or `~/.vscode/extensions`, whose
distributions and Marketplace extensions have their own licenses. The third-party grammars and
themes inside these extensions, and Code - OSS's own ThirdPartyNotices.txt, are reproduced in
THIRD_PARTY_NOTICES.txt.

### License policy

Dependencies may use only the licenses listed in [`deny.toml`](./deny.toml) (Rust, checked by
cargo-deny) and [`scripts/notices/policy.json`](./scripts/notices/policy.json) (npm, Go and
built-in extensions). CI fails when a dependency falls outside that list or when
THIRD_PARTY_NOTICES.txt no longer matches the dependencies; `npm run notices` regenerates it.
See [CONTRIBUTING](./CONTRIBUTING.md#licenses-and-third-party-notices).

The AI agent (`sidexai/`: the Go server and the Rust `sidex-agent` and `sidex-cli` crates) is
not included in the installers yet. Its dependencies are checked against the same policy, but
they are not in THIRD_PARTY_NOTICES.txt; they must be added before the agent ships.

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
