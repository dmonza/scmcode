//! scmCode: checks the IPC access control for app commands (SEC-05) with the
//! app's real Tauri context (tauri.conf.json, capabilities and the generated
//! `allow-app-commands` permission) on Tauri's mock runtime, so CI runs it on
//! every platform. Only the `main` window on the app's own origin may call
//! app commands.

use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// How the app's own origin looks to Tauri on this platform.
const APP_ORIGIN: &str = if cfg!(windows) {
    "http://tauri.localhost/"
} else {
    "tauri://localhost/"
};

/// Stand-in with the name of a real app command; the ACL decides by name.
#[tauri::command]
fn env_app_host() -> &'static str {
    "ok"
}

fn app() -> tauri::App<MockRuntime> {
    mock_builder()
        .invoke_handler(tauri::generate_handler![env_app_host])
        .build(tauri::generate_context!())
        .expect("failed to build the mock app")
}

fn window(app: &tauri::App<MockRuntime>, label: &str) -> WebviewWindow<MockRuntime> {
    app.get_webview_window(label).unwrap_or_else(|| {
        WebviewWindowBuilder::new(app, label, WebviewUrl::default())
            .build()
            .expect("failed to create the window")
    })
}

fn invoke_from(window: &WebviewWindow<MockRuntime>, origin: &str) -> bool {
    get_ipc_response(
        window,
        InvokeRequest {
            cmd: "env_app_host".into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: origin.parse().unwrap(),
            body: InvokeBody::default(),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .is_ok()
}

#[test]
fn app_commands_are_access_controlled() {
    let app = app();
    let main = window(&app, "main");
    let other = window(&app, "other");

    assert!(
        invoke_from(&main, APP_ORIGIN),
        "the workbench (main window, app origin) must be able to call app commands"
    );
    assert!(
        !invoke_from(&main, "https://evil.example/"),
        "a remote page in the main window must not call app commands"
    );
    // A webview iframe inside the main window, on its sandbox origin
    // (webview_host.rs). WebView2 gives it `__TAURI_INTERNALS__`; the ACL must
    // still refuse it.
    assert!(
        !invoke_from(
            &main,
            "http://0123456789abcdefghijklmnopqrstuv0123456789abcdefghij.localhost:45123/"
        ),
        "a webview's sandbox origin must not call app commands"
    );
    assert!(
        !invoke_from(&other, APP_ORIGIN),
        "another window must not call app commands"
    );
    assert!(
        !invoke_from(&other, "https://evil.example/"),
        "a remote page in another window must not call app commands"
    );
}
