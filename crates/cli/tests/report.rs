use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

const GIT_ENV: [(&str, &str); 6] = [
    ("GIT_CONFIG_GLOBAL", "/dev/null"),
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_AUTHOR_NAME", "Report Tester"),
    ("GIT_AUTHOR_EMAIL", "report@example.invalid"),
    ("GIT_COMMITTER_NAME", "Report Tester"),
    ("GIT_COMMITTER_EMAIL", "report@example.invalid"),
];

fn setup_workspace(files: &[(&str, &str)]) -> (TempDir, TempDir) {
    let dir = TempDir::new().expect("temp workspace");
    let cache = TempDir::new().expect("temp cache");
    git_in(dir.path(), &["init", "--quiet", "--initial-branch", "main"]);
    populate(dir.path(), files);
    (dir, cache)
}

fn populate(root: &Path, files: &[(&str, &str)]) {
    for &(rel, body) in files {
        let target = root.join(rel);
        fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
        fs::write(target, body).expect("write");
    }
}

fn git_in(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(root)
        .envs(GIT_ENV)
        .status()
        .expect("git");
    assert!(status.success(), "git {args:?} failed");
}

fn commit_all(root: &Path, message: &str) {
    git_in(root, &["add", "--all"]);
    git_in(root, &["commit", "--quiet", "--message", message]);
}

fn invoke(root: &Path, cache: &Path, args: &[&str], stdin_data: Option<&[u8]>) -> Output {
    let mut proc = Command::new(env!("CARGO_BIN_EXE_codeint"));
    proc.args(args)
        .current_dir(root)
        .env("CODEINT_CACHE_DIR", cache)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if stdin_data.is_some() {
        proc.stdin(Stdio::piped());
    }
    let mut child = proc.spawn().expect("spawn codeint");
    if let Some(payload) = stdin_data {
        child
            .stdin
            .take()
            .expect("stdin handle")
            .write_all(payload)
            .expect("write stdin");
    }
    child.wait_with_output().expect("collect output")
}

fn eroded_ts(name: &str, branches: usize) -> String {
    let arms: String = (1..=branches)
        .map(|i| format!("  if (x === {i}) return {i};\n"))
        .collect();
    format!("export function {name}(x: number): number {{\n{arms}  return 0;\n}}\n")
}

fn verbose_ts(name: &str) -> String {
    format!(
        "export function {name}(v: number): string {{\n  if (v < 0) {{\n    return \"neg\";\n  }} else {{\n    return \"pos\";\n  }}\n}}\n"
    )
}

fn clone_block_ts(name: &str) -> String {
    format!(
        "export function {name}(items: number[]): number {{\n  let total = 0;\n  for (const item of items) {{\n    const scaled = item * 2 + 1;\n    if (scaled > 10) {{\n      total += scaled - 3;\n    }} else {{\n      total += scaled + 5;\n    }}\n  }}\n  const adjusted = total * 3 - 7;\n  return adjusted > 0 ? adjusted : 0;\n}}\n"
    )
}

fn utf8(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("valid UTF-8")
}

fn verify_parity(
    root: &Path,
    cache: &Path,
    base_args: &[&str],
    saved_json: &[u8],
    file_name: &str,
) {
    fs::write(root.join(file_name), saved_json).expect("save json");

    for format in ["sarif", "markdown", "toon", "json"] {
        let mut direct_args = base_args.to_vec();
        direct_args.extend(["--format", format]);
        let direct = invoke(root, cache, &direct_args, None);
        assert!(
            matches!(direct.status.code(), Some(0 | 1)),
            "direct {direct_args:?} failed: {}",
            utf8(&direct.stderr)
        );
        let expected = utf8(&direct.stdout);

        for input in ["-", file_name] {
            let stdin = (input == "-").then_some(saved_json);
            let rerendered = invoke(root, cache, &["report", input, "--format", format], stdin);
            assert_eq!(
                rerendered.status.code(),
                Some(0),
                "report {input} --format {format} failed: {}",
                utf8(&rerendered.stderr)
            );
            assert_eq!(
                utf8(&rerendered.stdout),
                expected,
                "{input} parity mismatch for --format {format}"
            );
        }
    }

    let direct_default = invoke(root, cache, base_args, None);
    let from_text = invoke(
        root,
        cache,
        &["report", "-", "--format", "text"],
        Some(saved_json),
    );
    assert_eq!(from_text.status.code(), Some(0));
    assert_eq!(utf8(&from_text.stdout), utf8(&direct_default.stdout));
}

fn capture_and_verify_parity(
    root: &Path,
    cache: &Path,
    base_args: &[&str],
    expected_code: i32,
    file_name: &str,
) {
    let mut json_args = base_args.to_vec();
    json_args.extend(["--format", "json"]);
    let json_run = invoke(root, cache, &json_args, None);
    assert_eq!(json_run.status.code(), Some(expected_code));
    verify_parity(root, cache, base_args, &json_run.stdout, file_name);
}

#[test]
fn rerenders_check_json_from_stdin_and_file_with_full_format_parity() {
    let eroded = eroded_ts("dispatch", 12);
    let verbose = verbose_ts("label");
    let dup_a = clone_block_ts("calcA");
    let dup_b = clone_block_ts("calcB");
    let (dir, cache) = setup_workspace(&[
        ("src/eroded.ts", &eroded),
        ("src/verbose.ts", &verbose),
        ("src/dup_a.ts", &dup_a),
        ("src/dup_b.ts", &dup_b),
        (
            "src/cycle_a.ts",
            "import { b } from \"./cycle_b\";\nexport const a = (): number => b() + 1;\n",
        ),
        (
            "src/cycle_b.ts",
            "import { a } from \"./cycle_a\";\nexport const b = (): number => a() + 1;\n",
        ),
        ("src/broken.ts", "export const broken = ;\n"),
        ("src/types.d.ts", "export type Label = string;\n"),
        (
            "codeint.json",
            "{\"budgets\":{\"production\":{\"erosion\":0.1}}}\n",
        ),
    ]);

    capture_and_verify_parity(dir.path(), cache.path(), &["check"], 1, "check-report.json");

    let to_file = invoke(
        dir.path(),
        cache.path(),
        &[
            "report",
            "check-report.json",
            "--format",
            "markdown",
            "--output",
            "rendered.md",
        ],
        None,
    );
    assert_eq!(to_file.status.code(), Some(0));
    assert!(to_file.stdout.is_empty());
    let direct_md = invoke(
        dir.path(),
        cache.path(),
        &["check", "--format", "markdown"],
        None,
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("rendered.md")).expect("read rendered.md"),
        utf8(&direct_md.stdout)
    );
}

#[test]
fn rerenders_diff_json_from_stdin_and_file_with_full_format_parity() {
    let old = eroded_ts("legacy", 11);
    let sign = verbose_ts("sign");
    let (dir, cache) = setup_workspace(&[("src/old.ts", &old), ("src/verbose.ts", &sign)]);
    commit_all(dir.path(), "base");

    git_in(dir.path(), &["checkout", "--quiet", "-b", "feature"]);
    let added = eroded_ts("route", 14);
    populate(
        dir.path(),
        &[
            (
                "src/old.ts",
                "export function legacy(x: number): number {\n  return x;\n}\n",
            ),
            ("src/added.ts", &added),
        ],
    );
    commit_all(dir.path(), "feature");

    capture_and_verify_parity(
        dir.path(),
        cache.path(),
        &["diff", "main", "--head", "feature"],
        1,
        "diff-commit.json",
    );

    let extra = verbose_ts("extra");
    populate(dir.path(), &[("src/extra.ts", &extra)]);
    capture_and_verify_parity(
        dir.path(),
        cache.path(),
        &["diff", "main"],
        1,
        "diff-wt.json",
    );
}

#[test]
fn rejects_malformed_json_unsupported_schema_and_missing_input_with_exit_code_2() {
    struct ErrorCase<'a> {
        args: &'a [&'a str],
        stdin: Option<&'a [u8]>,
        expected: &'a str,
    }

    let (dir, cache) = setup_workspace(&[]);

    let cases = [
        ErrorCase {
            args: &["report", "-", "--format", "sarif"],
            stdin: Some(b"{not json"),
            expected: "JSON",
        },
        ErrorCase {
            args: &["report", "-", "--format", "markdown"],
            stdin: Some(b"[1, 2, 3]"),
            expected: "JSON object",
        },
        ErrorCase {
            args: &["report", "-", "--format", "sarif"],
            stdin: Some(
                br#"{"schema_version": 2, "files": 0, "source_lines": 0, "source_sets": {}}"#,
            ),
            expected: "unsupported report schema version",
        },
        ErrorCase {
            args: &["report", "-", "--format", "markdown"],
            stdin: Some(
                br#"{"$schema": "https://example.invalid/sarif.json", "version": "2.1.0", "runs": []}"#,
            ),
            expected: "unsupported report schema version",
        },
        ErrorCase {
            args: &["report", "-", "--format", "toon"],
            stdin: Some(br#"{"commits": 1, "source_sets": {}}"#),
            expected: "check or diff",
        },
        ErrorCase {
            args: &["report", "missing.json", "--format", "sarif"],
            stdin: None,
            expected: "missing.json",
        },
    ];

    for case in cases {
        let out = invoke(dir.path(), cache.path(), case.args, case.stdin);
        assert_eq!(
            out.status.code(),
            Some(2),
            "expected exit 2 for {:?}",
            case.args
        );
        let err = utf8(&out.stderr);
        assert!(
            err.contains(case.expected),
            "expected {:?} in stderr for {:?}, got: {err}",
            case.expected,
            case.args
        );
    }
}

#[test]
fn rerenders_multilanguage_boundaries_and_unresolved_imports_losslessly() {
    let (dir, cache) = setup_workspace(&[
        (
            "codeint.json",
            r#"{
  "boundaries": {
    "zones": {
      "domain": { "patterns": ["src/domain/**"] },
      "infra": { "patterns": ["src/infra/**"] }
    },
    "rules": [
      { "from": "domain", "deny": ["infra"], "reason": "domain stays pure" }
    ]
  }
}
"#,
        ),
        (
            "src/infra/db.ts",
            "export function query(): number {\n  return 1;\n}\n",
        ),
        (
            "src/domain/service.ts",
            "import { query } from \"../infra/db\";\nimport { missing } from \"./does-not-exist\";\nexport function run(): number {\n  return query() + Number(missing);\n}\n",
        ),
        (
            "src/helper.rs",
            "pub fn double(n: i32) -> i32 {\n    n * 2\n}\n",
        ),
    ]);
    commit_all(dir.path(), "base");

    capture_and_verify_parity(dir.path(), cache.path(), &["check"], 0, "multi-check.json");

    populate(
        dir.path(),
        &[(
            "src/domain/extra.ts",
            "import { query } from \"../infra/db\";\nexport const extra = (): number => query();\n",
        )],
    );
    capture_and_verify_parity(
        dir.path(),
        cache.path(),
        &["diff", "main"],
        1,
        "multi-diff.json",
    );
}

fn assert_markdown_handles_max_left_out(
    dir: &Path,
    cache: &Path,
    source_args: &[&str],
    expected_code: i32,
    left_out_field: &str,
) {
    let raw_json = invoke(dir, cache, source_args, None);
    assert_eq!(raw_json.status.code(), Some(expected_code));
    let mut doc: serde_json::Value =
        serde_json::from_slice(&raw_json.stdout).expect("valid report json");
    doc["source_sets"]["production"]["boundaries"][left_out_field] = serde_json::json!(usize::MAX);
    let payload = serde_json::to_vec(&doc).expect("serialize report json");
    let rendered = invoke(
        dir,
        cache,
        &["report", "-", "--format", "markdown"],
        Some(&payload),
    );
    assert_eq!(
        rendered.status.code(),
        Some(0),
        "stderr: {}",
        utf8(&rendered.stderr)
    );
    assert!(utf8(&rendered.stdout).contains(&usize::MAX.to_string()));
}

#[test]
fn rerenders_markdown_without_overflow_when_left_out_is_usize_max() {
    let (dir, cache) = setup_workspace(&[
        (
            "codeint.json",
            r#"{
  "boundaries": {
    "zones": {
      "domain": { "patterns": ["src/domain/**"] },
      "infra": { "patterns": ["src/infra/**"] }
    },
    "rules": [
      { "from": "domain", "deny": ["infra"] }
    ]
  }
}
"#,
        ),
        (
            "src/infra/db.ts",
            "export function query(): number {\n  return 1;\n}\n",
        ),
        (
            "src/domain/service.ts",
            "import { query } from \"../infra/db\";\nexport const run = (): number => query();\n",
        ),
    ]);
    commit_all(dir.path(), "base");

    assert_markdown_handles_max_left_out(
        dir.path(),
        cache.path(),
        &["check", "--format", "json"],
        0,
        "findings_left_out",
    );

    populate(
        dir.path(),
        &[(
            "src/domain/extra.ts",
            "import { query } from \"../infra/db\";\nexport const extra = (): number => query();\n",
        )],
    );
    assert_markdown_handles_max_left_out(
        dir.path(),
        cache.path(),
        &["diff", "main", "--format", "json"],
        1,
        "new_left_out",
    );
}
