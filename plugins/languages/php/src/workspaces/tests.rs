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

#[test]
fn analyze_manifests_reports_malformed_member_composer_json_and_keeps_valid_siblings() {
    let mut manifests = vec![Manifest {
        path: "composer.json".into(),
        text: r#"{"repositories":[{"type":"path","url":"packages/*"}]}"#.into(),
    }];
    manifests.push(Manifest {
        path: "packages/good/composer.json".into(),
        text: r#"{"name":"acme/good","require":{"monolog/monolog":"^3.0"}}"#.into(),
    });
    manifests.push(Manifest {
        path: "packages/broken/composer.json".into(),
        text: "{not json".into(),
    });
    let analysis = super::analyze_manifests(&manifests);
    assert_eq!(
        analysis.invalid_manifests[0].path,
        "packages/broken/composer.json"
    );
    assert_eq!(
        analysis.workspace_packages,
        ["packages/broken", "packages/good"]
    );
    assert_eq!(analysis.dependency_manifests.len(), 2);
    assert_eq!(analysis.declared_dependencies[0].name, "monolog/monolog");
}
