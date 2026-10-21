# Security Policy

scmCode is an early-release, single-maintainer project. Security reports are welcome and
are handled on a best-effort basis.

## Supported versions

Only the latest release and the `main` branch receive security fixes.

## Reporting a vulnerability

**Please do not open a public issue, discussion or pull request for a vulnerability.**

Report it privately through GitHub: open the repository's **Security** tab and choose
**Report a vulnerability**, or go directly to
<https://github.com/dmonza/scmcode/security/advisories/new>.

Please include:

- the scmCode version or commit, and your operating system;
- what an attacker can do, and what they need first (a web page you visit, a workspace you
  open, an extension, local access, …);
- steps to reproduce, or a proof of concept.

You will get an acknowledgment once the report has been read. Please allow reasonable time
for a fix before disclosing the issue publicly. Reporters are credited in the advisory
unless they prefer not to be.

## Scope

In scope is scmCode's own code: the Tauri backend (`src-tauri/`, `crates/`), the agent
server (`sidexai/`), scmCode's changes to the workbench (`src/`), and the build and release
workflows. Examples of what we especially want to hear about:

- a web page, a workspace, an extension or a remote host making the agent server run
  commands, edit or read files, or read sessions, transcripts or memory;
- a way around a permission prompt or another documented safeguard of the agent;
- a network connection made by scmCode's own code that is not listed in
  [PRIVACY.md](./PRIVACY.md);
- a way to make the updater install an update without a valid signature.

Out of scope:

- vulnerabilities in [SideX](https://github.com/Sidenai/sidex) or
  [Code - OSS](https://github.com/microsoft/vscode) that are not specific to scmCode —
  please report those upstream (you are welcome to tell us too);
- third-party extensions, MCP servers, language servers and model providers;
- issues that require an attacker who can already run code as your user;
- the known limitations below.

## Known limitations

- **The agent server has no authentication against local processes.** It listens on
  `127.0.0.1` only and rejects requests from other web origins, non-loopback `Host`
  headers (DNS rebinding) and non-JSON bodies, but any program or other user account on
  the same machine can connect to it.
- **Extensions and workspaces run code.** Extensions run with your permissions, and tasks,
  Git hooks, build scripts and MCP servers in a workspace execute on your machine. See
  [Security is your responsibility](./README.md#security-is-your-responsibility).
- **AI agents can be misled by the content they read** (prompt injection). Reports are in
  scope when such content gets past a permission prompt or another documented safeguard.
