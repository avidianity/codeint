use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};

use crate::common::{init_git_repo, mcp_payload, mcp_tool_call, send_mcp_requests};

const DEP_METRICS: [&str; 3] = [
    "unused_dependencies",
    "unlisted_dependencies",
    "dev_in_production_dependencies",
];

const VITEST_SUITE: &str = "import { describe } from 'vitest';\nexport const suite = describe;\n";

fn write_repo_file(root: &Path, rel: &str, body: &str) {
    let target = root.join(rel);
    target.parent().map(fs::create_dir_all).transpose().unwrap();
    fs::write(target, body).unwrap();
}

fn write_config(root: &Path, body: &str) {
    write_repo_file(root, "codeint.json", body);
}

fn run_codeint(root: &Path, cache: Option<&Path>, args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_codeint"));
    cmd.args(args).current_dir(root);
    if let Some(cache_dir) = cache {
        cmd.env("CODEINT_CACHE_DIR", cache_dir);
    }
    cmd.output().expect("execute codeint")
}

fn expect_stdout(root: &Path, cache: Option<&Path>, args: &[&str], code: i32) -> String {
    let out = run_codeint(root, cache, args);
    assert_eq!(
        out.status.code(),
        Some(code),
        "codeint {args:?} stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("utf8 stdout")
}

fn expect_json(root: &Path, cache: Option<&Path>, args: &[&str], code: i32) -> Value {
    serde_json::from_str(&expect_stdout(root, cache, args, code)).expect("parse JSON")
}

fn diff_json(root: &Path, cache: &Path, code: i32) -> Value {
    expect_json(
        root,
        Some(cache),
        &["diff", "main", "--format", "json"],
        code,
    )
}

fn dep_names(deps: &Value, metric: &str) -> Vec<String> {
    deps[metric]["findings"]
        .as_array()
        .expect("findings list")
        .iter()
        .map(|entry| entry["dependency"].as_str().expect("dep name").to_owned())
        .collect()
}

fn assert_all_dep_metrics_empty(deps: &Value) {
    for metric in DEP_METRICS {
        assert!(dep_names(deps, metric).is_empty());
    }
}

fn assert_language_hygiene(
    files: &[(&str, &str)],
    expected_unused: &[&str],
    expected_unlisted: &[&str],
    expected_dev_in_prod: &[&str],
) {
    let repo = init_git_repo(files);
    let report = expect_json(repo.path(), None, &["check", "--format", "json"], 0);
    let deps = &report["dependencies"];
    assert_eq!(dep_names(deps, "unused_dependencies"), expected_unused);
    assert_eq!(dep_names(deps, "unlisted_dependencies"), expected_unlisted);
    assert_eq!(
        dep_names(deps, "dev_in_production_dependencies"),
        expected_dev_in_prod
    );
}

#[test]
fn typescript_monorepo_reports_dependency_hygiene_across_all_formats_ignore_and_package_filter() {
    let root_manifest = json!({
        "name": "hygiene-workspace",
        "private": true,
        "workspaces": ["libs/*", "apps/*"],
    })
    .to_string();
    let shared_manifest = json!({
        "name": "@hygiene/shared",
        "main": "src/index.ts",
        "dependencies": { "zod": "^3.0.0" },
    })
    .to_string();
    let api_manifest = json!({
        "name": "@hygiene/api",
        "dependencies": { "zod": "^3.0.0", "moment": "^2.30.0" },
        "devDependencies": { "vitest": "^1.0.0", "faker": "^8.0.0", "eslint": "^9.0.0" },
    })
    .to_string();
    let repo = init_git_repo(&[
        ("package.json", &root_manifest),
        ("libs/shared/package.json", &shared_manifest),
        (
            "libs/shared/src/index.ts",
            "import { z } from 'zod';\nimport fs from 'node:fs';\nexport const ok = Boolean(z) && Boolean(fs);\n",
        ),
        ("apps/api/package.json", &api_manifest),
        (
            "apps/api/src/server.ts",
            concat!(
                "import fs from 'node:fs';\n",
                "import { ok } from '@hygiene/shared';\n",
                "import { z } from 'zod';\n",
                "import axios from 'axios';\n",
                "import { faker } from 'faker';\n",
                "export const run = () => Boolean(fs && ok && z && axios && faker);\n",
            ),
        ),
        ("apps/api/src/server.test.ts", VITEST_SUITE),
    ]);
    let root = repo.path();

    let json = expect_json(root, None, &["check", "--format", "json"], 0);
    assert!(json.get("score").is_none());
    let deps = &json["dependencies"];
    assert_eq!(dep_names(deps, "unused_dependencies"), ["eslint", "moment"]);
    assert_eq!(dep_names(deps, "unlisted_dependencies"), ["axios"]);
    assert_eq!(dep_names(deps, "dev_in_production_dependencies"), ["faker"]);

    let scoped = expect_json(
        root,
        None,
        &["check", "--package", "libs/shared", "--format", "json"],
        0,
    );
    assert_all_dep_metrics_empty(&scoped["dependencies"]);

    let toon = expect_stdout(root, None, &["check", "--format", "toon"], 0);
    for token in [
        "dependencies:",
        "unused_dependencies:",
        "unlisted_dependencies:",
        "dev_in_production_dependencies:",
    ] {
        assert!(toon.contains(token), "missing {token} in toon");
    }

    let markdown = expect_stdout(root, None, &["check", "--format", "markdown"], 0);
    for snippet in [
        "## Dependencies",
        "## Dependency Findings",
        "### Unused dependencies",
        "### Unlisted dependencies",
        "### Dev dependencies in production",
        "`moment`",
        "`axios`",
        "`faker`",
    ] {
        assert!(markdown.contains(snippet), "missing {snippet} in markdown");
    }

    let sarif = expect_json(root, None, &["check", "--format", "sarif"], 0);
    let sarif_results = sarif["runs"][0]["results"].as_array().expect("results");
    for rule in DEP_METRICS {
        assert!(sarif_results.iter().any(|item| item["ruleId"] == rule));
    }
    let unused_sarif = sarif_results
        .iter()
        .find(|item| item["ruleId"] == "unused_dependencies")
        .expect("unused SARIF");
    assert_eq!(
        unused_sarif["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "apps/api/package.json"
    );

    write_config(
        root,
        r#"{ "dependencies": { "ignore": ["eslint", "moment", "axios", "faker"] } }"#,
    );
    let ignored = expect_json(root, None, &["check", "--format", "json"], 0);
    assert_all_dep_metrics_empty(&ignored["dependencies"]);
}

#[test]
fn rust_python_php_and_dart_manifests_detect_dependency_hygiene_end_to_end() {
    assert_language_hygiene(
        &[
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"crates/core\", \"crates/app\"]\n",
            ),
            (
                "crates/core/Cargo.toml",
                "[package]\nname = \"my-core\"\nversion = \"0.1.0\"\n",
            ),
            ("crates/core/src/lib.rs", "pub fn val() -> u32 { 1 }\n"),
            (
                "crates/app/Cargo.toml",
                concat!(
                    "[package]\nname = \"my-app\"\nversion = \"0.1.0\"\n",
                    "[dependencies]\nmy-core = { path = \"../core\" }\nserde = \"1\"\nunused-crate = \"1\"\n",
                    "[dev-dependencies]\ntempfile = \"3\"\ninsta = \"1\"\n",
                ),
            ),
            (
                "crates/app/src/lib.rs",
                concat!(
                    "use std::collections::HashMap;\nuse my_core::val;\nuse serde::Serialize;\n",
                    "use reqwest::Client;\nuse insta::assert_snapshot;\n",
                    "pub fn run() -> u32 { let _ = (HashMap::<u32, u32>::new(), val(), std::any::type_name::<Client>()); 0 }\n",
                ),
            ),
            (
                "crates/app/tests/integration.rs",
                "use tempfile::TempDir;\npub fn helper() -> Option<TempDir> { None }\n",
            ),
        ],
        &["unused_crate"],
        &["reqwest"],
        &["insta"],
    );

    assert_language_hygiene(
        &[
            (
                "pyproject.toml",
                concat!(
                    "[project]\nname = \"demo\"\ndependencies = [\"Scikit_Learn>=1.4\", \"unused-lib>=0.1\"]\n",
                    "[dependency-groups]\ndev = [\"pytest>=8.0\", \"faker>=20.0\"]\n",
                ),
            ),
            (
                "src/app.py",
                "import os\nimport json\nimport scikit_learn\nimport httpx\nimport faker\nVALUE = (os, json, scikit_learn, httpx, faker)\n",
            ),
            ("tests/test_app.py", "import pytest\nTEST_REF = pytest\n"),
        ],
        &["unused-lib"],
        &["httpx"],
        &["faker"],
    );

    assert_language_hygiene(
        &[
            (
                "composer.json",
                r#"{
  "name": "acme/shop",
  "autoload": { "psr-4": { "Acme\\Shop\\": "src/" } },
  "require": { "php": "^8.3", "monolog/monolog": "^3.0", "ramsey/uuid": "^4.0" },
  "require-dev": { "phpunit/phpunit": "^11.0", "mockery/mockery": "^1.6" }
}"#,
            ),
            (
                "src/Order.php",
                concat!(
                    "<?php\nnamespace Acme\\Shop;\nuse DateTime;\nuse Monolog\\Logger;\n",
                    "use GuzzleHttp\\Client;\nuse Mockery\\MockInterface;\n",
                    "class Order { public function run(): void { new DateTime(); new Logger('x'); new Client(); MockInterface::class; } }\n",
                ),
            ),
            (
                "tests/OrderTest.php",
                "<?php\nuse PHPUnit\\Framework\\TestCase;\nclass OrderTest extends TestCase {}\n",
            ),
        ],
        &["ramsey/uuid"],
        &["guzzlehttp/client"],
        &["mockery/mockery"],
    );

    assert_language_hygiene(
        &[
            (
                "pubspec.yaml",
                "name: demo_app\ndependencies:\n  http: ^1.2.0\n  collection: ^1.18.0\ndev_dependencies:\n  test: ^1.25.0\n  mockito: ^5.4.0\n",
            ),
            (
                "lib/app.dart",
                "import 'dart:async';\nimport 'package:http/http.dart' as http;\nimport 'package:dio/dio.dart';\nimport 'package:mockito/mockito.dart';\nFuture<void> run() async { print(http); print(Dio); print(Mock); }\n",
            ),
            (
                "test/app_test.dart",
                "import 'package:test/test.dart';\nvoid main() { test('ok', () {}); }\n",
            ),
        ],
        &["collection"],
        &["dio"],
        &["mockito"],
    );
}

#[test]
fn dependency_budgets_diff_regressions_resolution_and_mcp_work_end_to_end() {
    let cache = tempfile::tempdir().expect("cache");
    let repo = init_git_repo(&[
        (
            "package.json",
            r#"{ "name": "shop", "dependencies": { "zod": "^3.0.0", "legacy-unused": "^1.0.0" }, "devDependencies": { "vitest": "^1.0.0" } }"#,
        ),
        (
            "src/index.ts",
            "import { z } from 'zod';\nexport const ok = Boolean(z);\n",
        ),
        ("src/index.test.ts", VITEST_SUITE),
    ]);
    let root = repo.path();

    write_config(root, r#"{ "budgets": { "unused_dependencies": 0 } }"#);
    let fail_budget = run_codeint(root, None, &["check", "--format", "json"]);
    assert_eq!(fail_budget.status.code(), Some(1));
    let budget_json: Value = serde_json::from_slice(&fail_budget.stdout).expect("json");
    assert_eq!(
        budget_json["over_budget"][0]["metric"],
        "unused_dependencies"
    );
    assert!(
        String::from_utf8_lossy(&fail_budget.stderr)
            .contains("unused_dependencies ratio 0.3333 is over its Budget 0")
    );

    write_config(
        root,
        r#"{ "budgets": { "unused_dependencies": 0.3333, "unlisted_dependencies": 0, "dev_in_production_dependencies": 0 } }"#,
    );
    expect_json(root, None, &["check", "--format", "json"], 0);

    write_config(root, "{}");
    let unchanged = diff_json(root, cache.path(), 0);
    assert_eq!(
        unchanged["dependencies"]["unused_dependencies"]["unchanged"],
        1
    );
    assert!(unchanged.get("regressions").is_none());

    write_repo_file(
        root,
        "src/index.ts",
        "import { z } from 'zod';\nimport axios from 'axios';\nimport { describe } from 'vitest';\nexport const ok = Boolean(z && axios && describe);\n",
    );
    let reg = diff_json(root, cache.path(), 1);
    assert_eq!(
        reg["dependencies"]["unlisted_dependencies"]["new"][0]["dependency"],
        "axios"
    );
    assert_eq!(
        reg["dependencies"]["dev_in_production_dependencies"]["new"][0]["dependency"],
        "vitest"
    );
    let reg_list = reg["regressions"].as_array().expect("regressions");
    assert!(
        reg_list
            .iter()
            .any(|r| r["metric"] == "unlisted_dependencies")
    );
    assert!(
        reg_list
            .iter()
            .any(|r| r["metric"] == "dev_in_production_dependencies")
    );

    let diff_md = expect_stdout(
        root,
        Some(cache.path()),
        &["diff", "main", "--format", "markdown"],
        1,
    );
    assert!(diff_md.contains("## Dependencies"));
    assert!(diff_md.contains("### New Unlisted dependencies Findings"));
    assert!(diff_md.contains("### New Dev dependencies in production Findings"));

    let diff_sarif = expect_json(
        root,
        Some(cache.path()),
        &["diff", "main", "--format", "sarif"],
        1,
    );
    let diff_rules: Vec<&str> = diff_sarif["runs"][0]["results"]
        .as_array()
        .expect("results")
        .iter()
        .map(|r| r["ruleId"].as_str().expect("ruleId"))
        .collect();
    assert!(diff_rules.contains(&"unlisted_dependencies"));
    assert!(diff_rules.contains(&"dev_in_production_dependencies"));
    assert!(!diff_rules.contains(&"unused_dependencies"));

    let mcp_responses = send_mcp_requests(
        root,
        &[
            mcp_tool_call(1, "check", json!({})),
            mcp_tool_call(2, "diff", json!({"base_ref": "main"})),
        ],
    );
    let mcp_check = mcp_payload(&mcp_responses[0]);
    assert_eq!(
        dep_names(
            &mcp_check["report"]["dependencies"],
            "unlisted_dependencies"
        ),
        ["axios"]
    );
    let mcp_diff = mcp_payload(&mcp_responses[1]);
    assert_eq!(mcp_diff["passed"], false);
    assert_eq!(
        mcp_diff["report"]["dependencies"]["unlisted_dependencies"]["new"][0]["dependency"],
        "axios"
    );

    write_config(
        root,
        r#"{ "regression_limits": { "unlisted_dependencies": { "ratio_rise": 1, "new_findings": "never" }, "dev_in_production_dependencies": { "ratio_rise": 1, "new_findings": "never" } } }"#,
    );
    diff_json(root, cache.path(), 0);

    write_config(root, "{}");
    write_repo_file(
        root,
        "package.json",
        r#"{ "name": "shop", "dependencies": { "zod": "^3.0.0" }, "devDependencies": { "vitest": "^1.0.0" } }"#,
    );
    write_repo_file(
        root,
        "src/index.ts",
        "import { z } from 'zod';\nexport const ok = Boolean(z);\n",
    );
    let resolved = diff_json(root, cache.path(), 0);
    assert_eq!(
        resolved["dependencies"]["unused_dependencies"]["resolved"][0]["dependency"],
        "legacy-unused"
    );
}

#[test]
fn rust_build_dependencies_php_internal_namespace_and_polyglot_empty_manifests_behave_cleanly() {
    assert_language_hygiene(
        &[
            (
                "Cargo.toml",
                concat!(
                    "[package]\nname = \"build-demo\"\nversion = \"0.1.0\"\n",
                    "[build-dependencies]\ncc = \"1.0\"\n",
                ),
            ),
            (
                "build.rs",
                "use cc::Build;\nfn main() { let _ = Build::new(); }\n",
            ),
            ("src/lib.rs", "pub fn ready() -> bool { true }\n"),
        ],
        &[],
        &[],
        &[],
    );

    assert_language_hygiene(
        &[
            (
                "composer.json",
                r#"{
  "name": "acme/shop",
  "autoload": { "psr-4": { "Acme\\Shop\\": "src/" } },
  "require": { "php": "^8.3", "monolog/monolog": "^3.0" }
}"#,
            ),
            (
                "src/Models/User.php",
                "<?php\nnamespace Acme\\Shop\\Models;\nclass User {}\n",
            ),
            (
                "src/Service.php",
                "<?php\nnamespace Acme\\Shop;\nuse Acme\\Shop\\Models;\nuse Monolog\\Logger;\nclass Service { public function user(): Models\\User { new Logger('shop'); return new Models\\User(); } }\n",
            ),
        ],
        &[],
        &[],
        &[],
    );

    let root_pkg = json!({
        "name": "polyglot-root",
        "private": true,
        "workspaces": ["apps/*"],
    })
    .to_string();
    let web_pkg = json!({
        "name": "@polyglot/web",
        "dependencies": {},
    })
    .to_string();
    let repo = init_git_repo(&[
        ("package.json", &root_pkg),
        ("apps/web/package.json", &web_pkg),
        ("apps/web/src/index.ts", "export const ok = true;\n"),
        (
            "services/ml/pyproject.toml",
            "[project]\nname = \"ml\"\ndependencies = []\n",
        ),
        ("services/ml/app.py", "import requests\nCLIENT = requests\n"),
    ]);
    let report = expect_json(repo.path(), None, &["check", "--format", "json"], 0);
    let unlisted = report["dependencies"]["unlisted_dependencies"]["findings"]
        .as_array()
        .expect("unlisted findings");
    assert_eq!(unlisted.len(), 1);
    assert_eq!(unlisted[0]["dependency"], "requests");
    assert_eq!(unlisted[0]["manifest"], "services/ml/pyproject.toml");
}

#[test]
fn nested_standalone_packages_and_virtual_cargo_workspaces_match_manifests() {
    assert_language_hygiene(
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"root-app\"\nversion = \"0.1.0\"\n[dependencies]\nserde = \"1\"\n",
            ),
            (
                "src/lib.rs",
                "use serde::Serialize;\npub fn marker() -> &'static str { std::any::type_name::<dyn Serialize>() }\n",
            ),
            (
                "tools/helper/Cargo.toml",
                "[package]\nname = \"helper\"\nversion = \"0.1.0\"\n[dependencies]\nanyhow = \"1\"\n",
            ),
            (
                "tools/helper/src/main.rs",
                "use anyhow::Result;\nfn main() -> Result<()> { Ok(()) }\n",
            ),
        ],
        &[],
        &[],
        &[],
    );

    assert_language_hygiene(
        &[
            (
                "package.json",
                r#"{ "name": "root-app", "dependencies": { "zod": "^3.0.0" } }"#,
            ),
            (
                "src/index.ts",
                "import { z } from 'zod';\nexport const ok = Boolean(z);\n",
            ),
            (
                "examples/demo/package.json",
                r#"{ "name": "demo", "dependencies": { "axios": "^1.0.0" } }"#,
            ),
            (
                "examples/demo/src/index.ts",
                "import axios from 'axios';\nexport const client = axios;\n",
            ),
        ],
        &[],
        &[],
        &[],
    );

    assert_language_hygiene(
        &[
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"crates/a\"]\nexclude = [\"examples/standalone\"]\n",
            ),
            (
                "crates/a/Cargo.toml",
                "[package]\nname = \"crate-a\"\nversion = \"0.1.0\"\n[dependencies]\nserde = \"1\"\n",
            ),
            (
                "crates/a/src/lib.rs",
                "use serde::Serialize;\npub fn ok() -> &'static str { std::any::type_name::<dyn Serialize>() }\n",
            ),
            (
                "examples/standalone/Cargo.toml",
                "[package]\nname = \"standalone\"\nversion = \"0.1.0\"\n[dependencies]\nanyhow = \"1\"\n",
            ),
            (
                "examples/standalone/src/main.rs",
                "use anyhow::Result;\nfn main() -> Result<()> { Ok(()) }\n",
            ),
        ],
        &[],
        &[],
        &[],
    );

    let excluded_empty = init_git_repo(&[
        (
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/a\"]\nexclude = [\"examples/standalone\"]\n",
        ),
        (
            "crates/a/Cargo.toml",
            "[package]\nname = \"crate-a\"\nversion = \"0.1.0\"\n",
        ),
        ("crates/a/src/lib.rs", "pub fn ok() -> bool { true }\n"),
        (
            "examples/standalone/Cargo.toml",
            "[package]\nname = \"standalone\"\nversion = \"0.1.0\"\n",
        ),
        (
            "examples/standalone/src/main.rs",
            "use anyhow::Result;\nfn main() -> Result<()> { Ok(()) }\n",
        ),
    ]);
    let excluded_report = expect_json(
        excluded_empty.path(),
        None,
        &["check", "--format", "json"],
        0,
    );
    let excluded_unlisted = excluded_report["dependencies"]["unlisted_dependencies"]["findings"]
        .as_array()
        .expect("unlisted findings");
    assert_eq!(excluded_unlisted.len(), 1);
    assert_eq!(excluded_unlisted[0]["dependency"], "anyhow");
    assert_eq!(
        excluded_unlisted[0]["manifest"],
        "examples/standalone/Cargo.toml"
    );

    assert_language_hygiene(
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"value-shadow\"\nversion = \"0.1.0\"\n[dependencies]\nregex = \"1\"\n",
            ),
            (
                "src/lib.rs",
                concat!(
                    "use serde::Serialize;\n",
                    "use regex::Regex;\n",
                    "pub fn serde() {}\n",
                    "pub fn regex() {}\n",
                ),
            ),
        ],
        &[],
        &["serde"],
        &[],
    );
}

#[test]
fn non_virtual_workspace_roots_without_declared_deps_report_unlisted_imports() {
    let rust_repo = init_git_repo(&[
        (
            "Cargo.toml",
            concat!(
                "[workspace]\nmembers = [\"crates/a\"]\n",
                "[package]\nname = \"root-app\"\nversion = \"0.1.0\"\n",
            ),
        ),
        (
            "src/lib.rs",
            "use anyhow::Result;\npub fn run() -> Result<()> { Ok(()) }\n",
        ),
        (
            "crates/a/Cargo.toml",
            "[package]\nname = \"crate-a\"\nversion = \"0.1.0\"\n",
        ),
        ("crates/a/src/lib.rs", "pub fn ok() -> bool { true }\n"),
    ]);
    let rust_report = expect_json(rust_repo.path(), None, &["check", "--format", "json"], 0);
    let rust_unlisted = rust_report["dependencies"]["unlisted_dependencies"]["findings"]
        .as_array()
        .expect("rust unlisted findings");
    assert_eq!(rust_unlisted.len(), 1);
    assert_eq!(rust_unlisted[0]["dependency"], "anyhow");
    assert_eq!(rust_unlisted[0]["manifest"], "Cargo.toml");

    let root_pkg = json!({
        "name": "ts-workspace-root",
        "private": true,
        "workspaces": ["packages/*"],
    })
    .to_string();
    let member_pkg = json!({
        "name": "@ts-workspace/pkg-a",
        "dependencies": {},
    })
    .to_string();
    let ts_repo = init_git_repo(&[
        ("package.json", &root_pkg),
        (
            "scripts/tool.ts",
            "import chalk from 'chalk';\nexport const paint = (text: string) => chalk.green(text);\n",
        ),
        ("packages/pkg-a/package.json", &member_pkg),
        ("packages/pkg-a/src/index.ts", "export const ok = true;\n"),
    ]);
    let ts_report = expect_json(ts_repo.path(), None, &["check", "--format", "json"], 0);
    let ts_unlisted = ts_report["dependencies"]["unlisted_dependencies"]["findings"]
        .as_array()
        .expect("ts unlisted findings");
    assert_eq!(ts_unlisted.len(), 1);
    assert_eq!(ts_unlisted[0]["dependency"], "chalk");
    assert_eq!(ts_unlisted[0]["manifest"], "package.json");
}
