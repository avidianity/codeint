use codeint_model::language::{ImportRequest, Manifest, Resolution};

use super::resolve_all;

#[test]
fn resolves_dart_builtins_and_normalizes_package_uris() {
    let files = vec!["pubspec.yaml".into(), "lib/app.dart".into()];
    let manifests = vec![Manifest {
        path: "pubspec.yaml".into(),
        text: "name: my_app\n".into(),
    }];
    let reqs = [
        "dart:io",
        "dart:typed_data",
        "package:http/http.dart",
        "package:Collection/src/list.dart",
        "package:my_app/app.dart",
        "package:/bad.dart",
    ]
    .into_iter()
    .map(|specifier| ImportRequest {
        importer: "lib/app.dart".into(),
        specifier: specifier.into(),
    })
    .collect::<Vec<_>>();
    let resolved = resolve_all(&files, &manifests, &reqs);
    assert_eq!(resolved[0], Resolution::Builtin);
    assert_eq!(resolved[1], Resolution::Builtin);
    assert_eq!(resolved[2], Resolution::External("http".into()));
    assert_eq!(resolved[3], Resolution::External("collection".into()));
    assert_eq!(resolved[4], Resolution::Builtin);
    assert!(matches!(resolved[5], Resolution::Unresolved(_)));
}

#[test]
fn single_pass_resolve_imports_isolates_heterogeneous_monorepo_profiles() {
    use codeint_model::PackageFrameworkProfiles;

    let files = ["packages/mobile/lib/app.dart", "packages/core/lib/app.dart"];
    let reqs = files.map(|importer| ImportRequest {
        importer: importer.into(),
        specifier: "app.g.dart".into(),
    });
    let profiles = [PackageFrameworkProfiles {
        package_root: "packages/mobile".into(),
        active_profiles: vec!["flutter".into()],
    }];
    let resolved = super::resolve_dart_imports(&files, &[], &reqs, &profiles);
    assert_eq!(resolved[0], Resolution::Builtin);
    assert!(matches!(resolved[1], Resolution::Unresolved(_)));
}
