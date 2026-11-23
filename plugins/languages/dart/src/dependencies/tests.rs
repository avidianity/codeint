use codeint_model::language::{DepKind, Manifest};

use super::{extract_declared, normalize_pub_name};

#[test]
fn normalizes_pub_package_names() {
    assert_eq!(normalize_pub_name("  Flutter_Test "), "flutter_test");
    assert_eq!(normalize_pub_name("http"), "http");
}

#[test]
fn handles_empty_invalid_and_sdk_pubspec_sections() {
    let deps = extract_declared(&[
        Manifest {
            path: "pubspec.yaml".into(),
            text: "name: solo\ndependencies:\n  Meta: ^1.15.0\n  empty_constraint:\ndev_dependencies:\n".into(),
        },
        Manifest {
            path: "packages/broken/pubspec.yaml".into(),
            text: "dependencies: [\n".into(),
        },
        Manifest {
            path: ".dart_tool/pubspec.yaml".into(),
            text: "name: ignored\ndependencies:\n  bad: ^1.0.0\n".into(),
        },
    ]);
    let names: Vec<(&str, DepKind)> = deps.iter().map(|d| (d.name.as_str(), d.kind)).collect();
    assert_eq!(
        names,
        [
            ("empty_constraint", DepKind::Runtime),
            ("meta", DepKind::Runtime),
        ]
    );
}
