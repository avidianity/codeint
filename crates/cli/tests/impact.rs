use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const GIT_ENV: [(&str, &str); 6] = [
    ("GIT_CONFIG_GLOBAL", "/dev/null"),
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_AUTHOR_NAME", "ImpactTest"),
    ("GIT_AUTHOR_EMAIL", "impact@example.invalid"),
    ("GIT_COMMITTER_NAME", "ImpactTest"),
    ("GIT_COMMITTER_EMAIL", "impact@example.invalid"),
];

struct RepoFixture {
    dir: TempDir,
}

impl RepoFixture {
    fn new() -> RepoFixture {
        let fixture = RepoFixture {
            dir: tempfile::tempdir().expect("tempdir"),
        };
        fixture.git(&["init", "--quiet", "--initial-branch", "main"]);
        fixture
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, relative: &str, content: &str) {
        let full = self.root().join(relative);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(full, content).expect("write file");
    }

    fn git(&self, args: &[&str]) -> String {
        let mut cmd = Command::new("git");
        cmd.args(args).current_dir(self.root());
        for (k, v) in GIT_ENV {
            cmd.env(k, v);
        }
        let output = cmd.output().expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("utf8")
    }

    fn commit(&self, msg: &str) {
        self.git(&["add", "--all"]);
        self.git(&["commit", "--quiet", "--message", msg]);
    }

    fn run_impact(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_codeint"))
            .arg("impact")
            .args(args)
            .current_dir(self.root())
            .output()
            .expect("run codeint impact")
    }

    fn impact_json(&self, args: &[&str]) -> Value {
        let mut full_args = args.to_vec();
        full_args.extend(["--format", "json"]);
        let output = self.run_impact(&full_args);
        assert_eq!(
            output.status.code(),
            Some(0),
            "codeint impact failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("valid JSON")
    }
}

fn dependent_paths_and_distances(list: &Value) -> Vec<(String, u64)> {
    list.as_array()
        .expect("array")
        .iter()
        .map(|item| {
            (
                item["path"].as_str().expect("path").to_owned(),
                item["distance"].as_u64().expect("distance"),
            )
        })
        .collect()
}

#[test]
fn impact_reports_direct_and_transitive_dependents_separating_production_and_test() {
    let repo = RepoFixture::new();
    repo.write("src/leaf.ts", "export const leaf = 1;\n");
    repo.write(
        "src/mid.ts",
        "import { leaf } from './leaf';\nexport const mid = leaf + 1;\n",
    );
    repo.write(
        "src/app.ts",
        "import { mid } from './mid';\nexport const app = mid + 1;\n",
    );
    repo.write("src/unrelated.ts", "export const unrelated = 0;\n");
    repo.write(
        "src/leaf.test.ts",
        "import { leaf } from './leaf';\nexport const testLeaf = leaf;\n",
    );
    repo.write(
        "src/mid.test.ts",
        "import { mid } from './mid';\nexport const testMid = mid;\n",
    );
    repo.commit("initial");

    let report = repo.impact_json(&["src/leaf.ts"]);

    assert_eq!(report["targets"].as_array().expect("targets").len(), 1);
    let target = &report["targets"][0];
    assert_eq!(target["path"], "src/leaf.ts");
    assert_eq!(target["source_set"], "production");
    assert_eq!(target["fan_in"], 2);
    assert_eq!(target["fan_out"], 0);
    assert_eq!(target["transitive_import_weight"], 4);

    assert_eq!(report["summary"]["direct_dependents"], 2);
    assert_eq!(report["summary"]["transitive_dependents"], 2);
    assert_eq!(report["summary"]["total_dependents"], 4);
    assert_eq!(report["summary"]["fan_in"], 2);
    assert_eq!(report["summary"]["fan_out"], 0);
    assert_eq!(report["summary"]["transitive_import_weight"], 4);

    assert_eq!(
        dependent_paths_and_distances(&report["production"]["direct"]),
        vec![("src/mid.ts".to_owned(), 1)]
    );
    assert_eq!(
        dependent_paths_and_distances(&report["production"]["transitive"]),
        vec![("src/app.ts".to_owned(), 2)]
    );
    assert_eq!(
        dependent_paths_and_distances(&report["test"]["direct"]),
        vec![("src/leaf.test.ts".to_owned(), 1)]
    );
    assert_eq!(
        dependent_paths_and_distances(&report["test"]["transitive"]),
        vec![("src/mid.test.ts".to_owned(), 2)]
    );
}

#[test]
fn impact_supports_depth_cap_and_handles_import_cycles_without_looping() {
    let repo = RepoFixture::new();
    repo.write(
        "src/a.ts",
        "import { c } from './c';\nexport const a = () => c;\n",
    );
    repo.write(
        "src/b.ts",
        "import { a } from './a';\nexport const b = () => a;\n",
    );
    repo.write(
        "src/c.ts",
        "import { b } from './b';\nexport const c = () => b;\n",
    );
    repo.write(
        "src/entry.ts",
        "import { c } from './c';\nexport const entry = () => c;\n",
    );
    repo.commit("cycle");

    let full = repo.impact_json(&["src/a.ts"]);
    assert_eq!(
        dependent_paths_and_distances(&full["production"]["direct"]),
        vec![("src/b.ts".to_owned(), 1)]
    );
    assert_eq!(
        dependent_paths_and_distances(&full["production"]["transitive"]),
        vec![("src/c.ts".to_owned(), 2), ("src/entry.ts".to_owned(), 3),]
    );
    assert_eq!(full["targets"][0]["fan_in"], 1);
    assert_eq!(full["targets"][0]["fan_out"], 1);
    assert_eq!(full["targets"][0]["transitive_import_weight"], 3);

    let capped = repo.impact_json(&["src/a.ts", "--depth", "1"]);
    assert_eq!(capped["depth_limit"], 1);
    assert_eq!(
        dependent_paths_and_distances(&capped["production"]["direct"]),
        vec![("src/b.ts".to_owned(), 1)]
    );
    assert_eq!(
        dependent_paths_and_distances(&capped["production"]["transitive"]),
        Vec::<(String, u64)>::new()
    );
    assert_eq!(capped["summary"]["total_dependents"], 1);
}

#[test]
fn impact_supports_multiple_target_paths_with_deterministic_ordering() {
    let repo = RepoFixture::new();
    repo.write("src/a.ts", "export const a = 1;\n");
    repo.write("src/b.ts", "export const b = 2;\n");
    repo.write(
        "src/z_direct.ts",
        "import { a } from './a';\nexport const z = a;\n",
    );
    repo.write(
        "src/c_direct.ts",
        "import { b } from './b';\nexport const c = b;\n",
    );
    repo.write(
        "src/top.ts",
        "import { c } from './c_direct';\nimport { z } from './z_direct';\nexport const top = c + z;\n",
    );
    repo.commit("multi");

    let report = repo.impact_json(&["src/b.ts", "src/a.ts"]);
    let target_paths: Vec<&str> = report["targets"]
        .as_array()
        .expect("targets")
        .iter()
        .map(|t| t["path"].as_str().expect("path"))
        .collect();
    assert_eq!(target_paths, vec!["src/a.ts", "src/b.ts"]);
    assert_eq!(
        dependent_paths_and_distances(&report["production"]["direct"]),
        vec![
            ("src/c_direct.ts".to_owned(), 1),
            ("src/z_direct.ts".to_owned(), 1),
        ]
    );
    assert_eq!(
        dependent_paths_and_distances(&report["production"]["transitive"]),
        vec![("src/top.ts".to_owned(), 2)]
    );
}

#[test]
fn impact_supports_changed_base_using_git_merge_base() {
    let repo = RepoFixture::new();
    repo.write("src/core.ts", "export const core = 1;\n");
    repo.write(
        "src/service.ts",
        "import { core } from './core';\nexport const service = core + 1;\n",
    );
    repo.write("src/other.ts", "export const other = 1;\n");
    repo.commit("base");
    let merge_base = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();

    repo.git(&["switch", "--quiet", "--create", "feature"]);
    repo.write("src/core.ts", "export const core = 42;\n");
    repo.write("README.md", "# docs change\n");
    repo.commit("feature change");

    repo.git(&["switch", "--quiet", "main"]);
    repo.write("src/other.ts", "export const other = 99;\n");
    repo.commit("main advance");

    repo.git(&["switch", "--quiet", "feature"]);

    let report = repo.impact_json(&["--changed", "main"]);
    assert_eq!(report["changed_base"]["ref"], "main");
    assert_eq!(report["changed_base"]["merge_base"], merge_base);
    let target_paths: Vec<&str> = report["targets"]
        .as_array()
        .expect("targets")
        .iter()
        .map(|t| t["path"].as_str().expect("path"))
        .collect();
    assert_eq!(target_paths, vec!["src/core.ts"]);
    assert_eq!(
        dependent_paths_and_distances(&report["production"]["direct"]),
        vec![("src/service.ts".to_owned(), 1)]
    );
}

#[test]
fn impact_lists_affected_workspace_packages_in_monorepo() {
    let repo = RepoFixture::new();
    repo.write(
        "package.json",
        r#"{ "name": "mono", "private": true, "workspaces": ["packages/*"] }"#,
    );
    repo.write(
        "packages/core/package.json",
        r#"{ "name": "@mono/core", "main": "src/index.ts" }"#,
    );
    repo.write("packages/core/src/index.ts", "export const coreVal = 1;\n");
    repo.write(
        "packages/api/package.json",
        r#"{ "name": "@mono/api", "main": "src/index.ts" }"#,
    );
    repo.write(
        "packages/api/src/index.ts",
        "import { coreVal } from '../../core/src/index';\nexport const apiVal = coreVal + 1;\n",
    );
    repo.write(
        "packages/web/package.json",
        r#"{ "name": "@mono/web", "main": "src/index.ts" }"#,
    );
    repo.write(
        "packages/web/src/index.ts",
        "import { apiVal } from '../../api/src/index';\nexport const webVal = apiVal + 1;\n",
    );
    repo.commit("monorepo");

    let report = repo.impact_json(&["packages/core/src/index.ts"]);
    let affected: Vec<&str> = report["affected_packages"]
        .as_array()
        .expect("affected_packages")
        .iter()
        .map(|v| v.as_str().expect("pkg"))
        .collect();
    assert_eq!(affected, vec!["packages/api", "packages/web"]);
    assert_eq!(report["targets"][0]["package"], "packages/core");
}

#[test]
fn impact_renders_text_and_toon_formats() {
    let repo = RepoFixture::new();
    repo.write("src/util.ts", "export const util = 1;\n");
    repo.write(
        "src/app.ts",
        "import { util } from './util';\nexport const app = util;\n",
    );
    repo.write(
        "src/util.test.ts",
        "import { util } from './util';\nexport const t = util;\n",
    );
    repo.commit("formats");

    let text_out = repo.run_impact(&["src/util.ts", "--format", "text"]);
    assert_eq!(text_out.status.code(), Some(0));
    let text = String::from_utf8(text_out.stdout).expect("utf8");
    assert!(text.contains("Targets (1):"), "missing targets: {text}");
    assert!(text.contains("src/util.ts"), "missing target path: {text}");
    assert!(
        text.contains("Production dependents (1):"),
        "missing production section: {text}"
    );
    assert!(text.contains("src/app.ts"), "missing prod dep: {text}");
    assert!(
        text.contains("Test dependents (1):"),
        "missing test section: {text}"
    );
    assert!(
        text.contains("src/util.test.ts"),
        "missing test dep: {text}"
    );

    let toon_out = repo.run_impact(&["src/util.ts", "--format", "toon"]);
    assert_eq!(toon_out.status.code(), Some(0));
    let toon_str = String::from_utf8(toon_out.stdout).expect("utf8");
    let decoded: Value = toon_format::decode_default(&toon_str).expect("valid TOON");
    assert_eq!(decoded["summary"]["total_dependents"], 2);
}

#[test]
fn impact_errors_when_target_is_not_in_measured_code_model() {
    let repo = RepoFixture::new();
    repo.write("src/real.ts", "export const real = 1;\n");
    repo.commit("initial");

    let missing = repo.run_impact(&["src/missing.ts"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(missing.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&missing.stderr);
    assert!(
        stderr.contains("src/missing.ts is not in the measured Code model"),
        "unexpected stderr: {stderr}"
    );

    let outside = repo.run_impact(&["../outside.ts"]);
    assert_eq!(outside.status.code(), Some(2));
    let outside_err = String::from_utf8_lossy(&outside.stderr);
    assert!(
        outside_err.contains("outside the Git repository"),
        "unexpected stderr: {outside_err}"
    );
}
