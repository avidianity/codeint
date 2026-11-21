use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{
    PLATFORM_PACKAGES, format_release_notes, replace_json_string_field,
    replace_workspace_package_version, version_sync,
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
        tree.write(
            "Cargo.toml",
            &format!("[workspace]\nmembers = []\n\n[workspace.package]\nversion = \"{version}\"\nedition = \"2024\"\n"),
        );
        let optional_deps: Vec<String> = PLATFORM_PACKAGES
            .iter()
            .map(|platform| format!("    \"@avidian/codeint-{platform}\": \"{version}\""))
            .collect();
        tree.write(
            "npm/codeint/package.json",
            &format!(
                "{{\n  \"name\": \"codeint\",\n  \"version\": \"{version}\",\n  \"optionalDependencies\": {{\n{}\n  }}\n}}\n",
                optional_deps.join(",\n")
            ),
        );
        for platform in PLATFORM_PACKAGES {
            tree.write(
                &format!("npm/platforms/{platform}/package.json"),
                &format!(
                    "{{\n  \"name\": \"@avidian/codeint-{platform}\",\n  \"version\": \"{version}\"\n}}\n"
                ),
            );
        }
        tree
    }

    fn write(&self, relative: &str, content: &str) {
        let full = self.path.join(relative);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(full, content).expect("write file");
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

#[test]
fn version_sync_passes_when_all_manifests_and_tag_match() {
    let tree = TempTree::new("0.2.0");
    assert_eq!(version_sync(tree.root(), None), Ok(Vec::new()));
    assert_eq!(version_sync(tree.root(), Some("v0.2.0")), Ok(Vec::new()));
}

#[test]
fn version_sync_fails_when_wrapper_version_differs() {
    let tree = TempTree::new("0.2.0");
    let wrapper = fs::read_to_string(tree.root().join("npm/codeint/package.json")).unwrap();
    let drifted = replace_json_string_field(&wrapper, "version", "0.1.0").unwrap();
    tree.write("npm/codeint/package.json", &drifted);
    let violations = version_sync(tree.root(), None).unwrap();
    assert_eq!(violations.len(), 1);
    assert!(violations[0].contains("npm/codeint/package.json version is \"0.1.0\""));
}

#[test]
fn version_sync_fails_when_optional_dependency_differs() {
    let tree = TempTree::new("0.2.0");
    let wrapper = fs::read_to_string(tree.root().join("npm/codeint/package.json")).unwrap();
    let drifted =
        replace_json_string_field(&wrapper, "@avidian/codeint-linux-x64", "0.1.0").unwrap();
    tree.write("npm/codeint/package.json", &drifted);
    let violations = version_sync(tree.root(), None).unwrap();
    assert_eq!(violations.len(), 1);
    assert!(violations[0].contains("@avidian/codeint-linux-x64"));
}

#[test]
fn version_sync_fails_when_platform_manifest_differs() {
    let tree = TempTree::new("0.2.0");
    tree.write(
        "npm/platforms/win32-x64/package.json",
        "{\n  \"name\": \"@avidian/codeint-win32-x64\",\n  \"version\": \"0.1.9\"\n}\n",
    );
    let violations = version_sync(tree.root(), None).unwrap();
    assert_eq!(violations.len(), 1);
    assert!(violations[0].contains("npm/platforms/win32-x64/package.json version is \"0.1.9\""));
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
