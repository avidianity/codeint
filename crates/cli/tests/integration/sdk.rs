use std::fs;
use std::path::Path;
use std::process::Command;

use codeint_cli::{
    AgentClient, AgentFormat, BaselineFormat, BaselineUpdateOptions, CheckOptions, ChurnFormat,
    ChurnOptions, ConfigFormat, ConfigOverrides, CouplingOptions, DiffOptions, DoctorFormat,
    GuardFormat, ImpactFormat, ImpactOptions, InitFormat, InitOptions, PluginHostConfig,
    PluginKind, PluginTestFormat, PluginTestOptions, RatchetFormat, RatchetUpdateOptions,
    ReportFormat, ReportRenderFormat, RerenderReportOptions, SchemaFormat, TraceFormat,
    TraceOptions, TrendFormat, TrendOptions, agent_install, agent_status, baseline_update, check,
    churn, coupling, diff, doctor, guard, impact, init, plugin_test, ratchet_update,
    render_baseline_update, rerender_report, resolved_config, schema, trace, trend,
};
use tempfile::TempDir;

pub(crate) fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed");
}

pub(crate) fn setup_repo() -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    git(dir.path(), &["init", "-b", "main"]);
    git(dir.path(), &["config", "user.email", "dev@example.com"]);
    git(dir.path(), &["config", "user.name", "Dev"]);
    fs::create_dir_all(dir.path().join("src")).expect("src dir");
    fs::write(
        dir.path().join("src/helper.ts"),
        "export function flag(x: boolean): boolean {\n  return x;\n}\n",
    )
    .expect("write helper.ts");
    fs::write(
        dir.path().join("src/app.ts"),
        "import { flag } from './helper';\nexport function ok(x: boolean): boolean {\n  return flag(x);\n}\n",
    )
    .expect("write app.ts");
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-m", "initial"]);
    dir
}

#[test]
fn rust_sdk_supports_init_formats_and_adopt() {
    for (format, expected_name) in [
        (InitFormat::Jsonc, "codeint.jsonc"),
        (InitFormat::Json, "codeint.json"),
        (InitFormat::Toml, "codeint.toml"),
        (InitFormat::Yaml, "codeint.yaml"),
    ] {
        let dir = setup_repo();
        let outcome = init(
            dir.path(),
            &InitOptions {
                format,
                adopt: false,
                plugin_host: PluginHostConfig::default(),
            },
        )
        .expect("init succeeds");
        assert!(outcome.configuration.ends_with(expected_name));
        assert!(outcome.adopted.is_none());
    }

    let adopt_dir = setup_repo();
    let adopted_outcome = init(
        adopt_dir.path(),
        &InitOptions {
            format: InitFormat::Json,
            adopt: true,
            plugin_host: PluginHostConfig::default(),
        },
    )
    .expect("init --adopt succeeds");
    assert!(adopted_outcome.configuration.ends_with("codeint.json"));
    let adopted = adopted_outcome.adopted.expect("adopted details");
    assert!(adopted.ratchet.exists());
    assert!(adopted.baseline.exists());
    assert!(adopted.ratchet_entries > 0);
}

#[test]
fn rust_sdk_supports_check_diff_trend_rerender_and_gated_failures() {
    let dir = setup_repo();
    let created = init(dir.path(), &InitOptions::default()).expect("init succeeds");
    assert!(created.configuration.ends_with("codeint.jsonc"));

    let checked = check(dir.path(), &CheckOptions::default()).expect("check succeeds");
    assert!(checked.passed);
    assert!(checked.failures.is_empty());
    let json = checked
        .report
        .render(ReportFormat::Json)
        .expect("render json");
    assert!(json.contains("\"files\": 2"));

    let saved_path = dir.path().join("saved-report.json");
    fs::write(&saved_path, &json).expect("write saved report");
    let rerendered = rerender_report(
        dir.path(),
        &saved_path,
        &RerenderReportOptions {
            format: Some(ReportRenderFormat::Markdown),
            all: true,
        },
    )
    .expect("rerender succeeds");
    assert!(rerendered.contains("Production"));

    fs::write(
        dir.path().join("codeint.jsonc"),
        "{\"budgets\":{\"production\":{\"verbosity\":0}}}\n",
    )
    .expect("write tight budget");
    fs::write(
        dir.path().join("src/app.ts"),
        "import { flag } from './helper';\nexport function verbose(x: boolean): boolean {\n  if (flag(x)) {\n    return true;\n  } else {\n    return false;\n  }\n}\n",
    )
    .expect("write verbose");

    let failed_check = check(dir.path(), &CheckOptions::default()).expect("returns outcome");
    assert!(!failed_check.passed);
    assert!(!failed_check.failures.is_empty());

    let diffed = diff(dir.path(), "HEAD", &DiffOptions::default()).expect("diff succeeds");
    assert!(!diffed.passed);
    assert!(!diffed.failures.is_empty());

    let trend_report = trend(dir.path(), &TrendOptions::default()).expect("trend succeeds");
    let trend_json = trend_report
        .render(TrendFormat::Json)
        .expect("render trend");
    assert!(trend_json.contains("\"commits\""));

    let outside = TempDir::new().expect("outside");
    let err = check(outside.path(), &CheckOptions::default())
        .err()
        .expect("operational error");
    assert!(err.message().contains("not inside a Git repository"));
    assert!(!err.remedy().is_empty());
}

#[test]
fn rust_sdk_supports_remaining_operations_and_gated_outcomes() {
    let dir = setup_repo();

    let impact_report = impact(
        dir.path(),
        &ImpactOptions {
            paths: vec!["src/helper.ts".to_owned()],
            ..ImpactOptions::default()
        },
    )
    .expect("impact succeeds");
    assert!(
        impact_report
            .render(ImpactFormat::Json)
            .expect("impact json")
            .contains("src/app.ts")
    );

    let trace_report = trace(
        dir.path(),
        "src/app.ts",
        "src/helper.ts",
        &TraceOptions::default(),
    )
    .expect("trace succeeds");
    assert!(trace_report.connected);
    assert!(
        trace_report
            .render(TraceFormat::Json)
            .expect("trace json")
            .contains("\"connected\": true")
    );

    let guard_report = guard(dir.path(), Path::new("src/app.ts")).expect("guard succeeds");
    assert!(
        guard_report
            .render(GuardFormat::Json)
            .expect("guard json")
            .contains("\"path\": \"src/app.ts\"")
    );

    let schema_report = schema();
    assert!(
        schema_report
            .render(SchemaFormat::Json)
            .expect("schema json")
            .contains("\"commands\"")
    );

    let config_report =
        resolved_config(dir.path(), &ConfigOverrides::default()).expect("config succeeds");
    assert!(
        config_report
            .render(ConfigFormat::Json)
            .expect("config json")
            .contains("\"settings\"")
    );

    let doctor_ok = doctor(dir.path(), &PluginHostConfig::default()).expect("doctor succeeds");
    assert!(doctor_ok.passed);
    assert!(doctor_ok.failures.is_empty());
    assert!(
        doctor_ok
            .report
            .render(DoctorFormat::Json)
            .expect("doctor json")
            .contains("\"checks\"")
    );

    let missing_folder = dir.path().join("does-not-exist");
    assert!(doctor(&missing_folder, &PluginHostConfig::default()).is_err());

    let home = TempDir::new().expect("agent home");
    let status = agent_status(home.path(), Some(AgentClient::ClaudeCode)).expect("agent status");
    assert!(
        status
            .render(AgentFormat::Json)
            .expect("status json")
            .contains("\"agents\"")
    );
    let installed =
        agent_install(home.path(), Some(AgentClient::ClaudeCode), true).expect("agent install");
    assert!(
        installed
            .render(AgentFormat::Json)
            .expect("install json")
            .contains("\"dry_run\": true")
    );

    let churn_report = churn(dir.path(), &ChurnOptions::default()).expect("churn succeeds");
    assert!(
        churn_report
            .render(ChurnFormat::Json)
            .expect("churn json")
            .contains("\"commits_analyzed\"")
    );

    let coupling_report =
        coupling(dir.path(), &CouplingOptions::default()).expect("coupling succeeds");
    assert!(
        coupling_report
            .render(ChurnFormat::Json)
            .expect("coupling json")
            .contains("\"pairs\"")
    );

    let ratchet = ratchet_update(dir.path(), &RatchetUpdateOptions::default()).expect("ratchet");
    assert!(
        ratchet
            .render(RatchetFormat::Json)
            .expect("ratchet json")
            .contains("\"created\": true")
    );

    let baseline =
        baseline_update(dir.path(), &BaselineUpdateOptions::default()).expect("baseline");
    assert!(
        render_baseline_update(&baseline, BaselineFormat::Json)
            .expect("baseline json")
            .contains("\"created\": true")
    );

    let plugin_path = crate::wasm_fixtures::wasm_fixture_path("sample_mini_plugin.wasm");
    let plugin_ok = plugin_test(&plugin_path, &PluginTestOptions::new(PluginKind::Language))
        .expect("plugin test succeeds");
    assert!(plugin_ok.passed);
    assert!(plugin_ok.failures.is_empty());
    assert!(
        plugin_ok
            .report
            .render(PluginTestFormat::Json)
            .expect("plugin test json")
            .contains("\"passed\": true")
    );

    let plugin_mismatch = plugin_test(&plugin_path, &PluginTestOptions::new(PluginKind::Metric))
        .expect("mismatched kind returns failed outcome");
    assert!(!plugin_mismatch.passed);
    assert!(!plugin_mismatch.failures.is_empty());

    let missing_plugin = dir.path().join("missing.wasm");
    assert!(
        plugin_test(
            &missing_plugin,
            &PluginTestOptions::new(PluginKind::Language)
        )
        .is_err()
    );
}
