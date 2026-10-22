use std::fs;
use std::path::Path;

fn main() {
    let scripts_src = Path::new("../src/vs/workbench/contrib/terminal/common/scripts");
    let scripts_dst = Path::new("shell-integration");

    if scripts_src.exists() {
        fs::create_dir_all(scripts_dst).expect("Failed to create shell-integration dir");

        let shell_files = [
            "shellIntegration-rc.zsh",
            "shellIntegration-env.zsh",
            "shellIntegration-profile.zsh",
            "shellIntegration-login.zsh",
            "shellIntegration-bash.sh",
            "shellIntegration.fish",
            "shellIntegration.ps1",
        ];

        for file in &shell_files {
            let src = scripts_src.join(file);
            let dst = scripts_dst.join(file);
            if src.exists() {
                let src_bytes =
                    fs::read(&src).unwrap_or_else(|e| panic!("Failed to read {file}: {e}"));
                let needs_copy = fs::read(&dst).map_or(true, |dst_bytes| dst_bytes != src_bytes);
                if needs_copy {
                    fs::write(&dst, &src_bytes)
                        .unwrap_or_else(|e| panic!("Failed to copy {file}: {e}"));
                }
                println!("cargo:rerun-if-changed={}", src.display());
            }
        }
    }

    let ext_host_src = Path::new("extension-host");
    let ext_host_targets = [
        Path::new("target/debug/extension-host"),
        Path::new("target/release/extension-host"),
    ];
    if ext_host_src.exists() {
        for ext_host_dst in &ext_host_targets {
            fs::create_dir_all(ext_host_dst).ok();
            for file in &["server.cjs", "host.cjs"] {
                let src = ext_host_src.join(file);
                let dst = ext_host_dst.join(file);
                if src.exists() {
                    let src_bytes = fs::read(&src).unwrap_or_default();
                    let needs_copy =
                        fs::read(&dst).map_or(true, |dst_bytes| dst_bytes != src_bytes);
                    if needs_copy {
                        let _ = fs::write(&dst, &src_bytes);
                    }
                }
            }
        }
        for file in &["server.cjs", "host.cjs"] {
            let src = ext_host_src.join(file);
            if src.exists() {
                println!("cargo:rerun-if-changed={}", src.display());
            }
        }
    }

    // scmCode: app commands are access-controlled (see app_command_permissions).
    let permissions = app_command_permissions();
    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().permissions_path_pattern(permissions));
    if let Err(error) = tauri_build::try_build(attributes) {
        panic!("tauri build failed: {error:#}");
    }

    // scmCode: tauri-build embeds the Windows app manifest only in the app
    // binary. Without its Common Controls v6 dependency, a test binary that
    // links Tauri fails to start (STATUS_ENTRYPOINT_NOT_FOUND), so give the
    // integration tests (tests/acl.rs) the same dependency.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-tests=/MANIFESTDEPENDENCY:type='win32' \
             name='Microsoft.Windows.Common-Controls' version='6.0.0.0' \
             processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
}

/// scmCode: generates the `allow-app-commands` permission, listing every
/// command registered in `generate_handler!` in `src/lib.rs`, and returns the
/// glob tauri-build reads it from.
///
/// Declaring app permissions turns on Tauri's access control for app commands.
/// Without it Tauri checks only plugin commands, so any page loaded in a
/// webview, from any origin, could call every command. The permission is
/// granted in `capabilities/default.json` to the `main` window on the app's
/// own origin only. Generating it keeps `generate_handler!` the single list
/// of commands, so an upstream merge that adds one doesn't need a manual edit.
fn app_command_permissions() -> &'static str {
    println!("cargo:rerun-if-changed=src/lib.rs");
    let lib = fs::read_to_string("src/lib.rs").expect("failed to read src/lib.rs");
    let start = lib
        .find("generate_handler![")
        .expect("generate_handler! not found in src/lib.rs")
        + "generate_handler![".len();
    let end = start
        + lib[start..]
            .find(']')
            .expect("unterminated generate_handler! in src/lib.rs");

    let mut commands: Vec<String> = Vec::new();
    for line in lib[start..end].lines() {
        let code = line.split("//").next().unwrap_or_default();
        for item in code.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let name = item.rsplit("::").next().unwrap_or(item);
            assert!(
                !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
                "unexpected entry {item:?} in generate_handler! (build.rs parses it; update the parser)"
            );
            if !commands.iter().any(|c| c == name) {
                commands.push(name.to_string());
            }
        }
    }
    assert!(
        !commands.is_empty(),
        "no commands found in generate_handler!"
    );

    let list = commands
        .iter()
        .map(|c| format!("  \"{c}\","))
        .collect::<Vec<_>>()
        .join("\n");
    let toml = format!(
        "# Generated by build.rs from generate_handler! in src/lib.rs. Do not edit.\n\n\
         [[permission]]\n\
         identifier = \"allow-app-commands\"\n\
         description = \"Allows every command the app registers.\"\n\
         commands.allow = [\n{list}\n]\n"
    );

    let out_dir =
        Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR not set")).join("app-permissions");
    fs::create_dir_all(&out_dir).expect("failed to create app permissions dir");
    let file = out_dir.join("app-commands.toml");
    if fs::read_to_string(&file).ok().as_deref() != Some(toml.as_str()) {
        fs::write(&file, toml).expect("failed to write app permissions");
    }

    // tauri-build takes a glob; forward slashes work on every platform.
    let pattern = format!("{}/*", out_dir.display()).replace('\\', "/");
    Box::leak(pattern.into_boxed_str())
}
