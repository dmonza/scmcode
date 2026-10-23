/*---------------------------------------------------------------------------------------------
 *  Local tool executor — delegates tool execution to the Rust sidex-agent crate
 *  via Tauri IPC invoke(). The entire tool implementation (file I/O, search,
 *  shell, git) runs natively in Rust. This TS file is just the bridge.
 *--------------------------------------------------------------------------------------------*/

export interface ILocalToolRequest {
	type: 'tool_request';
	tool_call_id: string;
	name: string;
	arguments: string;
}

export interface ILocalToolResponse {
	type: 'tool_response';
	tool_call_id: string;
	output: string;
	error: string;
}

export interface ILocalToolContext {
	cwd: string;
	token?: string;
	onProgress?: (event: { name: string; status: 'running' | 'done' | 'error'; detail?: string }) => void;
}

// Tauri invoke is available globally in the webview via @tauri-apps/api
 
declare function __TAURI_INVOKE__(cmd: string, args?: Record<string, unknown>): Promise<any>;

interface RustToolResponse {
	tool_call_id: string;
	output: string;
	error: string;
}

// Check if Tauri invoke is available (we're in the desktop app, not a browser)
function getTauriInvoke(): ((cmd: string, args?: Record<string, unknown>) => Promise<unknown>) | null {
	const g = globalThis as unknown as {
		__TAURI_INVOKE__?: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
		__TAURI_INTERNALS__?: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> };
	};
	if (typeof g.__TAURI_INVOKE__ === 'function') {
		return g.__TAURI_INVOKE__;
	}
	if (g.__TAURI_INTERNALS__?.invoke) {
		return g.__TAURI_INTERNALS__.invoke;
	}
	return null;
}

/** True when the Rust agent crate is reachable via Tauri IPC. */
export function LOCAL_TOOLS_SUPPORTED(): boolean {
	return getTauriInvoke() !== null;
}

/**
 * scmCode: the only tool names the agent server delegates to the client. A copy of
 * `LocalExecTools` in `sidexai/sidex-server/internal/agent/types.go`; keep the two in sync.
 * Any other name in a `tool_request` is refused here, so a server can't reach the rest of
 * the Rust tool set (network fetches, deletes, process monitors) through this bridge
 * (docs/audit TR-04).
 */
const LOCAL_EXEC_TOOLS: ReadonlySet<string> = new Set([
	'cwd', 'read_file', 'write_file', 'edit_file',
	'multi_edit', 'regex_replace', 'patch_file',
	'list_dir', 'tree', 'search_files', 'glob',
	'grep', 'batch_read', 'file_info',
	'shell', 'run_background', 'shell_output',
	'kill_shell', 'list_shells',
	'git_status', 'git_log', 'git_commit',
	'git_diff_file', 'diff',
	'lsp_hover', 'lsp_definition',
	'lsp_references', 'lsp_diagnostics',
	'read_lints',
	'context_index', 'context_search', 'context_status',
	'enter_worktree', 'exit_worktree', 'list_worktrees',
	'checkpoint', 'rollback',
]);

export class LocalToolExecutor {
	constructor(private readonly getContext: () => ILocalToolContext) {}

	/** Runs a `tool_request` from the agent server, if its tool is one the server delegates. */
	async execute(req: ILocalToolRequest): Promise<ILocalToolResponse> {
		if (!LOCAL_EXEC_TOOLS.has(req.name)) {
			return {
				type: 'tool_response',
				tool_call_id: req.tool_call_id,
				output: '',
				error: `tool ${JSON.stringify(req.name)} is not run locally`,
			};
		}
		return this.executeOwn(req);
	}

	/** Runs a request the workbench builds itself, with a fixed tool name. */
	async executeOwn(req: ILocalToolRequest): Promise<ILocalToolResponse> {
		const invoke = getTauriInvoke();
		if (!invoke) {
			return {
				type: 'tool_response',
				tool_call_id: req.tool_call_id,
				output: '',
				error: 'local tool execution unavailable (Tauri IPC not reachable)',
			};
		}

		const ctx = this.getContext();
		ctx.onProgress?.({ name: req.name, status: 'running' });

		try {
			const result = await invoke('agent_execute_tool', {
				request: {
					tool_call_id: req.tool_call_id,
					name: req.name,
					arguments: req.arguments || '{}',
					cwd: ctx.cwd,
					token: ctx.token || "",
				},
			}) as RustToolResponse;

			const status = result.error ? 'error' : 'done';
			ctx.onProgress?.({ name: req.name, status, detail: result.error || undefined });

			return {
				type: 'tool_response',
				tool_call_id: result.tool_call_id,
				output: result.output,
				error: result.error,
			};
		} catch (e) {
			const msg = (e as Error).message || String(e);
			ctx.onProgress?.({ name: req.name, status: 'error', detail: msg });
			return {
				type: 'tool_response',
				tool_call_id: req.tool_call_id,
				output: '',
				error: msg,
			};
		}
	}
}
