use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use codeint_plugin_host::{sha256, sha256_hex};
use tempfile::TempDir;

use crate::common::{eroded_ts, no_imports_language_plugin_wasm};

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

    for format in ["sarif", "markdown", "toon", "json", "text"] {
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

fn capture_containing_and_verify_parity(
    root: &Path,
    cache: &Path,
    base_args: &[&str],
    expected_code: i32,
    expected_snippet: &str,
    file_name: &str,
) {
    let mut json_args = base_args.to_vec();
    json_args.extend(["--format", "json"]);
    let json_run = invoke(root, cache, &json_args, None);
    assert_eq!(json_run.status.code(), Some(expected_code));
    assert!(utf8(&json_run.stdout).contains(expected_snippet));
    verify_parity(root, cache, base_args, &json_run.stdout, file_name);
}

#[test]
fn rerenders_check_and_diff_json_with_codeowners_owners_losslessly() {
    let base_file = verbose_ts("sign");
    let (dir, cache) = setup_workspace(&[
        (".github/CODEOWNERS", "src/ @platform @alice\n"),
        ("src/sign.ts", &base_file),
    ]);
    commit_all(dir.path(), "base");

    let added = eroded_ts("route", 14);
    populate(dir.path(), &[("src/route.ts", &added)]);

    capture_containing_and_verify_parity(
        dir.path(),
        cache.path(),
        &["check"],
        0,
        "\"@platform\"",
        "owned-check.json",
    );
    capture_containing_and_verify_parity(
        dir.path(),
        cache.path(),
        &["diff", "main"],
        1,
        "\"@platform\"",
        "owned-diff.json",
    );
}

#[test]
fn rerenders_check_and_diff_json_with_orphan_files_losslessly() {
    let (dir, cache) = setup_workspace(&[
        ("package.json", r#"{"name":"app","main":"src/index.ts"}"#),
        (
            "src/index.ts",
            "import { live } from \"./live\";\nexport const run = (): number => live();\n",
        ),
        ("src/live.ts", "export const live = (): number => 1;\n"),
        ("src/unused.ts", "export const unused = (): number => 2;\n"),
        (
            "src/tested_only.ts",
            "export const onlyTest = (): number => 3;\n",
        ),
        (
            "src/tested_only.test.ts",
            "import { onlyTest } from \"./tested_only\";\nexport const check = (): number => onlyTest();\n",
        ),
    ]);
    commit_all(dir.path(), "base");

    capture_containing_and_verify_parity(
        dir.path(),
        cache.path(),
        &["check"],
        0,
        "\"orphan_files\"",
        "orphans-check.json",
    );

    populate(
        dir.path(),
        &[(
            "src/new_orphan.ts",
            "export const extra = (): number => 4;\n",
        )],
    );
    capture_containing_and_verify_parity(
        dir.path(),
        cache.path(),
        &["diff", "main"],
        1,
        "\"orphan_files\"",
        "orphans-diff.json",
    );
}

fn near_clone_variant_ts(name: &str, bonus: i32) -> String {
    format!(
        "export function {name}(items: number[]): number {{\n  let total = 0;\n  for (const item of items) {{\n    const scaled = item * 2 + 1;\n    if (scaled > 10) {{\n      total += scaled - 3;\n    }} else {{\n      total += scaled + 5;\n    }}\n  }}\n  const extra = total + {bonus};\n  const adjusted = extra * 3 - 7;\n  return adjusted > 0 ? adjusted : 0;\n}}\n"
    )
}

#[test]
fn rerenders_check_and_diff_json_with_clone_families_and_near_clones_losslessly() {
    let block_a = clone_block_ts("calcA");
    let block_b = clone_block_ts("calcB");
    let block_c = near_clone_variant_ts("calcC", 1);
    let (dir, cache) = setup_workspace(&[
        (
            "codeint.json",
            "{\"duplication\":{\"near_clones\":true,\"min_similarity\":0.75}}\n",
        ),
        ("src/dup_a.ts", &block_a),
    ]);
    commit_all(dir.path(), "base");

    populate(
        dir.path(),
        &[("src/dup_b.ts", &block_b), ("src/dup_c.ts", &block_c)],
    );

    let check_json = invoke(
        dir.path(),
        cache.path(),
        &["check", "--format", "json"],
        None,
    );
    assert_eq!(check_json.status.code(), Some(0));
    let check_text = utf8(&check_json.stdout);
    assert!(check_text.contains("\"near_clones\""));
    assert!(check_text.contains("\"clone_families\""));
    verify_parity(
        dir.path(),
        cache.path(),
        &["check"],
        &check_json.stdout,
        "near-check.json",
    );

    let diff_json = invoke(
        dir.path(),
        cache.path(),
        &["diff", "main", "--format", "json"],
        None,
    );
    assert_eq!(diff_json.status.code(), Some(1));
    let diff_text = utf8(&diff_json.stdout);
    assert!(diff_text.contains("\"clone_families\""));
    verify_parity(
        dir.path(),
        cache.path(),
        &["diff", "main"],
        &diff_json.stdout,
        "near-diff.json",
    );
}

fn family_ts_source(family_id: usize, extra_ops: usize, compact: bool) -> String {
    const OPS: [&str; 5] = ["+", "-", "*", "%", "&"];
    const EXTRA_OUTER: [&str; 4] = ["^", "|", "+", "-"];
    let op0 = OPS[family_id % OPS.len()];
    let op1 = OPS[(family_id / OPS.len()) % OPS.len()];
    let op2 = OPS[(family_id / (OPS.len() * OPS.len())) % OPS.len()];
    let mut statements = vec![
        format!("let acc = (a {op0} b) + 1;"),
        "for (const item of items) {".to_owned(),
        format!("const x = (item {op1} a) - (b {op2} 2);"),
        format!("if (x > 10 && acc < 100) {{ acc = (acc {op0} x) ^ 3; }}"),
        format!("if (x <= 0 || acc >= 50) {{ acc = (acc {op1} b) | 5; }}"),
        "}".to_owned(),
        format!("const mid = acc > 0 ? (acc {op2} a) : (b {op0} 7);"),
        format!("let tail = (mid {op1} 11) + (acc {op2} 13);"),
    ];
    for step in 0..extra_ops {
        let outer = EXTRA_OUTER[step % EXTRA_OUTER.len()];
        let family_op = match step % 3 {
            0 => op0,
            1 => op1,
            _ => op2,
        };
        let rhs = step + 17;
        statements.push(format!(
            "tail = (tail {family_op} acc) {outer} (a {op0} {rhs});"
        ));
    }
    statements.push("return tail;".to_owned());
    let body = if compact {
        statements
            .chunks(4)
            .map(|chunk| chunk.join(" "))
            .collect::<Vec<String>>()
            .join("\n  ")
    } else {
        statements.join("\n  ")
    };
    format!(
        "export function calc(a: number, b: number, items: number[]): number {{\n  {body}\n}}\n"
    )
}

struct TruncatedFamiliesCase<'a> {
    base_args: &'a [&'a str],
    expected_code: i32,
    file_name: &'a str,
    list_key: &'a str,
    left_out_key: &'a str,
}

fn verify_truncated_families_sarif_parity(
    root: &Path,
    cache: &Path,
    case: &TruncatedFamiliesCase<'_>,
    families_container: impl FnOnce(&serde_json::Value) -> &serde_json::Value,
) {
    let mut json_args = case.base_args.to_vec();
    json_args.extend(["--format", "json"]);
    let json_run = invoke(root, cache, &json_args, None);
    assert_eq!(json_run.status.code(), Some(case.expected_code));
    let doc: serde_json::Value = serde_json::from_slice(&json_run.stdout).expect("parse json");
    let container = families_container(&doc);
    assert_eq!(container[case.list_key].as_array().map(Vec::len), Some(20));
    assert_eq!(container[case.left_out_key], 6);
    let unlisted = container["unlisted_clone_family_group_ids"]
        .as_array()
        .expect("unlisted_clone_family_group_ids");
    assert_eq!(unlisted.len(), 6);
    verify_parity(
        root,
        cache,
        case.base_args,
        &json_run.stdout,
        case.file_name,
    );

    let sarif = invoke(
        root,
        cache,
        &["report", "-", "--format", "sarif"],
        Some(&json_run.stdout),
    );
    assert_eq!(sarif.status.code(), Some(0));
    let sarif_doc: serde_json::Value = serde_json::from_slice(&sarif.stdout).expect("parse sarif");
    let results = sarif_doc["runs"][0]["results"]
        .as_array()
        .expect("sarif results");
    assert_eq!(results.len(), 21);
    for leaked in unlisted {
        let leaked_id = leaked.as_str().expect("id string");
        assert!(
            results.iter().all(|result| {
                result["partialFingerprints"]["codeintFindingId"].as_str() != Some(leaked_id)
            }),
            "{} SARIF leaked member CloneGroup {leaked_id}",
            case.file_name
        );
    }
}

#[test]
fn rerenders_truncated_clone_families_check_and_diff_json_with_exact_sarif_parity() {
    let (dir, cache) = setup_workspace(&[("src/base.ts", "export const base = 1;\n")]);
    commit_all(dir.path(), "base");

    let mut owned_files = Vec::new();
    for fam in 0..20usize {
        let source = family_ts_source(fam, 0, false);
        for copy in 0..3usize {
            owned_files.push((format!("src/tall_{fam:02}_{copy}.ts"), source.clone()));
        }
    }
    for fam in 20..26usize {
        let source = family_ts_source(fam, 8, true);
        for copy in 0..3usize {
            owned_files.push((format!("src/wide_{fam:02}_{copy}.ts"), source.clone()));
        }
    }
    let standalone_source = family_ts_source(26, 5, true);
    for copy in 0..2usize {
        owned_files.push((
            format!("src/standalone_{copy}.ts"),
            standalone_source.clone(),
        ));
    }
    let borrowed_files: Vec<(&str, &str)> = owned_files
        .iter()
        .map(|(path, body)| (path.as_str(), body.as_str()))
        .collect();
    populate(dir.path(), &borrowed_files);

    verify_truncated_families_sarif_parity(
        dir.path(),
        cache.path(),
        &TruncatedFamiliesCase {
            base_args: &["check"],
            expected_code: 0,
            file_name: "truncated-check.json",
            list_key: "clone_families",
            left_out_key: "clone_families_left_out",
        },
        |doc| &doc["source_sets"]["production"]["duplication"],
    );

    let check_all_json = invoke(
        dir.path(),
        cache.path(),
        &["check", "--all", "--format", "json"],
        None,
    );
    assert_eq!(check_all_json.status.code(), Some(0));
    let check_all_doc: serde_json::Value =
        serde_json::from_slice(&check_all_json.stdout).expect("parse check --all json");
    assert_eq!(
        check_all_doc["source_sets"]["production"]["duplication"]
            .get("unlisted_clone_family_group_ids"),
        None
    );

    verify_truncated_families_sarif_parity(
        dir.path(),
        cache.path(),
        &TruncatedFamiliesCase {
            base_args: &["diff", "main"],
            expected_code: 1,
            file_name: "truncated-diff.json",
            list_key: "new",
            left_out_key: "new_left_out",
        },
        |doc| &doc["source_sets"]["production"]["duplication"]["clone_families"],
    );
}

#[test]
fn rerenders_check_and_diff_json_with_cognitive_complexity_and_dependencies_losslessly() {
    let deep_fn = concat!(
        "import ghost from \"unlisted-pkg\";\n",
        "import devOnly from \"dev-pkg\";\n",
        "export function deepProcess(a: boolean, b: boolean, c: boolean, d: boolean, e: boolean, f: boolean): number {\n",
        "  if (a) { if (b) { if (c) { if (d) { if (e) { if (f) { return Number(ghost) + Number(devOnly); } } } } } }\n",
        "  return 0;\n",
        "}\n",
    );
    let (dir, cache) = setup_workspace(&[
        (
            "package.json",
            r#"{"name":"app","main":"src/index.ts","dependencies":{"unused-pkg":"^1.0.0"},"devDependencies":{"dev-pkg":"^1.0.0"}}"#,
        ),
        ("src/index.ts", "export const ok = (): number => 1;\n"),
    ]);
    commit_all(dir.path(), "base");

    populate(dir.path(), &[("src/deep.ts", deep_fn)]);

    capture_containing_and_verify_parity(
        dir.path(),
        cache.path(),
        &["check"],
        0,
        "\"dev_in_production_dependencies\"",
        "deps-check.json",
    );
    capture_containing_and_verify_parity(
        dir.path(),
        cache.path(),
        &["diff", "main"],
        1,
        "\"cognitive_complexity\"",
        "deps-diff.json",
    );
}

fn install_no_imports_plugin(root: &Path) -> String {
    let bytes = fs::read(no_imports_language_plugin_wasm()).expect("read no-imports wasm");
    let target = root.join("plugins/no_imports_language_plugin.wasm");
    fs::create_dir_all(target.parent().expect("plugin parent")).expect("mkdir plugins");
    fs::write(&target, &bytes).expect("write no-imports wasm");
    sha256_hex(&sha256(&bytes))
}

fn assert_import_metrics_absent_in_section(section: &serde_json::Value, label: &str) {
    assert_eq!(
        section.get("dependencies"),
        None,
        "{label} must omit dependencies"
    );
    let note = section
        .get("import_metrics_note")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("{label} must include import_metrics_note: {section}"));
    assert!(
        !note.contains('\n'),
        "{label} import_metrics_note must be one line: {note:?}"
    );
    assert!(
        note.contains("has no imports"),
        "{label} import_metrics_note must explain language has no imports: {note:?}"
    );
    let source_sets = section["source_sets"]
        .as_object()
        .unwrap_or_else(|| panic!("{label} missing source_sets"));
    assert!(!source_sets.is_empty(), "{label} source_sets empty");
    for (source_set_name, source_set) in source_sets {
        assert_eq!(
            source_set.get("import_cycles"),
            None,
            "{label}.{source_set_name} must omit import_cycles"
        );
        assert_eq!(
            source_set.get("orphan_files"),
            None,
            "{label}.{source_set_name} must omit orphan_files"
        );
        assert_eq!(
            source_set.get("boundaries"),
            None,
            "{label}.{source_set_name} must omit boundaries"
        );
        assert!(source_set.get("erosion").is_some());
        assert!(source_set.get("cognitive_complexity").is_some());
        assert!(source_set.get("duplication").is_some());
        assert!(source_set.get("verbosity").is_some());
        if let Some(packages) = source_set
            .get("packages")
            .and_then(serde_json::Value::as_array)
        {
            for package in packages {
                assert_eq!(
                    package.get("import_cycles"),
                    None,
                    "{label}.{source_set_name} package {} must omit import_cycles",
                    package["path"]
                );
            }
        }
    }
}

fn assert_toon_omits_import_metrics(toon_section: &str, label: &str) {
    assert!(
        toon_section.contains("import_metrics_note:") && toon_section.contains("has no imports"),
        "{label} TOON must include one-line import_metrics_note:\n{toon_section}"
    );
    for forbidden in [
        "import_cycles:",
        "orphan_files:",
        "boundaries:",
        "dependencies:",
        "unused_dependencies:",
        "unlisted_dependencies:",
        "dev_in_production_dependencies:",
    ] {
        assert!(
            !toon_section.contains(forbidden),
            "{label} TOON must omit {forbidden}:\n{toon_section}"
        );
    }
}

struct NoImportsCheckOutputs {
    json: serde_json::Value,
    toon: String,
    markdown: String,
    sarif: serde_json::Value,
}

fn run_no_imports_check_with_parity(
    files: &[(&str, &str)],
    entry_points: &[&str],
    zones: serde_json::Value,
    rules: serde_json::Value,
    budgets: Option<serde_json::Value>,
) -> NoImportsCheckOutputs {
    let (dir, cache) = setup_workspace(files);
    let hash = install_no_imports_plugin(dir.path());
    let mut config = serde_json::json!({
        "plugins": [{
            "name": "no-imports-lang",
            "path": "plugins/no_imports_language_plugin.wasm",
            "sha256": hash,
        }],
        "entry_points": entry_points,
        "boundaries": {
            "strict_zones": true,
            "zones": zones,
            "rules": rules,
        },
    });
    if let Some(budgets) = budgets {
        config["budgets"] = budgets;
    }
    let config_text = serde_json::to_string_pretty(&config).expect("serialize config");
    populate(dir.path(), &[("codeint.json", &config_text)]);

    let run_format = |format: &str| {
        let out = invoke(
            dir.path(),
            cache.path(),
            &["check", "--format", format],
            None,
        );
        assert_eq!(
            out.status.code(),
            Some(0),
            "check --format {format} failed: {}",
            utf8(&out.stderr)
        );
        out.stdout
    };
    let json_bytes = run_format("json");
    let json_text = utf8(&json_bytes);
    let toon_text = utf8(&run_format("toon"));
    let markdown_text = utf8(&run_format("markdown"));
    let sarif_text = utf8(&run_format("sarif"));
    for (format, expected) in [
        ("json", json_text.as_str()),
        ("toon", toon_text.as_str()),
        ("markdown", markdown_text.as_str()),
        ("sarif", sarif_text.as_str()),
    ] {
        let rerendered = invoke(
            dir.path(),
            cache.path(),
            &["report", "-", "--format", format],
            Some(&json_bytes),
        );
        assert_eq!(
            rerendered.status.code(),
            Some(0),
            "report - --format {format} failed: {}",
            utf8(&rerendered.stderr)
        );
        assert_eq!(
            utf8(&rerendered.stdout),
            expected,
            "report - parity mismatch for --format {format}"
        );
    }
    let json: serde_json::Value = serde_json::from_slice(&json_bytes).expect("parse json");
    let sarif: serde_json::Value = serde_json::from_str(&sarif_text).expect("parse sarif");
    NoImportsCheckOutputs {
        json,
        toon: toon_text,
        markdown: markdown_text,
        sarif,
    }
}

fn assert_markdown_omits_import_metrics_with_reason(markdown_section: &str, label: &str) {
    assert!(
        markdown_section.contains("This language has no imports, so Import cycles, Orphan files, Import boundaries, and Dependency hygiene are not measured."),
        "{label} Markdown must include one-line no-imports reason:\n{markdown_section}"
    );
    for required in [
        "| Erosion |",
        "| Cognitive complexity |",
        "| Duplication |",
        "| Verbosity |",
    ] {
        assert!(
            markdown_section.contains(required),
            "{label} Markdown must still show {required}:\n{markdown_section}"
        );
    }
    for forbidden in [
        "| Import cycles |",
        "| Boundaries |",
        "| Orphan files |",
        "| Unused dependencies |",
        "| Unlisted dependencies |",
        "| Dev dependencies in production |",
        "## Dependencies",
        "| not measured |",
    ] {
        assert!(
            !markdown_section.contains(forbidden),
            "{label} Markdown must omit {forbidden}:\n{markdown_section}"
        );
    }
}

fn assert_sarif_has_no_import_findings_for_extension(
    sarif: &serde_json::Value,
    extension: &str,
    label: &str,
) {
    const IMPORT_RULE_IDS: [&str; 6] = [
        "import_cycles",
        "orphan_files",
        "boundaries",
        "unused_dependencies",
        "unlisted_dependencies",
        "dev_in_production_dependencies",
    ];
    assert_eq!(sarif["version"], "2.1.0", "{label} must be SARIF 2.1.0");
    assert!(
        sarif["$schema"]
            .as_str()
            .is_some_and(|schema| schema.contains("sarif-schema-2.1.0.json")),
        "{label} missing SARIF 2.1.0 schema"
    );
    let runs = sarif["runs"].as_array().expect("SARIF runs array");
    assert_eq!(runs.len(), 1, "{label} expected one SARIF run");
    let results = runs[0]["results"].as_array().expect("SARIF results array");
    for result in results {
        let rule_id = result["ruleId"].as_str().unwrap_or("");
        if !IMPORT_RULE_IDS.contains(&rule_id) {
            continue;
        }
        let message = result["message"]["text"].as_str().unwrap_or("");
        assert!(
            !message.contains(extension),
            "{label} SARIF import finding ({rule_id}) references {extension}: {result}"
        );
        for key in ["locations", "relatedLocations"] {
            if let Some(locations) = result.get(key).and_then(serde_json::Value::as_array) {
                for location in locations {
                    let uri = location["physicalLocation"]["artifactLocation"]["uri"]
                        .as_str()
                        .unwrap_or("");
                    assert!(
                        !uri.ends_with(extension),
                        "{label} SARIF import finding ({rule_id}) points at {uri}: {result}"
                    );
                }
            }
        }
    }
}

fn run_mixed_no_imports_repo(
    include_ts_import_findings: bool,
    zones: serde_json::Value,
) -> NoImportsCheckOutputs {
    let mut files = vec![
        (
            "package.json",
            r#"{"name":"root","private":true,"workspaces":["packages/*"]}"#,
        ),
        (
            "packages/app/package.json",
            r#"{"name":"app","main":"src/index.ts","dependencies":{"unused-pkg":"^1.0.0"},"devDependencies":{"dev-pkg":"^1.0.0"}}"#,
        ),
        (
            "packages/app/src/orphan.ts",
            "export const unused = (): number => 2;\n",
        ),
        ("packages/app/queries/users.noimp", "fn select_users()\n"),
        (
            "packages/app/queries/users_test.noimp",
            "fn test_select_users()\n",
        ),
    ];
    if include_ts_import_findings {
        files.extend([
            (
                "packages/app/src/index.ts",
                "import ghost from \"unlisted-pkg\";\nimport devOnly from \"dev-pkg\";\nimport { a } from \"./cycle_a\";\nexport const ok = (): number => Number(ghost) + Number(devOnly) + a();\n",
            ),
            (
                "packages/app/src/cycle_a.ts",
                "import { b } from \"./cycle_b\";\nexport const a = (): number => b() + 1;\n",
            ),
            (
                "packages/app/src/cycle_b.ts",
                "import { a } from \"./cycle_a\";\nexport const b = (): number => a() + 1;\n",
            ),
        ]);
    } else {
        files.push((
            "packages/app/src/index.ts",
            "export const ok = (): number => 1;\n",
        ));
    }
    run_no_imports_check_with_parity(
        &files,
        &[
            "packages/app/src/index.ts",
            "packages/app/queries/users.noimp",
        ],
        zones,
        serde_json::json!([{ "from": "app", "allow": ["app"] }]),
        None,
    )
}

fn run_no_imports_only_repo(zones: serde_json::Value) -> NoImportsCheckOutputs {
    run_no_imports_check_with_parity(
        &[
            ("queries/users.noimp", "fn select_users()\n"),
            ("queries/users_test.noimp", "fn test_select_users()\n"),
        ],
        &["queries/users.noimp"],
        zones,
        serde_json::json!([]),
        Some(serde_json::json!({ "production": { "import_cycles": 0.0 } })),
    )
}

#[test]
fn no_imports_language_omits_import_metrics_with_reason_in_json_and_toon() {
    let outputs = run_mixed_no_imports_repo(
        false,
        serde_json::json!({
            "app": { "patterns": ["packages/app/src/**"] },
            "queries": { "patterns": ["packages/app/queries/**"] },
        }),
    );
    let doc = outputs.json;
    let toon = outputs.toon;

    let no_imports = &doc["languages"]["no-imports-lang"];
    assert_import_metrics_absent_in_section(no_imports, "languages.no-imports-lang");
    let no_imports_packages = no_imports["source_sets"]["production"]["packages"]
        .as_array()
        .expect("no-imports-lang production packages");
    assert!(
        !no_imports_packages.is_empty(),
        "expected workspace package entry in no-imports-lang section"
    );

    let ts = &doc["languages"]["typescript-javascript"];
    assert_eq!(ts.get("import_metrics_note"), None);
    assert!(ts.get("dependencies").is_some());
    assert!(ts["dependencies"].get("unused_dependencies").is_some());
    assert!(ts["dependencies"].get("unlisted_dependencies").is_some());
    assert!(
        ts["dependencies"]
            .get("dev_in_production_dependencies")
            .is_some()
    );
    let ts_prod = &ts["source_sets"]["production"];
    assert!(ts_prod.get("import_cycles").is_some());
    assert!(ts_prod.get("orphan_files").is_some());
    assert!(ts_prod.get("boundaries").is_some());
    let ts_packages = ts_prod["packages"].as_array().expect("ts packages");
    assert!(ts_packages[0].get("import_cycles").is_some());

    let no_imports_start = toon
        .find("\n  \"no-imports-lang\":\n")
        .unwrap_or_else(|| panic!("no-imports-lang section in TOON:\n{toon}"));
    let ts_start = toon
        .find("\n  \"typescript-javascript\":\n")
        .unwrap_or_else(|| panic!("typescript-javascript section in TOON:\n{toon}"));
    assert!(no_imports_start < ts_start);
    let no_imports_toon = &toon[no_imports_start..ts_start];
    let ts_toon = &toon[ts_start..];
    assert_toon_omits_import_metrics(no_imports_toon, "languages.no-imports-lang");
    assert!(ts_toon.contains("import_cycles:"));
    assert!(ts_toon.contains("orphan_files:"));
    assert!(ts_toon.contains("boundaries:"));
    assert!(ts_toon.contains("dependencies:"));
    assert!(!ts_toon.contains("import_metrics_note:"));
}

#[test]
fn no_imports_only_repo_omits_import_metrics_with_reason_in_json_and_toon() {
    let outputs =
        run_no_imports_only_repo(serde_json::json!({ "queries": { "patterns": ["queries/**"] } }));
    let doc = outputs.json;
    let toon = outputs.toon;
    assert_eq!(doc.get("languages"), None);
    assert_eq!(doc.get("over_budget"), None);
    assert_import_metrics_absent_in_section(&doc, "top-level no-imports-only repo");
    assert_toon_omits_import_metrics(&toon, "top-level no-imports-only repo");
}

#[test]
fn no_imports_language_omits_import_metrics_in_markdown_and_has_no_import_findings_in_sarif() {
    let outputs = run_mixed_no_imports_repo(
        true,
        serde_json::json!({
            "app": { "patterns": ["packages/app/src/**"] },
        }),
    );

    let md = &outputs.markdown;
    let no_imports_start = md
        .find("## Language plugin `no-imports-lang`")
        .unwrap_or_else(|| panic!("no-imports-lang heading missing in Markdown:\n{md}"));
    let ts_start = md
        .find("## Language plugin `typescript-javascript`")
        .unwrap_or_else(|| panic!("typescript-javascript heading missing in Markdown:\n{md}"));
    let findings_start = md
        .find("## Production Findings")
        .unwrap_or_else(|| panic!("Production Findings heading missing in Markdown:\n{md}"));
    assert!(no_imports_start < ts_start && ts_start < findings_start);

    let no_imports_md = &md[no_imports_start..ts_start];
    let ts_md = &md[ts_start..findings_start];
    let findings_md = &md[findings_start..];

    assert_markdown_omits_import_metrics_with_reason(no_imports_md, "languages.no-imports-lang");
    assert!(
        no_imports_md.contains("| `packages/app` |"),
        "expected workspace package row in no-imports-lang Markdown:\n{no_imports_md}"
    );
    assert!(
        !findings_md.contains(".noimp"),
        "findings Markdown must not report import findings for .noimp files:\n{findings_md}"
    );

    assert!(
        !ts_md.contains("has no imports"),
        "typescript-javascript Markdown must not show no-imports note:\n{ts_md}"
    );
    for required in [
        "| Import cycles |",
        "| Boundaries |",
        "| Orphan files |",
        "| Unused dependencies |",
        "| Unlisted dependencies |",
        "| Dev dependencies in production |",
    ] {
        assert!(
            ts_md.contains(required),
            "typescript-javascript Markdown must keep {required}:\n{ts_md}"
        );
    }

    assert_sarif_has_no_import_findings_for_extension(&outputs.sarif, ".noimp", "mixed repo");
    let rule_ids: std::collections::HashSet<&str> = outputs.sarif["runs"][0]["results"]
        .as_array()
        .expect("SARIF results")
        .iter()
        .filter_map(|result| result["ruleId"].as_str())
        .collect();
    for expected_ts_rule in [
        "import_cycles",
        "orphan_files",
        "unused_dependencies",
        "unlisted_dependencies",
        "dev_in_production_dependencies",
    ] {
        assert!(
            rule_ids.contains(expected_ts_rule),
            "expected TypeScript {expected_ts_rule} finding in mixed-repo SARIF; saw {rule_ids:?}"
        );
    }
}

#[test]
fn no_imports_only_repo_omits_import_metrics_in_markdown_and_has_no_import_findings_in_sarif() {
    let outputs =
        run_no_imports_only_repo(serde_json::json!({ "other": { "patterns": ["other/**"] } }));

    assert_markdown_omits_import_metrics_with_reason(
        &outputs.markdown,
        "top-level no-imports-only repo",
    );
    assert_sarif_has_no_import_findings_for_extension(
        &outputs.sarif,
        ".noimp",
        "no-imports-only repo",
    );
    assert!(
        outputs.sarif["runs"][0]["results"]
            .as_array()
            .expect("SARIF results")
            .is_empty(),
        "no-imports-only repo with clean functions must have zero SARIF results: {}",
        outputs.sarif
    );
}
