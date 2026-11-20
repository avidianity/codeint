use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn guard() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(".claude/hooks/no-hook-bypass.sh")
}

fn exit_code(tool_input: serde_json::Value) -> Option<i32> {
    let mut child = Command::new("bash")
        .arg(guard())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start the guard");
    let payload = serde_json::json!({ "tool_input": tool_input }).to_string();
    child
        .stdin
        .take()
        .expect("open the guard input")
        .write_all(payload.as_bytes())
        .expect("write the guard input");
    child.wait().expect("wait for the guard").code()
}

fn command(text: &str) -> Option<i32> {
    exit_code(serde_json::json!({ "command": text }))
}

#[test]
fn the_guard_refuses_every_known_way_to_skip_git_hooks() {
    let bypasses = [
        "git commit --no-verify -m x",
        "git commit --no-verif -m x",
        "git push --no-verify",
        "git commit -n -m x",
        "git commit -nm \"feat: x\"",
        "git commit -n\t-m x",
        "git -c core.hooksPath=/dev/null commit -m x",
        "git -c core.hookspath=/dev/null commit -m x",
        "git config core.hooksPath /tmp/none",
        "GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=core.x GIT_CONFIG_VALUE_0=y git commit -m x",
        "LEFTHOOK=0 git commit -m x",
        "LEFTHOOK='0' git commit -m x",
        "LEFTHOOK=false git commit -m x",
        "LEFTHOOK_BIN=/usr/bin/true git commit -m x",
        "LEFTHOOK_EXCLUDE=markdown git commit -m x",
        "printf 'pre-commit:\\n  skip: true\\n' > lefthook-local.yml",
        "npx lefthook uninstall",
        "rm .git/hooks/pre-commit",
    ];
    for bypass in bypasses {
        assert_eq!(command(bypass), Some(2), "the guard let through: {bypass}");
    }
}

#[test]
fn the_guard_refuses_a_file_tool_that_writes_a_local_hook_override() {
    let write = serde_json::json!({ "file_path": "/repo/lefthook-local.yml", "content": "x" });
    assert_eq!(exit_code(write), Some(2));
    let hook = serde_json::json!({ "file_path": "/repo/.git/hooks/pre-commit", "content": "x" });
    assert_eq!(exit_code(hook), Some(2));
}

#[test]
fn the_guard_allows_ordinary_commands_and_files() {
    let allowed = [
        "git commit -m \"fix(cli): refuse an empty path\"",
        "git commit --amend -m x",
        "git commit -am x",
        "LEFTHOOK_VERBOSE=1 npx lefthook run pre-commit",
        "cargo test --workspace --locked",
        "git push -u origin chore/bootstrap",
    ];
    for allowed_command in allowed {
        assert_eq!(
            command(allowed_command),
            Some(0),
            "the guard blocked: {allowed_command}"
        );
    }
    let write = serde_json::json!({ "file_path": "/repo/src/lib.rs", "content": "x" });
    assert_eq!(exit_code(write), Some(0));
}
