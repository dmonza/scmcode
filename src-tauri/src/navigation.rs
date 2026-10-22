//! scmCode: keeps the main window on the app's own origin.
//!
//! Every page that runs as the main frame of the `main` webview gets the Tauri
//! IPC and, through the capability in `capabilities/default.json`, every app
//! command when its origin is local. Tauri counts any registered custom scheme
//! (`sidex-asset`) as local, so the only safe rule is an exact one: the main
//! window may only load the app itself (see `app_origin`). A link or a script
//! that tries to go anywhere else is cancelled; web and mail links open in the
//! system browser instead.

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};

use crate::app_origin;

const MAIN_WEBVIEW: &str = "main";

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("navigation-guard")
        .on_navigation(|webview, url| {
            if webview.label() != MAIN_WEBVIEW
                || app_origin::is_app_url(url, app_origin::dev_url(webview.app_handle()).as_ref())
            {
                return true;
            }
            log::warn!(
                "blocked navigation of the main window to {}",
                url.origin().ascii_serialization()
            );
            if matches!(url.scheme(), "http" | "https" | "mailto") {
                let target = url.to_string();
                std::thread::spawn(move || {
                    if let Err(e) = open::that(&target) {
                        log::warn!("failed to open link in the system browser: {e}");
                    }
                });
            }
            false
        })
        .build()
}
