use std::io::Cursor;

use codeint_config::PluginHostConfig;
use codeint_git::Repository;
use serde_json::{Value, json};
use tempfile::TempDir;

use super::{McpFormat, OutcomeEnvelope, ServerContext, handle_line_to_writer, render_mcp_value};

fn init_test_repo(files: &[(&str, &str)]) -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    Repository::init(dir.path()).expect("init repo");
    for (relative, content) in files {
        let full = dir.path().join(relative);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(full, content).expect("write file");
    }
    let status = std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .status()
        .expect("git add");
    assert!(status.success());
    let status = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-q",
            "-m",
            "initial",
        ])
        .current_dir(dir.path())
        .status()
        .expect("git commit");
    assert!(status.success());
    dir
}

fn call(ctx: &mut ServerContext<'_>, request: Value) -> Value {
    let mut out = Vec::new();
    let line = serde_json::to_string(&request).expect("serialize request");
    let wrote = handle_line_to_writer(&line, ctx, &mut out).expect("handle_line_to_writer");
    assert!(wrote, "expected JSON-RPC response");
    serde_json::from_slice(&out).expect("valid JSON-RPC response")
}

fn tool_call(id: u64, name: &str, arguments: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments }
    })
}

#[test]
fn direct_serialization_matches_toon_and_json_formats() {
    let report = json!({ "files": 2, "ratio": 0.125 });
    let failures = vec!["erosion over budget".to_owned()];
    let envelope = OutcomeEnvelope {
        passed: false,
        failures: &failures,
        report: &report,
    };

    let toon_result = render_mcp_value(&envelope, McpFormat::Toon);
    assert!(!toon_result.is_error);
    assert_eq!(
        toon_result.content[0].text,
        codeint_report::render_toon(&envelope).expect("render_toon")
    );

    let json_result = render_mcp_value(&envelope, McpFormat::Json);
    assert!(!json_result.is_error);
    assert_eq!(
        json_result.content[0].text,
        serde_json::to_string(&envelope).expect("serde_json::to_string")
    );
}

#[test]
fn sequential_calls_on_unchanged_repo_reuse_cached_workspace_config_and_plugins() {
    let repo = init_test_repo(&[(
        "app.ts",
        "export function greet(x: number): number {\n  return x + 1;\n}\n",
    )]);
    let mut ctx = ServerContext::new(repo.path(), None, PluginHostConfig::default());

    let first = call(&mut ctx, tool_call(1, "check", json!({})));
    assert_eq!(first["result"]["isError"], false);
    assert_eq!(ctx.workspace_cache.config_load_count(), 1);
    assert_eq!(ctx.workspace_cache.plugin_load_count(), 1);

    let second = call(&mut ctx, tool_call(2, "diff", json!({"base_ref": "main"})));
    assert_eq!(second["result"]["isError"], false);
    assert_eq!(ctx.workspace_cache.config_load_count(), 1);
    assert_eq!(ctx.workspace_cache.plugin_load_count(), 1);

    let third = call(
        &mut ctx,
        tool_call(3, "impact", json!({"paths": ["app.ts"]})),
    );
    assert_eq!(third["result"]["isError"], false);
    assert_eq!(ctx.workspace_cache.config_load_count(), 1);
    assert_eq!(ctx.workspace_cache.plugin_load_count(), 1);

    let fourth = call(&mut ctx, tool_call(4, "guard", json!({"file": "app.ts"})));
    assert_eq!(fourth["result"]["isError"], false);
    assert_eq!(ctx.workspace_cache.config_load_count(), 1);
    assert_eq!(ctx.workspace_cache.plugin_load_count(), 1);
}

#[test]
fn config_add_modify_and_delete_invalidate_workspace_cache() {
    let repo = init_test_repo(&[(
        "app.ts",
        "export function f(x: number): number {\n  if (x === 1) return 1;\n  if (x === 2) return 2;\n  if (x === 3) return 3;\n  if (x === 4) return 4;\n  if (x === 5) return 5;\n  if (x === 6) return 6;\n  if (x === 7) return 7;\n  if (x === 8) return 8;\n  if (x === 9) return 9;\n  if (x === 10) return 10;\n  if (x === 11) return 11;\n  if (x === 12) return 12;\n  return 0;\n}\n",
    )]);
    let mut ctx = ServerContext::new(repo.path(), None, PluginHostConfig::default());

    let r1 = call(&mut ctx, tool_call(1, "check", json!({"format": "json"})));
    let p1: Value =
        serde_json::from_str(r1["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(p1["passed"], true);
    assert_eq!(ctx.workspace_cache.config_load_count(), 1);
    assert_eq!(ctx.workspace_cache.plugin_load_count(), 1);

    let config_path = repo.path().join("codeint.json");
    std::fs::write(
        &config_path,
        r#"{ "budgets": { "production": { "erosion": 0.00 } } }"#,
    )
    .expect("add codeint.json");
    let r2 = call(&mut ctx, tool_call(2, "check", json!({"format": "json"})));
    let p2: Value =
        serde_json::from_str(r2["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(p2["passed"], false);
    assert_eq!(ctx.workspace_cache.config_load_count(), 2);
    assert_eq!(ctx.workspace_cache.plugin_load_count(), 2);

    std::fs::write(
        &config_path,
        r#"{ "budgets": { "production": { "erosion": 1.00 } } }"#,
    )
    .expect("same-length modify codeint.json");
    let r3 = call(&mut ctx, tool_call(3, "check", json!({"format": "json"})));
    let p3: Value =
        serde_json::from_str(r3["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(p3["passed"], true);
    assert_eq!(ctx.workspace_cache.config_load_count(), 3);
    assert_eq!(ctx.workspace_cache.plugin_load_count(), 3);

    std::fs::remove_file(&config_path).expect("delete codeint.json");
    let r4 = call(&mut ctx, tool_call(4, "check", json!({"format": "json"})));
    let p4: Value =
        serde_json::from_str(r4["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(p4["passed"], true);
    assert_eq!(ctx.workspace_cache.config_load_count(), 4);
    assert_eq!(ctx.workspace_cache.plugin_load_count(), 4);

    let r5 = call(&mut ctx, tool_call(5, "check", json!({"format": "json"})));
    assert_eq!(r5["result"]["isError"], false);
    assert_eq!(ctx.workspace_cache.config_load_count(), 4);
    assert_eq!(ctx.workspace_cache.plugin_load_count(), 4);

    let mut reader = Cursor::new(format!(
        "{}\n",
        tool_call(6, "guard", json!({"file": "app.ts"}))
    ));
    let mut output = Vec::new();
    super::serve(
        &mut reader,
        &mut output,
        repo.path(),
        None,
        PluginHostConfig::default(),
    )
    .expect("serve");
    let parsed: Value = serde_json::from_slice(&output).expect("valid response");
    assert_eq!(parsed["result"]["isError"], false);
}

#[test]
fn ambiguous_config_workspace_switch_and_plugin_host_change_invalidate_appropriately() {
    let repo_a = init_test_repo(&[(
        "app.ts",
        "export function a(x: number): number {\n  return x + 1;\n}\n",
    )]);
    let repo_b = init_test_repo(&[(
        "app.ts",
        "export function b(x: number): number {\n  return x + 2;\n}\n",
    )]);
    let mut ctx = ServerContext::new(repo_a.path(), None, PluginHostConfig::default());

    let r_a1 = call(&mut ctx, tool_call(1, "check", json!({})));
    assert_eq!(r_a1["result"]["isError"], false);
    assert_eq!(ctx.workspace_cache.config_load_count(), 1);
    assert_eq!(ctx.workspace_cache.plugin_load_count(), 1);

    let r_b1 = call(
        &mut ctx,
        tool_call(2, "check", json!({"path": repo_b.path().to_string_lossy()})),
    );
    assert_eq!(r_b1["result"]["isError"], false);
    assert_eq!(ctx.workspace_cache.config_load_count(), 2);
    assert_eq!(ctx.workspace_cache.plugin_load_count(), 2);

    let r_a2 = call(&mut ctx, tool_call(3, "check", json!({})));
    assert_eq!(r_a2["result"]["isError"], false);
    assert_eq!(ctx.workspace_cache.config_load_count(), 2);
    assert_eq!(ctx.workspace_cache.plugin_load_count(), 2);

    std::fs::write(repo_a.path().join("codeint.json"), "{}").expect("write codeint.json");
    std::fs::write(repo_a.path().join("codeint.yaml"), "{}\n").expect("write codeint.yaml");
    let ambiguous = call(&mut ctx, tool_call(4, "check", json!({})));
    assert_eq!(ambiguous["result"]["isError"], true);

    std::fs::remove_file(repo_a.path().join("codeint.yaml")).expect("remove codeint.yaml");
    let recovered = call(&mut ctx, tool_call(5, "check", json!({})));
    assert_eq!(recovered["result"]["isError"], false);
    assert_eq!(ctx.workspace_cache.config_load_count(), 3);
    assert_eq!(ctx.workspace_cache.plugin_load_count(), 3);

    ctx.plugin_host.cache_dir = Some(repo_a.path().join(".custom-plugin-cache"));
    let host_changed = call(&mut ctx, tool_call(6, "check", json!({})));
    assert_eq!(host_changed["result"]["isError"], false);
    assert_eq!(ctx.workspace_cache.config_load_count(), 3);
    assert_eq!(ctx.workspace_cache.plugin_load_count(), 4);
}
