use codeint_model::language::Manifest;

use super::workspace_packages;

fn manifest(path: &str, text: &str) -> Manifest {
    Manifest {
        path: path.to_owned(),
        text: text.to_owned(),
    }
}

#[test]
fn workspace_members_globs_and_excludes_select_matching_crates() {
    let manifests = vec![
        manifest(
            "Cargo.toml",
            r#"
[workspace]
members = ["crates/*", "plugins/languages/*", "xtask"]
exclude = ["crates/ignored"]
"#,
        ),
        manifest("crates/cli/Cargo.toml", "[package]\nname = \"cli\"\n"),
        manifest(
            "crates/ignored/Cargo.toml",
            "[package]\nname = \"ignored\"\n",
        ),
        manifest(
            "plugins/languages/rust/Cargo.toml",
            "[package]\nname = \"lang-rust\"\n",
        ),
        manifest("xtask/Cargo.toml", "[package]\nname = \"xtask\"\n"),
        manifest("target/foo/Cargo.toml", "[package]\nname = \"built\"\n"),
    ];

    let packages = workspace_packages(&manifests).expect("workspace packages");
    assert_eq!(
        packages,
        vec!["crates/cli", "plugins/languages/rust", "xtask"]
    );
}

#[test]
fn single_crate_repository_without_workspace_table_returns_no_packages() {
    let manifests = vec![manifest("Cargo.toml", "[package]\nname = \"solo\"\n")];
    assert!(workspace_packages(&manifests).expect("ok").is_empty());
}

#[test]
fn invalid_root_cargo_toml_returns_workspace_error() {
    let manifests = vec![manifest("Cargo.toml", "[workspace\nmembers = [")];
    let error = workspace_packages(&manifests).expect_err("invalid toml");
    assert_eq!(error.path, "Cargo.toml");
    assert!(error.message.contains("is not valid TOML"));
}

#[test]
fn analyze_manifests_filters_virtual_workspace_and_ignored_dirs_in_one_pass() {
    let manifests = vec![
        manifest("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
        manifest(
            "crates/api/Cargo.toml",
            "[package]\nname = \"api\"\n[dependencies]\nserde = \"1\"\n",
        ),
        manifest(
            "target/tmp/Cargo.toml",
            "[package]\nname = \"tmp\"\n[dependencies]\nanyhow = \"1\"\n",
        ),
        manifest(
            "vendor/lib/Cargo.toml",
            "[package]\nname = \"lib\"\n[dependencies]\nlog = \"0.4\"\n",
        ),
    ];

    let analysis = super::analyze_manifests(&manifests);
    assert!(analysis.invalid_manifests.is_empty());
    assert_eq!(analysis.workspace_packages, vec!["crates/api"]);
    assert_eq!(analysis.dependency_manifests.len(), 1);
    assert_eq!(analysis.dependency_manifests[0].package_root, "crates/api");
    assert_eq!(
        analysis.dependency_manifests[0].path,
        "crates/api/Cargo.toml"
    );
    assert_eq!(analysis.declared_dependencies.len(), 1);
    assert_eq!(analysis.declared_dependencies[0].name, "serde");
}
