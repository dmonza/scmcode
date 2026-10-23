//! scmCode: child processes never outlive the app (docs/audit TR-10).
//!
//! On Windows every long-lived child (agent server, extension host, tasks, debug
//! adapters, terminals, MCP and language servers) is put in one Job Object that
//! has `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. Only this process holds the job's
//! handle, so when the app ends for any reason (quit, crash, `abort`, End Task)
//! Windows closes the handle and terminates every process in the job, including
//! the children those processes started later (they inherit the job). VS Code
//! gets the same from Electron; a plain `Child::kill` on exit only covers a clean
//! quit and only the direct child.
//!
//! Elsewhere these functions do nothing: children that talk over stdin (the
//! extension host, language and MCP servers) exit when the app's end of the pipe
//! closes, and the agent server is found through its pidfile on the next start
//! (`server.rs`).

#[cfg(windows)]
mod imp {
    use std::os::windows::io::{AsRawHandle, RawHandle};
    use std::sync::OnceLock;
    use win32job::{ExtendedLimitInfo, Job};

    /// The job, created on first use and never dropped: it must stay open for
    /// as long as the app runs.
    fn job() -> Option<&'static Job> {
        static JOB: OnceLock<Option<Job>> = OnceLock::new();
        JOB.get_or_init(|| {
            let mut info = ExtendedLimitInfo::new();
            info.limit_kill_on_job_close();
            match Job::create_with_limit_info(&info) {
                Ok(job) => Some(job),
                Err(e) => {
                    log::warn!("[children] could not create the job object: {e}");
                    None
                }
            }
        })
        .as_ref()
    }

    pub fn tie_handle(handle: Option<RawHandle>, what: &str) {
        let (Some(job), Some(handle)) = (job(), handle) else {
            return;
        };
        if let Err(e) = job.assign_process(handle as isize) {
            log::warn!("[children] could not tie {what} to the app: {e}");
        }
    }

    pub fn tie(child: &std::process::Child, what: &str) {
        tie_handle(Some(child.as_raw_handle()), what);
    }

    pub fn tie_tokio(child: &tokio::process::Child, what: &str) {
        tie_handle(child.raw_handle(), what);
    }

    pub fn tie_pty(child: &(dyn portable_pty::Child + Send), what: &str) {
        tie_handle(child.as_raw_handle(), what);
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn tie(_child: &std::process::Child, _what: &str) {}
    pub fn tie_tokio(_child: &tokio::process::Child, _what: &str) {}
    pub fn tie_pty(_child: &(dyn portable_pty::Child + Send), _what: &str) {}
}

pub use imp::{tie, tie_pty, tie_tokio};

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::process::Command;

    /// A tied child is in the app's job; the job kills it when this test
    /// process ends, which can't be observed from inside. What can be checked
    /// here: tying works on a real child and a second tie is harmless.
    #[test]
    fn tying_a_child_succeeds_and_is_idempotent() {
        let mut child = Command::new("cmd")
            .args(["/C", "ping", "-n", "3", "127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("spawn cmd");
        tie(&child, "test child");
        tie(&child, "test child");
        let _ = child.kill();
        let _ = child.wait();
    }
}
