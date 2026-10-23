package tools

import "os/exec"

// GitCommand builds a git command that never runs the repository's
// core.fsmonitor program.
//
// scmCode: git runs the program named in core.fsmonitor whenever it refreshes the
// index (status, diff, add, commit), and that setting can come with a folder the
// user opens (a downloaded archive that includes .git/config). VS Code avoids this
// with Workspace Trust, which scmCode does not have yet (docs/audit TR-05, SEC-19).
// fsmonitor only speeds up large repositories, so it is turned off for every git
// call scmCode makes.
func GitCommand(args ...string) *exec.Cmd {
	return exec.Command("git", append([]string{"-c", "core.fsmonitor=false"}, args...)...)
}
