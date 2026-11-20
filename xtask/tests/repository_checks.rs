use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

const CHECKS_TOML: &str = include_str!("../../checks.toml");
static NEXT_REPOSITORY: AtomicUsize = AtomicUsize::new(0);

struct Repository {
    path: PathBuf,
}

impl Repository {
    fn new() -> Repository {
        let unique = NEXT_REPOSITORY.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("xtask-check-{}-{unique}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create the temporary repository folder");
        let repository = Repository { path };
        repository.git(&["init", "--quiet", "--initial-branch", "main"]);
        repository.write("checks.toml", CHECKS_TOML);
        repository.commit_all("chore: start");
        repository
    }

    fn write(&self, relative: &str, text: &str) {
        let full = self.path.join(relative);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).expect("create the parent folder");
        }
        fs::write(full, text).expect("write the file");
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=Check",
                "-c",
                "user.email=check@example.invalid",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(&self.path)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    fn commit_all(&self, message: &str) {
        self.git(&["add", "--all"]);
        self.git(&["commit", "--quiet", "--allow-empty", "--message", message]);
    }

    fn branch(&self, name: &str) {
        self.git(&["checkout", "--quiet", "-b", name]);
    }

    fn xtask(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(args)
            .current_dir(&self.path)
            .output()
            .expect("run xtask")
    }
}

impl Drop for Repository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_fails_with(output: &Output, expected: &str) {
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout: {}\nstderr: {}",
        stdout(output),
        stderr(output)
    );
    assert!(
        stderr(output).contains(expected),
        "stderr lacks {expected:?}: {}",
        stderr(output)
    );
}

fn assert_passes(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout: {}\nstderr: {}",
        stdout(output),
        stderr(output)
    );
}

#[test]
fn no_comments_rejects_an_added_comment_and_accepts_a_rename() {
    let repository = Repository::new();
    repository.write("crates/a/src/lib.rs", "fn total() -> u32 {\n    1 + 2\n}\n");
    repository.commit_all("feat: add total");
    repository.branch("feature");
    repository.write(
        "crates/a/src/lib.rs",
        "fn total() -> u32 {\n    // add the numbers\n    1 + 2\n}\n",
    );
    repository.commit_all("feat: explain total");
    assert_fails_with(
        &repository.xtask(&["no-comments", "--base", "main"]),
        "crates/a/src/lib.rs:2 adds a comment",
    );
    repository.write(
        "crates/a/src/lib.rs",
        "fn sum_of_one_and_two() -> u32 {\n    1 + 2\n}\n",
    );
    repository.commit_all("refactor: rename total");
    assert_passes(&repository.xtask(&["no-comments", "--base", "main"]));
}

#[test]
fn no_comments_skips_test_fixtures_but_not_other_test_or_source_files() {
    let repository = Repository::new();
    repository.branch("feature");
    repository.write(
        "crates/lang-ts/tests/fixtures/commented.ts",
        "// a comment-only line is test input\nexport const one = 1;\n",
    );
    repository.commit_all("test: add a commented fixture");
    assert_passes(&repository.xtask(&["no-comments", "--base", "main"]));
    repository.write(
        "crates/lang-ts/tests/conformance.ts",
        "// not a fixture\nexport const two = 2;\n",
    );
    repository.commit_all("test: add a commented test file");
    assert_fails_with(
        &repository.xtask(&["no-comments", "--base", "main"]),
        "crates/lang-ts/tests/conformance.ts:1 adds a comment",
    );
    repository.git(&["rm", "-q", "crates/lang-ts/tests/conformance.ts"]);
    repository.write(
        "crates/lang-ts/src/fixtures/inline.ts",
        "// fixtures outside tests are source\nexport const three = 3;\n",
    );
    repository.commit_all("feat: add a source file named fixtures");
    assert_fails_with(
        &repository.xtask(&["no-comments", "--base", "main"]),
        "crates/lang-ts/src/fixtures/inline.ts:1 adds a comment",
    );
}

#[test]
fn no_comments_checks_staged_lines_for_the_hook() {
    let repository = Repository::new();
    repository.write("install.sh", "#!/usr/bin/env bash\necho hi\n");
    repository.git(&["add", "install.sh"]);
    assert_passes(&repository.xtask(&["no-comments", "--staged"]));
    repository.write("install.sh", "#!/usr/bin/env bash\necho hi # greet\n");
    repository.git(&["add", "install.sh"]);
    assert_fails_with(
        &repository.xtask(&["no-comments", "--staged"]),
        "install.sh:2 adds a comment",
    );
}

#[test]
fn risk_tier_raises_a_skipped_test_and_a_changed_assertion_to_tier_three() {
    let repository = Repository::new();
    repository.write(
        "crates/a/tests/flow.rs",
        "#[test]\nfn adds() {\n    assert_eq!(1 + 1, 2);\n}\n",
    );
    repository.commit_all("test: add flow");
    repository.branch("feature");
    repository.write(
        "crates/a/tests/flow.rs",
        "#[test]\n#[ignore]\nfn adds() {\n    assert_eq!(1 + 1, 3);\n}\n",
    );
    repository.commit_all("test: weaken flow");
    let output = repository.xtask(&["risk-tier", "--base", "main"]);
    assert_passes(&output);
    assert!(
        stdout(&output).starts_with("Tier 3."),
        "{}",
        stdout(&output)
    );
    assert!(stdout(&output).contains("skips a test"));
    assert!(stdout(&output).contains("removes or changes an assertion"));
}

#[test]
fn risk_tier_gives_tier_two_to_ordinary_code_and_tier_zero_to_docs() {
    let repository = Repository::new();
    repository.branch("feature");
    repository.write("README.md", "# Readme\n");
    repository.commit_all("docs: add readme");
    assert!(stdout(&repository.xtask(&["risk-tier", "--base", "main"])).starts_with("Tier 0."));
    repository.write("crates/a/src/lib.rs", "fn a() {}\n");
    repository.commit_all("feat: add a");
    assert!(stdout(&repository.xtask(&["risk-tier", "--base", "main"])).starts_with("Tier 2."));
}

#[test]
fn behavior_has_test_needs_a_test_or_a_reason() {
    let repository = Repository::new();
    repository.branch("feature");
    repository.write("crates/a/src/lib.rs", "fn a() {}\n");
    repository.commit_all("feat: add a");
    repository.write("body-empty.md", "## Summary\n\nAdds a.\n");
    repository.write(
        "body-reason.md",
        "## Fail-first test\n\nNo behavior change: rename only.\n",
    );
    assert_fails_with(
        &repository.xtask(&[
            "behavior-has-test",
            "--base",
            "main",
            "--body-file",
            "body-empty.md",
        ]),
        "changes source files but no tests",
    );
    assert_passes(&repository.xtask(&[
        "behavior-has-test",
        "--base",
        "main",
        "--body-file",
        "body-reason.md",
    ]));
    repository.write("crates/a/src/lib/tests.rs", "#[test]\nfn a() {}\n");
    repository.commit_all("test: cover a");
    assert_passes(&repository.xtask(&[
        "behavior-has-test",
        "--base",
        "main",
        "--body-file",
        "body-empty.md",
    ]));
}

#[test]
fn record_numbers_reject_a_duplicate_and_accept_a_sequence() {
    let repository = Repository::new();
    repository.write("docs/adr/0001-first.md", "# First\n");
    repository.write("docs/adr/0002-a.md", "# A\n");
    repository.write("docs/adr/0002-b.md", "# B\n");
    assert_fails_with(
        &repository.xtask(&["record-numbers"]),
        "Record number 0002 is used by docs/adr/0002-a.md and docs/adr/0002-b.md",
    );
    fs::rename(
        repository.path.join("docs/adr/0002-b.md"),
        repository.path.join("docs/adr/0003-b.md"),
    )
    .expect("rename");
    assert_passes(&repository.xtask(&["record-numbers"]));
}

#[test]
fn agent_entry_links_need_links_to_the_rules_file() {
    let repository = Repository::new();
    repository.write(".rules", "# Agent guidance\n");
    repository.write("AGENTS.md", "# Own rules\n");
    assert_fails_with(
        &repository.xtask(&["agent-entry-links"]),
        "AGENTS.md must be a link to .rules",
    );
    fs::remove_file(repository.path.join("AGENTS.md")).expect("remove");
    link(Path::new(".rules"), &repository.path.join("AGENTS.md"));
    link(Path::new(".rules"), &repository.path.join("CLAUDE.md"));
    assert_passes(&repository.xtask(&["agent-entry-links"]));
}

#[test]
fn skills_link_needs_one_folder_and_pinned_skills() {
    let repository = Repository::new();
    repository.write(".agents/skills/grilling/SKILL.md", "# grilling\n");
    repository.write(".agents/skills/correction/SKILL.md", "# correction\n");
    repository.write(".claude/skills/grilling/SKILL.md", "# copy\n");
    repository.write("skills-lock.json", "{\"version\": 2, \"skills\": {}}");
    let output = repository.xtask(&["skills-link"]);
    assert_fails_with(&output, ".claude/skills must be a link to .agents/skills");
    assert!(stderr(&output).contains("grilling is not pinned in the skills lock file"));
    fs::remove_dir_all(repository.path.join(".claude/skills")).expect("remove copy");
    link(
        Path::new("../.agents/skills"),
        &repository.path.join(".claude/skills"),
    );
    repository.write(
        "skills-lock.json",
        "{\"version\": 2, \"skills\": {\"grilling\": {}}}",
    );
    assert_passes(&repository.xtask(&["skills-link"]));
}

#[test]
fn doc_style_and_placeholders_reject_unfinished_docs() {
    let repository = Repository::new();
    repository.write(
        "docs/agents/git.md",
        "Allowed scopes: <scope list>.\nA dash \u{2014} here.\n",
    );
    repository.commit_all("docs: draft git page");
    assert_fails_with(
        &repository.xtask(&["no-placeholders"]),
        "docs/agents/git.md:1 still holds the placeholder <scope list>",
    );
    assert_fails_with(
        &repository.xtask(&["doc-style"]),
        "docs/agents/git.md:2 uses an em dash",
    );
    repository.write(
        "docs/agents/git.md",
        "Allowed scopes: see checks.toml.\nA dash - here.\n",
    );
    repository.commit_all("docs: finish git page");
    assert_passes(&repository.xtask(&["no-placeholders"]));
    assert_passes(&repository.xtask(&["doc-style"]));
}

#[test]
fn commit_message_range_rejects_attribution() {
    let repository = Repository::new();
    repository.branch("feature");
    repository.write("a.txt", "a\n");
    repository.commit_all("feat(cli): add a\n\nCo-Authored-By: Bot <bot@example.invalid>");
    assert_fails_with(
        &repository.xtask(&["commit-message", "--range", "main"]),
        "it has a co-author line",
    );
    repository.write("message.txt", "fix(cli): refuse an empty path\n");
    assert_passes(&repository.xtask(&["commit-message", "--file", "message.txt"]));
}

#[test]
fn config_access_rejects_direct_reads_outside_the_config_crate() {
    let repository = Repository::new();
    repository.write(
        "crates/cli/src/handler.rs",
        "fn token() -> String {\n    std::env::var(\"TOKEN\").unwrap_or_default()\n}\n",
    );
    repository.write(
        "crates/config/src/lib.rs",
        "fn token() -> String {\n    std::env::var(\"TOKEN\").unwrap_or_default()\n}\n",
    );
    repository.commit_all("feat: read token");
    assert_fails_with(
        &repository.xtask(&["config-access"]),
        "crates/cli/src/handler.rs:2 reads configuration directly",
    );
    let output = repository.xtask(&["config-access"]);
    assert!(!stderr(&output).contains("crates/config/src/lib.rs"));
}

#[test]
fn no_suppressions_rejects_an_allow_attribute() {
    let repository = Repository::new();
    repository.write("crates/a/src/lib.rs", "#[allow(dead_code)]\nfn a() {}\n");
    repository.commit_all("feat: add a");
    assert_fails_with(
        &repository.xtask(&["no-suppressions"]),
        "crates/a/src/lib.rs:1 suppresses allow(dead_code)",
    );
    repository.write("crates/a/src/lib.rs", "pub fn a() {}\n");
    repository.commit_all("fix: use a");
    assert_passes(&repository.xtask(&["no-suppressions"]));
}

#[test]
fn evidence_bundle_rejects_a_summary_only_body() {
    let repository = Repository::new();
    repository.write("body.md", "This change fixes the thing.\n");
    assert_fails_with(
        &repository.xtask(&["evidence-bundle", "--body-file", "body.md"]),
        "lacks the Tier section",
    );
}

#[cfg(unix)]
fn link(target: &Path, at: &Path) {
    std::os::unix::fs::symlink(target, at).expect("create the link");
}
