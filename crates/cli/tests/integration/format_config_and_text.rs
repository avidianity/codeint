use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::Value;
use tempfile::TempDir;

use crate::common::eroded_ts;

const GIT_ENV: [(&str, &str); 6] = [
    ("GIT_CONFIG_GLOBAL", "/dev/null"),
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_AUTHOR_NAME", "Format Tester"),
    ("GIT_AUTHOR_EMAIL", "format@example.invalid"),
    ("GIT_COMMITTER_NAME", "Format Tester"),
    ("GIT_COMMITTER_EMAIL", "format@example.invalid"),
];

fn git_in(root: &Path, args: &[&str], date: Option<&str>) {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(root).envs(GIT_ENV);
    if let Some(ts) = date {
        cmd.env("GIT_AUTHOR_DATE", ts).env("GIT_COMMITTER_DATE", ts);
    }
    let status = cmd.status().expect("run git");
    assert!(status.success(), "git {args:?} failed");
}

fn write_files(root: &Path, entries: &[(&str, &str)]) {
    entries.iter().for_each(|&(path, content)| {
        let full = root.join(path);
        fs::create_dir_all(full.parent().expect("dir")).expect("create dir");
        fs::write(full, content).expect("write");
    });
}

fn commit_all(root: &Path, message: &str, date: &str) {
    git_in(root, &["add", "--all"], None);
    git_in(
        root,
        &["commit", "--quiet", "--message", message],
        Some(date),
    );
}

fn invoke(root: &Path, cache: &Path, args: &[&str], stdin_data: Option<&[u8]>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeint"));
    command
        .current_dir(root)
        .env("CODEINT_CACHE_DIR", cache)
        .args(args);
    match stdin_data {
        None => command.output().expect("run codeint"),
        Some(payload) => {
            let mut child = command
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn codeint");
            child
                .stdin
                .take()
                .expect("stdin pipe")
                .write_all(payload)
                .expect("write stdin payload");
            child.wait_with_output().expect("wait for codeint")
        }
    }
}

fn utf8(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("utf8")
}

fn assert_no_ansi(text: &str) {
    assert!(
        !text.contains('\u{1b}'),
        "output must not contain ANSI escape sequences:\n{text}"
    );
}

fn invoke_json(root: &Path, cache: &Path, args: &[&str], stdin: Option<&[u8]>) -> (Vec<u8>, Value) {
    let out = invoke(root, cache, args, stdin);
    assert_eq!(
        out.status.code(),
        Some(0),
        "args={args:?}, stderr={}",
        utf8(&out.stderr)
    );
    let parsed: Value = serde_json::from_slice(&out.stdout).expect("valid json output");
    (out.stdout, parsed)
}

fn build_sample_repo() -> (TempDir, TempDir) {
    let dir = TempDir::new().expect("temp repo");
    let cache = TempDir::new().expect("temp cache");
    git_in(
        dir.path(),
        &["init", "--quiet", "--initial-branch", "main"],
        None,
    );
    write_files(
        dir.path(),
        &[
            (
                "src/leaf.ts",
                "export function leaf(n: number): number {\n  return n + 1;\n}\n",
            ),
            (
                "src/mid.ts",
                "import { leaf } from \"./leaf\";\nexport function mid(n: number): number {\n  return leaf(n) * 2;\n}\n",
            ),
            (
                "src/app.ts",
                "import { mid } from \"./mid\";\nexport function app(n: number): number {\n  if (n > 0) {\n    return mid(n);\n  } else {\n    return 0;\n  }\n}\n",
            ),
        ],
    );
    commit_all(dir.path(), "initial", "2026-11-01T12:00:00Z");

    for (step, date, msg) in [
        (2, "2026-11-05T12:00:00Z", "second"),
        (3, "2026-11-10T12:00:00Z", "third"),
    ] {
        let leaf =
            format!("export function leaf(n: number): number {{\n  return n + {step};\n}}\n");
        let mid = format!(
            "import {{ leaf }} from \"./leaf\";\nexport function mid(n: number): number {{\n  return leaf(n) * {};\n}}\n",
            step + 1
        );
        write_files(dir.path(), &[("src/leaf.ts", &leaf), ("src/mid.ts", &mid)]);
        commit_all(dir.path(), msg, date);
    }

    (dir, cache)
}

fn repo_with_format(format: &str) -> (TempDir, TempDir) {
    let (dir, cache) = build_sample_repo();
    write_files(
        dir.path(),
        &[("codeint.json", &format!("{{\"format\":\"{format}\"}}\n"))],
    );
    (dir, cache)
}

#[test]
fn configured_format_applies_to_all_eight_analysis_and_report_commands_and_cli_overrides_it() {
    let (dir, cache) = repo_with_format("json");

    let (check_bytes, check_json) = invoke_json(dir.path(), cache.path(), &["check"], None);
    assert_eq!(check_json["files"], 3);

    let check_override = invoke(
        dir.path(),
        cache.path(),
        &["check", "--format", "toon"],
        None,
    );
    assert_eq!(check_override.status.code(), Some(0));
    assert!(utf8(&check_override.stdout).contains("source_sets:"));

    let (_, diff_json) = invoke_json(dir.path(), cache.path(), &["diff", "HEAD~1"], None);
    assert!(diff_json["source_sets"].is_object());

    let (_, trend_json) = invoke_json(dir.path(), cache.path(), &["trend"], None);
    assert_eq!(trend_json["commits"], 3);

    invoke_json(
        dir.path(),
        cache.path(),
        &["report", "-"],
        Some(&check_bytes),
    );
    for args in [
        &["impact", "src/leaf.ts"][..],
        &["trace", "src/app.ts", "src/leaf.ts"][..],
        &["churn"][..],
        &["coupling"][..],
    ] {
        invoke_json(dir.path(), cache.path(), args, None);
    }
}

#[test]
fn unsupported_configured_format_fails_with_exit_code_2_naming_command_format_and_supported_formats()
 {
    let (dir, cache) = repo_with_format("sarif");

    let sarif_unsupported: [(&[&str], &str, &[&str]); 5] = [
        (&["trend"], "trend", &["toon", "text", "json", "markdown"]),
        (&["churn"], "churn", &["toon", "text", "json", "markdown"]),
        (
            &["coupling"],
            "coupling",
            &["toon", "text", "json", "markdown"],
        ),
        (
            &["impact", "src/leaf.ts"],
            "impact",
            &["toon", "text", "json"],
        ),
        (
            &["trace", "src/app.ts", "src/leaf.ts"],
            "trace",
            &["toon", "text", "json"],
        ),
    ];

    for (args, command_name, supported) in sarif_unsupported {
        let out = invoke(dir.path(), cache.path(), args, None);
        let stderr = utf8(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "args={args:?}, stderr={stderr}");
        assert!(
            stderr.contains(command_name),
            "missing {command_name}: {stderr}"
        );
        assert!(stderr.contains("sarif"), "missing sarif: {stderr}");
        for fmt in supported {
            assert!(stderr.contains(fmt), "missing {fmt}: {stderr}");
        }
    }

    invoke_json(
        dir.path(),
        cache.path(),
        &["trend", "--format", "json"],
        None,
    );

    write_files(
        dir.path(),
        &[("codeint.json", "{\"format\":\"markdown\"}\n")],
    );
    for (args, command_name) in [
        (&["impact", "src/leaf.ts"][..], "impact"),
        (&["trace", "src/app.ts", "src/leaf.ts"][..], "trace"),
    ] {
        let out = invoke(dir.path(), cache.path(), args, None);
        let stderr = utf8(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "args={args:?}, stderr={stderr}");
        assert!(stderr.contains(command_name) && stderr.contains("markdown"));
        assert!(stderr.contains("toon") && stderr.contains("text") && stderr.contains("json"));
    }
}

#[test]
fn utility_commands_and_schema_ignore_configured_format() {
    let (dir, cache) = repo_with_format("sarif");

    let (_, schema_doc) = invoke_json(dir.path(), cache.path(), &["schema"], None);
    assert_eq!(schema_doc["name"], "codeint");

    let cases: [(&[&str], &str); 5] = [
        (&["config", "--resolved"], "format = \"sarif\" [file]"),
        (&["doctor"], "codeint doctor"),
        (&["guard", "src/leaf.ts"], "src/leaf.ts"),
        (&["ratchet", "update"], "Created .codeint/ratchet.json"),
        (&["baseline", "update"], "baseline"),
    ];
    for (args, expected) in cases {
        let out = invoke(dir.path(), cache.path(), args, None);
        let stdout = utf8(&out.stdout);
        assert_eq!(out.status.code(), Some(0), "args={args:?}");
        assert!(stdout.contains(expected), "args={args:?} stdout:\n{stdout}");
        assert!(!stdout.starts_with('{'), "args={args:?} printed JSON");
    }
}

#[test]
fn report_reads_configured_format_inside_repo_fails_on_invalid_config_and_falls_back_to_toon_outside_git_repo()
 {
    let (dir, cache) = build_sample_repo();
    let (check_bytes, _) = invoke_json(
        dir.path(),
        cache.path(),
        &["check", "--format", "json"],
        None,
    );

    write_files(dir.path(), &[("codeint.json", "{\"format\":\"text\"}\n")]);
    let report_in_repo = invoke(
        dir.path(),
        cache.path(),
        &["report", "-"],
        Some(&check_bytes),
    );
    assert_eq!(report_in_repo.status.code(), Some(0));
    let report_text = utf8(&report_in_repo.stdout);
    assert!(report_text.starts_with("codeint check: PASS"));
    assert_no_ansi(&report_text);

    write_files(dir.path(), &[("codeint.json", "{\"format\":\"yaml\"}\n")]);
    let report_bad_cfg = invoke(
        dir.path(),
        cache.path(),
        &["report", "-"],
        Some(&check_bytes),
    );
    assert_eq!(report_bad_cfg.status.code(), Some(2));
    assert!(utf8(&report_bad_cfg.stderr).contains("format"));

    let outside = TempDir::new().expect("outside dir");
    let report_outside = invoke(
        outside.path(),
        cache.path(),
        &["report", "-"],
        Some(&check_bytes),
    );
    assert_eq!(report_outside.status.code(), Some(0));
    assert!(utf8(&report_outside.stdout).contains("source_sets:"));
}

fn count_eroded_lines(text: &str) -> usize {
    text.lines()
        .filter(|line| line.contains("packages/a/src/eroded_") && line.contains("cc="))
        .count()
}

fn assert_report_text_parity(root: &Path, cache: &Path, json_stdout: &[u8], expected_text: &str) {
    let rerendered = invoke(
        root,
        cache,
        &["report", "-", "--format", "text"],
        Some(json_stdout),
    );
    assert_eq!(rerendered.status.code(), Some(0));
    assert_eq!(utf8(&rerendered.stdout), expected_text);
}

#[test]
fn check_diff_trend_and_report_text_output_is_concise_monochrome_summary_first_and_respects_all() {
    let dir = TempDir::new().expect("temp repo");
    let cache = TempDir::new().expect("temp cache");
    git_in(
        dir.path(),
        &["init", "--quiet", "--initial-branch", "main"],
        None,
    );

    let mut base_files = vec![
        (
            "package.json".to_owned(),
            r#"{"name":"root","private":true,"workspaces":["packages/*"]}"#.to_owned(),
        ),
        (
            "packages/a/package.json".to_owned(),
            r#"{"name":"@ws/a"}"#.to_owned(),
        ),
        (
            "packages/b/package.json".to_owned(),
            r#"{"name":"@ws/b"}"#.to_owned(),
        ),
        (
            "packages/b/src/clean.ts".to_owned(),
            "export const ok = (n: number): number => n + 1;\n".to_owned(),
        ),
        (
            "packages/a/src/helper.rs".to_owned(),
            "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n".to_owned(),
        ),
    ];
    for i in 0..7 {
        base_files.push((
            format!("packages/a/src/eroded_{i}.ts"),
            eroded_ts(&format!("route_{i}"), 11 + i),
        ));
    }
    let borrowed: Vec<(&str, &str)> = base_files
        .iter()
        .map(|(p, b)| (p.as_str(), b.as_str()))
        .collect();
    write_files(dir.path(), &borrowed);
    commit_all(dir.path(), "base", "2026-11-02T12:00:00Z");

    write_files(
        dir.path(),
        &[(
            "codeint.json",
            "{\"budgets\":{\"production\":{\"erosion\":0.05}}}\n",
        )],
    );

    let check_text_out = invoke(
        dir.path(),
        cache.path(),
        &["check", "--format", "text"],
        None,
    );
    assert_eq!(check_text_out.status.code(), Some(1));
    let check_text = utf8(&check_text_out.stdout);
    assert_no_ansi(&check_text);
    assert!(check_text.starts_with("codeint check: FAIL"));
    assert!(check_text.contains("Budget") && check_text.contains("Production"));
    assert!(
        check_text.contains("Languages")
            && check_text.contains("typescript-javascript")
            && check_text.contains("rust")
    );
    assert!(check_text.contains("packages/a") && !check_text.contains("packages/b"));
    assert_eq!(count_eroded_lines(&check_text), 5);
    assert!(check_text.contains("2 more"));

    let check_all_out = invoke(
        dir.path(),
        cache.path(),
        &["check", "--all", "--format", "text"],
        None,
    );
    assert_eq!(check_all_out.status.code(), Some(1));
    let check_all = utf8(&check_all_out.stdout);
    assert_no_ansi(&check_all);
    assert_eq!(count_eroded_lines(&check_all), 7);
    assert!(check_all.contains("packages/b"));

    let check_json_out = invoke(
        dir.path(),
        cache.path(),
        &["check", "--format", "json"],
        None,
    );
    assert_report_text_parity(
        dir.path(),
        cache.path(),
        &check_json_out.stdout,
        &check_text,
    );

    write_files(
        dir.path(),
        &[
            (
                "packages/a/src/eroded_0.ts",
                "export const route_0 = (x: number): number => x;\n",
            ),
            ("packages/a/src/added.ts", &eroded_ts("added_route", 13)),
        ],
    );

    let diff_text_out = invoke(
        dir.path(),
        cache.path(),
        &["diff", "main", "--format", "text"],
        None,
    );
    assert_eq!(diff_text_out.status.code(), Some(1));
    let diff_text = utf8(&diff_text_out.stdout);
    assert_no_ansi(&diff_text);
    assert!(diff_text.starts_with("codeint diff: FAIL"));
    assert!(diff_text.contains("added_route") && !diff_text.contains("route_0"));

    let diff_all_out = invoke(
        dir.path(),
        cache.path(),
        &["diff", "main", "--all", "--format", "text"],
        None,
    );
    assert_eq!(diff_all_out.status.code(), Some(1));
    assert!(utf8(&diff_all_out.stdout).contains("route_0"));

    let diff_json_out = invoke(
        dir.path(),
        cache.path(),
        &["diff", "main", "--format", "json"],
        None,
    );
    assert_report_text_parity(dir.path(), cache.path(), &diff_json_out.stdout, &diff_text);

    commit_all(dir.path(), "second", "2026-11-09T12:00:00Z");
    let trend_text_out = invoke(
        dir.path(),
        cache.path(),
        &["trend", "--format", "text"],
        None,
    );
    assert_eq!(trend_text_out.status.code(), Some(0));
    let trend_text = utf8(&trend_text_out.stdout);
    assert_no_ansi(&trend_text);
    assert!(trend_text.starts_with("codeint trend"));
    assert!(trend_text.find("2026-11-02") < trend_text.find("2026-11-09"));
}
