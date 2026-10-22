//! scmCode: serves the VS Code webview host page (`webview-pre`) from a
//! loopback HTTP server, so that every webview runs on its own origin,
//! `http://<hash>.localhost:<port>`.
//!
//! Upstream SideX served `webview-pre` from the workbench origin, so a script
//! in any webview (an extension panel, a Markdown preview, an extension README)
//! could reach the workbench and its Tauri IPC. On its own origin:
//!
//! - the same-origin policy keeps it out of the workbench and out of other
//!   webviews (the `<hash>` is VS Code's per-webview `parentOriginHash`);
//! - Tauri's ACL treats the origin as remote and refuses every app command,
//!   even though WebView2 injects `__TAURI_INTERNALS__` into iframes;
//! - a plain `http` origin keeps the webview service worker working (custom
//!   schemes can't register one in WebView2).
//!
//! The server only answers `GET` for the three `webview-pre` files and only for
//! `<hash>.localhost:<port>` hosts. It never serves anything else, in
//! particular nothing from the workspace, so no content can run on a webview
//! origin unless the workbench put it there.

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Runtime, State};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// The files a webview host page needs, with their content types.
const FILES: [(&str, &str); 3] = [
    ("index.html", "text/html; charset=utf-8"),
    ("fake.html", "text/html; charset=utf-8"),
    ("service-worker.js", "text/javascript; charset=utf-8"),
];
/// VS Code's `parentOriginHash`: sha-256 in base 32, padded to 52 characters.
const HASH_LEN: usize = 52;
const MAX_REQUEST_HEAD: usize = 8 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const PORT_FILE: &str = "webview-host-port";

/// Managed state: the port the webview host listens on.
pub struct WebviewHost {
    port: u16,
}

/// The webview endpoint for the workbench (`webviewEndpoint`); VS Code replaces
/// `{{uuid}}` with each webview's hash.
#[tauri::command]
pub fn webview_host_endpoint(host: State<'_, WebviewHost>) -> String {
    format!("http://{{{{uuid}}}}.localhost:{}/", host.port)
}

/// Starts the server. Reuses the last port when it is free, so a webview keeps
/// its origin (and the state stored under it) across restarts.
pub fn start<R: Runtime>(app: &AppHandle<R>) -> Result<WebviewHost, String> {
    let files = Arc::new(load_files(app));
    let port_file = crate::app_dirs::app_data_dir().join(PORT_FILE);
    let previous = std::fs::read_to_string(&port_file)
        .ok()
        .and_then(|s| s.trim().parse::<u16>().ok())
        .filter(|port| *port != 0);
    let listener = previous
        .and_then(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .map_or_else(|| std::net::TcpListener::bind(("127.0.0.1", 0)), Ok)
        .map_err(|e| format!("webview host: bind failed: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("webview host: {e}"))?
        .port();
    if previous != Some(port) {
        let _ = std::fs::write(&port_file, port.to_string());
    }
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("webview host: {e}"))?;
    tauri::async_runtime::spawn(async move {
        match tokio::net::TcpListener::from_std(listener) {
            Ok(listener) => serve(listener, port, files).await,
            Err(e) => log::error!("webview host: {e}"),
        }
    });
    log::info!("webview host listening on 127.0.0.1:{port}");
    Ok(WebviewHost { port })
}

type Files = Vec<(&'static str, &'static str, Vec<u8>)>;

fn load_files<R: Runtime>(app: &AppHandle<R>) -> Files {
    FILES
        .iter()
        .filter_map(|(name, mime)| match read_file(app, name) {
            Some(bytes) => Some((*name, *mime, bytes)),
            None => {
                log::error!("webview host: webview-pre/{name} not found");
                None
            }
        })
        .collect()
}

fn read_file<R: Runtime>(app: &AppHandle<R>, name: &str) -> Option<Vec<u8>> {
    // `tauri dev` doesn't embed the frontend; read the source copy.
    #[cfg(debug_assertions)]
    if tauri::is_dev() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../public/webview-pre")
            .join(name);
        return std::fs::read(path).ok();
    }
    app.asset_resolver()
        .get(format!("webview-pre/{name}"))
        .map(|asset| asset.bytes)
}

async fn serve(listener: tokio::net::TcpListener, port: u16, files: Arc<Files>) {
    loop {
        let Ok((mut stream, _)) = listener.accept().await else {
            continue;
        };
        let files = Arc::clone(&files);
        tauri::async_runtime::spawn(async move {
            let response = match tokio::time::timeout(READ_TIMEOUT, read_head(&mut stream)).await {
                Ok(Some(head)) => respond(&head, port, &files),
                _ => status_only(400, "Bad Request"),
            };
            let _ = stream.write_all(&response).await;
            let _ = stream.shutdown().await;
        });
    }
}

/// Reads up to the end of the request head; `None` if it is too large or the
/// connection ends first.
async fn read_head(stream: &mut tokio::net::TcpStream) -> Option<Vec<u8>> {
    let mut head = Vec::new();
    let mut chunk = [0u8; 1024];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        if head.len() > MAX_REQUEST_HEAD {
            return None;
        }
        let n = stream.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        head.extend_from_slice(&chunk[..n]);
    }
    Some(head)
}

/// Builds the full HTTP response for a request head.
fn respond(head: &[u8], port: u16, files: &Files) -> Vec<u8> {
    let Ok(head) = std::str::from_utf8(head) else {
        return status_only(400, "Bad Request");
    };
    let mut lines = head.split("\r\n");
    let mut request_line = lines.next().unwrap_or_default().split(' ');
    let (method, target) = (request_line.next(), request_line.next());
    let host = lines.find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case("host")
            .then(|| value.trim().to_ascii_lowercase())
    });

    if method != Some("GET") {
        return status_only(405, "Method Not Allowed");
    }
    if !host.is_some_and(|host| is_webview_host(&host, port)) {
        return status_only(403, "Forbidden");
    }
    let path = target.unwrap_or_default();
    let name = path
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim_start_matches('/');
    let Some((_, mime, body)) = files.iter().find(|(file, _, _)| *file == name) else {
        return status_only(404, "Not Found");
    };
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\n\
         X-Content-Type-Options: nosniff\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}

/// `<52 base-32 chars>.localhost:<port>`, the only host a webview uses.
fn is_webview_host(host: &str, port: u16) -> bool {
    let Some(label) = host
        .strip_suffix(&format!(".localhost:{port}"))
        .filter(|label| label.len() == HASH_LEN)
    else {
        return false;
    };
    label
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'v').contains(&b))
}

fn status_only(code: u16, reason: &str) -> Vec<u8> {
    format!("HTTP/1.1 {code} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
        .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PORT: u16 = 45123;
    const HASH: &str = "0123456789abcdefghijklmnopqrstuv0123456789abcdefghij";

    fn files() -> Files {
        vec![
            (
                "index.html",
                "text/html; charset=utf-8",
                b"<html>host</html>".to_vec(),
            ),
            (
                "service-worker.js",
                "text/javascript; charset=utf-8",
                b"// sw".to_vec(),
            ),
        ]
    }

    fn request(method: &str, target: &str, host: Option<&str>) -> Vec<u8> {
        let host = host.map(|h| format!("Host: {h}\r\n")).unwrap_or_default();
        format!("{method} {target} HTTP/1.1\r\n{host}User-Agent: test\r\n\r\n").into_bytes()
    }

    fn status(response: &[u8]) -> String {
        String::from_utf8_lossy(response)
            .lines()
            .next()
            .unwrap_or_default()
            .to_string()
    }

    #[test]
    fn serves_the_webview_files_to_a_webview_host() {
        let host = format!("{HASH}.localhost:{PORT}");
        let response = respond(
            &request("GET", "/index.html?id=1", Some(&host)),
            PORT,
            &files(),
        );
        assert_eq!(status(&response), "HTTP/1.1 200 OK");
        let text = String::from_utf8_lossy(&response);
        assert!(text.contains("Content-Type: text/html; charset=utf-8"));
        assert!(text.contains("X-Content-Type-Options: nosniff"));
        assert!(text.ends_with("<html>host</html>"));

        let response = respond(
            &request(
                "GET",
                "/service-worker.js?v=4&x=y",
                Some(&host.to_uppercase()),
            ),
            PORT,
            &files(),
        );
        assert_eq!(status(&response), "HTTP/1.1 200 OK");
    }

    #[test]
    fn serves_nothing_else() {
        let host = format!("{HASH}.localhost:{PORT}");
        for target in [
            "/",
            "/secret.txt",
            "/../Cargo.toml",
            "/fake.html",
            "//index.html/x",
        ] {
            let response = respond(&request("GET", target, Some(&host)), PORT, &files());
            assert_eq!(status(&response), "HTTP/1.1 404 Not Found", "{target}");
        }
    }

    #[test]
    fn rejects_other_hosts_and_methods() {
        for host in [
            None,
            Some(format!("localhost:{PORT}")),
            Some(format!("127.0.0.1:{PORT}")),
            Some(format!("evil.localhost:{PORT}")),
            Some(format!("{HASH}.localhost:{}", PORT + 1)),
            Some(format!("{HASH}.localhost")),
            Some(format!("{HASH}.evil.example:{PORT}")),
            Some(format!("{}w.localhost:{PORT}", &HASH[..HASH_LEN - 1])),
            Some(format!("x{HASH}.localhost:{PORT}")),
        ] {
            let response = respond(
                &request("GET", "/index.html", host.as_deref()),
                PORT,
                &files(),
            );
            assert_eq!(status(&response), "HTTP/1.1 403 Forbidden", "{host:?}");
        }
        let host = format!("{HASH}.localhost:{PORT}");
        for method in ["POST", "PUT", "HEAD", "OPTIONS"] {
            let response = respond(&request(method, "/index.html", Some(&host)), PORT, &files());
            assert_eq!(
                status(&response),
                "HTTP/1.1 405 Method Not Allowed",
                "{method}"
            );
        }
        assert_eq!(
            status(&respond(&[0xff, 0xfe], PORT, &files())),
            "HTTP/1.1 400 Bad Request"
        );
    }

    async fn get(port: u16, host: &str, target: &str) -> String {
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        stream
            .write_all(&request("GET", target, Some(host)))
            .await
            .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        status(&response)
    }

    #[tokio::test]
    async fn answers_over_tcp() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(serve(listener, port, Arc::new(files())));

        let webview = format!("{HASH}.localhost:{port}");
        assert_eq!(get(port, &webview, "/index.html").await, "HTTP/1.1 200 OK");
        let evil = format!("evil.localhost:{port}");
        assert_eq!(
            get(port, &evil, "/index.html").await,
            "HTTP/1.1 403 Forbidden"
        );
    }
}
