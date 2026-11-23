use std::fs;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

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
        for &(rel, body) in files {
            let dst = dir.path().join(rel);
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent).expect("mkdir");
            }
            fs::write(dst, body).expect("write");
        }
        BoundaryFixture(dir)
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_codeint"))
            .args(args)
            .current_dir(self.0.path())
            .output()
            .expect("run codeint")
    }

    fn check_json(&self, expected_code: i32) -> Value {
        let out = self.run(&["check", "--format", "json"]);
        assert_eq!(
            out.status.code(),
            Some(expected_code),
            "stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).expect("valid JSON")
    }
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
    let prod_boundaries = &json["source_sets"]["production"]["boundaries"];
    assert!(prod_boundaries.is_object());
    assert!(json["source_sets"]["test"].get("boundaries").is_none());
    let findings = prod_boundaries["findings"].as_array().expect("findings");
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

    let toon = fixture.run(&["check"]);
    assert_eq!(toon.status.code(), Some(0));
    let toon_text = String::from_utf8(toon.stdout).expect("utf8");
    assert!(toon_text.contains("boundaries:"));
    assert!(toon_text.contains("src/domain/internal.ts"));
    assert!(toon_text.contains("src/infra/db.ts"));
    assert!(toon_text.contains("Domain logic must not depend on infrastructure"));

    let sarif_out = fixture.run(&["check", "--format", "sarif"]);
    assert_eq!(sarif_out.status.code(), Some(0));
    let sarif: Value = serde_json::from_slice(&sarif_out.stdout).expect("valid SARIF");
    let boundary_sarif: Vec<&Value> = sarif["runs"][0]["results"]
        .as_array()
        .expect("SARIF results")
        .iter()
        .filter(|result| result["ruleId"] == "boundaries")
        .collect();
    assert_eq!(boundary_sarif.len(), 3);
    assert!(boundary_sarif.iter().any(|result| {
        let text = result["message"]["text"].as_str().unwrap_or_default();
        text.contains("domain")
            && text.contains("infra")
            && text.contains("Domain logic must not depend on infrastructure")
    }));

    let md = fixture.run(&["check", "--format", "markdown"]);
    assert_eq!(md.status.code(), Some(0));
    let md_text = String::from_utf8(md.stdout).expect("utf8");
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

    let json = fixture.check_json(0);
    let findings = json["source_sets"]["production"]["boundaries"]["findings"]
        .as_array()
        .expect("findings");
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

    let json = fixture.check_json(0);
    let findings = json["source_sets"]["production"]["boundaries"]["findings"]
        .as_array()
        .expect("findings");
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
    let prod_findings = json["source_sets"]["production"]["boundaries"]["findings"]
        .as_array()
        .expect("production boundaries findings");
    assert_eq!(prod_findings.len(), 1);
    assert_eq!(prod_findings[0]["source_zone"], "domain");
    assert_eq!(prod_findings[0]["target_zone"], "infra");

    let test_findings = json["source_sets"]["test"]["boundaries"]["findings"]
        .as_array()
        .expect("test boundaries findings");
    assert_eq!(test_findings.len(), 1);
    assert_eq!(test_findings[0]["kind"], "public_files");
    assert_eq!(test_findings[0]["source_zone"], "core");
    assert_eq!(test_findings[0]["target_zone"], "domain");
}
