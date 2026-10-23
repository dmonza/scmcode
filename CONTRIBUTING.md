# Contributing to scmCode

Thanks for your interest. scmCode is an MIT-licensed derivative of
[SideX](https://github.com/Sidenai/sidex). Contributions are welcome as long as they
respect the project's two pillars (see the [README](./README.md#pillars)):

1. **Minimalism.** Prefer removing code over adding it. New features should be optional
   and should not pull in hosted services.
2. **Privacy.** No telemetry, analytics, crash reporting, remote feature flags,
   or "phone home" requests. Any new outbound network call must be triggered by an
   explicit user action (or an explicit, opt-in setting), must be documented in
   [PRIVACY.md](./PRIVACY.md), and must go to a service the user chose.

Never remove the Microsoft or Siden Technologies copyright headers from files that carry
them: the MIT License requires that they be preserved. Dependencies and vendored code are
covered in [Licenses and third-party notices](#licenses-and-third-party-notices).

## Quick Start

```bash
git clone https://github.com/dmonza/scmcode.git
cd scmcode
npm install
npm run tauri dev
```

See [Prerequisites](./README.md#prerequisites) in the README. The editor runs standalone with no account of any kind; the AI chat panel additionally needs the agent server built (Go 1.26+) and a model provider of your own — see [AI Agent](./README.md#ai-agent) in the README.

## How to Contribute

1. **Fork** the repo
2. **Create a branch** — `git checkout -b my-fix`
3. **Make your changes** and test with `npm run tauri dev`
4. **Submit a PR** with a clear description of what you changed and why

## What to Work On

Check [Issues](https://github.com/dmonza/scmcode/issues) for open tasks. If you don't see an issue for what you want to fix, just open a PR.

Found a security vulnerability? Don't open an issue or a PR for it; report it privately as described in [SECURITY.md](./SECURITY.md).

## Code Guidelines

### Internal names

Internal identifiers (crate names `sidex-*`, `SideX*` classes, `sidexChat`, `SIDEX_*`
environment variables, the `sidexai/` directory) intentionally keep the upstream
names so that changes from SideX can still be merged. Only user-visible text uses the
scmCode name. Don't mass-rename internals.

### TypeScript
- Follow the existing VSCode patterns in the codebase
- Use `.js` extensions on imports (ES modules)
- Use VS Code's dependency injection: services declared with `createDecorator` and injected as constructor parameter decorators

### Rust
- Commands go in `src-tauri/src/commands/`
- Register new commands in `src-tauri/src/lib.rs`
- Use `Result<T, String>` for command return types
- Use `tokio` for async work

### Go (`sidexai/sidex-server`)
- This is the agent server: agent loop, tools, MCP, memory, context compression
- Build with `go build -tags fts5 -o sidex-server ./cmd/server` from `sidexai/sidex-server`
- Before a PR, run `gofmt -l .`, `go vet -tags fts5 ./...` and `go test -tags fts5 ./...` there (CI runs the same on Linux)
- The server is reachable by anything on the machine: keep new endpoints behind `auth.LocalGuard` (wired in `cmd/server/main.go`) and accept JSON bodies only
- It must keep working with no account and no hosted service — credentials come from its process environment only (`SIDEX_PROVIDER_<PROVIDER>_KEY` / `_BASE_URL`), never a config file. Never read other applications' credentials or impersonate other clients
- Test against a provider you have your own credentials for; there's no shared or hosted backend to test against

### General
- Keep PRs focused — one feature or fix per PR when possible
- If you're making a big architectural change, open an issue first to discuss

### Licenses and third-party notices

Every installer ships `THIRD_PARTY_NOTICES.txt`, the notices and license texts of all the
third-party code it contains. When a PR adds, removes or updates a dependency, a vendored
file or a built-in extension:

- The dependency must use a license in the policy: `deny.toml` for Rust crates,
  `scripts/notices/policy.json` for npm packages, Go modules and built-in extensions. Check
  Rust with `cargo deny --manifest-path Cargo.toml check licenses` ([cargo-deny](https://github.com/EmbarkStudios/cargo-deny)).
  Widening the policy is a licensing decision; explain it in the PR.
- Regenerate the notices and commit `THIRD_PARTY_NOTICES.txt`:

  ```bash
  npm ci
  bash scripts/setup-extensions.sh
  cargo install cargo-about --locked --features cli --version 0.9.2   # once
  npm run notices
  ```

  CI (`.github/workflows/licenses.yml`) fails when the file is out of date.
- Third-party code copied into the repository needs its license file next to it and an entry
  in `scripts/notices/embedded.json` (the generator fails on a `cgmanifest.json` under `src/`
  that is not listed there).
- Built-in extensions come only from Code - OSS, through `scripts/builtin-extensions.txt`.
  Never copy extensions from an installed VS Code, Cursor or `~/.vscode/extensions`: those are
  not redistributable.

## Project Layout

- `src/vs/` — The VSCode workbench (TypeScript)
- `src-tauri/src/` — Rust backend replacing Electron
- `sidexai/sidex-server/` — Go agent server (chat, tools, MCP, memory)
- `crates/` — Rust support crates: agent tools, context engine, git, LSP, DAP, terminal, extensions, and more
- `ARCHITECTURE.md` — How VSCode's architecture maps to Tauri

## License

By contributing, you agree that:

- your contributions are licensed under the [MIT License](./LICENSE);
- you have the right to submit them (they are your own work, or you are allowed to
  contribute them under the MIT License) and they do not include code you may not license;
- your contributions are provided "AS IS", without warranty of any kind, and the project
  maintainers accept no liability for them, as described in the LICENSE.
