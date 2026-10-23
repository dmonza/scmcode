//! File operations — read, write, create, rename, delete, stat, directory listing.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::error::{WorkspaceError, WorkspaceResult};

/// A directory entry returned by [`read_dir`].
#[derive(Debug, Clone, Serialize)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_file: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: u64,
}

/// Metadata about a file or directory.
#[derive(Debug, Clone, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct FileStat {
    pub size: u64,
    pub modified: u64,
    pub created: u64,
    pub is_dir: bool,
    pub is_file: bool,
    pub is_symlink: bool,
    pub readonly: bool,
}

/// Read a file as a UTF-8 string.
pub fn read_file(path: &Path) -> WorkspaceResult<String> {
    fs::read_to_string(path).map_err(|e| WorkspaceError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

/// Read a file as raw bytes.
pub fn read_file_bytes(path: &Path) -> WorkspaceResult<Vec<u8>> {
    fs::read(path).map_err(|e| WorkspaceError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

/// Write UTF-8 content to a file, creating parent directories as needed.
///
/// The write is atomic and durable; see [`atomic_write`].
pub fn write_file(path: &Path, content: &str) -> WorkspaceResult<()> {
    atomic_write(path, content.as_bytes())
}

/// Write raw bytes to a file, creating parent directories as needed.
///
/// The write is atomic and durable; see [`atomic_write`].
pub fn write_file_bytes(path: &Path, content: &[u8]) -> WorkspaceResult<()> {
    atomic_write(path, content)
}

/// Write `content` to `path` atomically and durably, creating parent directories as
/// needed.
///
/// scmCode: VS Code never writes a file in place. It writes the new bytes to a
/// temporary sibling, flushes them to disk, then renames that temp over the target —
/// an atomic operation on every OS. A crash, power loss or failed write can therefore
/// never leave a half-written, truncated or empty file: readers always see either the
/// complete old file or the complete new one. The SideX port used `fs::write` (an
/// in-place `O_TRUNC`), which loses the file if the process dies mid-write. Following
/// `src/vs/platform/files/node/diskFileSystemProvider.ts` (`doWriteFileAtomic`), and
/// recorded in `docs/audit/2026-10-23-safety-reliability-and-data-durability-review.md`
/// (DL-01).
///
/// Notes:
/// - The temp file is created in the target's own directory so the rename stays on one
///   filesystem (a cross-device rename is not atomic and would fail).
/// - A symlink target is resolved first, so we replace the file it points at instead of
///   turning the link into a regular file.
/// - On an existing file, its permissions are carried over to the replacement.
/// - We `fsync` the new contents before the rename (durability against power loss). We
///   deliberately do not also `fsync` the directory: the rename itself is atomic, so the
///   worst a lost directory flush can do is keep the intact old file — never corruption —
///   and skipping it keeps saves fast and avoids platform-specific directory-handle
///   quirks.
fn atomic_write(path: &Path, content: &[u8]) -> WorkspaceResult<()> {
    // Resolve a symlink to its real target so we overwrite the pointed-at file.
    let target = match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
        }
        _ => path.to_path_buf(),
    };

    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| WorkspaceError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
    }

    let tmp = temp_sibling(&target);

    // Write and flush to disk, then close the handle (Windows cannot rename over a file
    // while a handle to the source is open).
    let write_result = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(content)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(WorkspaceError::Io {
            path: tmp,
            source: e,
        });
    }

    // Carry over the existing file's permissions to the replacement, if any.
    if let Ok(meta) = fs::metadata(&target) {
        let _ = fs::set_permissions(&tmp, meta.permissions());
    }

    // Atomic replace. `fs::rename` overwrites an existing destination on both Unix and
    // Windows (MoveFileExW with MOVEFILE_REPLACE_EXISTING).
    if let Err(e) = fs::rename(&tmp, &target) {
        let _ = fs::remove_file(&tmp);
        return Err(WorkspaceError::Io {
            path: target,
            source: e,
        });
    }

    Ok(())
}

/// A unique temp path in the target's own directory, hidden with a leading dot so it
/// does not flicker into file listings. Uniqueness comes from the process id, a
/// monotonic counter and the current time, so concurrent writers never collide.
fn temp_sibling(target: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let pid = std::process::id();
    let name = target
        .file_name()
        .map_or_else(|| "file".to_string(), |n| n.to_string_lossy().into_owned());

    let temp_name = format!(".{name}.{pid}.{seq}.{nanos}.scmcode-tmp");
    match target.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(temp_name),
        _ => PathBuf::from(temp_name),
    }
}

/// Create an empty file, creating parent directories as needed.
pub fn create_file(path: &Path) -> WorkspaceResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| WorkspaceError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    fs::File::create(path).map_err(|e| WorkspaceError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

/// Create a directory. When `recursive` is true, creates all parent directories.
pub fn mkdir(path: &Path, recursive: bool) -> WorkspaceResult<()> {
    let op = if recursive {
        fs::create_dir_all(path)
    } else {
        fs::create_dir(path)
    };
    op.map_err(|e| WorkspaceError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

/// Create a directory and all parents (shorthand for `mkdir(path, true)`).
pub fn create_dir(path: &Path) -> WorkspaceResult<()> {
    mkdir(path, true)
}

/// Rename / move a file or directory.
pub fn rename(from: &Path, to: &Path) -> WorkspaceResult<()> {
    fs::rename(from, to).map_err(|e| WorkspaceError::Io {
        path: from.to_path_buf(),
        source: e,
    })
}

/// Remove a file or directory. When `recursive` is true, removes non-empty directories.
pub fn remove(path: &Path, recursive: bool) -> WorkspaceResult<()> {
    let meta = fs::metadata(path).map_err(|e| WorkspaceError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    if meta.is_dir() {
        if recursive {
            fs::remove_dir_all(path)
        } else {
            fs::remove_dir(path)
        }
    } else {
        fs::remove_file(path)
    }
    .map_err(|e| WorkspaceError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

/// Delete a file or directory. Removes directories recursively.
pub fn delete(path: &Path) -> WorkspaceResult<()> {
    let meta = fs::symlink_metadata(path).map_err(|e| WorkspaceError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    if meta.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
    .map_err(|e| WorkspaceError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

/// Copy a file.
pub fn copy_file(from: &Path, to: &Path) -> WorkspaceResult<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|e| WorkspaceError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    fs::copy(from, to).map_err(|e| WorkspaceError::Io {
        path: from.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

/// Check whether a path exists.
pub fn exists(path: &Path) -> bool {
    path.exists()
}

/// List directory contents, sorted with directories first, then case-insensitive by name.
pub fn read_dir(path: &Path) -> WorkspaceResult<Vec<DirEntry>> {
    let entries = fs::read_dir(path).map_err(|e| WorkspaceError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    let mut result = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| WorkspaceError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let metadata = entry.metadata().map_err(|e| WorkspaceError::Io {
            path: entry.path(),
            source: e,
        })?;
        let file_type = entry.file_type().map_err(|e| WorkspaceError::Io {
            path: entry.path(),
            source: e,
        })?;

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());

        result.push(DirEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            path: entry.path().to_string_lossy().to_string(),
            is_dir: file_type.is_dir(),
            is_file: file_type.is_file(),
            is_symlink: file_type.is_symlink(),
            size: metadata.len(),
            modified,
        });
    }

    result.sort_unstable_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then_with(|| {
            a.name
                .to_ascii_lowercase()
                .cmp(&b.name.to_ascii_lowercase())
        })
    });

    Ok(result)
}

/// Get metadata for a path.
pub fn stat(path: &Path) -> WorkspaceResult<FileStat> {
    let meta = fs::symlink_metadata(path).map_err(|e| WorkspaceError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());

    let created = meta
        .created()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());

    Ok(FileStat {
        size: meta.len(),
        modified,
        created,
        is_dir: meta.is_dir(),
        is_file: meta.file_type().is_file(),
        is_symlink: meta.file_type().is_symlink(),
        readonly: meta.permissions().readonly(),
    })
}

/// Check if a file appears to be binary (null-byte heuristic on first 8 KiB).
pub fn is_binary_file(path: &Path) -> WorkspaceResult<bool> {
    use std::io::Read;
    let mut file = fs::File::open(path).map_err(|e| WorkspaceError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let mut buf = [0u8; 8192];
    let n = file.read(&mut buf).map_err(|e| WorkspaceError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(buf[..n].contains(&0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn read_write_round_trip() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("test.txt");
        write_file(&p, "hello").unwrap();
        assert_eq!(read_file(&p).unwrap(), "hello");
    }

    #[test]
    fn write_bytes_round_trip() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("data.bin");
        write_file_bytes(&p, &[0xDE, 0xAD, 0xBE, 0xEF]).unwrap();
        assert_eq!(read_file_bytes(&p).unwrap(), vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn create_and_stat() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("sub/deep/file.txt");
        create_file(&p).unwrap();
        let s = stat(&p).unwrap();
        assert!(s.is_file);
        assert!(!s.is_dir);
    }

    #[test]
    fn mkdir_non_recursive() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("single");
        mkdir(&p, false).unwrap();
        assert!(p.is_dir());
    }

    #[test]
    fn mkdir_recursive() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("a/b/c");
        mkdir(&p, true).unwrap();
        assert!(p.is_dir());
    }

    #[test]
    fn read_dir_sorted() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("zdir")).unwrap();
        fs::write(tmp.path().join("afile.txt"), "a").unwrap();
        fs::write(tmp.path().join("bfile.txt"), "b").unwrap();

        let entries = read_dir(tmp.path()).unwrap();
        assert!(entries[0].is_dir, "directories come first");
        assert_eq!(entries[0].name, "zdir");
    }

    #[test]
    fn rename_and_delete() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.txt");
        let b = tmp.path().join("b.txt");
        write_file(&a, "data").unwrap();
        rename(&a, &b).unwrap();
        assert!(!a.exists());
        assert_eq!(read_file(&b).unwrap(), "data");
        delete(&b).unwrap();
        assert!(!b.exists());
    }

    #[test]
    fn remove_with_flags() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("f.txt");
        write_file(&f, "x").unwrap();
        remove(&f, false).unwrap();
        assert!(!exists(&f));

        let d = tmp.path().join("dir/sub");
        mkdir(&d, true).unwrap();
        write_file(&d.join("nested.txt"), "y").unwrap();
        remove(&tmp.path().join("dir"), true).unwrap();
        assert!(!exists(&tmp.path().join("dir")));
    }

    #[test]
    fn exists_works() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("nope.txt");
        assert!(!exists(&p));
        write_file(&p, "yes").unwrap();
        assert!(exists(&p));
    }

    #[test]
    fn copy_file_works() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src.txt");
        let dst = tmp.path().join("nested/dst.txt");
        write_file(&src, "content").unwrap();
        copy_file(&src, &dst).unwrap();
        assert_eq!(read_file(&dst).unwrap(), "content");
    }

    #[test]
    fn binary_detection() {
        let tmp = TempDir::new().unwrap();
        let text = tmp.path().join("text.txt");
        write_file(&text, "hello world").unwrap();
        assert!(!is_binary_file(&text).unwrap());

        let bin = tmp.path().join("binary.bin");
        write_file_bytes(&bin, &[0x00, 0xFF, 0x00]).unwrap();
        assert!(is_binary_file(&bin).unwrap());
    }

    #[test]
    fn overwrite_replaces_content_fully() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("f.txt");
        write_file(&p, "a much longer first version").unwrap();
        // A shorter second write must not leave trailing bytes of the first.
        write_file(&p, "short").unwrap();
        assert_eq!(read_file(&p).unwrap(), "short");
    }

    #[test]
    fn write_leaves_no_temp_files() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("f.txt");
        write_file(&p, "v1").unwrap();
        write_file(&p, "v2").unwrap();

        let leftovers: Vec<_> = fs::read_dir(tmp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains("scmcode-tmp"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp files left behind: {leftovers:?}"
        );
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 1);
    }

    #[test]
    fn write_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("deep/nested/dir/file.txt");
        write_file(&p, "ok").unwrap();
        assert_eq!(read_file(&p).unwrap(), "ok");
    }

    #[test]
    fn temp_sibling_is_in_target_dir_and_unique() {
        let dir = Path::new("/some/dir");
        let target = dir.join("file.txt");
        let a = temp_sibling(&target);
        let b = temp_sibling(&target);
        assert_eq!(a.parent(), Some(dir));
        assert_ne!(a, b, "consecutive temp names must differ");
    }

    #[cfg(unix)]
    #[test]
    fn write_through_symlink_updates_target_not_link() {
        use std::os::unix::fs::symlink;
        let tmp = TempDir::new().unwrap();
        let real = tmp.path().join("real.txt");
        let link = tmp.path().join("link.txt");
        write_file(&real, "original").unwrap();
        symlink(&real, &link).unwrap();

        write_file(&link, "updated").unwrap();

        // The link is still a symlink and the real file received the new content.
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(read_file(&real).unwrap(), "updated");
    }
}
