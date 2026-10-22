//! scmCode: what counts as the app's own origin.
//!
//! Used by the navigation guard (the main window may only load the app) and by
//! the `sidex-asset` protocol (only the app may read its responses). Webviews
//! live on other origins (`webview_host`) and must never match.

use tauri::http::{header, response::Builder, HeaderMap};
use tauri::{AppHandle, Runtime, Url};

/// `devUrl` under `tauri dev`, `None` otherwise.
pub fn dev_url<R: Runtime>(app: &AppHandle<R>) -> Option<Url> {
    if tauri::is_dev() {
        app.config().build.dev_url.clone()
    } else {
        None
    }
}

/// Whether `url` is the app itself. `dev_url` is `devUrl`, passed only under
/// `tauri dev`.
pub fn is_app_url(url: &Url, dev_url: Option<&Url>) -> bool {
    let host = url.host_str();
    match url.scheme() {
        // macOS and Linux serve the app from tauri://localhost.
        "tauri" => host == Some("localhost"),
        // Windows serves it from http(s)://tauri.localhost; `tauri dev` from devUrl.
        "http" | "https" => {
            host == Some("tauri.localhost")
                || dev_url.is_some_and(|dev| dev.origin() == url.origin())
        }
        _ => false,
    }
}

/// Adds CORS headers that let only the app read a response: the request's
/// `Origin` is echoed when it is the app, and omitted for every other origin
/// (webviews, remote pages, and the opaque `null` origin).
pub fn app_only_cors(builder: Builder, request: &HeaderMap, dev_url: Option<&Url>) -> Builder {
    let builder = builder.header(header::VARY, "Origin");
    let app_origin = request
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .filter(|origin| {
            origin
                .parse::<Url>()
                .is_ok_and(|url| is_app_url(&url, dev_url))
        });
    match app_origin {
        Some(origin) => builder.header(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin),
        None => builder,
    }
}

#[cfg(test)]
mod tests {
    use super::{app_only_cors, is_app_url};
    use tauri::http::{header, HeaderMap, HeaderValue, Response};
    use tauri::Url;

    fn url(s: &str) -> Url {
        s.parse().unwrap()
    }

    #[test]
    fn allows_the_app_origin_on_every_platform() {
        for app in [
            "tauri://localhost/",
            "tauri://localhost/index.html?x=1",
            "http://tauri.localhost/",
            "https://tauri.localhost/workbench",
        ] {
            assert!(is_app_url(&url(app), None), "{app} should be allowed");
        }
    }

    #[test]
    fn allows_dev_url_only_when_given() {
        let dev = url("http://localhost:1420");
        assert!(is_app_url(&url("http://localhost:1420/x"), Some(&dev)));
        assert!(!is_app_url(&url("http://localhost:1420/x"), None));
        assert!(!is_app_url(&url("http://localhost:1421/"), Some(&dev)));
    }

    #[test]
    fn rejects_everything_else() {
        let dev = url("http://localhost:1420");
        for other in [
            "https://example.com/",
            "http://tauri.localhost.evil.example/",
            "https://evil.example/?u=tauri.localhost",
            "tauri://evil/",
            "file:///C:/Windows/win.ini",
            "ftp://example.com/",
            "data:text/html,hi",
            "about:blank",
            // Custom schemes count as local for Tauri's IPC, so the main window
            // must never load them (Windows and other platforms' forms).
            "https://sidex-asset.localhost/x",
            "sidex-asset://localhost/x",
            // A webview's sandbox origin (webview_host).
            "http://0123456789abcdefghijklmnopqrstuv0123456789abcdefghij.localhost:45000/",
        ] {
            assert!(
                !is_app_url(&url(other), Some(&dev)),
                "{other} should be rejected"
            );
        }
    }

    fn cors_for(origin: Option<&str>) -> Option<String> {
        let mut headers = HeaderMap::new();
        if let Some(origin) = origin {
            headers.insert(header::ORIGIN, HeaderValue::from_str(origin).unwrap());
        }
        let response = app_only_cors(Response::builder(), &headers, None)
            .body(())
            .unwrap();
        assert_eq!(response.headers()[header::VARY], "Origin");
        response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .map(|v| v.to_str().unwrap().to_string())
    }

    #[test]
    fn cors_is_granted_to_the_app_only() {
        assert_eq!(
            cors_for(Some("https://tauri.localhost")).as_deref(),
            Some("https://tauri.localhost")
        );
        assert_eq!(
            cors_for(Some("tauri://localhost")).as_deref(),
            Some("tauri://localhost")
        );
        assert_eq!(cors_for(None), None);
        assert_eq!(cors_for(Some("null")), None);
        assert_eq!(cors_for(Some("https://evil.example")), None);
        assert_eq!(
            cors_for(Some(
                "http://0123456789abcdefghijklmnopqrstuv0123456789abcdefghij.localhost:45000"
            )),
            None
        );
    }
}
