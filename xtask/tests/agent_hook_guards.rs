use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("resolve repository root")
}

fn codex_guard() -> PathBuf {
    repo_root().join(".codex/hooks/no-hook-bypass.sh")
}

fn opencode_plugin() -> PathBuf {
    repo_root().join(".opencode/plugins/no-hook-bypass.js")
}

fn run_codex_hook(payload: serde_json::Value) -> Option<i32> {
    let mut child = Command::new("bash")
        .arg(codex_guard())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start the Codex guard");
    child
        .stdin
        .take()
        .expect("open Codex guard stdin")
        .write_all(payload.to_string().as_bytes())
        .expect("write Codex guard payload");
    child.wait().expect("wait for Codex guard").code()
}

fn run_opencode_batch(events: &[serde_json::Value]) -> Vec<bool> {
    let driver = r#"
import { pathToFileURL } from "node:url";
const plugin = (await import(pathToFileURL(process.argv[1]).href)).default;
if (!plugin || typeof plugin.id !== "string" || typeof plugin.setup !== "function") {
  throw new Error("invalid OpenCode v2 plugin export");
}
let hook;
await plugin.setup({
  tool: {
    hook(name, fn) {
      if (name === "execute.before") hook = fn;
    },
  },
});
if (typeof hook !== "function") {
  throw new Error("execute.before hook was not registered");
}
const chunks = [];
for await (const chunk of process.stdin) chunks.push(chunk);
const events = JSON.parse(Buffer.concat(chunks).toString("utf8"));
const results = [];
for (const event of events) {
  let blocked = false;
  try {
    await hook(event);
  } catch {
    blocked = true;
  }
  results.push(blocked);
}
process.stdout.write(JSON.stringify(results));
"#;
    let mut child = Command::new("node")
        .args(["--input-type=module", "-e", driver])
        .arg(opencode_plugin())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start node for OpenCode plugin");
    child
        .stdin
        .take()
        .expect("open node stdin")
        .write_all(
            serde_json::to_string(events)
                .expect("serialize events")
                .as_bytes(),
        )
        .expect("write events to node");
    let output = child.wait_with_output().expect("wait for node");
    assert!(
        output.status.success(),
        "node driver failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("parse OpenCode plugin results")
}

fn extract_rust_string_array(source: &str, marker: &str) -> Vec<String> {
    let start = source
        .find(marker)
        .unwrap_or_else(|| panic!("missing {marker} in xtask/tests/hook_guard.rs"))
        + marker.len();
    let rest = &source[start..];
    let end = rest
        .find("\n    ];")
        .unwrap_or_else(|| panic!("missing closing ]; after {marker}"));
    rest[..end]
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('"'))
        .map(|line| {
            let literal = line.trim_end_matches(',');
            serde_json::from_str::<String>(literal)
                .unwrap_or_else(|error| panic!("parse string literal {literal}: {error}"))
        })
        .collect()
}

fn hook_guard_source() -> String {
    fs::read_to_string(repo_root().join("xtask/tests/hook_guard.rs"))
        .expect("read xtask/tests/hook_guard.rs")
}

fn hook_bypass_commands() -> Vec<String> {
    let cases = extract_rust_string_array(&hook_guard_source(), "let bypasses = [");
    assert!(
        cases.len() >= 20,
        "expected at least 20 bypass cases, got {}",
        cases.len()
    );
    cases
}

fn safe_commands() -> Vec<String> {
    let cases = extract_rust_string_array(&hook_guard_source(), "let allowed = [");
    assert!(
        cases.len() >= 6,
        "expected at least 6 allowed cases, got {}",
        cases.len()
    );
    cases
}

#[test]
fn codex_hooks_json_registers_pre_tool_use_guard_for_bash_and_apply_patch() {
    let raw =
        fs::read_to_string(repo_root().join(".codex/hooks.json")).expect("read .codex/hooks.json");
    let config: serde_json::Value = serde_json::from_str(&raw).expect("parse .codex/hooks.json");
    let entries = config["hooks"]["PreToolUse"]
        .as_array()
        .expect("PreToolUse entries");
    assert!(!entries.is_empty());
    let matcher = entries[0]["matcher"].as_str().expect("matcher string");
    assert!(matcher.contains("Bash"));
    assert!(matcher.contains("apply_patch"));
    let command = entries[0]["hooks"][0]["command"]
        .as_str()
        .expect("hook command string");
    assert!(command.contains(".codex/hooks/no-hook-bypass.sh"));
}

#[test]
fn codex_guard_refuses_bypasses_and_allows_ordinary_tools() {
    for cmd in hook_bypass_commands() {
        let payload = serde_json::json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": { "command": cmd }
        });
        assert_eq!(run_codex_hook(payload), Some(2), "Codex let through: {cmd}");
    }

    let blocked_patches = [
        "*** Begin Patch\n*** Add File: lefthook-local.yml\n+pre-commit:\n*** End Patch",
        "*** Begin Patch\n*** Update File: .git/hooks/pre-commit\n@@\n+exit 0\n*** End Patch",
        "*** Begin Patch\n*** Update File: src/lib.rs\n*** Move to: lefthook-local.yml\n@@\n+pre-commit:\n*** End Patch",
        "*** Begin Patch\n*** Update File: src/lib.rs\n*** Move to: .git/hooks/pre-commit\n@@\n+exit 0\n*** End Patch",
        "*** Begin Patch\r\n*** Update File:  .git/hooks/pre-commit  \r\n@@\r\n+exit 0\r\n*** End Patch",
    ];
    for patch in blocked_patches {
        let payload = serde_json::json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "apply_patch",
            "tool_input": { "command": patch }
        });
        assert_eq!(
            run_codex_hook(payload),
            Some(2),
            "Codex let through patch: {patch}"
        );
    }

    for cmd in safe_commands() {
        let payload = serde_json::json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": { "command": cmd }
        });
        assert_eq!(run_codex_hook(payload), Some(0), "Codex blocked: {cmd}");
    }

    let safe_patch = serde_json::json!({
        "hook_event_name": "PreToolUse",
        "tool_name": "apply_patch",
        "tool_input": {
            "command": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n+pub fn ok() {}\n*** End Patch"
        }
    });
    assert_eq!(run_codex_hook(safe_patch), Some(0));
}

#[test]
fn opencode_plugin_refuses_bypasses_and_allows_ordinary_tools() {
    let mut events = Vec::new();
    let mut expected = Vec::new();

    for cmd in hook_bypass_commands() {
        events.push(serde_json::json!({
            "tool": "bash",
            "args": { "command": cmd }
        }));
        expected.push((true, format!("bash bypass: {cmd}")));
    }

    events.push(serde_json::json!({
        "tool": "write",
        "args": { "filePath": "/repo/lefthook-local.yml", "content": "x" }
    }));
    expected.push((true, "write lefthook-local.yml".to_string()));

    events.push(serde_json::json!({
        "tool": "edit",
        "args": { "filePath": "/repo/.git/hooks/pre-commit" }
    }));
    expected.push((true, "edit .git/hooks/pre-commit".to_string()));

    let blocked_patches = [
        (
            "apply_patch lefthook-local.yml",
            "*** Begin Patch\n*** Add File: lefthook-local.yml\n+x\n*** End Patch",
        ),
        (
            "apply_patch Move to lefthook-local.yml",
            "*** Begin Patch\n*** Update File: src/lib.rs\n*** Move to: lefthook-local.yml\n@@\n+x\n*** End Patch",
        ),
        (
            "apply_patch Move to .git/hooks/pre-commit",
            "*** Begin Patch\n*** Update File: src/lib.rs\n*** Move to: .git/hooks/pre-commit\n@@\n+exit 0\n*** End Patch",
        ),
        (
            "apply_patch Update File with extra spaces and CR",
            "*** Begin Patch\r\n*** Update File:  .git/hooks/pre-commit  \r\n@@\r\n+exit 0\r\n*** End Patch",
        ),
    ];
    for (label, patch) in blocked_patches {
        events.push(serde_json::json!({
            "tool": "apply_patch",
            "args": { "patchText": patch }
        }));
        expected.push((true, label.to_string()));
    }

    for cmd in safe_commands() {
        events.push(serde_json::json!({
            "tool": "bash",
            "args": { "command": cmd }
        }));
        expected.push((false, format!("safe bash: {cmd}")));
    }

    events.push(serde_json::json!({
        "tool": "write",
        "args": { "filePath": "/repo/src/lib.rs", "content": "x" }
    }));
    expected.push((false, "write src/lib.rs".to_string()));

    events.push(serde_json::json!({
        "tool": "apply_patch",
        "args": { "patchText": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n+x\n*** End Patch" }
    }));
    expected.push((false, "apply_patch src/lib.rs".to_string()));

    let actual = run_opencode_batch(&events);
    assert_eq!(actual.len(), expected.len());
    for (got, (want, label)) in actual.into_iter().zip(expected) {
        assert_eq!(got, want, "OpenCode mismatch for {label}");
    }
}
