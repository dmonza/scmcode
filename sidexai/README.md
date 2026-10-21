# scmCode AI agent server

Local agent server used by the scmCode desktop app (`sidexai/sidex-server`). Go process on loopback: tools, MCP, memory, and provider calls. The directory and binary keep their upstream SideX names (`sidexai`, `sidex-server`, `sidex`) so changes can still be merged from upstream.

The desktop app builds and supervises this server. You only need this README if you are running the server by hand.

> **Warning.** This server executes shell commands and edits files on your machine on behalf
> of an AI model. AI output can be wrong or destructive. Run it only on loopback, review
> what it does, and keep backups. It is provided "AS IS", without warranty of any kind and
> without liability, under the [MIT License](../LICENSE); see the
> [Disclaimer](../README.md#disclaimer-and-limitation-of-liability).

## Architecture

```
┌─────────────────┐         WebSocket          ┌──────────────────────┐
│  client (Rust)   │ ◄──────────────────────► │   sidex-server (Go)   │
│                  │                           │                       │
│  • Interactive   │                           │  • Model providers    │
│  • Syntax HL     │                           │  • Context Compress   │
│  • Tool Display  │                           │  • Memory (BoltDB)    │
│  • Streaming     │                           │  • Tool Execution     │
└─────────────────┘                            │  • Session Mgmt       │
                                               └──────────────────────┘
```

## Tools

| Tool | Description |
|------|-------------|
| `read_file` | Read files with line numbers, auto-compress large files |
| `write_file` | Create/overwrite files, auto-create directories |
| `edit_file` | Find-and-replace with uniqueness checks |
| `list_dir` | List directory contents with sizes |
| `search_files` | Glob-based recursive file search |
| `grep` | Ripgrep-powered content search |
| `shell` | Execute shell commands |
| `glob` | Find files by pattern |
| `tree` | Directory tree visualization |
| `diff` | Git diff output |
| `batch_read` | Read multiple files at once |
| `file_info` | File metadata (size, permissions, timestamps) |

## Features

- **Context Compression** — Automatically compresses conversation history when it gets large, prioritizing code-significant lines (function defs, imports, error handling) and dropping filler
- **Persistent Memory** — BoltDB-backed key-value memory store with tags and search, injected into system prompts so the AI remembers your project
- **Streaming** — Real-time WebSocket streaming with tool call display
- **Syntax Highlighting** — Code blocks highlighted in the terminal via syntect
- **Session Management** — Resume previous conversations
- **Tool Output Compression** — Large shell/file outputs are automatically truncated with head+tail preservation

## Setup

### Prerequisites

- Go 1.26+ and a C compiler (CGO, for SQLite and tree-sitter)
- Rust 1.75+ (CLI only)
- `rg` (ripgrep) installed and on PATH
- A model provider of your own: an API key or a local server. There is no default provider.

### Server

```bash
cd sidex-server
go build -tags fts5 -o sidex-server ./cmd/server
./sidex-server
```

Server starts on `127.0.0.1:7433` by default. Override the port with `SIDEX_PORT`. Provider credentials and all other options are described in [`sidex-server/.env.example`](./sidex-server/.env.example).

### CLI

```bash
cd sidex-cli
cargo build --release
```

Binary lands at `target/release/sidex.exe` (Windows) or `target/release/sidex` (Unix).

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `SIDEX_PORT` | `7433` | Server port |
| `SIDEX_SERVER` | `ws://localhost:7433` | Server URL (CLI) |
| `SIDEX_MODEL` | — | Default model ID |
| `SIDEX_PROVIDER_<PROVIDER>_KEY` / `_BASE_URL` | — | Provider credentials |

See [`sidex-server/.env.example`](./sidex-server/.env.example) for the full list.

## Usage

```bash
# Interactive mode (default)
sidex

# One-shot question
sidex ask "explain this codebase"

# List sessions
sidex sessions

# Search memories
sidex memory "database schema"

# Check server
sidex status

# Resume session
sidex chat --session <id>

# Custom server
sidex --server ws://myserver:7433
```

### In-Session Commands

| Command | Action |
|---------|--------|
| `/quit` | Exit |
| `/clear` | Clear screen |
| `/memory` | Show stored memories |
| `/sessions` | List sessions |
| `/status` | Server health check |

## Project Structure

```
sidex-server/
├── cmd/server/main.go        # Entry point
├── internal/
│   ├── ai/                   # Provider clients + compression
│   ├── api/handler.go        # HTTP/WebSocket API handlers
│   ├── compress/             # Context compression engine
│   ├── memory/               # BoltDB persistence layer
│   ├── session/              # Session management
│   └── tools/                # Tool execution engine and definitions
└── go.mod

sidex-cli/
├── src/
│   ├── main.rs               # CLI entry, arg parsing, client and terminal UI
│   └── config.rs             # Configuration
└── Cargo.toml
```
