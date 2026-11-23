use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{Codeowners, OwnerFilter};

struct TempFolder(PathBuf);

impl TempFolder {
    fn new(name: &str) -> TempFolder {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "codeint-codeowners-{name}-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp folder");
        TempFolder(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_owners_table(rules: &str, cases: &[(&str, &[&str])]) {
    let codeowners = Codeowners::parse(rules);
    for (path, expected) in cases {
        let expected: Vec<String> = expected.iter().map(|s| (*s).to_owned()).collect();
        assert_eq!(codeowners.owners_for(path), expected, "path: {path}");
    }
}

#[test]
fn load_returns_none_when_no_codeowners_file_exists() {
    let dir = TempFolder::new("none");
    let loaded = Codeowners::load(dir.path()).expect("load succeeds");
    assert!(loaded.is_none());
}

#[test]
fn load_prefers_github_over_root_over_docs() {
    let dir = TempFolder::new("precedence");
    fs::create_dir_all(dir.path().join("docs")).expect("create docs");
    fs::create_dir_all(dir.path().join(".github")).expect("create .github");
    for (file, team) in [
        ("docs/CODEOWNERS", "@docs-team"),
        ("CODEOWNERS", "@root-team"),
        (".github/CODEOWNERS", "@github-team"),
    ] {
        fs::write(dir.path().join(file), format!("* {team}\n")).expect("write CODEOWNERS");
        let loaded = Codeowners::load(dir.path())
            .expect("load codeowners")
            .expect("found codeowners");
        assert_eq!(loaded.owners_for("src/a.ts"), vec![team]);
    }
}

#[test]
fn last_matching_pattern_wins_and_empty_owners_clears_ownership() {
    assert_owners_table(
        "\
# Global default
* @global-owner1 @global-owner2
*.js @js-owner
src/special.js @special-owner
src/unowned.js
",
        &[
            ("README.md", &["@global-owner1", "@global-owner2"]),
            ("src/app.js", &["@js-owner"]),
            ("src/special.js", &["@special-owner"]),
            ("src/unowned.js", &[]),
        ],
    );
}

#[test]
fn anchored_paths_match_only_from_repository_root() {
    assert_owners_table(
        "\
/build/logs/ @logs-team
/root-only.ts @root-owner
docs/guide.md @docs-owner
",
        &[
            ("build/logs/app.log", &["@logs-team"]),
            ("build/logs/nested/app.log", &["@logs-team"]),
            ("sub/build/logs/app.log", &[]),
            ("root-only.ts", &["@root-owner"]),
            ("src/root-only.ts", &[]),
            ("docs/guide.md", &["@docs-owner"]),
            ("pkg/docs/guide.md", &[]),
        ],
    );
}

#[test]
fn directory_and_wildcard_patterns_follow_codeowners_rules() {
    assert_owners_table(
        "\
docs/* @docs-shallow
apps/ @apps-team
**/logs/** @logs-deep
src/**/test/*.ts @test-team
",
        &[
            ("docs/a.md", &["@docs-shallow"]),
            ("docs/sub/b.md", &[]),
            ("apps/web/a.ts", &["@apps-team"]),
            ("pkg/apps/web/a.ts", &["@apps-team"]),
            ("logs/a.txt", &["@logs-deep"]),
            ("a/b/logs/c/d.txt", &["@logs-deep"]),
            ("src/test/a.ts", &["@test-team"]),
            ("src/pkg/mod/test/a.ts", &["@test-team"]),
        ],
    );
}

#[test]
fn inline_comments_and_escaped_spaces_are_parsed() {
    assert_owners_table(
        "\
src/my\\ file.ts @space-owner # trailing comment
*.ts @ts-owner # another comment
",
        &[("src/my file.ts", &["@ts-owner"])],
    );
    assert_owners_table(
        "src/my\\ file.ts @space-owner # trailing comment\n",
        &[("src/my file.ts", &["@space-owner"])],
    );
}

#[test]
fn owner_filter_matches_case_insensitively_and_supports_wildcards() {
    let exact = OwnerFilter::new("@Org/Backend").expect("exact filter");
    assert!(exact.matches("@org/backend"));
    assert!(exact.matches("@ORG/BACKEND"));
    assert!(!exact.matches("@org/frontend"));

    let glob = OwnerFilter::new("@org/*-core").expect("glob filter");
    assert!(glob.matches("@org/api-core"));
    assert!(glob.matches("@ORG/UI-CORE"));
    assert!(!glob.matches("@other/api-core"));
}

#[test]
fn escaped_glob_metacharacters_match_literals_and_compile() {
    assert_owners_table(
        "\
app/users/\\[id\\]/page.ts @users-team
app/routes/\\[slug\\] @routes-team
src/a\\[b.ts @unclosed-team
src/star\\*.ts @star-team
src/q\\?.ts @question-team
src/\\{a,b\\}.ts @brace-team
",
        &[
            ("app/users/[id]/page.ts", &["@users-team"]),
            ("app/users/i/page.ts", &[]),
            ("app/routes/[slug]/index.ts", &["@routes-team"]),
            ("app/routes/s/index.ts", &[]),
            ("src/a[b.ts", &["@unclosed-team"]),
            ("src/star*.ts", &["@star-team"]),
            ("src/starX.ts", &[]),
            ("src/q?.ts", &["@question-team"]),
            ("src/q1.ts", &[]),
            ("src/{a,b}.ts", &["@brace-team"]),
            ("src/a.ts", &[]),
        ],
    );
}

#[test]
fn leading_utf8_bom_is_stripped_before_parsing_rules() {
    assert_owners_table(
        "\u{feff}/src/ @src-team\n",
        &[("src/app.ts", &["@src-team"]), ("other/app.ts", &[])],
    );
    assert_owners_table(
        "\u{feff}* @global-team\n",
        &[("README.md", &["@global-team"])],
    );
    assert_owners_table(
        "\u{feff}# leading comment\nsrc/a.ts @a-team\n",
        &[("src/a.ts", &["@a-team"]), ("src/b.ts", &[])],
    );
}
