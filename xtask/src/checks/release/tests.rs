use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{
    PLATFORM_PACKAGES, changelog_has_version, dart_codeint_version, format_release_notes,
    prepare_release, prepend_changelog_version, pubspec_version, replace_dart_codeint_version,
    replace_json_string_field, replace_pubspec_version, replace_workspace_package_version,
    version_sync,
};

static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

struct TempTree {
    path: PathBuf,
}

impl TempTree {
    fn new(version: &str) -> Self {
        let id = NEXT_TEMP.fetch_add(1, Ordering::SeqCst);
        let path =
            crate::config::temp_dir().join(format!("xtask-release-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp dir");
        let tree = Self { path };
        tree.seed_all_manifests(version);
        tree
    }

    fn seed_all_manifests(&self, version: &str) {
        let mut files = vec![
            (
                "Cargo.toml".to_string(),
                format!(
                    "[workspace]\nmembers = []\n\n[workspace.package]\nversion = \"{version}\"\nedition = \"2024\"\n"
                ),
            ),
            (
                "sdks/dart/pubspec.yaml".to_string(),
                format!("name: codeint\nversion: {version}\nenvironment:\n  sdk: ^3.6.0\n"),
            ),
            (
                "sdks/dart/CHANGELOG.md".to_string(),
                format!("# Changelog\n\n## {version}\n\n- Initial release.\n"),
            ),
            (
                "sdks/dart/lib/src/options.dart".to_string(),
                format!("import 'enums.dart';\n\nconst String codeintVersion = '{version}';\n"),
            ),
        ];
        let deps = PLATFORM_PACKAGES
            .map(|p| format!("    \"@avidian/codeint-{p}\": \"{version}\""))
            .join(",\n");
        files.push((
            "npm/codeint/package.json".to_string(),
            format!(
                "{{\n  \"name\": \"codeint\",\n  \"version\": \"{version}\",\n  \"optionalDependencies\": {{\n{deps}\n  }}\n}}\n"
            ),
        ));
        for p in PLATFORM_PACKAGES {
            files.push((
                format!("npm/platforms/{p}/package.json"),
                format!(
                    "{{\n  \"name\": \"@avidian/codeint-{p}\",\n  \"version\": \"{version}\"\n}}\n"
                ),
            ));
        }
        for (rel, body) in files {
            self.write(&rel, &body);
        }
    }

    fn write(&self, relative: &str, content: &str) {
        let full = self.path.join(relative);
        fs::create_dir_all(full.parent().expect("parent")).expect("create parent");
        fs::write(full, content).expect("write file");
    }

    fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.path.join(relative)).expect("read file")
    }

    fn root(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn assert_single_violation(tree: &TempTree, expected: &str) {
    let violations = version_sync(tree.root(), None).unwrap();
    assert_eq!(violations.len(), 1);
    assert!(violations[0].contains(expected));
}

#[test]
fn version_sync_passes_when_all_manifests_and_tag_match() {
    let tree = TempTree::new("0.2.0");
    assert_eq!(version_sync(tree.root(), None), Ok(Vec::new()));
    assert_eq!(version_sync(tree.root(), Some("v0.2.0")), Ok(Vec::new()));
}

#[test]
fn version_sync_fails_when_wrapper_version_differs() {
    let tree = TempTree::new("0.2.0");
    let drifted =
        replace_json_string_field(&tree.read("npm/codeint/package.json"), "version", "0.1.0")
            .unwrap();
    tree.write("npm/codeint/package.json", &drifted);
    assert_single_violation(&tree, "npm/codeint/package.json version is \"0.1.0\"");
}

#[test]
fn version_sync_fails_when_optional_dependency_differs() {
    let tree = TempTree::new("0.2.0");
    let drifted = replace_json_string_field(
        &tree.read("npm/codeint/package.json"),
        "@avidian/codeint-linux-x64",
        "0.1.0",
    )
    .unwrap();
    tree.write("npm/codeint/package.json", &drifted);
    assert_single_violation(&tree, "@avidian/codeint-linux-x64");
}

#[test]
fn version_sync_fails_when_platform_manifest_differs() {
    let tree = TempTree::new("0.2.0");
    tree.write(
        "npm/platforms/win32-x64/package.json",
        "{\n  \"name\": \"@avidian/codeint-win32-x64\",\n  \"version\": \"0.1.9\"\n}\n",
    );
    assert_single_violation(
        &tree,
        "npm/platforms/win32-x64/package.json version is \"0.1.9\"",
    );
}

#[test]
fn version_sync_fails_when_tag_disagrees_with_manifest_version() {
    let tree = TempTree::new("0.2.0");
    let violations = version_sync(tree.root(), Some("v0.3.0")).unwrap();
    assert_eq!(violations.len(), 1);
    assert!(
        violations[0].contains("Release tag \"v0.3.0\" disagrees with manifest version \"0.2.0\"")
    );
}

#[test]
fn version_sync_detects_dart_pubspec_options_and_extra_sdk_drift() {
    let cases = [
        (
            "sdks/dart/pubspec.yaml",
            "name: codeint\nversion: 0.7.0\n",
            "sdks/dart/pubspec.yaml version is \"0.7.0\"",
        ),
        (
            "sdks/dart/lib/src/options.dart",
            "const String codeintVersion = '0.7.0';\n",
            "sdks/dart/lib/src/options.dart codeintVersion is \"0.7.0\"",
        ),
        (
            "sdks/flutter/pubspec.yaml",
            "name: codeint_flutter\nversion: 0.7.9\n",
            "sdks/flutter/pubspec.yaml version is \"0.7.9\"",
        ),
    ];
    for (path, content, expected) in cases {
        let tree = TempTree::new("0.8.0");
        tree.write(path, content);
        assert_single_violation(&tree, expected);
    }
}

#[test]
fn format_release_notes_groups_conventional_commits_by_type() {
    let subjects = vec![
        "feat(cli): add diff command (#12)".to_string(),
        "fix(metrics): avoid division by zero (#13)".to_string(),
        "perf(cli): lower files in parallel (#14)".to_string(),
        "feat(cli)!: drop legacy flag (#15)".to_string(),
        "docs(docs): clarify entropy formula (#16)".to_string(),
        "chore: update lockfile".to_string(),
    ];
    let notes = format_release_notes(&subjects, Some("v0.1.0"));
    assert!(notes.contains("### Breaking Changes\n\n- **cli:** drop legacy flag (#15)"));
    assert!(notes.contains("### Features\n\n- **cli:** add diff command (#12)"));
    assert!(notes.contains("### Fixes\n\n- **metrics:** avoid division by zero (#13)"));
    assert!(notes.contains("### Performance\n\n- **cli:** lower files in parallel (#14)"));
    assert!(notes.contains("### Documentation\n\n- **docs:** clarify entropy formula (#16)"));
    assert!(notes.contains("### Other Changes\n\n- update lockfile"));
}

#[test]
fn format_release_notes_handles_empty_commit_list() {
    assert_eq!(
        format_release_notes(&[], Some("v0.1.0")),
        "No changes since v0.1.0.\n"
    );
    assert_eq!(format_release_notes(&[], None), "No changes.\n");
}

#[test]
fn replace_workspace_package_version_updates_only_workspace_package_section() {
    let input = "[package]\nversion = \"0.0.0\"\n\n[workspace.package]\nversion = \"0.1.0\"\nedition = \"2024\"\n";
    let updated = replace_workspace_package_version(input, "0.2.0").unwrap();
    assert_eq!(
        updated,
        "[package]\nversion = \"0.0.0\"\n\n[workspace.package]\nversion = \"0.2.0\"\nedition = \"2024\"\n"
    );
}

#[test]
fn replace_pubspec_and_dart_options_version_helpers_update_cleanly() {
    let pubspec = "name: codeint\nversion: 0.8.0\ndependencies:\n  crypto: ^3.0.8\n";
    let updated_pubspec = replace_pubspec_version(pubspec, "0.9.0").unwrap();
    assert_eq!(pubspec_version(&updated_pubspec), Some("0.9.0"));
    assert!(updated_pubspec.contains("  crypto: ^3.0.8\n"));

    let options = "import 'enums.dart';\n\nconst String codeintVersion = '0.8.0';\n";
    let updated_options = replace_dart_codeint_version(options, "0.9.0").unwrap();
    assert_eq!(dart_codeint_version(&updated_options), Some("0.9.0"));
    assert!(updated_options.contains("const String codeintVersion = '0.9.0';\n"));
}

#[test]
fn prepare_release_updates_dart_pubspec_and_options_and_dry_run_does_not_mutate() {
    let tree = TempTree::new("0.8.0");
    let run_git = |args: &[&str]| {
        let status = Command::new("git")
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(tree.root())
            .status()
            .expect("run git");
        assert!(status.success());
    };
    run_git(&["init", "--quiet"]);
    run_git(&["add", "."]);
    run_git(&["commit", "--quiet", "-m", "chore: initial"]);

    assert!(
        prepare_release(tree.root(), "0.9.0", None, true)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        pubspec_version(&tree.read("sdks/dart/pubspec.yaml")),
        Some("0.8.0")
    );
    assert_eq!(
        dart_codeint_version(&tree.read("sdks/dart/lib/src/options.dart")),
        Some("0.8.0")
    );
    assert!(!changelog_has_version(
        &tree.read("sdks/dart/CHANGELOG.md"),
        "0.9.0"
    ));

    tree.write("CHANGELOG.md", "# Root Changelog\n\n## 0.8.0\n");

    assert!(
        prepare_release(tree.root(), "0.9.0", None, false)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        pubspec_version(&tree.read("sdks/dart/pubspec.yaml")),
        Some("0.9.0")
    );
    assert_eq!(
        dart_codeint_version(&tree.read("sdks/dart/lib/src/options.dart")),
        Some("0.9.0")
    );
    let updated_changelog = tree.read("sdks/dart/CHANGELOG.md");
    assert!(changelog_has_version(&updated_changelog, "0.9.0"));
    assert!(updated_changelog.contains("## 0.9.0\n\n- Release 0.9.0.\n\n## 0.8.0"));
    assert_eq!(tree.read("CHANGELOG.md"), "# Root Changelog\n\n## 0.8.0\n");
    assert_eq!(version_sync(tree.root(), Some("v0.9.0")), Ok(Vec::new()));
}

#[test]
fn pubspec_and_dart_version_parsers_handle_comments_and_quotes() {
    assert_eq!(
        pubspec_version("name: codeint\nversion: 0.8.0 # inline comment\n"),
        Some("0.8.0")
    );
    assert_eq!(
        pubspec_version("name: codeint\nversion: '0.8.0' # inline comment\n"),
        Some("0.8.0")
    );
    assert_eq!(pubspec_version("name: codeint\nversion: '0.8.0\"\n"), None);
    assert_eq!(
        pubspec_version("name: codeint\nversion: # only comment\n"),
        None
    );
    assert_eq!(
        dart_codeint_version("const String codeintVersion = '0.8.0'; // inline comment\n"),
        Some("0.8.0")
    );

    let unquoted = replace_pubspec_version("version: 0.8.0 # inline comment\n", "0.9.0").unwrap();
    assert_eq!(unquoted, "version: 0.9.0 # inline comment\n");
    let quoted = replace_pubspec_version("version: '0.8.0' # inline comment\n", "0.9.0").unwrap();
    assert_eq!(quoted, "version: '0.9.0' # inline comment\n");
    assert!(replace_pubspec_version("version: # only comment\n", "0.9.0").is_err());
}

#[test]
fn version_sync_detects_missing_sdk_changelog_heading_and_prepend_is_idempotent() {
    let tree = TempTree::new("0.8.0");
    tree.write(
        "sdks/dart/CHANGELOG.md",
        "# Changelog\n\n## 0.7.0\n\n- Older.\n",
    );
    assert_single_violation(&tree, "sdks/dart/CHANGELOG.md lacks a \"## 0.8.0\" heading");

    let once = prepend_changelog_version(&tree.read("sdks/dart/CHANGELOG.md"), "0.8.0");
    assert!(changelog_has_version(&once, "0.8.0"));
    assert_eq!(prepend_changelog_version(&once, "0.8.0"), once);
}
