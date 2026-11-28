use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

use crate::common::{eroded_ts, init_git_repo, mcp_payload, mcp_tool_call};

struct LiveMcpServer {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl LiveMcpServer {
    fn spawn(dir: &std::path::Path) -> LiveMcpServer {
        let mut child = Command::new(env!("CARGO_BIN_EXE_codeint"))
            .arg("mcp")
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn codeint mcp");
        let stdin = child.stdin.take().expect("mcp stdin");
        let stdout = BufReader::new(child.stdout.take().expect("mcp stdout"));
        LiveMcpServer {
            child,
            stdin: Some(stdin),
            stdout,
        }
    }

    fn send(&mut self, request: &Value) -> Value {
        let stdin = self.stdin.as_mut().expect("mcp stdin open");
        writeln!(stdin, "{request}").expect("write mcp request");
        stdin.flush().expect("flush mcp stdin");
        let mut line = String::new();
        let read = self.stdout.read_line(&mut line).expect("read mcp response");
        assert!(read > 0, "unexpected EOF from codeint mcp");
        serde_json::from_str(&line).expect("valid JSON-RPC response")
    }

    fn finish(mut self) {
        self.stdin.take();
        let status = self.child.wait().expect("wait for codeint mcp");
        assert!(status.success(), "codeint mcp exited with {status}");
    }
}

#[test]
fn mcp_stdio_reflects_config_add_same_length_modify_delete_and_yaml_switch() {
    let repo = init_git_repo(&[("src/legacy.ts", &eroded_ts("legacy", 12))]);
    let mut server = LiveMcpServer::spawn(repo.path());

    let initial_check = server.send(&mcp_tool_call(1, "check", json!({})));
    let initial_payload = mcp_payload(&initial_check);
    assert_eq!(initial_payload["passed"], true);

    let initial_json = server.send(&mcp_tool_call(2, "check", json!({"format": "json"})));
    assert_eq!(initial_json["result"]["isError"], false);
    let initial_json_payload: Value = serde_json::from_str(
        initial_json["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .expect("valid JSON tool payload");
    assert_eq!(initial_json_payload["passed"], true);

    let initial_guard = server.send(&mcp_tool_call(3, "guard", json!({"file": "src/legacy.ts"})));
    assert_eq!(mcp_payload(&initial_guard)["configured"], false);

    let config_path = repo.path().join("codeint.json");
    std::fs::write(
        &config_path,
        r#"{ "budgets": { "production": { "erosion": 0.00 } } }"#,
    )
    .expect("write strict codeint.json");

    let after_add = server.send(&mcp_tool_call(4, "check", json!({})));
    assert_eq!(mcp_payload(&after_add)["passed"], false);

    std::fs::write(
        &config_path,
        r#"{ "budgets": { "production": { "erosion": 1.00 } } }"#,
    )
    .expect("overwrite relaxed codeint.json with identical byte length");

    let after_modify = server.send(&mcp_tool_call(5, "check", json!({})));
    assert_eq!(mcp_payload(&after_modify)["passed"], true);

    std::fs::remove_file(&config_path).expect("delete codeint.json");

    let after_delete_check = server.send(&mcp_tool_call(6, "check", json!({})));
    assert_eq!(mcp_payload(&after_delete_check)["passed"], true);

    let dot_codeint = repo.path().join(".codeint");
    std::fs::create_dir_all(&dot_codeint).expect("create .codeint");
    let yaml_path = dot_codeint.join("config.yaml");
    std::fs::write(
        &yaml_path,
        "boundaries:\n  zones:\n    core:\n      patterns:\n        - \"src/**\"\n  rules: []\n",
    )
    .expect("write .codeint/config.yaml");

    let after_yaml_add = server.send(&mcp_tool_call(7, "guard", json!({"file": "src/legacy.ts"})));
    let yaml_guard_payload = mcp_payload(&after_yaml_add);
    assert_eq!(yaml_guard_payload["configured"], true);
    assert_eq!(yaml_guard_payload["zone"], "core");

    std::fs::remove_file(&yaml_path).expect("delete .codeint/config.yaml");

    let after_yaml_delete =
        server.send(&mcp_tool_call(8, "guard", json!({"file": "src/legacy.ts"})));
    assert_eq!(mcp_payload(&after_yaml_delete)["configured"], false);

    server.finish();
}
