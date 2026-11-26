mod wasm_fixtures;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use codeint_plugin_host::{sha256, sha256_hex};
use serde_json::{Value, json};
use tempfile::TempDir;

const MONOREPO_ROOT_PKG: &str = r#"{"name":"mono","private":true,"workspaces":["packages/*"]}"#;
const VUE_PKG: &str = r#"{"name":"@mono/web","dependencies":{"vue":"^3.4.0"}}"#;
const REACT_PKG: &str =
    r#"{"name":"@mono/react-app","dependencies":{"react":"^18.3.1","react-dom":"^18.3.1"}}"#;
const SIMPLE_APP_TSX: &str = "export function App() { return <div>ok</div>; }\n";
const VUE_APP_WITH_HELPER: &str = "<script setup lang=\"ts\">\nimport { helper } from '@/helper';\nexport const v = helper();\n</script>\n";

fn write_config(dir: &Path, body: &str) {
    fs::write(dir.join("codeint.json"), body).expect("write codeint.json");
}

fn run_codeint(dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeint"));
    command.current_dir(dir);
    command.args(args);
    command.output().expect("codeint")
}

fn check_format_text(dir: &Path, format: &str) -> String {
    let out = run_codeint(dir, &["check", "--format", format]);
    assert!(out.status.success());
    String::from_utf8(out.stdout).expect("utf8")
}

fn run_json(dir: &Path, args: &[&str]) -> Value {
    let mut cli_args = Vec::with_capacity(args.len() + 2);
    cli_args.extend_from_slice(args);
    cli_args.push("--format");
    cli_args.push("json");
    let out = run_codeint(dir, &cli_args);
    assert!(out.status.success());
    serde_json::from_slice(&out.stdout).expect("json")
}

fn init_repo(files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    for (rel, body) in files {
        let full = dir.path().join(rel);
        fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
        fs::write(full, body).expect("write");
    }
    let init = Command::new("git")
        .args(["init", "-q", "-b", "main"])
        .current_dir(dir.path())
        .envs([
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_CONFIG_NOSYSTEM", "1"),
        ])
        .status()
        .expect("git init");
    assert!(init.success());
    dir
}

fn init_sample_mini_repo(extra_config: Value, files: &[(&str, &str)]) -> (TempDir, Value) {
    let wasm_path = wasm_fixtures::wasm_fixture_path("sample_mini_plugin.wasm");
    let wasm_bytes = fs::read(&wasm_path).expect("read sample wasm");
    let hash = sha256_hex(&sha256(&wasm_bytes));
    let plugin_entry = json!({
        "name": "sample-mini",
        "kind": "language",
        "path": "plugins/sample.wasm",
        "sha256": hash,
    });
    let mut config = extra_config;
    config
        .as_object_mut()
        .expect("config object")
        .insert("plugins".to_owned(), json!([plugin_entry.clone()]));
    let config_text = serde_json::to_string_pretty(&config).expect("serialize config");
    let mut all_files = vec![("codeint.json", config_text.as_str())];
    all_files.extend_from_slice(files);
    let repo = init_repo(&all_files);
    fs::create_dir_all(repo.path().join("plugins")).expect("create plugins dir");
    fs::write(repo.path().join("plugins/sample.wasm"), &wasm_bytes).expect("write wasm");
    (repo, plugin_entry)
}

fn render_saved_text(dir: &Path, report: &Value, file_name: &str) -> String {
    let saved_path = dir.join(file_name);
    fs::write(
        &saved_path,
        serde_json::to_string(report).expect("serialize report"),
    )
    .expect("write saved report");
    let output = run_codeint(
        dir,
        &[
            "report",
            saved_path.to_str().expect("path"),
            "--format",
            "text",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    String::from_utf8(output.stdout).expect("utf8 text")
}

fn assert_exit_2_stderr(dir: &Path, args: &[&str], needles: &[&str]) {
    let output = run_codeint(dir, args);
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2 for {args:?}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    for needle in needles {
        assert!(
            stderr.contains(needle),
            "expected {needle:?} in stderr:\n{stderr}"
        );
    }
}

fn assert_zero_unresolved(report: &Value) {
    let count = report
        .pointer("/source_sets/production/import_cycles/unresolved_imports/count")
        .and_then(Value::as_u64);
    assert_eq!(count, Some(0));
}

fn assert_source_set_files(report: &Value, source_set: &str, expected: u64) {
    let pointer = format!("/source_sets/{source_set}/files");
    assert_eq!(report.pointer(&pointer), Some(&json!(expected)));
}

fn orphan_paths(report: &Value) -> Vec<&str> {
    report
        .pointer("/source_sets/production/orphan_files/findings")
        .and_then(Value::as_array)
        .expect("orphan findings")
        .iter()
        .filter_map(|item| item["path"].as_str())
        .collect()
}

fn assert_erosion_source_contains(report: &Value, needle: &str) {
    let source = report
        .pointer("/source_sets/production/erosion/reference_range/source")
        .and_then(Value::as_str)
        .expect("erosion source");
    assert!(source.contains(needle), "expected {needle} in {source}");
}

fn package_entry<'a>(report: &'a Value, path: &str) -> &'a Value {
    report
        .pointer("/source_sets/production/packages")
        .and_then(Value::as_array)
        .expect("packages")
        .iter()
        .find(|pkg| pkg["path"] == path)
        .unwrap_or_else(|| panic!("missing package {path}"))
}

fn assert_only_web_profile_active(report: &Value) {
    assert_eq!(report["framework_profiles"], json!(["vue3"]));
    assert_eq!(
        package_entry(report, "packages/web")["framework_profiles"],
        json!(["vue3"])
    );
    assert!(
        package_entry(report, "packages/react-app")
            .get("framework_profiles")
            .is_none()
    );
}

fn mini_fn_body(name: &str) -> String {
    format!("fn {name}() {{\n  return 1\n}}\n")
}

fn init_monorepo_with_files(extra_files: &[(&str, &str)]) -> TempDir {
    let mut files = vec![
        ("package.json", MONOREPO_ROOT_PKG),
        ("packages/web/package.json", VUE_PKG),
        ("packages/react-app/package.json", REACT_PKG),
    ];
    files.extend_from_slice(extra_files);
    init_repo(&files)
}

#[test]
fn framework_profiles_wire_entry_points_import_hints_and_report_visibility_across_formats() {
    let repo = init_monorepo_with_files(&[
        (
            "packages/web/src/main.ts",
            "import App from '@/App.vue';\nexport const mount = () => App;\n",
        ),
        ("packages/web/src/App.vue", VUE_APP_WITH_HELPER),
        (
            "packages/web/src/helper.ts",
            "export function helper(): number { return 1; }\n",
        ),
        (
            "packages/web/src/orphan.ts",
            "export function unused(): number { return 2; }\n",
        ),
        ("packages/react-app/index.html", "<html></html>\n"),
        (
            "packages/react-app/src/main.tsx",
            "import { App } from './App';\nexport const root = () => <App />;\n",
        ),
        ("packages/react-app/src/App.tsx", SIMPLE_APP_TSX),
        (
            "packages/api/package.json",
            r#"{"name":"@mono/api","main":"src/index.ts"}"#,
        ),
        ("packages/api/src/index.ts", "export const ok = true;\n"),
    ]);

    let report = run_json(repo.path(), &["check"]);
    assert_eq!(report["framework_profiles"], json!(["react", "vue3"]));
    assert_zero_unresolved(&report);
    assert_eq!(orphan_paths(&report), vec!["packages/web/src/orphan.ts"]);

    assert_eq!(
        package_entry(&report, "packages/web")["framework_profiles"],
        json!(["vue3"])
    );
    assert_eq!(
        package_entry(&report, "packages/react-app")["framework_profiles"],
        json!(["react"])
    );
    assert!(
        package_entry(&report, "packages/api")
            .get("framework_profiles")
            .is_none()
    );

    let web_scoped = run_json(repo.path(), &["check", "--package", "packages/web"]);
    assert_eq!(web_scoped["framework_profiles"], json!(["vue3"]));
    assert_erosion_source_contains(&web_scoped, "20 Vue 3 repositories");

    let toon = check_format_text(repo.path(), "toon");
    assert!(toon.contains("framework_profiles"), "{toon}");

    let md = check_format_text(repo.path(), "markdown");
    assert!(md.contains("Framework profiles: `react`, `vue3`."), "{md}");
    assert!(md.contains("| Framework profiles |"), "{md}");

    let text = render_saved_text(repo.path(), &report, "saved-check.json");
    assert!(text.contains("framework_profiles"), "{text}");
}

#[test]
fn disabling_framework_profile_disables_fallback_import_hints_and_conventions() {
    let repo = init_repo(&[
        ("codeint.json", r#"{ "framework_profiles": false }"#),
        ("package.json", VUE_PKG),
        (
            "src/main.ts",
            "import { helper } from '@/helper';\nexport const run = () => helper();\n",
        ),
        ("src/helper.ts", "export const helper = (): number => 1;\n"),
        (
            "src/App.vue",
            "<script setup lang=\"ts\">\nconst x = 1;\n</script>\n",
        ),
    ]);

    let report = run_json(repo.path(), &["check"]);
    assert!(report.get("framework_profiles").is_none());
    assert_erosion_source_contains(&report, "50 TypeScript and JavaScript repositories");
    let unlisted = report["dependencies"]["unlisted_dependencies"]["findings"]
        .as_array()
        .expect("unlisted");
    assert!(
        unlisted.iter().any(|item| item["dependency"] == "@/helper"),
        "expected @/helper fallback hint to be disabled: {report}"
    );
}

fn init_sample_mini_with_fns(
    extra_config: Value,
    workspace: &str,
    pages: &[(&str, &str)],
    plain: &[(&str, &str)],
) -> (TempDir, Value) {
    let mut owned: Vec<(&str, String)> = vec![("mini.workspace", workspace.to_owned())];
    for &(path, fn_name) in pages {
        owned.push((path, format!("import \"@/lib\"\n{}", mini_fn_body(fn_name))));
    }
    for &(path, fn_name) in plain {
        owned.push((path, mini_fn_body(fn_name)));
    }
    let refs: Vec<(&str, &str)> = owned.iter().map(|(p, b)| (*p, b.as_str())).collect();
    init_sample_mini_repo(extra_config, &refs)
}

#[test]
fn source_set_priority_respects_vendored_and_explicit_overrides_over_framework_generated_paths() {
    let (repo, _) = init_sample_mini_with_fns(
        json!({ "source_sets": { "production": ["gen/forced_prod.mini"] } }),
        "framework:mini-web\n",
        &[("pages/index.mini", "home")],
        &[
            ("src/lib.mini", "lib"),
            ("gen/auto.mini", "generated_fn"),
            ("gen/forced_prod.mini", "forced_prod_fn"),
            ("vendor/gen/vendored.mini", "vendored_fn"),
        ],
    );

    let mini_report = run_json(repo.path(), &["check"]);
    assert_eq!(mini_report["framework_profiles"], json!(["mini-web"]));
    assert_source_set_files(&mini_report, "production", 3);
    assert_source_set_files(&mini_report, "generated", 1);
    assert_source_set_files(&mini_report, "vendored", 1);
    assert_zero_unresolved(&mini_report);
}

#[test]
fn unknown_framework_profile_fails_fast_with_exit_code_2_and_remedy() {
    let repo = init_repo(&[
        ("codeint.json", r#"{ "framework_profiles": ["reacct"] }"#),
        ("src/index.ts", "export const x = 1;\n"),
    ]);

    for args in [&["check"][..], &["config", "--resolved"][..]] {
        assert_exit_2_stderr(repo.path(), args, &["reacct", "react", "codeint schema"]);
    }
}

#[test]
fn sibling_framework_conflict_is_order_independent_and_emits_reference_range_note_across_formats() {
    let build_repo = |deps: &str| {
        init_repo(&[
            (
                "package.json",
                &format!(r#"{{"name":"hybrid","dependencies":{deps}}}"#),
            ),
            ("index.html", "<html></html>\n"),
            ("vite.config.ts", "export default {};\n"),
            (
                "src/main.tsx",
                "import { Widget } from './Widget';\nexport const mount = () => <Widget />;\n",
            ),
            (
                "src/Widget.tsx",
                "export function Widget() { return <div>ok</div>; }\n",
            ),
            ("src/App.vue", VUE_APP_WITH_HELPER),
            ("src/helper.ts", "export const helper = (): number => 1;\n"),
            ("components.d.ts", "export {};\n"),
        ])
    };
    let repo_a = build_repo(r#"{"react":"^18.3.1","react-dom":"^18.3.1","vue":"^3.4.0"}"#);
    let repo_b = build_repo(r#"{"vue":"^3.4.0","react-dom":"^18.3.1","react":"^18.3.1"}"#);

    let report_a = run_json(repo_a.path(), &["check"]);
    let report_b = run_json(repo_b.path(), &["check"]);

    assert_eq!(report_a["framework_profiles"], json!(["react", "vue3"]));
    assert_eq!(report_b["framework_profiles"], json!(["react", "vue3"]));
    assert_erosion_source_contains(&report_a, "50 TypeScript and JavaScript repositories");
    assert_erosion_source_contains(&report_b, "50 TypeScript and JavaScript repositories");
    assert_zero_unresolved(&report_a);
    assert_zero_unresolved(&report_b);

    let note_a = report_a["reference_range_note"]
        .as_str()
        .expect("top-level reference_range_note");
    let note_b = report_b["reference_range_note"]
        .as_str()
        .expect("top-level reference_range_note");
    assert_eq!(note_a, note_b);
    assert!(
        note_a.contains("react") && note_a.contains("vue3") && note_a.contains("falling back"),
        "{note_a}"
    );

    let toon = check_format_text(repo_a.path(), "toon");
    assert!(toon.contains("reference_range_note:"), "{toon}");
    assert!(toon.contains("falling back"), "{toon}");

    let md = check_format_text(repo_a.path(), "markdown");
    assert!(md.contains(note_a), "{md}");

    let text = render_saved_text(repo_a.path(), &report_a, "saved-conflict.json");
    assert!(text.contains("reference_range_note:"), "{text}");

    fs::write(
        repo_a.path().join("src/extra.rs"),
        "pub fn ok() -> i32 { 1 }\n",
    )
    .expect("write rust file");
    let multi = run_json(repo_a.path(), &["check"]);
    assert_eq!(multi["reference_range_note"].as_str(), Some(note_a));
    assert_eq!(
        multi["languages"]["typescript-javascript"]["reference_range_note"].as_str(),
        Some(note_a)
    );
    assert!(
        multi["languages"]["rust"]
            .get("reference_range_note")
            .is_none()
    );
}

#[test]
fn parent_child_framework_profiles_let_child_win_reference_ranges_while_unioning_conventions() {
    let (repo, _) = init_sample_mini_with_fns(
        json!({}),
        "framework:mini-web\nframework:mini-app\n",
        &[("pages/index.mini", "home"), ("routes/about.mini", "about")],
        &[
            ("src/lib.mini", "lib"),
            ("gen/parent.mini", "parent_gen"),
            ("dist-gen/child.mini", "child_gen"),
        ],
    );

    let report = run_json(repo.path(), &["check"]);
    assert_eq!(
        report["framework_profiles"],
        json!(["mini-app", "mini-web"])
    );
    assert_erosion_source_contains(&report, "mini-app framework profile");
    assert!(report.get("reference_range_note").is_none());
    assert_source_set_files(&report, "production", 3);
    assert_source_set_files(&report, "generated", 2);
    assert!(orphan_paths(&report).is_empty());
    assert_zero_unresolved(&report);
}

fn init_monorepo_for_overrides(initial_config: &str) -> TempDir {
    init_monorepo_with_files(&[
        (
            "packages/web/src/App.vue",
            "<script setup lang=\"ts\">\nexport const x = 1;\n</script>\n",
        ),
        ("packages/react-app/src/App.tsx", SIMPLE_APP_TSX),
        ("codeint.json", initial_config),
    ])
}

#[test]
fn per_package_framework_profile_overrides_and_validation_work_across_builtin_and_external_plugins()
{
    let repo = init_monorepo_for_overrides(
        r#"{ "framework_profiles": false, "packages": { "packages/web": { "framework_profiles": true } } }"#,
    );
    assert_only_web_profile_active(&run_json(repo.path(), &["check"]));

    write_config(
        repo.path(),
        r#"{ "framework_profiles": { "exclude": ["vue3"] }, "packages": { "packages/web": { "framework_profiles": { "only": ["vue3"] } }, "packages/react-app": { "framework_profiles": false } } }"#,
    );
    assert_only_web_profile_active(&run_json(repo.path(), &["check"]));

    let rejection_cases: [(&str, &[&str]); 2] = [
        (
            r#"{ "packages": { "packages/web": { "framework_profiles": ["reacct"] } } }"#,
            &["reacct", "react"],
        ),
        (
            r#"{ "packages": { "packages/web": { "framework_profiles": { "unknown": ["react"] } } } }"#,
            &["unknown", "exclude", "only"],
        ),
    ];
    for (config_body, needles) in rejection_cases {
        write_config(repo.path(), config_body);
        assert_exit_2_stderr(repo.path(), &["check"], needles);
    }

    let home = mini_fn_body("home");
    let (ext_repo, plugin_entry) = init_sample_mini_repo(
        json!({ "framework_profiles": ["mini-web"] }),
        &[("mini.workspace", ""), ("pages/index.mini", &home)],
    );
    let ext_report = run_json(ext_repo.path(), &["check"]);
    assert_eq!(ext_report["framework_profiles"], json!(["mini-web"]));

    let bad_ext_config = json!({
        "framework_profiles": ["mini-typo"],
        "plugins": [plugin_entry],
    });
    write_config(
        ext_repo.path(),
        &serde_json::to_string_pretty(&bad_ext_config).expect("serialize bad config"),
    );
    assert_exit_2_stderr(ext_repo.path(), &["check"], &["mini-typo", "mini-web"]);
}
