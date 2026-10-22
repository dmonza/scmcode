//! scmCode: decide what the `sidex-asset://` protocol serves, and with which
//! headers (TR-02).
//!
//! `sidex-asset` turns a workbench `file://` URI into a fetchable URL for local
//! images, icons and fonts (`src/vs/base/common/network.ts`, `uriToBrowserUri`).
//! Tauri's ACL treats this scheme as the app's own origin, so a *document*
//! fetched from it (an `.svg` carrying a script, say) would run with that
//! origin's trust. Every response therefore carries
//! `Content-Security-Policy: sandbox` — an opaque origin, no scripts — and
//! `X-Content-Type-Options: nosniff`. A browser ignores both when the file is
//! used as a subresource (an `<img>`, a `@font-face`) and applies them only
//! when the file is loaded as a document, which is the attack.
//!
//! The workbench always sends an absolute path, so relative paths (which would
//! resolve against the process working directory) are refused, and a deny list
//! keeps obvious secrets out. A positive allowlist of roots (app resources,
//! extension directories, open workspace folders) is the next step; it needs a
//! runtime check of every legitimate asset path first, so it is not here yet.

use std::path::Path;

/// Put on every `sidex-asset` response so a served file can't act as a
/// privileged document even if something loads it as one.
pub const SANDBOX_CSP: &str = "sandbox; default-src 'none'";

/// Whether a decoded request path may be served before it is canonicalized.
/// The workbench sends absolute file paths; a relative one is never legitimate
/// and would resolve against the process working directory.
pub fn is_servable_request(decoded: &str) -> bool {
    !decoded.is_empty() && Path::new(decoded).is_absolute()
}

/// Whether a canonicalized path is safe to serve. Compares with `/` so the
/// checks also match canonical Windows paths, which use `\`.
pub fn is_safe_canonical(canonical: &Path) -> bool {
    let s = canonical.to_string_lossy().replace('\\', "/");
    !s.contains("/.ssh/")
        && !s.contains("/.gnupg/")
        && !s.contains("/.aws/")
        && !s.contains("/.npmrc")
        && !s.contains("/etc/")
        && !s.starts_with("/System")
        && !s.contains("/.env")
        && !s.ends_with(".vscdb")
}

/// The content type for a file extension. Unknown types are served as opaque
/// bytes, never as `text/html`, so nothing served here is ever treated as a
/// web page.
pub fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("css") => "text/css",
        Some("js") => "text/javascript",
        Some("json") => "application/json",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_relative_and_empty_request_paths() {
        assert!(!is_servable_request(""));
        assert!(!is_servable_request("evil.svg"));
        assert!(!is_servable_request("foo/bar.png"));
        assert!(!is_servable_request("../secret"));
    }

    #[test]
    fn accepts_absolute_request_paths() {
        #[cfg(windows)]
        assert!(is_servable_request("C:/Users/x/pic.png"));
        #[cfg(not(windows))]
        assert!(is_servable_request("/home/x/pic.png"));
    }

    #[test]
    fn deny_list_blocks_secrets_on_either_separator() {
        for p in [
            "/home/u/.ssh/id_ed25519",
            "C:\\Users\\u\\.ssh\\id_ed25519",
            "/home/u/.aws/credentials",
            "/home/u/.gnupg/secring.gpg",
            "/home/u/project/.env",
            "/etc/shadow",
            "C:\\Users\\u\\AppData\\Roaming\\scmCode\\state.vscdb",
            "/home/u/.npmrc",
        ] {
            assert!(!is_safe_canonical(Path::new(p)), "{p} should be denied");
        }
    }

    #[test]
    fn deny_list_allows_ordinary_assets() {
        for p in [
            "/home/u/project/icon.png",
            "C:\\Users\\u\\project\\theme\\font.woff2",
            "/home/u/.vscode/extensions/pub.ext/logo.svg",
        ] {
            assert!(is_safe_canonical(Path::new(p)), "{p} should be allowed");
        }
    }

    #[test]
    fn unknown_extensions_are_opaque_bytes() {
        assert_eq!(
            content_type(Path::new("/x/page.html")),
            "application/octet-stream"
        );
        assert_eq!(content_type(Path::new("/x/a.svg")), "image/svg+xml");
    }
}
