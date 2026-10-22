//! scmCode: runtime self-test of the webview sandbox (`webview_host.rs`), run by
//! CI on Windows, macOS and Linux, where no browser automation can reach the
//! system webview.
//!
//! Built only with the `selftest` feature and active only with
//! `SCMCODE_SELFTEST=1`. It marks the main page so the workbench contribution
//! `scmcodeSelftest.contribution.ts` opens a real webview and checks its
//! isolation from inside, then prints the report (and writes it to
//! `SCMCODE_SELFTEST_OUT` when set) and exits: 0 passed, 1 failed, 2 timed out.

use std::time::Duration;

use tauri::{AppHandle, Listener, Manager, Runtime};

const REPORT_EVENT: &str = "scmcode-selftest-report";
const START_EVENT: &str = "scmcode-selftest";
const TIMEOUT: Duration = Duration::from_secs(180);
/// A local image for the webview to load through its service worker.
const IMAGE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4"/></svg>"#;

pub fn start<R: Runtime>(app: &AppHandle<R>) {
    if std::env::var("SCMCODE_SELFTEST").as_deref() != Ok("1") {
        return;
    }
    let dir = std::env::temp_dir().join(format!("scmcode-selftest-{}", std::process::id()));
    let image = dir.join("pixel.svg");
    if let Err(e) = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&image, IMAGE)) {
        finish(
            &serde_json::json!({ "ok": false, "error": format!("selftest setup: {e}") })
                .to_string(),
        );
    }

    app.listen_any(REPORT_EVENT, |event| finish(event.payload()));
    std::thread::spawn(|| {
        std::thread::sleep(TIMEOUT);
        finish(r#"{"ok":false,"error":"timeout: no report from the workbench"}"#);
    });

    // Mark the page until the workbench picks it up; it ignores repeats.
    let request = serde_json::json!({ "imagePath": image }).to_string();
    let script = format!(
        "window.__SCMCODE_SELFTEST__ = {request}; \
         window.dispatchEvent(new CustomEvent('{START_EVENT}', {{ detail: window.__SCMCODE_SELFTEST__ }}));"
    );
    let app = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.eval(&script);
        }
    });
}

fn finish(report: &str) -> ! {
    println!("SCMCODE_SELFTEST_REPORT {report}");
    if let Ok(path) = std::env::var("SCMCODE_SELFTEST_OUT") {
        let _ = std::fs::write(path, report);
    }
    let passed = serde_json::from_str::<serde_json::Value>(report)
        .ok()
        .and_then(|value| value["ok"].as_bool())
        .unwrap_or(false);
    let timed_out = report.contains("\"error\":\"timeout");
    std::process::exit(match (passed, timed_out) {
        (true, _) => 0,
        (false, true) => 2,
        (false, false) => 1,
    });
}
