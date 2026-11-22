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
        "cd .git && rm hooks/pre-commit",
        "cd .git/hooks && : > commit-msg",
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

#[test]
fn the_guard_refuses_git_commit_tree_and_allows_it_in_commit_messages() {
    let refused = [
        "git commit-tree HEAD^{tree} -m \"x\"",
        "git -C /repo commit-tree HEAD^{tree} -m \"x\"",
        "git --git-dir=.git commit-tree HEAD^{tree}",
        "/usr/bin/git commit-tree HEAD^{tree} -m \"x\"",
        "env git commit-tree HEAD^{tree} -m \"x\"",
        "GIT_AUTHOR_NAME=x git commit-tree HEAD^{tree} -m \"x\"",
        "echo $(git commit-tree HEAD^{tree} -m \"x\")",
        "(git commit-tree HEAD^{tree} -m \"x\")",
        "git commit -m \"ok\" && git commit-tree HEAD^{tree} -m \"x\"",
    ];
    for bypass in refused {
        assert_eq!(command(bypass), Some(2), "the guard let through: {bypass}");
    }

    let allowed = [
        "git commit -m \"fix(checks): refuse git commit-tree\"",
        "git commit -m 'fix(checks): refuse git commit-tree'",
        "git commit --message=\"mention commit-tree in docs\"",
        "git commit -m \"docs: explain commit-tree\"",
    ];
    for safe in allowed {
        assert_eq!(command(safe), Some(0), "the guard blocked: {safe}");
    }
}

#[test]
fn the_guard_refuses_git_hooks_reached_across_cd_commands_and_allows_other_cd_chains() {
    let refused = [
        "cd .git && cd hooks && : > pre-commit",
        "cd .git && cd hooks && rm pre-commit",
        "cd .git && cd hooks && chmod -x pre-commit",
        "cd .git/hooks && : > commit-msg",
        "cd subdir && cd ../.git && cd hooks && rm pre-commit",
        "cd a/b && cd ../../.git && cd hooks && : > pre-push",
        "cd /repo/.git && cd hooks && touch pre-commit",
        "pushd .git && pushd hooks && rm pre-commit",
        "cd .git && (cd hooks && : > pre-commit)",
        "cd subdir && cd ../.git/hooks && rm pre-commit",
        "cd .git && rm ./hooks/pre-commit",
        "cd .git && cp /dev/null hooks/commit-msg",
    ];
    for bypass in refused {
        assert_eq!(command(bypass), Some(2), "the guard let through: {bypass}");
    }

    let allowed = [
        "cd subdir && cd .. && cargo test",
        "cd .claude && cd hooks && ls",
        "cd docs && cd checks && ls",
        "cd .git && cd .. && cd hooks && ls",
        "cd src && cd hooks && cat index.ts",
    ];
    for safe in allowed {
        assert_eq!(command(safe), Some(0), "the guard blocked: {safe}");
    }
}

#[test]
fn the_guard_allows_blocked_strings_inside_commit_messages_but_refuses_real_flags() {
    let allowed = [
        "git commit -m \"fix(checks): allow -n in commit messages\"",
        "git commit -m 'fix(checks): allow -n in commit messages'",
        "git commit -m \"fix(checks): refuse --no-verify and LEFTHOOK=0\"",
        "git commit -m 'fix(checks): refuse --no-verify and LEFTHOOK=0'",
        "git commit -m \"fix(checks): refuse git commit-tree and core.hooksPath\"",
        "git commit -m \"fix(checks): refuse GIT_CONFIG_COUNT=1 and lefthook-local.yml\"",
        "git commit -m \"fix(checks): block .git/hooks and hooks/pre-commit\"",
        "git commit --message=\"fix(checks): block --no-verify -n LEFTHOOK=0 commit-tree\"",
        "git commit --message 'fix(checks): block --no-verify -n LEFTHOOK=0 commit-tree'",
        "git commit -am \"fix(checks): mention -n and --no-verify\"",
        "git commit -m \"title\" -m \"body with -n, --no-verify, LEFTHOOK=0, and git commit-tree\"",
        "git commit -m \"$(cat <<'EOF'\nfix(checks): block --no-verify, -n, LEFTHOOK=0, git commit-tree\nEOF\n)\"",
    ];
    for safe in allowed {
        assert_eq!(command(safe), Some(0), "the guard blocked: {safe}");
    }

    let refused = [
        "git commit -n -m \"fix: harmless\"",
        "git commit -m \"fix: harmless\" -n",
        "git commit --no-verify -m \"fix: harmless\"",
        "git commit -m \"fix: harmless\" --no-verify",
        "LEFTHOOK=0 git commit -m \"fix: harmless\"",
        "git commit -m \"fix: $(git commit --no-verify -m inner)\"",
        "git commit -m \"fix: $(git commit-tree HEAD^{tree} -m inner)\"",
        "git commit -m \"fix: harmless\" && git commit --no-verify -m \"second\"",
        "git commit -m \"fix: harmless\" && LEFTHOOK=0 git push",
    ];
    for bypass in refused {
        assert_eq!(command(bypass), Some(2), "the guard let through: {bypass}");
    }
}
