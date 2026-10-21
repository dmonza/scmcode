# scmCode Privacy

> **This is a technical description of how scmCode is designed, not a privacy policy,
> contract or warranty.** It reflects the authors' best understanding of the code and may
> be incomplete or become outdated. scmCode is provided "AS IS", without warranty of any
> kind and without liability, under the [MIT License](./LICENSE); see the
> [Disclaimer](./README.md#disclaimer-and-limitation-of-liability).

scmCode is designed not to collect telemetry, analytics, crash reports or usage data, and
has no account system or hosted backend. There is no scmCode server, so the scmCode project
does not receive, store or process your personal data and is not a data controller for it.
Any data you send to third-party services (model providers, extension registries, Git
hosts, extensions) is handled by those services under their own terms and privacy
policies.

This document lists the network connections that scmCode's own code is known to make and
what triggers them. If you find one that is not listed here, please report it as a bug.

## Off by default (nothing happens unless you turn it on)

| What | Where | How to enable |
|---|---|---|
| Live model pricing refresh | `openrouter.ai` (unauthenticated GET, at startup and hourly) | `SIDEX_LIVE_PRICING=1` in the environment |
| Fallback model endpoint | Whatever `OPENROUTER_BASE_URL` points to | Set `OPENROUTER_BASE_URL` |
| Update checks | The endpoint you configure | `plugins.updater.endpoints` in `tauri.conf.json` or `SIDEX_UPDATE_ENDPOINT`, plus `update.mode` in settings. Updates without a valid signature from your own `pubkey` are refused. |
| Workspace extension recommendations | Extension registry | Set `extensions.ignoreRecommendations` to `false` |
| Hosted semantic index / multi-user backend | Voyage, Cohere, Turbopuffer, Postgres | Only if `VOYAGE_API_KEY`, `COHERE_API_KEY`, `TURBOPUFFER_API_KEY` or `SIDEX_DATABASE_URL` is set |
| Remote sandbox for agent commands | Daytona (`app.daytona.io`, or `DAYTONA_API_URL`): the commands the agent runs in the sandbox and the files it reads and writes there | Only if `DAYTONA_API_KEY` is set; without it the sandbox uses a local Docker container |

Note: the agent server inherits the environment the app was launched from, so these
variables also take effect if they are already exported in your shell.

## Only when you do something

| Trigger | Where | What is sent |
|---|---|---|
| Browsing, searching or installing extensions | `open-vsx.org` (and its asset storage) | Search terms, extension IDs, your IP |
| Choosing a non-English display language whose pack is not installed | `open-vsx.org` | Language-pack ID |
| Sending a chat message, inline edit or completion request | **Only** the model provider(s) you configured in Settings → Models | Your prompt, the code context the agent selects, tool results |
| The agent using its web tools during a task | DuckDuckGo (search), the URLs it fetches, image-generation provider if configured | Search queries / requested URLs |
| Opening a file whose language server is not installed locally (e.g. rust-analyzer) | `github.com` releases | Download request for the server binary |
| Git push / pull / fetch / clone | Your Git remote | Standard Git traffic |
| Remote development (SSH, WSL, containers) | The host you connect to | Standard remote traffic |
| "Run Diagnostic" in Settings → Network | `open-vsx.org` and your providers | HEAD requests to test reachability |
| Windows installer, if WebView2 is missing | Microsoft (WebView2 bootstrapper) | Install-time download only |

For maximum privacy, use a local model server (Ollama, LM Studio, llama.cpp, vLLM): it is
detected automatically on loopback, and then prompts and code are sent only to that local
server.

## Not done by scmCode's own code (by design)

- No telemetry: VS Code's telemetry service is a no-op stub and `telemetry.telemetryLevel`
  defaults to `off`. Microsoft 1DS telemetry code and dependencies are removed.
- No product analytics: the agent server's analytics package is a no-op and the PostHog SDK
  is not included.
- No remote experiments or feature flags (`workbench.enableExperiments` is `false`).
- No implicit default AI provider: with nothing configured, AI requests fail locally.
- No app-attribution headers (`HTTP-Referer` / `X-Title`) sent to model providers.
- No reading of other applications' logins or tokens (for example Claude Code or Codex CLI
  credentials), and no impersonation of other clients.
- No calls to SideX, Siden Technologies or Microsoft Marketplace services.

## The system webview (Windows: Microsoft Edge WebView2)

scmCode's interface runs in the operating system's webview. On Windows that is the
Microsoft Edge WebView2 runtime, which can make background requests of its own. scmCode
starts every webview with arguments that turn off the runtime's background networking
(remote configuration and experiments, component updates, domain reliability reporting
and pings); see `WEBVIEW_BROWSER_ARGS` in `src-tauri/src/commands/window.rs`.

The same arguments turn off **Microsoft Defender SmartScreen** in scmCode's webviews
(`msSmartScreenProtection`), because SmartScreen sends the addresses the webview loads to
Microsoft to check their reputation. This is a trade-off: pages and links opened inside
scmCode's webviews are not checked against SmartScreen's list of malicious sites.

In a test on a clean profile (Windows 11, WebView2 155), this removed the runtime's request
to `config.edge.skype.com`. One connection from the WebView2 runtime to a Microsoft 365
host (`outlook.office.com` / `substrate.office.com`) was still observed on a machine signed
in with a Microsoft 365 account. It is made by the runtime outside its browser network stack
and could not be disabled with webview arguments. This behavior belongs to the WebView2
runtime and Windows, not to scmCode's code, and may vary by version and configuration.
Tracked in [ISSUE-0001](./docs/issues/ISSUE-0001-webview2-m365-startup-connection.md).

## Third-party extensions

Extensions you install run their own code and may collect data or make their own network
requests; scmCode cannot control that and is not responsible for it. scmCode reports
telemetry as disabled to extensions (`vscode.env.isTelemetryEnabled === false`), which
well-behaved extensions respect. Review each extension's own privacy practices.

## Where your data is stored

scmCode stores its own data locally, on your machine. You are responsible for securing and
backing it up:

- `~/.scmcode/` — agent state, sessions, memory, skills, rules, MCP configuration
- the OS application-data folder named `scmCode` — editor settings, extensions, profiles
- your OS keyring (with a local fallback store in the app-data folder) — API keys entered
  in Settings → Models
- `<workspace>/.scmcode/` — optional per-project rules, skills, hooks and MCP servers
