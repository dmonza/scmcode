<h1 align="center">scmCode</h1>

<p align="center">
  <strong>A minimal, privacy-focused code editor based on VS Code.</strong>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue?style=for-the-badge" alt="MIT License"></a>
  <img src="https://img.shields.io/badge/Built_with-Tauri_2-FFC131?style=for-the-badge&logo=tauri&logoColor=white" alt="Built with Tauri">
  <img src="https://img.shields.io/badge/Telemetry-None-3ddc97?style=for-the-badge" alt="No telemetry">
</p>

<p align="center">
  <a href="#pillars">Pillars</a> · <a href="#security-is-your-responsibility">Security</a> · <a href="#disclaimer-and-limitation-of-liability">Disclaimer</a> · <a href="#privacy">Privacy</a> · <a href="#whats-different-from-sidex">What's different</a> · <a href="#ai-agent">AI Agent</a> · <a href="#getting-started">Getting Started</a> · <a href="#credits-and-license">Credits &amp; License</a>
</p>

---

## An MIT-licensed derivative of SideX (VS Code on Tauri with a Rust backend)

**scmCode is a derivative work (fork) of [SideX](https://github.com/Sidenai/sidex),
distributed under the MIT License, the same license as SideX.**

- SideX is Copyright (c) 2025 - present Siden Technologies Inc., and is itself a port of
  Visual Studio Code ([Code - OSS](https://github.com/microsoft/vscode), Copyright (c) 2015 -
  present Microsoft Corporation, MIT) from Electron to [Tauri](https://tauri.app/).
- Their copyright and permission notices are preserved in [LICENSE](./LICENSE) and in the
  source file headers, as the MIT License requires.
- scmCode is an independent project. It is **not affiliated with, sponsored by, or endorsed
  by Siden Technologies Inc. or Microsoft Corporation**. "SideX" and "Visual Studio Code" are
  names and trademarks of their respective owners and are used here only to say where this
  code comes from.

See [NOTICE.md](./NOTICE.md) for the full attribution and the list of changes from SideX.

scmCode is the VS Code workbench running on a Rust backend and your OS's native webview: the same TypeScript editor, terminal and Git integration, without a bundled browser.

> **Early release.** Core editing and the terminal are solid. The extension host and debugger are still in progress.

---

## Pillars

scmCode is built on two pillars. Every change to the project is judged against them.

### 1. Minimalism

- Keep only what a code editor needs. Remove features, services and dependencies that
  are not essential.
- No accounts, no hosted backend, no vendor integrations, no community or marketing
  surfaces in the app.
- When in doubt, remove code instead of adding it. New features must be optional.

### 2. Privacy

- **Design goal: nothing leaves your machine unless you asked for it.**
- scmCode's own code is designed with no telemetry, no analytics, no crash reporting, no
  remote experiments and no silent fallbacks to third-party services.
- The network connections scmCode's own code is known to make are documented in
  [PRIVACY.md](./PRIVACY.md), together with what triggers them.

These are design goals, not guarantees. See the [Disclaimer](#disclaimer-and-limitation-of-liability).

---

## Security is your responsibility

scmCode does not send your data anywhere on its own, but **it does not make your machine
secure for you. Your security depends on how you use the editor and on the extensions you
install.**

- **Extensions run with your permissions.** Extensions you install from Open VSX (or
  anywhere else) run their own code. They can read your files, run processes and make
  network requests, including telemetry. scmCode does not audit, sandbox or vouch for them.
  Install only extensions you trust, and review what they do. (The extension host they run
  in listens on `127.0.0.1` and accepts only the editor, which holds a per-session token.)
- **Webviews are sandboxed, extensions are not.** What a webview shows (an extension panel,
  a Markdown preview, an extension's README) runs on its own loopback origin,
  `http://<id>.localhost:<port>`, served by the app with nothing but VS Code's webview host
  files. From there it can't reach the editor window, the app's commands or your files
  through the app. The extension behind a panel still runs with your permissions.
- **The AI agent acts on your behalf.** It can run shell commands and edit files. Review
  what it does, and choose model providers you trust with your code. A local model keeps
  everything on your machine.
- **Workspaces can contain code that runs.** Tasks, Git hooks, build scripts and MCP servers
  from a repository execute on your machine. Open only workspaces you trust.
- **You choose what connects outward.** Model providers, Git remotes, remote hosts and
  any opt-in setting described in [PRIVACY.md](./PRIVACY.md) are your decisions.
- **Keep backups.** Use version control and backups for anything you open in scmCode.

To report a security vulnerability in scmCode, see [SECURITY.md](./SECURITY.md). Please don't open a public issue for it.

---

## Disclaimer and limitation of liability

scmCode is free, open-source software distributed under the [MIT License](./LICENSE).
By using, building, modifying or distributing it, you accept the following:

- **No warranty.** The software is provided **"AS IS", without warranty of any kind**,
  express or implied, including but not limited to the warranties of merchantability,
  fitness for a particular purpose and non-infringement. Nothing in this README,
  [PRIVACY.md](./PRIVACY.md), [NOTICE.md](./NOTICE.md) or any other documentation is a
  warranty, guarantee or contractual commitment. Statements about privacy, security or
  behavior describe the intended design to the best of the authors' knowledge and may be
  incomplete or wrong.
- **No liability.** In no event shall the authors, contributors or copyright holders of
  scmCode, SideX or Code - OSS be liable for any claim, damages or other liability —
  including loss or corruption of data or code, security incidents, data exposure,
  service charges, downtime or any direct, indirect, incidental or consequential damage —
  whether in an action of contract, tort or otherwise, arising from, out of or in
  connection with the software or its use.
- **Use at your own risk.** You alone are responsible for how you use scmCode: the files
  and workspaces you open, the commands you run, the extensions you install, and every
  action taken by the AI agent on your behalf, including file edits, deletions, shell
  commands and Git operations. AI output can be wrong, insecure or destructive; review it
  before relying on it.
- **Third-party services and code are not ours.** Model providers, extension registries,
  extensions, MCP servers, language servers, Git hosts and any other service or code you
  connect to are operated by third parties under their own terms, privacy policies and
  pricing. You are responsible for complying with those terms and for any costs incurred
  (for example, model API usage billed by your provider).
- **No professional advice.** Nothing in scmCode or its documentation is legal, security,
  compliance or other professional advice.
- **Your own compliance.** If you use scmCode in a regulated environment or with data
  belonging to others, you are responsible for meeting the applicable legal, contractual
  and security requirements.

Where applicable law does not allow some of these exclusions, they apply to the maximum
extent permitted by that law. The binding terms are those of the [LICENSE](./LICENSE).

---

## Privacy

How the privacy pillar is implemented (design description, not a guarantee):

- **No telemetry or analytics.** The VS Code telemetry service is a no-op stub, the
  Microsoft 1DS telemetry code and dependencies are removed, and the agent server's
  analytics package (PostHog upstream) is a no-op with the SDK removed.
- **No account, no hosted backend.** There is no sign-in and no scmCode server. The AI
  agent runs locally on `127.0.0.1` and talks only to the model provider *you* configure.
- **No silent fallbacks.** If no model provider is configured, AI requests fail locally
  instead of being sent to a default third-party endpoint.
- **No background update checks.** There is no update feed. Updates are not checked or
  installed automatically, and the updater refuses updates without a valid signature.
- **No vendor proxies.** Extensions come straight from [Open VSX](https://open-vsx.org/).
  Workspace extension recommendations are not looked up automatically.

The list of network connections scmCode's own code is known to make, and what triggers
each one, is in **[PRIVACY.md](./PRIVACY.md)**.

---

## What's different from SideX

| Area | SideX | scmCode |
|---|---|---|
| Extension gallery | SideX proxy (`marketplace.siden.ai`) merging Microsoft Marketplace + Open VSX | Open VSX directly |
| Agent server analytics | PostHog client, enabled by an env var | Removed (no-op) |
| Live model pricing | Fetched from openrouter.ai at startup and hourly | Off; opt-in with `SIDEX_LIVE_PRICING=1` |
| Default model endpoint | Implicit fallback to openrouter.ai | None — must be configured explicitly |
| Provider attribution headers | `HTTP-Referer` / `X-Title` sent to OpenRouter | Not sent |
| Claude Code / Codex CLI logins | Optional reuse of the CLI's subscription token, presented as the official CLI | Removed; API keys or local models only |
| Subscription quota checks | `api.anthropic.com` / `chatgpt.com` usage endpoints for connected logins | Removed |
| Auto-update | Background checks; SideX signing key; SideX CDN release job | No feed, no SideX key, checks only if enabled; unsigned updates refused |
| Workspace recommendations | Looked up in the gallery on folder open | Disabled by default |
| Language servers | Downloaded even if installed locally | Local binary preferred |
| Commit/PR attribution toggles | On | Off |
| Telemetry code | Dormant 1DS appenders and completion telemetry | Deleted |
| Branding | SideX name, logo, community links | scmCode name and icon |

Internal identifiers (`sidex-*` crates, `SideX*` classes, `SIDEX_*` environment variables,
the `sidexai/` directory) intentionally keep their upstream names so that SideX changes can
still be merged.

---

## AI Agent

scmCode has a built-in coding agent that needs no account of any kind. Every request goes from the editor to a server running on your own machine, and from there straight to whichever model provider you point it at.

### Turning it on

AI is off by default. While it's off there is no chat panel, no inline edit (`Ctrl+K` keeps its usual editor chords), no inline completions, and no agent server process. Turn it on in **scmCode Settings → General → AI Features** (from the profile button in the title bar), or set `"sidex.ai.enabled": true` in your user settings, then reload the window when prompted. Turning it off again stops the agent server.

### How it starts

While AI is on, the app spawns its own agent server (`sidexai/sidex-server`, written in Go) as a child process bound to `127.0.0.1` on a random free port. It's supervised from [`src-tauri/src/server.rs`](./src-tauri/src/server.rs) and stopped when the app exits. If the server binary isn't built, the rest of the editor still works — the chat panel just reports itself as disconnected.

### Bring your own model

There are no built-in model presets and no default provider. You add a provider and a model ID yourself in **Settings → Models**. Credentials are resolved in this order, first match wins (see [`src-tauri/src/commands/providers.rs`](./src-tauri/src/commands/providers.rs)):

1. a key entered in **Settings → Models**
2. an environment variable already in your shell (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, and so on — see [`.env.example`](./.env.example) for the full list)
3. a keyless local model server on loopback — Ollama, LM Studio, llama.cpp, or vLLM — detected automatically

For maximum privacy, use a local model server: then prompts and code are sent only to that local server.

Credentials reach the agent server only through its process environment (`SIDEX_PROVIDER_<PROVIDER>_KEY` / `_BASE_URL`), set fresh each time the app starts or restarts it. API keys entered in Settings are kept in the OS keyring (or a local fallback store), not in the agent server's configuration, and are not sent to the webview.

scmCode does not read or reuse logins from other applications. SideX's option to reuse an
existing Claude Code or Codex CLI login was removed: it worked by taking that CLI's
subscription token and presenting itself to the provider as the official CLI. Use an API key
from your provider's console, or a local model, instead. You are responsible for complying
with your provider's terms of service.

### Security: this server runs commands on your machine

The agent server executes shell commands and edits files on your behalf. Binding it to `127.0.0.1` only is a deliberate security boundary: nothing else on your network can reach it.

`SIDEX_BIND_ADDR` can widen that, but the server refuses to start on a non-loopback address unless you explicitly opt in with `SIDEX_ALLOW_UNAUTHENTICATED=1`. Don't set that unless you understand the exposure — an open port on this server means arbitrary code execution for whoever can reach it.

In the default and auto-edit modes the agent asks before it runs a command, starts or calls an MCP server, or uses any tool it doesn't classify as read-only or safe, including tools an MCP server provides; the default mode also asks before file edits. Agent command hooks (commands the agent runs at fixed points of a turn) are read only from `~/.scmcode/agent-hooks.json` in your home folder, never from the workspace, and can't replace the built-in safety hook.

Web pages you open in a browser can also send requests to `127.0.0.1`. The server rejects requests from any origin other than the scmCode webview, requests whose `Host` is not a loopback address (DNS rebinding), and request bodies that are not JSON, so a page cannot drive it or read your sessions. The server has no authentication of its own, though: other programs and other user accounts on the same machine can connect to it. Don't run scmCode's agent on a machine you share with people you don't trust.

### Building the agent server

The agent server needs **Go 1.26 or later** and a C compiler (CGO, for SQLite and tree-sitter):

```bash
cd sidexai/sidex-server
go build -tags fts5 -o sidex-server ./cmd/server
```

The app looks for a binary at that path automatically when running from a source checkout. See [`sidexai/sidex-server/.env.example`](./sidexai/sidex-server/.env.example) if you'd rather run the server yourself.

The release workflow does not build or bundle the agent server yet, so prebuilt installers contain the editor without the AI agent. To use the agent, build scmCode and the agent server from source.

---

## Getting Started

### Prerequisites

- **Node.js 20 or later** (CI uses Node 20 and 24).
- **Rust 1.91 or later**, via [rustup](https://rustup.rs/).
- **The Tauri 2 system dependencies** for your OS (see Tauri's [prerequisites guide](https://v2.tauri.app/start/prerequisites/)):
  - **Windows:** Microsoft C++ Build Tools ("Desktop development with C++") and the WebView2 runtime (preinstalled on Windows 10 and 11). `aws-lc-sys` would normally need [NASM](https://www.nasm.us/); `.cargo/config.toml` sets `AWS_LC_SYS_PREBUILT_NASM=1` so it uses the prebuilt objects bundled in the crate instead.
  - **macOS:** Xcode Command Line Tools (`xcode-select --install`).
  - **Linux (Debian/Ubuntu):** `sudo apt install build-essential libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev`.
- **For the AI agent only:** Go 1.26 or later and a C compiler for CGO (on Windows, for example, MinGW-w64 `gcc`). The editor runs without them. See [Building the agent server](#building-the-agent-server).

### Run in Development

```bash
git clone https://github.com/dmonza/scmcode.git
cd scmcode
npm install
npm run tauri dev
```

### Build from Source

```bash
npm install
npx tauri build
```

`npx tauri build` runs the frontend build for you (`npm run build`), which raises Vite's Node heap limit internally. If you still hit an out-of-memory error, set `NODE_OPTIONS="--max-old-space-size=16384"`.

### Shipping your own updates (optional)

scmCode ships with no update feed and no signing key. To distribute signed updates for your own builds, generate a key pair with `npx tauri signer generate`, put the public key and your endpoint under `plugins.updater` in [`src-tauri/tauri.conf.json`](./src-tauri/tauri.conf.json) (`pubkey`, `endpoints`), and set `update.mode` in settings. Without a public key, the updater refuses to install anything.

---

## How It's Built

scmCode (like SideX) maps VS Code's Electron architecture onto Tauri layer by layer:

| VS Code (Electron) | scmCode (Tauri) |
|---|---|
| Electron main process | Tauri Rust backend |
| `BrowserWindow` | `WebviewWindow` |
| `ipcMain` / `ipcRenderer` | `invoke()` + Tauri events |
| Node.js `fs`, `pty`, etc. | Rust commands (`std::fs`, `portable-pty`) |
| Menu / Dialog / Clipboard | Tauri plugins |
| Renderer (DOM + TypeScript) | Same — runs in native webview |
| Extension host | Sidecar process (in progress) |

### Project Layout

```
scmcode/
├── src/                    # TypeScript workbench (ported from VS Code)
├── src-tauri/              # Rust backend (commands, agent-server supervisor, app setup)
├── sidexai/sidex-server/   # Go agent server (chat, tools, MCP, memory)
├── crates/                 # Rust support crates — agent tools, context engine, git, LSP, DAP, terminal, extensions, …
├── index.html
├── vite.config.ts
└── package.json
```

For a deeper dive, see [ARCHITECTURE.md](./ARCHITECTURE.md). To contribute, see [CONTRIBUTING.md](./CONTRIBUTING.md).

---

## Credits and License

scmCode is released under the [MIT License](./LICENSE).

It is built on the work of:

- **[SideX](https://github.com/Sidenai/sidex)** — Copyright (c) 2025 - present Siden Technologies Inc., MIT License.
- **[Visual Studio Code — Code - OSS](https://github.com/microsoft/vscode)** — Copyright (c) 2015 - present Microsoft Corporation, MIT License.

Their copyright and permission notices are preserved in [LICENSE](./LICENSE) and in source file headers, as the MIT License requires. Details in [NOTICE.md](./NOTICE.md).
