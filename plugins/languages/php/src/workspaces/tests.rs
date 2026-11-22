use codeint_model::language::Manifest;

use super::workspace_packages;

fn manifest(path: &str, text: &str) -> Manifest {
    Manifest {
        path: path.to_owned(),
        text: text.to_owned(),
    }
}

#[test]
fn composer_path_repositories_discover_workspace_packages() {
    let manifests = vec![
        manifest(
            "composer.json",
            r#"{
                "repositories": [
                    {"type": "path", "url": "packages/*"},
                    {"type": "vcs", "url": "https://github.com/example/ext"}
                ]
            }"#,
        ),
        manifest("packages/auth/composer.json", r#"{"name": "acme/auth"}"#),
        manifest(
            "packages/billing/composer.json",
            r#"{"name": "acme/billing"}"#,
        ),
        manifest("vendor/pkg/composer.json", r#"{"name": "vendor/pkg"}"#),
    ];

    let packages = workspace_packages(&manifests).expect("packages");
    assert_eq!(packages, vec!["packages/auth", "packages/billing"]);
}

#[test]
fn single_package_composer_json_returns_no_workspace_packages() {
    let manifests = vec![manifest("composer.json", r#"{"name": "acme/app"}"#)];
    assert!(workspace_packages(&manifests).expect("ok").is_empty());
}

#[test]
fn invalid_root_composer_json_returns_workspace_error() {
    let manifests = vec![manifest("composer.json", "{not json")];
    let err = workspace_packages(&manifests).expect_err("invalid json");
    assert_eq!(err.path, "composer.json");
    assert!(err.message.contains("is not valid JSON"));
}
