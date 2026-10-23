//! scmCode: a repository's `core.fsmonitor` program must not run when scmCode reads
//! the repository's status (docs/audit TR-05).

use std::fs;
use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn get_status_does_not_run_the_repository_fsmonitor() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    git(&repo, &["config", "user.email", "test@test.com"]);
    git(&repo, &["config", "user.name", "Test"]);
    fs::write(repo.join("a.txt"), "a").unwrap();
    git(&repo, &["add", "a.txt"]);
    git(&repo, &["commit", "-m", "init"]);

    // The marker lives outside the work tree so it doesn't show up in the status.
    let marker = tmp.path().join("fsmonitor-ran");
    let marker_sh = marker.to_string_lossy().replace('\\', "/");
    // git runs the hook through the shell and appends its arguments; `#` drops them.
    git(
        &repo,
        &[
            "config",
            "core.fsmonitor",
            &format!("echo ran > '{marker_sh}' #"),
        ],
    );

    // Control: a plain `git status` does run it, so the setup is effective.
    let _ = Command::new("git")
        .current_dir(&repo)
        .args(["status", "--porcelain=v2"])
        .output()
        .unwrap();
    assert!(
        marker.exists(),
        "control failed: plain git status didn't run the fsmonitor hook"
    );
    fs::remove_file(&marker).unwrap();

    let entries = sidex_git::status::get_status(&repo).unwrap();
    assert!(entries.is_empty(), "{entries:?}");
    assert!(!marker.exists(), "get_status ran core.fsmonitor");
}
