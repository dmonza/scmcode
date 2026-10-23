#[cfg(test)]
mod tests {
    use crate::{execute, ToolContext, ToolRequest};
    use std::fs;

    fn ctx(dir: &std::path::Path) -> ToolContext {
        ToolContext {
            cwd: dir.display().to_string(),
            token: String::new(),
        }
    }

    fn req(name: &str, args: &str) -> ToolRequest {
        ToolRequest {
            tool_call_id: "t1".into(),
            name: name.into(),
            arguments: args.into(),
        }
    }

    #[test]
    fn test_cwd() {
        let dir = tempfile::tempdir().unwrap();
        let resp = execute(&req("cwd", "{}"), &ctx(dir.path()));
        assert!(resp.error.is_empty(), "error: {}", resp.error);
        assert!(resp.output.contains("cwd:"), "output: {}", resp.output);
        assert!(resp.output.contains("exists: true"));
    }

    #[test]
    fn test_read_write_file() {
        let dir = tempfile::tempdir().unwrap();
        let c = ctx(dir.path());

        let resp = execute(
            &req(
                "write_file",
                r#"{"path":"hello.txt","content":"hello world"}"#,
            ),
            &c,
        );
        assert!(resp.error.is_empty(), "{}", resp.error);

        let resp = execute(&req("read_file", r#"{"path":"hello.txt"}"#), &c);
        assert!(resp.error.is_empty(), "{}", resp.error);
        assert!(resp.output.contains("hello world"));
    }

    #[test]
    fn test_edit_file() {
        let dir = tempfile::tempdir().unwrap();
        let c = ctx(dir.path());
        let path = dir.path().join("test.txt");
        fs::write(&path, "hello world").unwrap();

        let resp = execute(
            &req(
                "edit_file",
                r#"{"path":"test.txt","old_string":"hello","new_string":"goodbye"}"#,
            ),
            &c,
        );
        assert!(resp.error.is_empty(), "{}", resp.error);

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "goodbye world");
    }

    #[test]
    fn test_edit_file_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let c = ctx(dir.path());
        fs::write(dir.path().join("f.txt"), "abc").unwrap();

        let resp = execute(
            &req(
                "edit_file",
                r#"{"path":"f.txt","old_string":"xyz","new_string":"q"}"#,
            ),
            &c,
        );
        assert!(!resp.error.is_empty(), "should fail");
        assert!(resp.error.contains("not found"), "error: {}", resp.error);
    }

    #[test]
    fn test_list_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "A").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();

        let resp = execute(&req("list_dir", r#"{"path":"."}"#), &ctx(dir.path()));
        assert!(resp.error.is_empty(), "{}", resp.error);
        assert!(resp.output.contains("a.txt"));
        assert!(resp.output.contains("sub"));
    }

    #[test]
    fn test_tree() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("a/b")).unwrap();
        fs::write(dir.path().join("a/b/c.txt"), "x").unwrap();

        let resp = execute(&req("tree", r#"{"depth":3}"#), &ctx(dir.path()));
        assert!(resp.error.is_empty(), "{}", resp.error);
        assert!(resp.output.contains("c.txt"));
    }

    #[test]
    fn test_shell() {
        let dir = tempfile::tempdir().unwrap();
        let resp = execute(
            &req("shell", r#"{"command":"echo hello-sidex"}"#),
            &ctx(dir.path()),
        );
        assert!(resp.error.is_empty(), "{}", resp.error);
        assert!(resp.output.contains("hello-sidex"));
    }

    #[test]
    fn test_shell_bad_dir() {
        let resp = execute(
            &req(
                "shell",
                r#"{"command":"echo hi","working_directory":"/nope/fake"}"#,
            ),
            &ToolContext {
                cwd: "/tmp".into(),
                token: String::new(),
            },
        );
        assert!(!resp.error.is_empty());
        assert!(resp.error.contains("does not exist"), "{}", resp.error);
    }

    #[test]
    fn test_file_info() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "hello").unwrap();
        let resp = execute(&req("file_info", r#"{"path":"f.txt"}"#), &ctx(dir.path()));
        assert!(resp.error.is_empty(), "{}", resp.error);
        assert!(resp.output.contains("size: 5"));
    }

    #[test]
    fn test_multi_edit() {
        let dir = tempfile::tempdir().unwrap();
        let c = ctx(dir.path());
        let path = dir.path().join("m.txt");
        fs::write(&path, "alpha beta gamma").unwrap();

        let resp = execute(
            &req(
                "multi_edit",
                r#"{"path":"m.txt","edits":[{"old":"alpha","new":"one"},{"old":"beta","new":"two"}]}"#,
            ),
            &c,
        );
        assert!(resp.error.is_empty(), "{}", resp.error);
        assert_eq!(fs::read_to_string(&path).unwrap(), "one two gamma");
    }

    #[test]
    fn test_glob() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rs"), "").unwrap();
        fs::write(dir.path().join("b.txt"), "").unwrap();

        let resp = execute(&req("glob", r#"{"pattern":"*.rs"}"#), &ctx(dir.path()));
        assert!(resp.error.is_empty(), "{}", resp.error);
        assert!(resp.output.contains("a.rs"));
        assert!(!resp.output.contains("b.txt"));
    }

    #[test]
    fn test_batch_read() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("x.txt"), "hello").unwrap();
        fs::write(dir.path().join("y.txt"), "world").unwrap();

        let resp = execute(
            &req("batch_read", r#"{"paths":["x.txt","y.txt"]}"#),
            &ctx(dir.path()),
        );
        assert!(resp.error.is_empty(), "{}", resp.error);
        assert!(resp.output.contains("hello"));
        assert!(resp.output.contains("world"));
    }

    #[test]
    fn test_empty_args() {
        let dir = tempfile::tempdir().unwrap();
        let resp = execute(&req("cwd", ""), &ctx(dir.path()));
        assert!(
            resp.error.is_empty(),
            "empty args should be OK: {}",
            resp.error
        );
    }

    #[test]
    fn test_unknown_tool() {
        let resp = execute(
            &req("not_real", "{}"),
            &ToolContext {
                cwd: "/tmp".into(),
                token: String::new(),
            },
        );
        assert!(!resp.error.is_empty());
        assert!(resp.error.contains("not implemented"));
    }

    #[test]
    fn test_read_file_with_offset_limit() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("lines.txt"), "a\nb\nc\nd\ne").unwrap();

        let resp = execute(
            &req("read_file", r#"{"path":"lines.txt","offset":2,"limit":2}"#),
            &ctx(dir.path()),
        );
        assert!(resp.error.is_empty(), "{}", resp.error);
        assert!(resp.output.contains("|b"));
        assert!(resp.output.contains("|c"));
        assert!(!resp.output.contains("|a"));
        assert!(!resp.output.contains("|d"));
    }

    #[test]
    fn test_image_read() {
        let dir = tempfile::tempdir().unwrap();
        #[rustfmt::skip]
        let png_data: Vec<u8> = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02,
            0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00,
            0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF,
            0xC0, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0xE2, 0x21, 0xBC,
            0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
            0x42, 0x60, 0x82,
        ];
        fs::write(dir.path().join("test.png"), &png_data).unwrap();

        let resp = execute(
            &req("image_read", r#"{"path":"test.png"}"#),
            &ctx(dir.path()),
        );
        assert!(resp.error.is_empty(), "{}", resp.error);
        assert!(resp.output.contains("image/png"));
        assert!(resp.output.contains("base64_data"));
    }

    // scmCode: no panic on agent input, shell tree supervision and fresh reads
    // (docs/audit TR-07, TR-09, TR-15).

    fn args_json(pairs: &[(&str, serde_json::Value)]) -> String {
        let map: serde_json::Map<String, serde_json::Value> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect();
        serde_json::Value::Object(map).to_string()
    }

    #[test]
    fn batch_read_truncates_multibyte_content_at_every_cut_point() {
        let dir = tempfile::tempdir().unwrap();
        let c = ctx(dir.path());
        // Shift a run of 4-byte characters by 0..4 ASCII bytes so the 10 000-byte
        // cut lands on every position inside a character.
        for pad in 0..4 {
            let content = format!("{}{}", "a".repeat(pad), "\u{1F600}".repeat(3000));
            fs::write(dir.path().join("emoji.txt"), &content).unwrap();
            let resp = execute(&req("batch_read", r#"{"paths":["emoji.txt"]}"#), &c);
            assert!(resp.error.is_empty(), "pad {pad}: {}", resp.error);
            assert!(resp.output.contains("...(truncated)"), "pad {pad}");
        }
    }

    #[test]
    fn edit_hint_truncates_multibyte_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let c = ctx(dir.path());
        for pad in 0..3 {
            // The first line matches and is longer than the 160-byte hint, so the
            // hint cuts it; the second line doesn't match.
            let first = format!("{}{}", "a".repeat(pad), "ñ".repeat(120));
            fs::write(dir.path().join("t.txt"), format!("{first}\nrest\n")).unwrap();
            let old = format!("{first}\nmissing");
            let args = args_json(&[
                ("path", "t.txt".into()),
                ("old_string", old.into()),
                ("new_string", "x".into()),
            ]);
            let resp = execute(&req("edit_file", &args), &c);
            assert!(
                resp.error.contains("DO appear"),
                "pad {pad}: {}",
                resp.error
            );
        }
    }

    #[test]
    fn read_file_treats_negative_offset_and_limit_as_unset() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("lines.txt"), "a\nb\nc").unwrap();
        let c = ctx(dir.path());
        for args in [
            r#"{"path":"lines.txt","offset":-5,"limit":-1}"#,
            r#"{"path":"lines.txt","offset":-9223372036854775808}"#,
            r#"{"path":"lines.txt","limit":9223372036854775807}"#,
            r#"{"path":"lines.txt","offset":9223372036854775807}"#,
        ] {
            let resp = execute(&req("read_file", args), &c);
            assert!(resp.error.is_empty(), "{args}: {}", resp.error);
        }
        let resp = execute(
            &req(
                "read_file",
                r#"{"path":"lines.txt","offset":-5,"limit":-1}"#,
            ),
            &c,
        );
        assert!(resp.output.contains("|a") && resp.output.contains("|c"));
    }

    #[test]
    fn read_file_refuses_files_over_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        let file = fs::File::create(dir.path().join("big.txt")).unwrap();
        file.set_len(crate::tools::file_ops::MAX_READ_BYTES + 1)
            .unwrap();
        let resp = execute(&req("read_file", r#"{"path":"big.txt"}"#), &ctx(dir.path()));
        assert!(resp.error.contains("limit"), "{}", resp.error);
    }

    /// Deterministic fuzzing: `execute` must return an error, never panic, for any
    /// combination of argument types and values. Only tools confined to the temp
    /// dir (no network, no processes) are exercised.
    #[test]
    fn execute_never_panics_on_arbitrary_arguments() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "ñandú\n\u{1F600}\nline").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        let c = ctx(dir.path());

        let values: Vec<serde_json::Value> = vec![
            serde_json::Value::Null,
            true.into(),
            (-1).into(),
            0.into(),
            i64::MIN.into(),
            i64::MAX.into(),
            u64::MAX.into(),
            1.5.into(),
            "".into(),
            "a.txt".into(),
            "sub".into(),
            "missing.txt".into(),
            "ñ".into(),
            "\u{1F600}".repeat(100).into(),
            serde_json::json!([]),
            serde_json::json!(["a.txt", 3, null]),
            serde_json::json!([{"old_text": "ñ", "new_text": "\u{1F600}"}]),
            serde_json::json!({"k": "v"}),
        ];
        let keys = [
            "path",
            "paths",
            "offset",
            "limit",
            "content",
            "old_string",
            "new_string",
            "replace_all",
            "edits",
            "pattern",
            "depth",
            "max_size",
            "query",
            "budget",
            "name",
        ];
        let tools = [
            "read_file",
            "write_file",
            "edit_file",
            "multi_edit",
            "list_dir",
            "tree",
            "glob",
            "grep",
            "file_info",
            "batch_read",
            "image_read",
            "cwd",
            "skill",
        ];

        // A small linear congruential generator keeps the run reproducible.
        let mut seed: u64 = 0x5eed;
        let mut next = |n: usize| {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (seed >> 33) as usize % n
        };

        for _ in 0..3000 {
            let tool = tools[next(tools.len())];
            let mut map = serde_json::Map::new();
            for _ in 0..next(5) {
                map.insert(
                    keys[next(keys.len())].to_string(),
                    values[next(values.len())].clone(),
                );
            }
            let args = serde_json::Value::Object(map).to_string();
            let outcome = std::panic::catch_unwind(|| execute(&req(tool, &args), &c));
            assert!(outcome.is_ok(), "{tool} panicked with {args}");
        }
    }

    #[test]
    fn read_after_edit_returns_new_content_with_index_built() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("f.rs"), "fn old() {}\n").unwrap();
        let c = ctx(dir.path());

        // Building the index is what used to enable the predictive read cache.
        let resp = execute(&req("context_index", "{}"), &c);
        assert!(resp.error.is_empty(), "{}", resp.error);
        let resp = execute(&req("read_file", r#"{"path":"f.rs"}"#), &c);
        assert!(resp.output.contains("old"), "{}", resp.output);

        let resp = execute(
            &req(
                "edit_file",
                r#"{"path":"f.rs","old_string":"old","new_string":"renamed"}"#,
            ),
            &c,
        );
        assert!(resp.error.is_empty(), "{}", resp.error);
        let resp = execute(&req("read_file", r#"{"path":"f.rs"}"#), &c);
        assert!(resp.output.contains("renamed"), "{}", resp.output);

        // A change made outside the agent is seen too.
        fs::write(dir.path().join("f.rs"), "fn external() {}\n").unwrap();
        let resp = execute(&req("batch_read", r#"{"paths":["f.rs"]}"#), &c);
        assert!(resp.output.contains("external"), "{}", resp.output);
    }

    #[test]
    fn shell_returns_large_output_without_deadlock() {
        let dir = tempfile::tempdir().unwrap();
        // 200 KB on stdout: more than a pipe buffer, so waiting before reading
        // would deadlock until the timeout.
        let resp = execute(
            &req(
                "shell",
                r#"{"command":"i=0; while [ $i -lt 2000 ]; do printf '%0100d\n' 0; i=$((i+1)); done","timeout":60}"#,
            ),
            &ctx(dir.path()),
        );
        assert!(resp.error.is_empty(), "{}", resp.error);
        assert!(resp.output.contains("...(truncated)"));
        assert!(resp.output.len() <= 100_000 + 64);
    }

    #[test]
    fn shell_timeout_kills_the_whole_tree() {
        let dir = tempfile::tempdir().unwrap();
        let start = std::time::Instant::now();
        // A background child that would write a marker after 3 s, and a
        // foreground command that never ends on its own.
        let resp = execute(
            &req(
                "shell",
                r#"{"command":"(sleep 3; echo late > marker.txt) & echo started; sleep 60","timeout":1}"#,
            ),
            &ctx(dir.path()),
        );
        assert!(resp.error.contains("timed out"), "{}", resp.error);
        assert!(resp.error.contains("started"), "{}", resp.error);
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
        std::thread::sleep(std::time::Duration::from_secs(4));
        assert!(
            !dir.path().join("marker.txt").exists(),
            "child outlived the timeout"
        );
    }

    #[test]
    fn shell_kills_background_jobs_when_the_command_exits() {
        let dir = tempfile::tempdir().unwrap();
        let resp = execute(
            &req(
                "shell",
                r#"{"command":"(sleep 3; echo late > marker.txt) & echo done"}"#,
            ),
            &ctx(dir.path()),
        );
        assert!(resp.error.is_empty(), "{}", resp.error);
        assert!(resp.output.contains("done"));
        std::thread::sleep(std::time::Duration::from_secs(4));
        assert!(
            !dir.path().join("marker.txt").exists(),
            "background job outlived the call"
        );
    }

    #[test]
    fn shell_reports_exit_code() {
        let dir = tempfile::tempdir().unwrap();
        let resp = execute(
            &req("shell", r#"{"command":"echo out; echo err >&2; exit 3"}"#),
            &ctx(dir.path()),
        );
        assert!(resp.error.is_empty(), "{}", resp.error);
        assert!(resp.output.contains("out") && resp.output.contains("err"));
        assert!(resp.output.contains("exit code: 3"), "{}", resp.output);
    }
}
