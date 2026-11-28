use codeint_model::language::Manifest;

use super::discover;

#[test]
fn discovers_pub_workspace_members() {
    let items = vec![
        Manifest {
            path: "pubspec.yaml".into(),
            text: "name: root\nworkspace:\n  - packages/core\n  - packages/ui\n".into(),
        },
        Manifest {
            path: "packages/core/pubspec.yaml".into(),
            text: "name: core\n".into(),
        },
        Manifest {
            path: "packages/ui/pubspec.yaml".into(),
            text: "name: ui\n".into(),
        },
        Manifest {
            path: "examples/demo/pubspec.yaml".into(),
            text: "name: demo\n".into(),
        },
    ];
    assert_eq!(discover(&items).unwrap(), ["packages/core", "packages/ui"]);
}

#[test]
fn handles_single_and_invalid_pubspec() {
    let ok = discover(&[Manifest {
        path: "pubspec.yaml".into(),
        text: "name: solo\n".into(),
    }]);
    assert_eq!(ok.unwrap(), Vec::<String>::new());

    let bad = discover(&[Manifest {
        path: "pubspec.yaml".into(),
        text: "workspace: [\n".into(),
    }])
    .unwrap_err();
    assert!(bad.path == "pubspec.yaml" && bad.message.contains("not valid YAML"));
}

#[test]
fn analyze_manifests_reports_malformed_member_pubspec_and_keeps_valid_siblings() {
    let items: Vec<Manifest> = [
        ("pubspec.yaml", "name: root\nworkspace:\n  - packages/*\n"),
        (
            "packages/good/pubspec.yaml",
            "name: good\ndependencies:\n  http: ^1.0.0\n",
        ),
        ("packages/broken/pubspec.yaml", "name: [broken\n"),
    ]
    .into_iter()
    .map(|(p, t)| Manifest {
        path: p.into(),
        text: t.into(),
    })
    .collect();
    let analysis = super::analyze_manifests(&items);
    assert_eq!(
        analysis.invalid_manifests[0].path,
        "packages/broken/pubspec.yaml"
    );
    assert_eq!(
        analysis.workspace_packages,
        ["packages/broken", "packages/good"]
    );
    assert_eq!(analysis.dependency_manifests.len(), 2);
}
