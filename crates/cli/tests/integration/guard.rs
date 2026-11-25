use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};
use tempfile::TempDir;

fn create_guard_repo(config: Option<Value>, files: &[(&str, &str)]) -> TempDir {
    let repo = TempDir::new().expect("create tempdir");
    assert!(
        Command::new("git")
            .args(["init", "-q", "-b", "main"])
            .current_dir(repo.path())
            .status()
            .expect("git init")
            .success()
    );
    if let Some(cfg) = config {
        fs::write(repo.path().join("codeint.json"), cfg.to_string()).expect("write config");
    }
    for (relative, contents) in files {
        let target = repo.path().join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(target, contents).expect("write source");
    }
    repo
}

fn run_codeint(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run codeint")
}

fn guard_json(cwd: &Path, target: &str) -> Value {
    let out = run_codeint(cwd, &["guard", target, "--format", "json"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("valid JSON")
}

fn layered_guard_config() -> Value {
    json!({
        "boundaries": {
            "zones": {
                "core": { "patterns": ["src/core/**"], "public_files": ["src/core/public.ts"] },
                "storage": { "patterns": ["src/storage/**"] },
                "ui": { "patterns": ["src/ui/**"] }
            },
            "rules": [
                { "from": "core", "deny": ["storage"], "reason": "Core logic cannot depend on storage" },
                { "from": "ui", "allow": ["core"], "reason": "UI may only import core" }
            ]
        }
    })
}

#[test]
fn guard_reports_zone_rules_public_files_and_importers_for_existing_and_planned_files() {
    let repo = create_guard_repo(
        Some(layered_guard_config()),
        &[
            ("src/core/public.ts", "export const api = 1;\n"),
            ("src/core/internal.ts", "export const secret = 2;\n"),
            ("src/storage/db.ts", "export const db = 3;\n"),
        ],
    );

    let internal = guard_json(repo.path(), "src/core/internal.ts");
    assert_eq!(internal["path"], "src/core/internal.ts");
    assert_eq!(internal["exists"], true);
    assert_eq!(internal["configured"], true);
    assert_eq!(internal["zone"], "core");
    assert_eq!(internal["is_public_file"], false);
    assert_eq!(internal["public_files"], json!(["src/core/public.ts"]));
    assert_eq!(internal["allowed_importers"], json!(["core"]));
    let denied = internal["denied_targets"]
        .as_array()
        .expect("denied_targets");
    assert_eq!(denied.len(), 1);
    assert_eq!(denied[0]["zone"], "storage");
    assert_eq!(denied[0]["directive"], "deny");
    assert_eq!(denied[0]["reason"], "Core logic cannot depend on storage");

    let public_entry = guard_json(repo.path(), "src/core/public.ts");
    assert_eq!(public_entry["is_public_file"], true);
    assert_eq!(
        public_entry["allowed_importers"],
        json!(["core", "storage", "ui"])
    );

    let planned = guard_json(repo.path(), "src/ui/planned/dashboard.ts");
    assert_eq!(planned["path"], "src/ui/planned/dashboard.ts");
    assert_eq!(planned["exists"], false);
    assert_eq!(planned["configured"], true);
    assert_eq!(planned["zone"], "ui");
    let allowed = planned["allowed_targets"]
        .as_array()
        .expect("allowed_targets");
    let core_target = allowed
        .iter()
        .find(|target| target["zone"] == "core")
        .expect("core allowed target");
    assert_eq!(core_target["public_files"], json!(["src/core/public.ts"]));
    assert_eq!(core_target["reason"], "UI may only import core");
    let planned_denied = planned["denied_targets"]
        .as_array()
        .expect("denied_targets");
    assert_eq!(planned_denied.len(), 1);
    assert_eq!(planned_denied[0]["zone"], "storage");
    assert_eq!(planned_denied[0]["directive"], "allow");
    assert_eq!(planned_denied[0]["reason"], "UI may only import core");

    let text_out = run_codeint(repo.path(), &["guard", "src/ui/planned/dashboard.ts"]);
    assert_eq!(text_out.status.code(), Some(0));
    let text = String::from_utf8(text_out.stdout).expect("utf8");
    assert!(text.contains("src/ui/planned/dashboard.ts"));
    assert!(text.contains("zone: ui"));
    assert!(text.contains("core"));
    assert!(text.contains("src/core/public.ts"));
    assert!(text.contains("UI may only import core"));

    let toon_out = run_codeint(
        repo.path(),
        &["guard", "src/core/internal.ts", "--format", "toon"],
    );
    assert_eq!(toon_out.status.code(), Some(0));
    let toon = String::from_utf8(toon_out.stdout).expect("utf8");
    assert!(toon.contains("zone: core"));
    assert!(toon.contains("Core logic cannot depend on storage"));
}

#[test]
fn guard_handles_unzoned_files_and_unconfigured_repositories_deterministically() {
    let strict_repo = create_guard_repo(
        Some(json!({
            "boundaries": {
                "zones": { "domain": { "patterns": ["src/domain/**"] } },
                "strict_zones": true
            }
        })),
        &[("src/orphan.ts", "export const orphan = 1;\n")],
    );

    let unzoned = guard_json(strict_repo.path(), "src/orphan.ts");
    assert_eq!(unzoned["configured"], true);
    assert_eq!(unzoned["strict_zones"], true);
    assert!(unzoned.get("zone").is_none());
    assert_eq!(unzoned["allowed_targets"], json!([]));
    assert_eq!(unzoned["denied_targets"], json!([]));
    assert!(
        unzoned["note"]
            .as_str()
            .unwrap_or_default()
            .contains("strict_zones")
    );

    let unconfigured_repo = create_guard_repo(None, &[("src/app.ts", "export const a = 1;\n")]);
    let unconfigured = guard_json(unconfigured_repo.path(), "src/app.ts");
    assert_eq!(unconfigured["configured"], false);
    assert_eq!(unconfigured["exists"], true);
    assert!(unconfigured.get("zone").is_none());
    assert!(
        unconfigured["note"]
            .as_str()
            .unwrap_or_default()
            .contains("No boundaries")
    );
}

#[test]
fn guard_resolves_subdirectory_and_absolute_paths_and_rejects_outside_repo() {
    let repo = create_guard_repo(
        Some(layered_guard_config()),
        &[("src/core/public.ts", "export const api = 1;\n")],
    );

    let sub_json = guard_json(&repo.path().join("src/core"), "planned.ts");
    assert_eq!(sub_json["path"], "src/core/planned.ts");
    assert_eq!(sub_json["zone"], "core");
    assert_eq!(sub_json["exists"], false);

    let abs_target = repo.path().join("src/storage/new_adapter.ts");
    let abs_json = guard_json(repo.path(), abs_target.to_str().expect("utf8 path"));
    assert_eq!(abs_json["path"], "src/storage/new_adapter.ts");
    assert_eq!(abs_json["zone"], "storage");
    assert_eq!(abs_json["exists"], false);

    let outside = run_codeint(repo.path(), &["guard", "../outside.ts"]);
    assert_eq!(outside.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&outside.stderr);
    assert!(
        stderr.contains("outside the Git repository"),
        "stderr: {stderr}"
    );
}

#[test]
fn guard_surfaces_external_package_rules_in_text_toon_and_json() {
    let repo = create_guard_repo(
        Some(json!({
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
                        "deny_external": ["prisma", "@aws-sdk/*"],
                        "reason": "Keep persistence and cloud SDKs in infra"
                    }
                ]
            }
        })),
        &[
            ("src/domain/order.ts", "export const order = 1;\n"),
            ("src/app/service.ts", "export const service = 2;\n"),
            ("src/infra/db.ts", "export const db = 3;\n"),
        ],
    );

    let domain = guard_json(repo.path(), "src/domain/order.ts");
    assert_eq!(
        domain["external_imports"],
        json!({
            "directive": "allow_external",
            "patterns": [],
            "reason": "Domain must stay free of third-party packages"
        })
    );

    let app = guard_json(repo.path(), "src/app/service.ts");
    assert_eq!(
        app["external_imports"],
        json!({
            "directive": "deny_external",
            "patterns": ["prisma", "@aws-sdk/*"],
            "reason": "Keep persistence and cloud SDKs in infra"
        })
    );

    let infra = guard_json(repo.path(), "src/infra/db.ts");
    assert!(infra.get("external_imports").is_none());

    let domain_text =
        String::from_utf8(run_codeint(repo.path(), &["guard", "src/domain/order.ts"]).stdout)
            .expect("utf8");
    assert!(domain_text.contains("external_imports:"));
    assert!(domain_text.contains("allow_external"));
    assert!(domain_text.contains("Domain must stay free of third-party packages"));

    let app_text =
        String::from_utf8(run_codeint(repo.path(), &["guard", "src/app/service.ts"]).stdout)
            .expect("utf8");
    assert!(app_text.contains("deny_external"));
    assert!(app_text.contains("prisma"));
    assert!(app_text.contains("@aws-sdk/*"));
    assert!(app_text.contains("Keep persistence and cloud SDKs in infra"));

    let app_toon = String::from_utf8(
        run_codeint(
            repo.path(),
            &["guard", "src/app/service.ts", "--format", "toon"],
        )
        .stdout,
    )
    .expect("utf8");
    assert!(app_toon.contains("external_imports:"));
    assert!(app_toon.contains("deny_external"));
    assert!(app_toon.contains("@aws-sdk/*"));
}
