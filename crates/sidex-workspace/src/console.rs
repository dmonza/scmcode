//! scmCode: keep `git` subprocesses from opening console windows on Windows.
//!
//! The release app is a GUI process with no console of its own, so Windows
//! gives every console program it starts a new, visible console window unless
//! `CREATE_NO_WINDOW` is set.

use std::process::Command;

/// Mark `cmd` to run without a console window. A no-op off Windows.
#[cfg(windows)]
pub(crate) fn no_console(mut cmd: Command) -> Command {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd
}

/// Mark `cmd` to run without a console window. A no-op off Windows.
#[cfg(not(windows))]
pub(crate) fn no_console(cmd: Command) -> Command {
    cmd
}
