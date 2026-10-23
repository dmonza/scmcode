use super::{get_int, get_str, get_str_or, resolve_path, Args};
use crate::ToolContext;
use anyhow::{bail, Result};
use std::io::Read;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Bytes kept from each of stdout and stderr; the rest is read and discarded so
/// the command never blocks on a full pipe.
const MAX_STREAM_BYTES: usize = 100_000;
/// Bytes of combined output returned to the model.
const MAX_OUTPUT_BYTES: usize = 100_000;
/// How long to wait for the output pipes once the process tree is gone. A
/// descendant that escaped the tree (its own session or job) can keep a pipe
/// open forever; its output is then left behind instead of hanging the tool.
const PIPE_GRACE: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

pub fn run(args: &Args, ctx: &ToolContext) -> Result<String> {
    let command = get_str(args, "command");
    if command.is_empty() {
        bail!("command is required");
    }
    let timeout_sec = get_int(args, "timeout", 30).clamp(1, 300) as u64;
    let work_dir = resolve_path(ctx, get_str_or(args, "working_directory", &ctx.cwd));

    if !std::path::Path::new(&work_dir).is_dir() {
        bail!("working_directory {work_dir:?} does not exist");
    }

    let mut cmd = Command::new("sh");
    cmd.args(["-c", command])
        .current_dir(&work_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = tree::spawn(cmd).map_err(|e| anyhow::anyhow!("failed to spawn: {e}"))?;

    // scmCode: drain both pipes while the command runs. Waiting first and reading
    // after deadlocks as soon as the command writes more than a pipe buffer (~64 KB).
    let stdout = drain(child.take_stdout());
    let stderr = drain(child.take_stderr());

    let timeout = Duration::from_secs(timeout_sec);
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(Some(status)),
            Ok(None) if Instant::now() >= deadline => break Ok(None),
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(e) => break Err(e),
        }
    };
    // Kill what is left of the tree: everything on timeout, and on a normal exit
    // any background job the command started, so nothing outlives the tool call.
    child.kill_tree();

    let mut result = collect(stdout);
    result.push_str(&collect(stderr));

    let status = match status {
        Ok(Some(status)) => status,
        Ok(None) => {
            let partial = &result[..result.floor_char_boundary(MAX_OUTPUT_BYTES)];
            bail!(
                "command timed out after {timeout_sec}s and was killed. Output so far:\n{partial}"
            );
        }
        Err(e) => bail!("failed to wait on process: {e}"),
    };

    append_status(&mut result, status);
    if result.is_empty() {
        result = "(command produced no output)".into();
    }
    if result.len() > MAX_OUTPUT_BYTES {
        result.truncate(result.floor_char_boundary(MAX_OUTPUT_BYTES));
        result.push_str("\n...(truncated)");
    }
    Ok(result)
}

fn append_status(result: &mut String, status: ExitStatus) {
    if status.success() {
        return;
    }
    match status.code() {
        Some(code) => result.push_str(&format!("\nexit code: {code}")),
        None => result.push_str("\nterminated by a signal"),
    }
}

/// Read `pipe` to the end on its own thread, keeping the first
/// `MAX_STREAM_BYTES`.
fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> mpsc::Receiver<(Vec<u8>, bool)> {
    let (tx, rx) = mpsc::channel();
    if let Some(mut pipe) = pipe {
        thread::spawn(move || {
            let mut kept = Vec::new();
            let mut truncated = false;
            let mut buf = [0u8; 8192];
            loop {
                match pipe.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let room = MAX_STREAM_BYTES.saturating_sub(kept.len());
                        kept.extend_from_slice(&buf[..n.min(room)]);
                        truncated |= n > room;
                    }
                }
            }
            let _ = tx.send((kept, truncated));
        });
    }
    rx
}

fn collect(rx: mpsc::Receiver<(Vec<u8>, bool)>) -> String {
    match rx.recv_timeout(PIPE_GRACE) {
        Ok((bytes, truncated)) => {
            let mut text = String::from_utf8_lossy(&bytes).into_owned();
            if truncated {
                text.push_str("\n...(truncated)\n");
            }
            text
        }
        Err(_) => String::new(),
    }
}

/// scmCode: spawn the shell as the root of a process tree that can be killed as a
/// whole. A plain `Child::kill` only reaches `sh`, leaving its children running.
#[cfg(windows)]
mod tree {
    use process_wrap::std::{ChildWrapper, CommandWrap, CreationFlags, JobObject};
    use std::io;
    use std::process::{ChildStderr, ChildStdout, Command, ExitStatus};
    use windows::Win32::System::Threading::CREATE_NO_WINDOW;

    pub struct Tree(Box<dyn ChildWrapper>);

    pub fn spawn(cmd: Command) -> io::Result<Tree> {
        let mut cmd = CommandWrap::from(cmd);
        // The JobObject wrapper sets the creation flags itself, so CREATE_NO_WINDOW
        // goes through its shim rather than `console::no_console`.
        cmd.wrap(CreationFlags(CREATE_NO_WINDOW)).wrap(JobObject);
        cmd.spawn().map(Tree)
    }

    impl Tree {
        pub fn take_stdout(&mut self) -> Option<ChildStdout> {
            self.0.stdout().take()
        }
        pub fn take_stderr(&mut self) -> Option<ChildStderr> {
            self.0.stderr().take()
        }
        pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
            self.0.try_wait()
        }
        /// Terminate every process still in the job.
        pub fn kill_tree(&mut self) {
            let _ = self.0.start_kill();
        }
    }
}

#[cfg(unix)]
mod tree {
    use nix::sys::signal::{killpg, Signal};
    use nix::unistd::Pid;
    use std::io;
    use std::os::unix::process::CommandExt;
    use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus};

    pub struct Tree(Child);

    pub fn spawn(mut cmd: Command) -> io::Result<Tree> {
        // A new process group led by `sh`, so `killpg` reaches its children.
        cmd.process_group(0);
        cmd.spawn().map(Tree)
    }

    impl Tree {
        pub fn take_stdout(&mut self) -> Option<ChildStdout> {
            self.0.stdout.take()
        }
        pub fn take_stderr(&mut self) -> Option<ChildStderr> {
            self.0.stderr.take()
        }
        pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
            self.0.try_wait()
        }
        /// SIGKILL the whole process group, then reap `sh`. Orphaned members are
        /// reparented to init, which reaps them.
        pub fn kill_tree(&mut self) {
            if let Ok(pgid) = i32::try_from(self.0.id()) {
                let _ = killpg(Pid::from_raw(pgid), Signal::SIGKILL);
            }
            let _ = self.0.wait();
        }
    }
}
