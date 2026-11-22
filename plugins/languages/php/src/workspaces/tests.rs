use codeint_model::language::Manifest;

use super::workspace_packages;

#[test]
fn composer_workspace_discovery_and_error_cases() {
    let entries = [
        (
            "composer.json",
            r#"{"repositories":[{"type":"path","url":"packages/*"},{"type":"vcs","url":"https://example.invalid/x"}]}"#,
        ),
        ("packages/auth/composer.json", r#"{"name":"acme/auth"}"#),
        ("packages/billing/composer.json", r#"{"name":"acme/billing"}"#),
        ("vendor/pkg/composer.json", r#"{"name":"vendor/pkg"}"#),
    ]
    .into_iter()
    .map(|(path, text)| Manifest {
        path: path.to_owned(),
        text: text.to_owned(),
    })
    .collect::<Vec<_>>();
    assert_eq!(
        workspace_packages(&entries).expect("workspace"),
        ["packages/auth", "packages/billing"]
    );

    let single = [Manifest {
        path: "composer.json".to_owned(),
        text: r#"{"name":"acme/app"}"#.to_owned(),
    }];
    assert!(workspace_packages(&single).expect("single").is_empty());

    let broken = [Manifest {
        path: "composer.json".to_owned(),
        text: "{not json".to_owned(),
    }];
    let err = workspace_packages(&broken).expect_err("broken");
    assert_eq!(err.path, "composer.json");
    assert!(err.message.contains("is not valid JSON"));
}
