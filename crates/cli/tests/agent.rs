use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};
use tempfile::TempDir;

fn run_agent_cli(cwd: &Path, agent_home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(args)
        .current_dir(cwd)
        .env("CODEINT_AGENT_HOME", agent_home)
        .output()
        .expect("run codeint")
}

fn run_ok_stdout(cwd: &Path, agent_home: &Path, args: &[&str]) -> String {
    let output = run_agent_cli(cwd, agent_home, args);
    assert_eq!(
        output.status.code(),
        Some(0),
        "args={args:?} stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf8 stdout")
}

fn run_ok_json(cwd: &Path, agent_home: &Path, args: &[&str]) -> Value {
    let raw = run_ok_stdout(cwd, agent_home, args);
    serde_json::from_str(&raw).expect("valid JSON stdout")
}

fn write_file(root: &Path, relative: &str, contents: &str) {
    let target = root.join(relative);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(target, contents).expect("write file");
}

fn assert_clean_temp_files(home: &Path) {
    for rel in [".", ".cursor", ".codex"] {
        let dir = home.join(rel);
        if !dir.is_dir() {
            continue;
        }
        for entry in fs::read_dir(&dir).expect("read dir") {
            let name = entry.expect("dir entry").file_name();
            let text = name.to_string_lossy();
            assert!(
                !text.contains(".codeint-tmp-"),
                "unexpected leftover temp file {text} in {}",
                dir.display()
            );
        }
    }
}

fn assert_claude_preserved(doc: &Value) {
    assert_eq!(doc["theme"], "dark");
    assert_eq!(doc["numStartups"], 7);
    assert_eq!(doc["mcpServers"]["other-tool"]["command"], "npx");
    assert_eq!(
        doc["mcpServers"]["codeint"],
        json!({"command": "codeint", "args": ["mcp"]})
    );
}

fn seed_three_detected_agents(home: &Path) -> (String, String) {
    let claude_initial = serde_json::to_string_pretty(&json!({
        "theme": "dark",
        "numStartups": 7,
        "mcpServers": {
            "other-tool": {
                "command": "npx",
                "args": ["-y", "other-tool"]
            }
        }
    }))
    .expect("serialize claude");
    write_file(home, ".claude.json", &claude_initial);
    fs::create_dir_all(home.join(".cursor")).expect("create cursor dir");

    let codex_initial = concat!(
        "model = \"gpt-5\"\n",
        "\n",
        "[mcp_servers.memory]\n",
        "command = \"npx\"\n",
        "args = [\"-y\", \"@modelcontextprotocol/server-memory\"]\n",
    )
    .to_owned();
    write_file(home, ".codex/config.toml", &codex_initial);
    (claude_initial, codex_initial)
}

#[test]
fn agent_status_reports_detected_clients_and_registration_state() {
    let home = TempDir::new().expect("create temp home");
    write_file(
        home.path(),
        ".claude.json",
        &serde_json::to_string_pretty(&json!({
            "theme": "dark",
            "mcpServers": {
                "codeint": {
                    "command": "codeint",
                    "args": ["mcp"]
                }
            }
        }))
        .expect("serialize json"),
    );
    fs::create_dir_all(home.path().join(".cursor")).expect("create cursor dir");

    let payload = run_ok_json(
        home.path(),
        home.path(),
        &["agent", "status", "--format", "json"],
    );
    let agents = payload["agents"].as_array().expect("agents array");
    assert_eq!(agents.len(), 3);

    let claude = agents
        .iter()
        .find(|entry| entry["client"] == "claude-code")
        .expect("claude-code entry");
    assert_eq!(claude["detected"], true);
    assert_eq!(claude["config_exists"], true);
    assert_eq!(claude["registered"], true);

    let cursor = agents
        .iter()
        .find(|entry| entry["client"] == "cursor")
        .expect("cursor entry");
    assert_eq!(cursor["detected"], true);
    assert_eq!(cursor["config_exists"], false);
    assert_eq!(cursor["registered"], false);

    let codex = agents
        .iter()
        .find(|entry| entry["client"] == "codex")
        .expect("codex entry");
    assert_eq!(codex["detected"], false);
    assert_eq!(codex["config_exists"], false);
    assert_eq!(codex["registered"], false);

    let text = run_ok_stdout(home.path(), home.path(), &["agent", "status"]);
    assert!(text.contains("claude-code (Claude Code): detected, registered"));
    assert!(text.contains("cursor (Cursor): detected, not registered"));
    assert!(text.contains("codex (Codex): not detected, not registered"));

    let toon = run_ok_stdout(
        home.path(),
        home.path(),
        &["agent", "status", "--format", "toon"],
    );
    assert!(toon.contains("claude-code"));
    assert!(toon.contains("cursor"));
    assert!(toon.contains("codex"));
}

#[test]
fn agent_install_dry_run_previews_exact_changes_without_writing_to_disk() {
    let home = TempDir::new().expect("create temp home");
    let (claude_initial, codex_initial) = seed_three_detected_agents(home.path());

    let payload = run_ok_json(
        home.path(),
        home.path(),
        &["agent", "install", "--dry-run", "--format", "json"],
    );
    assert_eq!(payload["dry_run"], true);
    let results = payload["results"].as_array().expect("results array");
    assert_eq!(results.len(), 3);
    for entry in results {
        assert_eq!(entry["written"], false);
        assert!(entry["preview"].as_str().is_some());
    }

    assert_eq!(results[0]["action"], "update");
    let claude_preview: Value =
        serde_json::from_str(results[0]["preview"].as_str().expect("claude preview"))
            .expect("valid claude JSON preview");
    assert_claude_preserved(&claude_preview);

    assert_eq!(results[1]["action"], "create");
    let cursor_preview: Value =
        serde_json::from_str(results[1]["preview"].as_str().expect("cursor preview"))
            .expect("valid cursor JSON preview");
    assert_eq!(
        cursor_preview["mcpServers"]["codeint"],
        json!({"command": "codeint", "args": ["mcp"]})
    );

    assert_eq!(results[2]["action"], "update");
    let codex_preview = results[2]["preview"].as_str().expect("codex preview");
    assert!(codex_preview.contains("model = \"gpt-5\""));
    assert!(codex_preview.contains("[mcp_servers.memory]"));
    assert!(codex_preview.contains("[mcp_servers.codeint]"));

    assert_eq!(
        fs::read_to_string(home.path().join(".claude.json")).expect("read claude"),
        claude_initial
    );
    assert!(!home.path().join(".cursor/mcp.json").exists());
    assert_eq!(
        fs::read_to_string(home.path().join(".codex/config.toml")).expect("read codex"),
        codex_initial
    );
    assert_clean_temp_files(home.path());

    let text = run_ok_stdout(home.path(), home.path(), &["agent", "install", "--dry-run"]);
    assert!(text.contains("mode: dry-run (no files modified)"));
    assert!(text.contains("would update"));
    assert!(text.contains("would create"));
}

#[test]
fn agent_install_updates_atomically_preserves_settings_and_is_idempotent() {
    let home = TempDir::new().expect("create temp home");
    seed_three_detected_agents(home.path());

    let first = run_ok_json(
        home.path(),
        home.path(),
        &["agent", "install", "--format", "json"],
    );
    assert_eq!(first["dry_run"], false);
    let first_results = first["results"].as_array().expect("results");
    assert_eq!(first_results.len(), 3);
    assert_eq!(first_results[0]["action"], "update");
    assert_eq!(first_results[0]["written"], true);
    assert_eq!(first_results[1]["action"], "create");
    assert_eq!(first_results[1]["written"], true);
    assert_eq!(first_results[2]["action"], "update");
    assert_eq!(first_results[2]["written"], true);

    let claude_after_first =
        fs::read_to_string(home.path().join(".claude.json")).expect("read claude");
    let cursor_after_first =
        fs::read_to_string(home.path().join(".cursor/mcp.json")).expect("read cursor");
    let codex_after_first =
        fs::read_to_string(home.path().join(".codex/config.toml")).expect("read codex");

    let claude_json: Value = serde_json::from_str(&claude_after_first).expect("parse claude");
    assert_claude_preserved(&claude_json);

    let cursor_json: Value = serde_json::from_str(&cursor_after_first).expect("parse cursor");
    assert_eq!(
        cursor_json["mcpServers"]["codeint"],
        json!({"command": "codeint", "args": ["mcp"]})
    );

    assert!(codex_after_first.contains("model = \"gpt-5\""));
    assert!(codex_after_first.contains("[mcp_servers.memory]"));
    assert!(codex_after_first.contains("[mcp_servers.codeint]"));
    assert!(codex_after_first.contains("command = \"codeint\""));
    assert_clean_temp_files(home.path());

    let second = run_ok_json(
        home.path(),
        home.path(),
        &["agent", "install", "--format", "json"],
    );
    for entry in second["results"].as_array().expect("second results") {
        assert_eq!(entry["action"], "unchanged");
        assert_eq!(entry["written"], false);
        assert!(entry.get("preview").is_none());
    }
    assert_eq!(
        fs::read_to_string(home.path().join(".claude.json")).expect("re-read claude"),
        claude_after_first
    );
    assert_eq!(
        fs::read_to_string(home.path().join(".cursor/mcp.json")).expect("re-read cursor"),
        cursor_after_first
    );
    assert_eq!(
        fs::read_to_string(home.path().join(".codex/config.toml")).expect("re-read codex"),
        codex_after_first
    );

    let status = run_ok_json(
        home.path(),
        home.path(),
        &["agent", "status", "--format", "json"],
    );
    for entry in status["agents"].as_array().expect("status agents") {
        assert_eq!(entry["detected"], true);
        assert_eq!(entry["config_exists"], true);
        assert_eq!(entry["registered"], true);
    }
}

#[test]
fn agent_install_client_targets_single_agent_and_config_root_flag_overrides_env() {
    let env_home = TempDir::new().expect("create env home");
    let flag_home = TempDir::new().expect("create flag home");

    let empty = run_ok_json(
        flag_home.path(),
        env_home.path(),
        &["agent", "install", "--format", "json"],
    );
    assert_eq!(empty["results"], json!([]));
    assert!(
        empty["note"]
            .as_str()
            .unwrap_or_default()
            .contains("--client")
    );

    let flag_root = flag_home.path().to_str().expect("utf8 path");
    let toon = run_ok_stdout(
        flag_home.path(),
        env_home.path(),
        &[
            "agent",
            "install",
            "--client",
            "codex",
            "--config-root",
            flag_root,
            "--format",
            "toon",
        ],
    );
    assert!(toon.contains("codex"));
    assert!(toon.contains("create"));

    assert!(flag_home.path().join(".codex/config.toml").is_file());
    assert!(!flag_home.path().join(".claude.json").exists());
    assert!(!flag_home.path().join(".cursor/mcp.json").exists());
    assert!(!env_home.path().join(".codex/config.toml").exists());

    let status = run_ok_json(
        flag_home.path(),
        env_home.path(),
        &[
            "agent",
            "status",
            "--client",
            "codex",
            "--config-root",
            flag_root,
            "--format",
            "json",
        ],
    );
    let agents = status["agents"].as_array().expect("agents");
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0]["client"], "codex");
    assert_eq!(agents[0]["detected"], true);
    assert_eq!(agents[0]["registered"], true);
}

#[test]
fn agent_rejects_malformed_or_invalid_config_without_modifying_files() {
    let home = TempDir::new().expect("create temp home");
    write_file(home.path(), ".claude.json", "{ broken json");
    write_file(
        home.path(),
        ".cursor/mcp.json",
        "{\"mcpServers\": \"not-an-object\"}",
    );
    write_file(home.path(), ".codex/config.toml", "[mcp_servers\nbroken");

    let bad_json = run_agent_cli(
        home.path(),
        home.path(),
        &["agent", "install", "--client", "claude-code"],
    );
    assert_eq!(bad_json.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&bad_json.stderr).contains("not valid JSON"));
    assert_eq!(
        fs::read_to_string(home.path().join(".claude.json")).expect("read claude"),
        "{ broken json"
    );

    let bad_section = run_agent_cli(
        home.path(),
        home.path(),
        &["agent", "install", "--client", "cursor"],
    );
    assert_eq!(bad_section.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&bad_section.stderr).contains("mcpServers"));
    assert_eq!(
        fs::read_to_string(home.path().join(".cursor/mcp.json")).expect("read cursor"),
        "{\"mcpServers\": \"not-an-object\"}"
    );

    let bad_toml = run_agent_cli(
        home.path(),
        home.path(),
        &["agent", "status", "--client", "codex"],
    );
    assert_eq!(bad_toml.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&bad_toml.stderr).contains("not valid TOML"));
}

#[test]
fn agent_install_validates_all_clients_before_writing_any_file() {
    let home = TempDir::new().expect("create temp home");
    let claude_initial = serde_json::to_string_pretty(&json!({
        "theme": "dark",
        "mcpServers": {}
    }))
    .expect("serialize claude");
    write_file(home.path(), ".claude.json", &claude_initial);
    write_file(home.path(), ".cursor/mcp.json", "{ broken json");

    let output = run_agent_cli(home.path(), home.path(), &["agent", "install"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("not valid JSON"));
    assert_eq!(
        fs::read_to_string(home.path().join(".claude.json")).expect("read claude"),
        claude_initial
    );
    assert_eq!(
        fs::read_to_string(home.path().join(".cursor/mcp.json")).expect("read cursor"),
        "{ broken json"
    );
    assert_clean_temp_files(home.path());
}

#[test]
fn agent_status_and_install_both_reject_non_object_codeint_entry() {
    let home = TempDir::new().expect("create temp home");
    write_file(
        home.path(),
        ".claude.json",
        r#"{"mcpServers":{"codeint":"not-an-object"}}"#,
    );
    write_file(
        home.path(),
        ".codex/config.toml",
        "[mcp_servers]\ncodeint = \"not-an-object\"\n",
    );

    for (subcommand, client) in [
        ("status", "claude-code"),
        ("install", "claude-code"),
        ("status", "codex"),
        ("install", "codex"),
    ] {
        let output = run_agent_cli(
            home.path(),
            home.path(),
            &["agent", subcommand, "--client", client],
        );
        assert_eq!(
            output.status.code(),
            Some(2),
            "expected exit 2 for agent {subcommand} --client {client}"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("non-object codeint"),
            "unexpected stderr for {subcommand}/{client}: {stderr}"
        );
    }
}

#[cfg(unix)]
#[test]
fn agent_install_preserves_existing_file_permissions_and_creates_private_new_files() {
    use std::os::unix::fs::PermissionsExt;

    let home = TempDir::new().expect("create temp home");
    seed_three_detected_agents(home.path());

    let claude_path = home.path().join(".claude.json");
    let codex_path = home.path().join(".codex/config.toml");
    let cursor_path = home.path().join(".cursor/mcp.json");

    fs::set_permissions(&claude_path, fs::Permissions::from_mode(0o600)).expect("chmod claude");
    fs::set_permissions(&codex_path, fs::Permissions::from_mode(0o640)).expect("chmod codex");

    run_ok_json(
        home.path(),
        home.path(),
        &["agent", "install", "--format", "json"],
    );

    let claude_mode = fs::metadata(&claude_path)
        .expect("stat claude")
        .permissions()
        .mode()
        & 0o777;
    let codex_mode = fs::metadata(&codex_path)
        .expect("stat codex")
        .permissions()
        .mode()
        & 0o777;
    let cursor_mode = fs::metadata(&cursor_path)
        .expect("stat cursor")
        .permissions()
        .mode()
        & 0o777;

    assert_eq!(claude_mode, 0o600);
    assert_eq!(codex_mode, 0o640);
    assert_eq!(cursor_mode, 0o600);
}

#[cfg(unix)]
#[test]
fn agent_install_and_status_preserve_symlinks_and_reject_dangling_symlinks() {
    use std::os::unix::fs::symlink;

    let home = TempDir::new().expect("create temp home");
    let dotfiles = TempDir::new().expect("create dotfiles");

    let claude_target = dotfiles.path().join("claude.json");
    let cursor_target = dotfiles.path().join("cursor-mcp.json");
    let codex_target = dotfiles.path().join("codex-config.toml");

    fs::write(&claude_target, r#"{"theme":"dark"}"#).expect("write claude target");
    fs::write(&cursor_target, r#"{"mcpServers":{}}"#).expect("write cursor target");
    fs::write(&codex_target, "model = \"gpt-5\"\n").expect("write codex target");

    fs::create_dir_all(home.path().join(".cursor")).expect("mkdir .cursor");
    fs::create_dir_all(home.path().join(".codex")).expect("mkdir .codex");

    let claude_link = home.path().join(".claude.json");
    let cursor_link = home.path().join(".cursor/mcp.json");
    let codex_link = home.path().join(".codex/config.toml");

    symlink(&claude_target, &claude_link).expect("symlink claude");
    symlink(&cursor_target, &cursor_link).expect("symlink cursor");
    symlink(&codex_target, &codex_link).expect("symlink codex");

    run_ok_json(
        home.path(),
        home.path(),
        &["agent", "install", "--format", "json"],
    );

    for link in [&claude_link, &cursor_link, &codex_link] {
        assert!(
            fs::symlink_metadata(link)
                .expect("lstat link")
                .file_type()
                .is_symlink(),
            "{} must remain a symlink",
            link.display()
        );
    }

    let claude_doc: Value =
        serde_json::from_str(&fs::read_to_string(&claude_target).expect("read claude target"))
            .expect("parse claude target");
    assert_eq!(claude_doc["theme"], "dark");
    assert_eq!(
        claude_doc["mcpServers"]["codeint"],
        json!({"command": "codeint", "args": ["mcp"]})
    );
    let cursor_doc: Value =
        serde_json::from_str(&fs::read_to_string(&cursor_target).expect("read cursor target"))
            .expect("parse cursor target");
    assert_eq!(
        cursor_doc["mcpServers"]["codeint"],
        json!({"command": "codeint", "args": ["mcp"]})
    );
    let codex_doc = fs::read_to_string(&codex_target).expect("read codex target");
    assert!(codex_doc.contains("[mcp_servers.codeint]"));

    let dangling_home = TempDir::new().expect("create dangling home");
    let dangling_link = dangling_home.path().join(".claude.json");
    symlink(dotfiles.path().join("does-not-exist.json"), &dangling_link)
        .expect("create dangling symlink");

    for subcommand in ["status", "install"] {
        let output = run_agent_cli(
            dangling_home.path(),
            dangling_home.path(),
            &["agent", subcommand],
        );
        assert_eq!(output.status.code(), Some(2));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("dangling symlink"),
            "unexpected stderr for {subcommand}: {stderr}"
        );
    }
    assert!(
        fs::symlink_metadata(&dangling_link)
            .expect("lstat dangling link")
            .file_type()
            .is_symlink()
    );
    assert!(!dangling_link.exists());
}
