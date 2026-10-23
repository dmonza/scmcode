use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tauri::State;
use tokio::sync::Semaphore;

use super::next_gen_tools::CheckpointStore;

/// Limit concurrent agent tool executions to prevent memory blowup from
/// parallel indexing/graph-building operations.
static TOOL_SEMAPHORE: OnceLock<Semaphore> = OnceLock::new();

fn tool_semaphore() -> &'static Semaphore {
    TOOL_SEMAPHORE.get_or_init(|| Semaphore::new(2))
}

/// Longest a single tool call may hold a permit: the shell tool's 300 s maximum
/// plus room for its cleanup.
const TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(330);

/// Tools that mutate files on disk; their targets must be snapshotted for
/// checkpoint rollback BEFORE execution.
fn is_write_tool(name: &str) -> bool {
    matches!(
        name,
        "edit_file"
            | "write_file"
            | "multi_edit"
            | "patch_file"
            | "regex_replace"
            | "notebook_edit"
    )
}

/// Extract the target file path from a write tool's JSON arguments,
/// resolved against the request cwd.
fn extract_target_path(arguments: &str, cwd: &str) -> Option<String> {
    let args: serde_json::Value = serde_json::from_str(arguments).ok()?;
    let path = args
        .get("path")
        .or_else(|| args.get("file_path"))
        .and_then(|v| v.as_str())?;
    if std::path::Path::new(path).is_absolute() {
        Some(path.to_string())
    } else {
        Some(
            std::path::Path::new(cwd)
                .join(path)
                .to_string_lossy()
                .to_string(),
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct AgentToolRequest {
    pub tool_call_id: String,
    pub name: String,
    pub arguments: String,
    pub cwd: String,
    #[serde(default)]
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct AgentToolResponse {
    pub tool_call_id: String,
    pub output: String,
    pub error: String,
}

#[tauri::command]
pub async fn agent_execute_tool(
    checkpoint_state: State<'_, CheckpointStore>,
    request: AgentToolRequest,
) -> Result<AgentToolResponse, String> {
    let fallback_id = request.tool_call_id.clone();
    let name = request.name.clone();

    // Snapshot pre-edit content for write tools so checkpoint rollback and
    // chat "Reject" can actually restore files modified by the agent.
    // block_in_place keeps the (size-capped) file read off the async executor.
    if is_write_tool(&request.name) {
        if let Some(path) = extract_target_path(&request.arguments, &request.cwd) {
            tokio::task::block_in_place(|| checkpoint_state.track_modified(&path));
        }
    }

    let _permit = match tool_semaphore().acquire().await {
        Ok(p) => p,
        Err(_) => {
            return Ok(AgentToolResponse {
                tool_call_id: fallback_id,
                output: String::new(),
                error: "tool execution semaphore closed".to_string(),
            });
        }
    };

    // scmCode: bound the wait so a tool that never returns can't hold one of the two
    // permits forever and stall every later tool call (docs/audit TR-09). The blocking
    // thread can't be cancelled; it is left to finish on its own. The shell tool kills
    // its process tree after at most 300 s, so this only fires for a tool that hangs.
    let task = tauri::async_runtime::spawn_blocking(move || {
        let ctx = sidex_agent::ToolContext {
            cwd: request.cwd.clone(),
            token: request.token.clone(),
        };
        let req = sidex_agent::ToolRequest {
            tool_call_id: request.tool_call_id.clone(),
            name: request.name,
            arguments: request.arguments,
        };
        let resp = sidex_agent::execute(&req, &ctx);
        AgentToolResponse {
            tool_call_id: resp.tool_call_id,
            output: resp.output,
            error: resp.error,
        }
    });
    let error = match tokio::time::timeout(TOOL_TIMEOUT, task).await {
        Ok(Ok(resp)) => return Ok(resp),
        // A panic in the tool surfaces here as a JoinError (release builds unwind).
        Ok(Err(e)) => format!("internal error: {e}"),
        Err(_) => format!(
            "tool {name} did not finish within {}s",
            TOOL_TIMEOUT.as_secs()
        ),
    };
    Ok(AgentToolResponse {
        tool_call_id: fallback_id,
        output: String::new(),
        error,
    })
}
