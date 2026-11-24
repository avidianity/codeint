use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};
use tempfile::TempDir;

fn run_codeint(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run codeint")
}

fn schema_json(cwd: &Path, extra_args: &[&str]) -> Value {
    let mut args = vec!["schema"];
    args.extend_from_slice(extra_args);
    let output = run_codeint(cwd, &args);
    assert_eq!(
        output.status.code(),
        Some(0),
        "codeint {args:?} failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("schema output must be valid JSON")
}

fn help_subcommands(cwd: &Path) -> BTreeSet<String> {
    let output = run_codeint(cwd, &["--help"]);
    assert_eq!(output.status.code(), Some(0));
    let help = String::from_utf8(output.stdout).expect("utf8 help");
    let mut in_commands = false;
    let mut commands = BTreeSet::new();
    for line in help.lines() {
        if line.trim() == "Commands:" {
            in_commands = true;
            continue;
        }
        if !in_commands {
            continue;
        }
        if line.trim().is_empty() {
            break;
        }
        if let Some(name) = line.split_whitespace().next()
            && name != "help"
        {
            commands.insert(name.to_owned());
        }
    }
    commands
}

fn live_mcp_tools(cwd: &Path) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_codeint"))
        .arg("mcp")
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn codeint mcp");
    {
        let mut stdin = child.stdin.take().expect("mcp stdin");
        writeln!(
            stdin,
            "{}",
            json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})
        )
        .expect("write tools/list");
    }
    let output = child.wait_with_output().expect("wait mcp");
    assert_eq!(
        output.status.code(),
        Some(0),
        "mcp stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let line = String::from_utf8(output.stdout)
        .expect("utf8 mcp stdout")
        .lines()
        .next()
        .expect("mcp response line")
        .to_owned();
    let response: Value = serde_json::from_str(&line).expect("valid mcp json");
    response["result"]["tools"].clone()
}

fn find_command<'a>(manifest: &'a Value, name: &str) -> &'a Value {
    manifest["commands"]
        .as_array()
        .expect("commands array")
        .iter()
        .find(|cmd| cmd["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("missing command {name} in schema"))
}

fn flag_names(command: &Value) -> BTreeSet<String> {
    command["flags"]
        .as_array()
        .expect("flags array")
        .iter()
        .filter_map(|flag| flag["long"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn schema_matches_live_cli_commands_flags_metrics_formats_mcp_tools_version_and_exit_codes() {
    let non_repo = TempDir::new().expect("tempdir");
    let manifest = schema_json(non_repo.path(), &[]);

    assert_eq!(manifest["name"], "codeint");
    assert_eq!(manifest["version"], env!("CARGO_PKG_VERSION"));
    let version_out = run_codeint(non_repo.path(), &["--version"]);
    assert_eq!(version_out.status.code(), Some(0));
    let version_text = String::from_utf8(version_out.stdout).expect("utf8 version");
    assert_eq!(
        version_text.trim(),
        format!(
            "{} {}",
            manifest["name"].as_str().unwrap(),
            manifest["version"].as_str().unwrap()
        )
    );

    let live_commands = help_subcommands(non_repo.path());
    let schema_commands: BTreeSet<String> = manifest["commands"]
        .as_array()
        .expect("commands array")
        .iter()
        .map(|cmd| cmd["name"].as_str().expect("command name").to_owned())
        .collect();
    assert_eq!(schema_commands, live_commands);
    assert!(schema_commands.contains("schema"));

    let check_cmd = find_command(&manifest, "check");
    assert_eq!(
        flag_names(check_cmd),
        BTreeSet::from([
            "--all".to_owned(),
            "--coverage-report".to_owned(),
            "--format".to_owned(),
            "--output".to_owned(),
            "--owner".to_owned(),
            "--package".to_owned(),
        ])
    );
    let diff_cmd = find_command(&manifest, "diff");
    assert_eq!(
        flag_names(diff_cmd),
        BTreeSet::from([
            "--all".to_owned(),
            "--format".to_owned(),
            "--head".to_owned(),
            "--output".to_owned(),
            "--owner".to_owned(),
            "--package".to_owned(),
        ])
    );
    assert_eq!(diff_cmd["arguments"][0]["value_name"], "BASE_REF");
    assert_eq!(diff_cmd["arguments"][0]["required"], true);

    let schema_cmd = find_command(&manifest, "schema");
    assert_eq!(
        flag_names(schema_cmd),
        BTreeSet::from(["--format".to_owned(), "--output".to_owned()])
    );

    let metric_keys: Vec<&str> = manifest["metrics"]
        .as_array()
        .expect("metrics array")
        .iter()
        .map(|m| m["key"].as_str().expect("metric key"))
        .collect();
    assert_eq!(
        metric_keys,
        [
            "erosion",
            "duplication",
            "verbosity",
            "import_cycles",
            "cognitive_complexity",
            "orphan_files",
            "hazards",
            "unused_dependencies",
            "unlisted_dependencies",
            "dev_in_production_dependencies",
            "test_gap",
        ]
    );
    let config_keys: BTreeSet<&str> = manifest["configuration_keys"]
        .as_array()
        .expect("configuration_keys array")
        .iter()
        .map(|k| k.as_str().expect("config key string"))
        .collect();
    assert!(config_keys.contains("coverage_reports"));
    assert!(config_keys.contains("budgets"));
    assert!(config_keys.contains("sql"));
    assert!(config_keys.contains("hazards"));
    let sql_lang = manifest["languages"]
        .as_array()
        .expect("languages array")
        .iter()
        .find(|entry| entry["name"].as_str() == Some("sql"))
        .expect("sql language in schema");
    assert_eq!(sql_lang["extensions"], json!(["sql"]));
    let sql_rules = sql_lang["hazard_rules"]
        .as_array()
        .expect("sql hazard_rules array");
    assert_eq!(sql_rules.len(), 32);
    assert!(sql_rules.iter().any(|r| r.as_str() == Some("select-star")));
    assert!(
        sql_rules
            .iter()
            .any(|r| r.as_str() == Some("filter-on-unindexed-column"))
    );
    for metric in manifest["metrics"].as_array().expect("metrics array") {
        assert!(!metric["name"].as_str().unwrap_or_default().is_empty());
        assert!(
            !metric["description"]
                .as_str()
                .unwrap_or_default()
                .is_empty()
        );
    }

    let format_names: BTreeSet<&str> = manifest["output_formats"]
        .as_array()
        .expect("output_formats array")
        .iter()
        .map(|f| f["name"].as_str().expect("format name"))
        .collect();
    assert_eq!(
        format_names,
        BTreeSet::from(["json", "markdown", "sarif", "text", "toon"])
    );

    let mcp_tools = live_mcp_tools(non_repo.path());
    assert_eq!(manifest["mcp_tools"], mcp_tools);

    let exit_codes: Vec<i64> = manifest["exit_codes"]
        .as_array()
        .expect("exit_codes array")
        .iter()
        .map(|entry| entry["code"].as_i64().expect("exit code integer"))
        .collect();
    assert_eq!(exit_codes, [0, 1, 2]);
    let exit_one = manifest["exit_codes"]
        .as_array()
        .expect("exit_codes array")
        .iter()
        .find(|entry| entry["code"].as_i64() == Some(1))
        .expect("exit code 1 entry");
    let exit_one_desc = exit_one["description"].as_str().unwrap_or_default();
    assert!(exit_one_desc.contains("Budget"));
    assert!(exit_one_desc.contains("Regression"));
    assert!(exit_one_desc.contains("doctor"));
}

#[test]
fn schema_supports_json_and_toon_formats_output_file_and_deterministic_rendering() {
    let dir = TempDir::new().expect("tempdir");

    let first_json = run_codeint(dir.path(), &["schema", "--format", "json"]);
    let second_json = run_codeint(dir.path(), &["schema"]);
    assert_eq!(first_json.status.code(), Some(0));
    assert_eq!(second_json.status.code(), Some(0));
    assert_eq!(first_json.stdout, second_json.stdout);
    let expected: Value = serde_json::from_slice(&first_json.stdout).expect("valid json");

    let toon_output = run_codeint(dir.path(), &["schema", "--format", "toon"]);
    assert_eq!(
        toon_output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&toon_output.stderr)
    );
    let toon_text = String::from_utf8(toon_output.stdout).expect("utf8 toon");
    assert!(toon_text.contains("name: codeint"));
    assert!(toon_text.contains(&format!("version: {}", env!("CARGO_PKG_VERSION"))));
    let decoded: Value = toon_format::decode_default(&toon_text).expect("valid toon");
    assert_eq!(decoded, expected);

    let out_file = dir.path().join("manifest.json");
    let file_output = run_codeint(
        dir.path(),
        &[
            "schema",
            "--format",
            "json",
            "--output",
            out_file.to_str().expect("utf8 path"),
        ],
    );
    assert_eq!(file_output.status.code(), Some(0));
    assert!(file_output.stdout.is_empty());
    let written = std::fs::read(&out_file).expect("read written manifest");
    assert_eq!(written, first_json.stdout);

    let invalid = run_codeint(dir.path(), &["schema", "--format", "yaml"]);
    assert_eq!(invalid.status.code(), Some(2));
}
