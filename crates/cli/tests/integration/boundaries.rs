use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use codeint_plugin_host::{sha256, sha256_hex};
use serde_json::Value;
use tempfile::TempDir;

use crate::common::folder_import_plugin_wasm;

const GIT_ENV: [(&str, &str); 6] = [
    ("GIT_CONFIG_GLOBAL", "/dev/null"),
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_AUTHOR_NAME", "Fixture"),
    ("GIT_AUTHOR_EMAIL", "fixture@example.invalid"),
    ("GIT_COMMITTER_NAME", "Fixture"),
    ("GIT_COMMITTER_EMAIL", "fixture@example.invalid"),
];

struct BoundaryFixture(TempDir);

impl BoundaryFixture {
    fn init(files: &[(&str, &str)]) -> BoundaryFixture {
        let dir = tempfile::tempdir().expect("tempdir");
        let init = Command::new("git")
            .args(["init", "--quiet", "--initial-branch", "main"])
            .current_dir(dir.path())
            .status()
            .expect("git init");
        assert!(init.success());
        let fixture = BoundaryFixture(dir);
        for &(rel, body) in files {
            fixture.write(rel, body);
        }
        fixture
    }

    fn write(&self, rel: &str, body: &str) {
        let dst = self.0.path().join(rel);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).expect("mkdir");
        }
        fs::write(dst, body).expect("write");
    }

    fn git(&self, args: &[&str]) {
        let mut command = Command::new("git");
        command.args(args).current_dir(self.0.path());
        for (key, value) in GIT_ENV {
            command.env(key, value);
        }
        let output = command.output().expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn commit(&self, message: &str) {
        self.git(&["add", "--all"]);
        self.git(&["commit", "--quiet", "--message", message]);
    }

    fn run(&self, args: &[&str]) -> Output {
        self.run_with_optional_cache(None, args)
    }

    fn run_with_optional_cache(&self, cache_dir: Option<&Path>, args: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_codeint"));
        command.args(args).current_dir(self.0.path());
        if let Some(cache) = cache_dir {
            command.env("CODEINT_CACHE_DIR", cache);
        }
        command.output().expect("run codeint")
    }

    fn run_stdout(&self, cache_dir: Option<&Path>, args: &[&str], expected_code: i32) -> String {
        let out = self.run_with_optional_cache(cache_dir, args);
        assert_eq!(
            out.status.code(),
            Some(expected_code),
            "stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).expect("utf8")
    }

    fn run_json(&self, cache_dir: Option<&Path>, args: &[&str], expected_code: i32) -> Value {
        let stdout = self.run_stdout(cache_dir, args, expected_code);
        serde_json::from_str(&stdout).expect("valid JSON")
    }

    fn check_json(&self, expected_code: i32) -> Value {
        self.run_json(None, &["check", "--format", "json"], expected_code)
    }

    fn diff_json(&self, cache_dir: &Path, expected_code: i32) -> Value {
        self.run_json(
            Some(cache_dir),
            &["diff", "main", "--format", "json"],
            expected_code,
        )
    }

    fn sarif_boundary_results(
        &self,
        cache_dir: Option<&Path>,
        args: &[&str],
        expected_code: i32,
    ) -> Vec<Value> {
        let sarif = self.run_json(cache_dir, args, expected_code);
        sarif["runs"][0]["results"]
            .as_array()
            .expect("SARIF results")
            .iter()
            .filter(|result| result["ruleId"] == "boundaries")
            .cloned()
            .collect()
    }
}

fn boundary_findings(report: &Value, source_set: &str) -> Vec<Value> {
    report["source_sets"][source_set]["boundaries"]["findings"]
        .as_array()
        .expect("boundaries findings")
        .clone()
}

fn assert_diff_counts(
    diff: &Value,
    expected_new: usize,
    expected_resolved: usize,
    expected_unchanged: u64,
) {
    assert_eq!(diff["new"].as_array().expect("new").len(), expected_new);
    assert_eq!(
        diff["resolved"].as_array().expect("resolved").len(),
        expected_resolved
    );
    assert_eq!(diff["unchanged"], expected_unchanged);
}

#[test]
fn check_reports_directional_deny_allow_and_public_files_findings_across_formats() {
    let fixture = BoundaryFixture::init(&[
        (
            "codeint.json",
            r#"{
  "boundaries": {
    "zones": {
      "domain": {
        "patterns": ["src/domain/**"],
        "public_files": ["src/domain/index.ts"]
      },
      "infra": {
        "patterns": ["src/infra/**"]
      },
      "app": {
        "patterns": ["src/app/**"]
      }
    },
    "rules": [
      {
        "from": "domain",
        "deny": ["infra"],
        "reason": "Domain logic must not depend on infrastructure"
      },
      {
        "from": "app",
        "allow": ["domain"],
        "reason": "App layer may only import domain"
      }
    ]
  }
}
"#,
        ),
        ("src/infra/db.ts", "export const db = 1;\n"),
        (
            "src/domain/internal.ts",
            "import { db } from '../infra/db';\nexport const secret = db;\n",
        ),
        (
            "src/domain/index.ts",
            "import { secret } from './internal';\nexport const api = secret;\n",
        ),
        (
            "src/app/ok.ts",
            "import { api } from '../domain/index';\nexport const ok = api;\n",
        ),
        (
            "src/app/deep.ts",
            "import { secret } from '../domain/internal';\nexport const deep = secret;\n",
        ),
        (
            "src/app/bad_infra.ts",
            "import { db } from '../infra/db';\nexport const bad = db;\n",
        ),
        (
            "src/app/deep.test.ts",
            "import { secret } from '../domain/internal';\nexport const tested = secret;\n",
        ),
    ]);

    let json = fixture.check_json(0);
    assert!(json["source_sets"]["production"]["boundaries"].is_object());
    assert!(json["source_sets"]["test"].get("boundaries").is_none());
    let findings = boundary_findings(&json, "production");
    assert_eq!(findings.len(), 3, "got: {findings:?}");

    let domain_to_infra = findings
        .iter()
        .find(|f| f["importer"] == "src/domain/internal.ts" && f["target"] == "src/infra/db.ts")
        .expect("domain -> infra deny finding");
    assert_eq!(domain_to_infra["kind"], "zone_rule");
    assert_eq!(domain_to_infra["source_zone"], "domain");
    assert_eq!(domain_to_infra["target_zone"], "infra");
    assert_eq!(
        domain_to_infra["reason"],
        "Domain logic must not depend on infrastructure"
    );

    let app_to_infra = findings
        .iter()
        .find(|f| f["importer"] == "src/app/bad_infra.ts" && f["target"] == "src/infra/db.ts")
        .expect("app -> infra allow-violation finding");
    assert_eq!(app_to_infra["kind"], "zone_rule");
    assert_eq!(app_to_infra["source_zone"], "app");
    assert_eq!(app_to_infra["target_zone"], "infra");
    assert_eq!(app_to_infra["reason"], "App layer may only import domain");

    let app_deep_import = findings
        .iter()
        .find(|f| f["importer"] == "src/app/deep.ts" && f["target"] == "src/domain/internal.ts")
        .expect("app -> domain/internal public_files finding");
    assert_eq!(app_deep_import["kind"], "public_files");
    assert_eq!(app_deep_import["source_zone"], "app");
    assert_eq!(app_deep_import["target_zone"], "domain");

    let toon_text = fixture.run_stdout(None, &["check"], 0);
    assert!(toon_text.contains("boundaries:"));
    assert!(toon_text.contains("src/domain/internal.ts"));
    assert!(toon_text.contains("src/infra/db.ts"));
    assert!(toon_text.contains("Domain logic must not depend on infrastructure"));

    let boundary_sarif = fixture.sarif_boundary_results(None, &["check", "--format", "sarif"], 0);
    assert_eq!(boundary_sarif.len(), 3);
    assert!(boundary_sarif.iter().any(|result| {
        let text = result["message"]["text"].as_str().unwrap_or_default();
        text.contains("domain")
            && text.contains("infra")
            && text.contains("Domain logic must not depend on infrastructure")
    }));

    let md_text = fixture.run_stdout(None, &["check", "--format", "markdown"], 0);
    assert!(md_text.contains("### Boundaries"));
    assert!(md_text.contains("`src/domain/internal.ts`"));
    assert!(md_text.contains("`src/infra/db.ts`"));
    assert!(md_text.contains("Domain logic must not depend on infrastructure"));
}

#[test]
fn check_rejects_invalid_boundary_configurations_with_exit_code_two() {
    let cases = [
        (
            "unknown_from_zone",
            r#"{
  "boundaries": {
    "zones": { "domain": { "patterns": ["src/domain/**"] } },
    "rules": [{ "from": "missing", "deny": ["domain"] }]
  }
}"#,
            "missing",
        ),
        (
            "unknown_target_zone",
            r#"{
  "boundaries": {
    "zones": { "domain": { "patterns": ["src/domain/**"] } },
    "rules": [{ "from": "domain", "allow": ["ghost"] }]
  }
}"#,
            "ghost",
        ),
        (
            "invalid_glob",
            r#"{
  "boundaries": {
    "zones": { "domain": { "patterns": ["src/[unclosed"] } },
    "rules": []
  }
}"#,
            "glob",
        ),
        (
            "conflicting_allow_and_deny_on_same_rule",
            r#"{
  "boundaries": {
    "zones": {
      "domain": { "patterns": ["src/domain/**"] },
      "infra": { "patterns": ["src/infra/**"] }
    },
    "rules": [{ "from": "domain", "allow": ["domain"], "deny": ["infra"] }]
  }
}"#,
            "allow",
        ),
        (
            "missing_allow_and_deny_on_rule",
            r#"{
  "boundaries": {
    "zones": { "domain": { "patterns": ["src/domain/**"] } },
    "rules": [{ "from": "domain" }]
  }
}"#,
            "allow",
        ),
        (
            "conflicting_allow_and_deny_across_rules_for_same_from",
            r#"{
  "boundaries": {
    "zones": {
      "domain": { "patterns": ["src/domain/**"] },
      "infra": { "patterns": ["src/infra/**"] }
    },
    "rules": [
      { "from": "domain", "allow": ["domain"] },
      { "from": "domain", "deny": ["infra"] }
    ]
  }
}"#,
            "domain",
        ),
    ];

    for (name, config_json, expected_substring) in cases {
        let fixture = BoundaryFixture::init(&[
            ("codeint.json", config_json),
            ("src/domain/a.ts", "export const a = 1;\n"),
        ]);
        let output = fixture.run(&["check"]);
        let err = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "case {name}: {err}");
        assert!(
            err.contains(expected_substring),
            "case {name}: expected {expected_substring:?} in {err}"
        );
    }
}

#[test]
fn check_strict_zones_flags_unzoned_production_files() {
    let fixture = BoundaryFixture::init(&[
        (
            "codeint.json",
            r#"{
  "boundaries": {
    "zones": { "domain": { "patterns": ["src/domain/**"] } },
    "strict_zones": true
  }
}
"#,
        ),
        ("src/domain/a.ts", "export const a = 1;\n"),
        ("src/orphan.ts", "export const orphan = 2;\n"),
    ]);

    let findings = boundary_findings(&fixture.check_json(0), "production");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["kind"], "unzoned_file");
    assert_eq!(findings[0]["importer"], "src/orphan.ts");
}

#[test]
fn check_leaves_output_unchanged_when_boundaries_not_configured() {
    let fixture = BoundaryFixture::init(&[("src/a.ts", "export const a = 1;\n")]);
    let json = fixture.check_json(0);
    assert!(
        json["source_sets"]["production"]
            .get("boundaries")
            .is_none()
    );
}

#[test]
fn check_enforces_boundaries_across_python_and_typescript() {
    let fixture = BoundaryFixture::init(&[
        (
            "codeint.json",
            r#"{
  "boundaries": {
    "zones": {
      "domain": { "patterns": ["py/domain/**", "ts/domain/**"] },
      "infra": { "patterns": ["py/infra/**", "ts/infra/**"] }
    },
    "rules": [
      {
        "from": "domain",
        "deny": ["infra"],
        "reason": "Domain cannot import infra in any language"
      }
    ]
  }
}
"#,
        ),
        ("py/infra/db.py", "VALUE = 1\n"),
        (
            "py/domain/service.py",
            "from py.infra.db import VALUE\nX = VALUE\n",
        ),
        ("ts/infra/db.ts", "export const value = 1;\n"),
        (
            "ts/domain/service.ts",
            "import { value } from '../infra/db';\nexport const x = value;\n",
        ),
    ]);

    let findings = boundary_findings(&fixture.check_json(0), "production");
    assert_eq!(findings.len(), 2, "got: {findings:?}");
    assert!(
        findings
            .iter()
            .any(|f| f["importer"] == "py/domain/service.py")
    );
    assert!(
        findings
            .iter()
            .any(|f| f["importer"] == "ts/domain/service.ts")
    );
}

#[test]
fn check_enforces_boundary_budget_overlap_precedence_and_optional_test_source_set() {
    let fixture = BoundaryFixture::init(&[
        (
            "codeint.json",
            r#"{
  "budgets": {
    "production": {
      "boundaries": 0
    }
  },
  "boundaries": {
    "zones": {
      "core": { "patterns": ["src/**"] },
      "domain": {
        "patterns": ["src/domain/**"],
        "public_files": ["src/domain/index.ts"]
      },
      "infra": { "patterns": ["src/infra/**"] }
    },
    "rules": [
      {
        "from": "domain",
        "deny": ["infra"],
        "reason": "Domain must stay independent of infra"
      }
    ],
    "source_sets": ["production", "test"]
  }
}
"#,
        ),
        ("src/infra/db.ts", "export const db = 1;\n"),
        (
            "src/domain/service.ts",
            "import { db } from '../infra/db';\nexport const svc = db;\n",
        ),
        (
            "src/domain/index.ts",
            "import { svc } from './service';\nexport const api = svc;\n",
        ),
        (
            "src/app.test.ts",
            "import { svc } from './domain/service';\nexport const tested = svc;\n",
        ),
    ]);

    let json = fixture.check_json(1);
    let prod_findings = boundary_findings(&json, "production");
    assert_eq!(prod_findings.len(), 1);
    assert_eq!(prod_findings[0]["source_zone"], "domain");
    assert_eq!(prod_findings[0]["target_zone"], "infra");

    let test_findings = boundary_findings(&json, "test");
    assert_eq!(test_findings.len(), 1);
    assert_eq!(test_findings[0]["kind"], "public_files");
    assert_eq!(test_findings[0]["source_zone"], "core");
    assert_eq!(test_findings[0]["target_zone"], "domain");
}

fn check_preset_findings(preset: &str, files: &[(&str, &str)]) -> Vec<Value> {
    let config = format!(r#"{{"boundaries":{{"preset":"{preset}"}}}}"#);
    let fixture = BoundaryFixture::init(&[("codeint.json", &config)]);
    for &(rel, body) in files {
        fixture.write(rel, body);
    }
    boundary_findings(&fixture.check_json(0), "production")
}

#[test]
fn check_supports_layered_preset() {
    let findings = check_preset_findings(
        "layered",
        &[
            ("src/infrastructure/db.ts", "export const db = 1;\n"),
            (
                "src/domain/order.ts",
                "import { db } from '../infrastructure/db';\nexport const order = db;\n",
            ),
            (
                "src/application/service.ts",
                "import { order } from '../domain/order';\nexport const service = order;\n",
            ),
            (
                "src/presentation/view.ts",
                "import { db } from '../infrastructure/db';\nexport const view = db;\n",
            ),
        ],
    );
    assert_eq!(findings.len(), 2, "got: {findings:?}");
    let pairs: Vec<(&str, &str, &str)> = findings
        .iter()
        .map(|f| {
            (
                f["importer"].as_str().unwrap_or_default(),
                f["source_zone"].as_str().unwrap_or_default(),
                f["target_zone"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    assert!(pairs.contains(&("src/domain/order.ts", "domain", "infrastructure")));
    assert!(pairs.contains(&("src/presentation/view.ts", "presentation", "infrastructure")));
}

#[test]
fn check_supports_layered_preset_with_user_overrides() {
    let layered_override = BoundaryFixture::init(&[
        (
            "codeint.json",
            r#"{
  "boundaries": {
    "preset": "layered",
    "zones": {
      "domain": {
        "patterns": ["src/business/**"],
        "public_files": ["src/business/index.ts"]
      },
      "shared": {
        "patterns": ["src/shared/**"]
      }
    },
    "rules": [
      {
        "from": "domain",
        "allow": ["shared"],
        "reason": "Domain may import shared helpers only"
      }
    ]
  }
}
"#,
        ),
        ("src/shared/id.ts", "export const id = 1;\n"),
        ("src/infrastructure/db.ts", "export const db = 2;\n"),
        (
            "src/business/internal.ts",
            "import { id } from '../shared/id';\nexport const secret = id;\n",
        ),
        (
            "src/business/index.ts",
            "import { secret } from './internal';\nexport const api = secret;\n",
        ),
        (
            "src/business/bad.ts",
            "import { db } from '../infrastructure/db';\nexport const bad = db;\n",
        ),
        (
            "src/application/deep.ts",
            "import { secret } from '../business/internal';\nexport const deep = secret;\n",
        ),
    ]);

    let findings = boundary_findings(&layered_override.check_json(0), "production");
    assert_eq!(findings.len(), 2, "got: {findings:?}");
    assert!(findings.iter().any(|f| {
        f["importer"] == "src/business/bad.ts"
            && f["kind"] == "zone_rule"
            && f["reason"] == "Domain may import shared helpers only"
    }));
    assert!(findings.iter().any(|f| {
        f["importer"] == "src/application/deep.ts"
            && f["kind"] == "public_files"
            && f["target"] == "src/business/internal.ts"
    }));
}

#[test]
fn check_supports_hexagonal_preset() {
    let findings = check_preset_findings(
        "hexagonal",
        &[
            ("src/domain/model.ts", "export const model = 1;\n"),
            (
                "src/ports/repo.ts",
                "import { model } from '../domain/model';\nexport const repo = model;\n",
            ),
            (
                "src/adapters/http.ts",
                "import { repo } from '../ports/repo';\nexport const http = repo;\n",
            ),
            (
                "src/application/usecase.ts",
                "import { repo } from '../ports/repo';\nimport { http } from '../adapters/http';\nexport const run = repo + http;\n",
            ),
        ],
    );
    assert_eq!(findings.len(), 1, "got: {findings:?}");
    assert_eq!(findings[0]["importer"], "src/application/usecase.ts");
    assert_eq!(findings[0]["target"], "src/adapters/http.ts");
    assert_eq!(findings[0]["source_zone"], "application");
    assert_eq!(findings[0]["target_zone"], "adapters");
}

#[test]
fn check_enforces_per_source_set_boundary_budgets() {
    let fixture = BoundaryFixture::init(&[
        (
            "codeint.json",
            r#"{
  "budgets": {
    "production": {
      "boundaries": 1
    },
    "test": {
      "boundaries": 0
    }
  },
  "boundaries": {
    "preset": "layered",
    "source_sets": ["production", "test"]
  }
}
"#,
        ),
        ("src/infrastructure/db.ts", "export const db = 1;\n"),
        (
            "src/domain/order.ts",
            "import { db } from '../infrastructure/db';\nexport const order = db;\n",
        ),
        (
            "src/domain/order.test.ts",
            "import { db } from '../infrastructure/db';\nexport const tested = db;\n",
        ),
    ]);

    let json = fixture.check_json(1);
    let over_budget = json["over_budget"].as_array().expect("over_budget");
    assert_eq!(over_budget.len(), 1, "got: {over_budget:?}");
    assert_eq!(over_budget[0]["source_set"], "test");
    assert_eq!(over_budget[0]["metric"], "boundaries");
    assert_eq!(over_budget[0]["budget"], 0.0);
}

#[test]
fn diff_classifies_boundary_findings_and_gates_new_regressions_without_failing_on_preexisting() {
    let cache = tempfile::tempdir().expect("cache tempdir");
    let fixture = BoundaryFixture::init(&[
        ("codeint.json", r#"{"boundaries":{"preset":"layered"}}"#),
        ("src/infrastructure/db.ts", "export const db = 1;\n"),
        (
            "src/domain/legacy.ts",
            "import { db } from '../infrastructure/db';\nexport const legacy = db;\n",
        ),
        (
            "src/application/clean.ts",
            "import { legacy } from '../domain/legacy';\nexport const clean = legacy;\n",
        ),
    ]);
    fixture.commit("base with legacy boundary violation");

    fixture.write(
        "src/domain/legacy.ts",
        "\n\nimport { db } from '../infrastructure/db';\nexport const legacy = db + 1;\n",
    );
    let no_reg_json = fixture.diff_json(cache.path(), 0);
    assert_diff_counts(
        &no_reg_json["source_sets"]["production"]["boundaries"],
        0,
        0,
        1,
    );
    assert!(no_reg_json.get("regressions").is_none());

    fixture.write(
        "src/domain/new_feature.ts",
        "import { db } from '../infrastructure/db';\nexport const feature = db;\n",
    );
    let reg_json = fixture.diff_json(cache.path(), 1);
    let prod_reg_diff = &reg_json["source_sets"]["production"]["boundaries"];
    assert_diff_counts(prod_reg_diff, 1, 0, 1);
    assert_eq!(
        prod_reg_diff["new"][0]["importer"],
        "src/domain/new_feature.ts"
    );
    assert_eq!(
        prod_reg_diff["new"][0]["target"],
        "src/infrastructure/db.ts"
    );
    let regressions = reg_json["regressions"].as_array().expect("regressions");
    assert!(
        regressions
            .iter()
            .any(|r| r["source_set"] == "production" && r["metric"] == "boundaries"),
        "got: {regressions:?}"
    );

    let sarif_results = fixture.sarif_boundary_results(
        Some(cache.path()),
        &["diff", "main", "--format", "sarif"],
        1,
    );
    assert_eq!(sarif_results.len(), 1);
    assert_eq!(
        sarif_results[0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "src/domain/new_feature.ts"
    );

    let md_text = fixture.run_stdout(
        Some(cache.path()),
        &["diff", "main", "--format", "markdown"],
        1,
    );
    assert!(md_text.contains("| Production | Boundaries |"));
    assert!(md_text.contains("### New Boundaries Findings"));
    assert!(md_text.contains("`src/domain/new_feature.ts`"));

    let toon_text = fixture.run_stdout(Some(cache.path()), &["diff", "main"], 1);
    assert!(toon_text.contains("boundaries:"));
    assert!(toon_text.contains("src/domain/new_feature.ts"));

    fixture.write(
        "codeint.json",
        r#"{
  "boundaries": { "preset": "layered" },
  "regression_limits": {
    "production": {
      "boundaries": { "ratio_rise": 1, "new_findings": "never" }
    }
  }
}"#,
    );
    fixture.diff_json(cache.path(), 0);

    fixture.write("codeint.json", r#"{"boundaries":{"preset":"layered"}}"#);
    fixture.write("src/domain/legacy.ts", "export const legacy = 1;\n");
    fixture.write("src/domain/new_feature.ts", "export const feature = 2;\n");
    let resolved_json = fixture.diff_json(cache.path(), 0);
    let resolved_diff = &resolved_json["source_sets"]["production"]["boundaries"];
    assert_diff_counts(resolved_diff, 0, 1, 0);
    assert_eq!(
        resolved_diff["resolved"][0]["importer"],
        "src/domain/legacy.ts"
    );
}

fn fixture_with_folder_import_plugin(
    boundaries_json: &str,
    files: &[(&str, &str)],
) -> BoundaryFixture {
    let fixture = BoundaryFixture::init(files);
    let wasm_bytes = fs::read(folder_import_plugin_wasm()).expect("read folder import wasm");
    let hash = sha256_hex(&sha256(&wasm_bytes));
    let plugin_dest = fixture
        .0
        .path()
        .join("plugins/folder_import_language_plugin.wasm");
    fs::create_dir_all(plugin_dest.parent().expect("plugin parent")).expect("mkdir plugins");
    fs::write(plugin_dest, wasm_bytes).expect("write folder import wasm");
    let config = format!(
        r#"{{
  "plugins": [
    {{
      "name": "folder-import-lang",
      "path": "plugins/folder_import_language_plugin.wasm",
      "sha256": "{hash}"
    }}
  ],
  "boundaries": {boundaries_json}
}}"#
    );
    fixture.write("codeint.json", &config);
    fixture
}

#[test]
fn check_multi_file_import_flags_only_forbidden_targets_and_matches_configured_rule() {
    let fixture = fixture_with_folder_import_plugin(
        r#"{
    "zones": {
      "ui": { "patterns": ["ui/**"] },
      "public": { "patterns": ["shared/public.fld"] },
      "secret": { "patterns": ["shared/secret.fld"] }
    },
    "rules": [
      {
        "from": "ui",
        "deny": ["secret"],
        "reason": "UI must not depend on secret files"
      }
    ]
  }"#,
        &[
            ("shared/public.fld", "fn api()\n"),
            ("shared/secret.fld", "fn internal()\n"),
            ("ui/page.fld", "use shared\nfn render()\n"),
        ],
    );

    let json = fixture.check_json(0);
    let boundaries = &json["source_sets"]["production"]["boundaries"];
    assert_eq!(boundaries["resolved_imports"], 2);
    let findings = boundary_findings(&json, "production");
    assert_eq!(findings.len(), 1, "got: {findings:?}");
    let finding = &findings[0];
    assert_eq!(finding["kind"], "zone_rule");
    assert_eq!(finding["importer"], "ui/page.fld");
    assert_eq!(finding["target"], "shared/secret.fld");
    assert_eq!(finding["specifier"], "shared");
    assert_eq!(finding["source_zone"], "ui");
    assert_eq!(finding["target_zone"], "secret");
    assert_eq!(finding["reason"], "UI must not depend on secret files");
    assert!(
        !findings
            .iter()
            .any(|item| item["target"] == "shared/public.fld")
    );
}

#[test]
fn check_multi_file_import_flags_each_forbidden_target_with_distinct_ids() {
    let fixture = fixture_with_folder_import_plugin(
        r#"{
    "zones": {
      "ui": { "patterns": ["ui/**"] },
      "public": { "patterns": ["public/**"] },
      "secret": { "patterns": ["secret/**"] }
    },
    "rules": [
      {
        "from": "ui",
        "allow": ["public"],
        "reason": "UI may import public modules only"
      }
    ]
  }"#,
        &[
            ("secret/alpha.fld", "fn alpha()\n"),
            ("secret/beta.fld", "fn beta()\n"),
            ("public/ok.fld", "fn ok()\n"),
            ("ui/page.fld", "use secret\nuse public\nfn render()\n"),
        ],
    );

    let json = fixture.check_json(0);
    let boundaries = &json["source_sets"]["production"]["boundaries"];
    assert_eq!(boundaries["resolved_imports"], 3);
    let findings = boundary_findings(&json, "production");
    assert_eq!(findings.len(), 2, "got: {findings:?}");
    let targets: Vec<&str> = findings
        .iter()
        .map(|item| item["target"].as_str().expect("target path"))
        .collect();
    assert_eq!(targets, vec!["secret/alpha.fld", "secret/beta.fld"]);
    for finding in &findings {
        assert_eq!(finding["kind"], "zone_rule");
        assert_eq!(finding["importer"], "ui/page.fld");
        assert_eq!(finding["specifier"], "secret");
        assert_eq!(finding["source_zone"], "ui");
        assert_eq!(finding["target_zone"], "secret");
        assert_eq!(finding["reason"], "UI may import public modules only");
    }
    let first_id = findings[0]["id"].as_str().expect("first id");
    let second_id = findings[1]["id"].as_str().expect("second id");
    assert!(!first_id.is_empty());
    assert_ne!(first_id, second_id);

    let sarif = fixture.sarif_boundary_results(None, &["check", "--format", "sarif"], 0);
    assert_eq!(sarif.len(), 2);
    let sarif_targets: Vec<&str> = sarif
        .iter()
        .map(|result| {
            result["relatedLocations"][0]["physicalLocation"]["artifactLocation"]["uri"]
                .as_str()
                .expect("sarif related target")
        })
        .collect();
    assert_eq!(sarif_targets, vec!["secret/alpha.fld", "secret/beta.fld"]);

    let markdown = fixture.run_stdout(None, &["check", "--format", "markdown"], 0);
    assert!(markdown.contains("`ui/page.fld`"));
    assert!(markdown.contains("`secret/alpha.fld`"));
    assert!(markdown.contains("`secret/beta.fld`"));
    assert!(!markdown.contains("`public/ok.fld`"));
}

#[test]
fn check_enforces_allow_external_deny_external_globs_builtins_and_source_sets_across_formats() {
    let fixture = BoundaryFixture::init(&[
        (
            "codeint.json",
            r#"{
  "boundaries": {
    "zones": {
      "domain": { "patterns": ["src/domain/**"] },
      "app": { "patterns": ["src/app/**"] },
      "infra": { "patterns": ["src/infra/**"] }
    },
    "rules": [
      {
        "from": "domain",
        "allow": ["domain"],
        "allow_external": [],
        "reason": "Domain must stay free of third-party packages"
      },
      {
        "from": "app",
        "allow_external": ["zod", "@acme/*"],
        "reason": "App may only use approved schema helpers"
      },
      {
        "from": "infra",
        "deny_external": ["prisma", "@aws-sdk/*"],
        "reason": "Use repository adapters instead of direct ORM or AWS SDK imports"
      }
    ]
  }
}
"#,
        ),
        (
            "src/domain/pure.ts",
            "import path from 'node:path';\nexport const base = path.basename('a/b');\n",
        ),
        (
            "src/domain/bad.ts",
            "import { PrismaClient } from 'prisma';\nexport const client = PrismaClient;\n",
        ),
        (
            "src/domain/bad.test.ts",
            "import { PrismaClient } from 'prisma';\nexport const tested = PrismaClient;\n",
        ),
        (
            "src/app/ok.ts",
            "import { z } from 'zod';\nimport { fmt } from '@acme/utils';\nexport const ok = [z, fmt];\n",
        ),
        (
            "src/app/bad.ts",
            "import axios from 'axios';\nexport const bad = axios;\n",
        ),
        (
            "src/infra/client.ts",
            "import { S3Client } from '@aws-sdk/client-s3';\nimport { z } from 'zod';\nexport const out = [S3Client, z];\n",
        ),
    ]);

    let json = fixture.check_json(0);
    assert!(json["source_sets"]["test"].get("boundaries").is_none());
    let boundaries = &json["source_sets"]["production"]["boundaries"];
    assert_eq!(boundaries["resolved_imports"], 6);
    let findings = boundary_findings(&json, "production");
    assert_eq!(findings.len(), 3, "got: {findings:?}");

    let domain_prisma = findings
        .iter()
        .find(|f| f["importer"] == "src/domain/bad.ts")
        .expect("domain -> prisma finding");
    assert_eq!(domain_prisma["kind"], "external_package");
    assert!(domain_prisma.get("target").is_none());
    assert_eq!(domain_prisma["external_package"], "prisma");
    assert_eq!(domain_prisma["specifier"], "prisma");
    assert_eq!(domain_prisma["source_zone"], "domain");
    assert!(domain_prisma.get("target_zone").is_none());
    assert_eq!(
        domain_prisma["reason"],
        "Domain must stay free of third-party packages"
    );

    let app_axios = findings
        .iter()
        .find(|f| f["importer"] == "src/app/bad.ts")
        .expect("app -> axios finding");
    assert_eq!(app_axios["kind"], "external_package");
    assert_eq!(app_axios["external_package"], "axios");
    assert_eq!(app_axios["source_zone"], "app");

    let infra_aws = findings
        .iter()
        .find(|f| f["importer"] == "src/infra/client.ts")
        .expect("infra -> @aws-sdk/client-s3 finding");
    assert_eq!(infra_aws["kind"], "external_package");
    assert_eq!(infra_aws["external_package"], "@aws-sdk/client-s3");
    assert_eq!(infra_aws["source_zone"], "infra");
    assert_eq!(
        infra_aws["reason"],
        "Use repository adapters instead of direct ORM or AWS SDK imports"
    );

    let sarif_results = fixture.sarif_boundary_results(None, &["check", "--format", "sarif"], 0);
    assert_eq!(sarif_results.len(), 3);
    let sarif_messages: Vec<&str> = sarif_results
        .iter()
        .filter_map(|result| result["message"]["text"].as_str())
        .collect();
    assert!(sarif_messages.iter().any(|text| {
        text.contains("@aws-sdk/client-s3")
            && text.contains("Use repository adapters instead of direct ORM or AWS SDK imports")
    }));

    for (args, expected) in [
        (
            &["check"][..],
            &[
                "external_package",
                "@aws-sdk/client-s3",
                "Domain must stay free of third-party packages",
            ][..],
        ),
        (
            &["check", "--format", "markdown"][..],
            &["`@aws-sdk/client-s3`", "`prisma`", r"external\_package"][..],
        ),
    ] {
        let rendered = fixture.run_stdout(None, args, 0);
        for snippet in expected {
            assert!(
                rendered.contains(snippet),
                "missing {snippet:?} in {rendered}"
            );
        }
    }

    let report_path = fixture.0.path().join("saved-check.json");
    fs::write(
        &report_path,
        fixture.run_stdout(None, &["check", "--format", "json"], 0),
    )
    .expect("write saved report");
    let rerendered_text =
        fixture.run_stdout(None, &["report", "saved-check.json", "--format", "text"], 0);
    assert!(rerendered_text.contains("@aws-sdk/client-s3"));
    assert!(rerendered_text.contains("Boundaries findings"));

    fixture.write(
        "codeint.json",
        r#"{
  "boundaries": {
    "zones": {
      "domain": { "patterns": ["src/domain/**"] }
    },
    "rules": [
      {
        "from": "domain",
        "allow_external": [],
        "reason": "Domain must stay free of third-party packages"
      }
    ],
    "source_sets": ["production", "test"]
  }
}
"#,
    );
    let scoped_json = fixture.check_json(0);
    let test_findings = boundary_findings(&scoped_json, "test");
    assert_eq!(test_findings.len(), 1, "got: {test_findings:?}");
    assert_eq!(test_findings[0]["importer"], "src/domain/bad.test.ts");
    assert_eq!(test_findings[0]["external_package"], "prisma");
}

#[test]
fn check_preserves_resolved_imports_denominator_when_no_external_rules_exist() {
    let fixture = BoundaryFixture::init(&[
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
        ("src/infra/db.ts", "export const db = 1;\n"),
        (
            "src/domain/service.ts",
            "import { db } from '../infra/db';\nimport { z } from 'zod';\nimport axios from 'axios';\nexport const out = [db, z, axios];\n",
        ),
    ]);

    let without_external = fixture.check_json(0);
    let boundaries = &without_external["source_sets"]["production"]["boundaries"];
    assert_eq!(boundaries["resolved_imports"], 1);
    assert_eq!(boundaries["ratio"], 1.0);

    fixture.write(
        "codeint.json",
        r#"{
  "boundaries": {
    "zones": {
      "domain": { "patterns": ["src/domain/**"] },
      "infra": { "patterns": ["src/infra/**"] }
    },
    "rules": [
      { "from": "domain", "deny": ["infra"], "deny_external": ["axios"] }
    ]
  }
}
"#,
    );

    let with_external = fixture.check_json(0);
    let boundaries_ext = &with_external["source_sets"]["production"]["boundaries"];
    assert_eq!(boundaries_ext["resolved_imports"], 3);
    assert_eq!(boundary_findings(&with_external, "production").len(), 2);
}

fn assert_invalid_external_rule(name: &str, rules_json: &str, expected_substring: &str) {
    let config = format!(
        r#"{{"boundaries":{{"zones":{{"domain":{{"patterns":["src/domain/**"]}}}},"rules":{rules_json}}}}}"#
    );
    let fixture = BoundaryFixture::init(&[
        ("codeint.json", &config),
        ("src/domain/a.ts", "export const a = 1;\n"),
    ]);
    let output = fixture.run(&["check"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "case {name}: {stderr}");
    assert!(
        stderr.contains(expected_substring),
        "case {name}: expected {expected_substring:?} in {stderr}"
    );
}

#[test]
fn check_rejects_invalid_external_boundary_configurations_with_exit_code_two() {
    assert_invalid_external_rule(
        "conflicting_allow_external_and_deny_external_on_same_rule",
        r#"[{"from":"domain","allow_external":["zod"],"deny_external":["prisma"]}]"#,
        "allow_external",
    );
    assert_invalid_external_rule(
        "conflicting_allow_external_and_deny_external_across_rules_for_same_from",
        r#"[{"from":"domain","allow_external":["zod"]},{"from":"domain","deny_external":["prisma"]}]"#,
        "domain",
    );
    assert_invalid_external_rule(
        "invalid_glob_in_allow_external",
        r#"[{"from":"domain","allow_external":["@aws-sdk/[unclosed"]}]"#,
        "glob",
    );
    assert_invalid_external_rule(
        "invalid_glob_in_deny_external",
        r#"[{"from":"domain","deny_external":["[unclosed"]}]"#,
        "glob",
    );
    assert_invalid_external_rule(
        "empty_glob_in_deny_external",
        r#"[{"from":"domain","deny_external":["   "]}]"#,
        "empty",
    );
}

#[test]
fn diff_classifies_external_package_boundary_findings_across_formats() {
    let cache = tempfile::tempdir().expect("cache tempdir");
    let fixture = BoundaryFixture::init(&[
        (
            "codeint.json",
            r#"{
  "boundaries": {
    "zones": {
      "domain": { "patterns": ["src/domain/**"] }
    },
    "rules": [
      {
        "from": "domain",
        "deny_external": ["prisma", "@aws-sdk/*"],
        "reason": "Domain must not import persistence or cloud SDKs"
      }
    ]
  }
}
"#,
        ),
        (
            "src/domain/legacy.ts",
            "import { PrismaClient } from 'prisma';\nexport const legacy = PrismaClient;\n",
        ),
    ]);
    fixture.commit("base with legacy external violation");

    fixture.write(
        "src/domain/legacy.ts",
        "\nimport { PrismaClient } from 'prisma';\nexport const legacy = PrismaClient;\n",
    );
    let unchanged_json = fixture.diff_json(cache.path(), 0);
    assert_diff_counts(
        &unchanged_json["source_sets"]["production"]["boundaries"],
        0,
        0,
        1,
    );

    fixture.write(
        "src/domain/upload.ts",
        "import { S3Client } from '@aws-sdk/client-s3';\nexport const s3 = S3Client;\n",
    );
    let reg_json = fixture.diff_json(cache.path(), 1);
    let prod_diff = &reg_json["source_sets"]["production"]["boundaries"];
    assert_diff_counts(prod_diff, 1, 0, 1);
    assert_eq!(prod_diff["new"][0]["kind"], "external_package");
    assert_eq!(prod_diff["new"][0]["importer"], "src/domain/upload.ts");
    assert_eq!(
        prod_diff["new"][0]["external_package"],
        "@aws-sdk/client-s3"
    );

    let md = fixture.run_stdout(
        Some(cache.path()),
        &["diff", "main", "--format", "markdown"],
        1,
    );
    assert!(md.contains("`src/domain/upload.ts`"));
    assert!(md.contains("`@aws-sdk/client-s3`"));
    assert!(md.contains(r"external\_package"));

    let sarif = fixture.sarif_boundary_results(
        Some(cache.path()),
        &["diff", "main", "--format", "sarif"],
        1,
    );
    assert_eq!(sarif.len(), 1);
    assert!(
        sarif[0]["message"]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("@aws-sdk/client-s3")
    );

    let toon = fixture.run_stdout(Some(cache.path()), &["diff", "main"], 1);
    assert!(toon.contains("@aws-sdk/client-s3"));
    assert!(toon.contains("external_package"));

    fixture.write("src/domain/legacy.ts", "export const legacy = 1;\n");
    fixture.write("src/domain/upload.ts", "export const s3 = 2;\n");
    let resolved_json = fixture.diff_json(cache.path(), 0);
    assert_diff_counts(
        &resolved_json["source_sets"]["production"]["boundaries"],
        0,
        1,
        0,
    );
}

#[test]
fn presets_have_no_external_rules_and_keep_internal_rules_when_adding_external_only_rule() {
    for preset in ["layered", "hexagonal"] {
        let config = format!(r#"{{"boundaries":{{"preset":"{preset}"}}}}"#);
        let fixture = BoundaryFixture::init(&[
            ("codeint.json", &config),
            (
                "src/domain/order.ts",
                "import { PrismaClient } from 'prisma';\nexport const order = PrismaClient;\n",
            ),
        ]);
        let resolved = fixture.run_json(None, &["config", "--resolved", "--format", "json"], 0);
        let rules = resolved["settings"]
            .as_array()
            .expect("settings")
            .iter()
            .find(|entry| entry["key"] == "boundaries.rules")
            .expect("boundaries.rules")["value"]
            .as_array()
            .expect("rules array");
        for rule in rules {
            assert!(
                rule.get("allow_external").is_none(),
                "preset {preset}: {rule}"
            );
            assert!(
                rule.get("deny_external").is_none(),
                "preset {preset}: {rule}"
            );
        }
        assert!(boundary_findings(&fixture.check_json(0), "production").is_empty());
    }

    let fixture = BoundaryFixture::init(&[
        (
            "codeint.json",
            r#"{"boundaries":{"preset":"hexagonal","rules":[{"from":"domain","allow_external":[],"reason":"Domain must stay dependency-free"}]}}"#,
        ),
        ("src/adapters/db.ts", "export const db = 1;\n"),
        (
            "src/domain/order.ts",
            "import { db } from '../adapters/db';\nimport { PrismaClient } from 'prisma';\nexport const order = [db, PrismaClient];\n",
        ),
    ]);
    let findings = boundary_findings(&fixture.check_json(0), "production");
    let kinds: Vec<&str> = findings
        .iter()
        .filter_map(|finding| finding["kind"].as_str())
        .collect();
    assert_eq!(kinds, vec!["external_package", "zone_rule"]);
}
