use codeint_lang_dart::Dart;
use codeint_model::language::{FrameworkProfileResult, ImportRequest, Manifest, Resolution};
use codeint_model::{FrameworkProfileSelection, LanguagePlugin, RepoPath};

struct DartCase<'a> {
    mode: FrameworkProfileSelection,
    root: &'a str,
    paths: &'a [&'a str],
    pubspecs: &'a [(&'a str, &'a str)],
}

impl DartCase<'_> {
    fn run(self) -> Result<FrameworkProfileResult, codeint_model::LanguagePluginError> {
        let mut files = Vec::with_capacity(self.paths.len());
        for raw in self.paths {
            files.push(RepoPath::new(*raw).unwrap());
        }
        let mut manifests = Vec::with_capacity(self.pubspecs.len());
        for &(path, text) in self.pubspecs {
            manifests.push(Manifest {
                path: path.to_owned(),
                text: text.to_owned(),
            });
        }
        Dart.resolve_framework_profiles(self.root, &files, manifests, self.mode)
    }
}

fn eval_auto(root: &str, paths: &[&str], pubspecs: &[(&str, &str)]) -> FrameworkProfileResult {
    DartCase {
        mode: FrameworkProfileSelection::Auto,
        root,
        paths,
        pubspecs,
    }
    .run()
    .unwrap()
}

#[test]
fn known_framework_profiles_lists_flutter() {
    assert_eq!(Dart.known_framework_profiles(), ["flutter"]);
}

#[test]
fn flutter_app_detects_sdk_dependency_or_flutter_section_entry_points_and_generated_paths() {
    let app = eval_auto(
        "",
        &[
            "pubspec.yaml",
            "lib/main.dart",
            "lib/main_staging.dart",
            "lib/features/home.dart",
            "lib/generated_plugin_registrant.dart",
            "lib/models/user.g.dart",
            "lib/models/user.freezed.dart",
            "lib/gen/assets.gen.dart",
            "lib/l10n/app_localizations.dart",
            "lib/l10n/app_localizations_en.dart",
            ".dart_tool/flutter_gen/gen_l10n/app_localizations.dart",
            "build/app/generated.dart",
            "integration_test/app_test.dart",
        ],
        &[(
            "pubspec.yaml",
            "name: shop_app\ndependencies:\n  flutter:\n    sdk: flutter\nflutter:\n  uses-material-design: true\n",
        )],
    );
    assert_eq!(app.active_profiles, ["flutter"]);
    assert_eq!(
        app.entry_points,
        [
            "integration_test/app_test.dart",
            "lib/main.dart",
            "lib/main_staging.dart",
        ]
    );
    assert_eq!(
        app.generated_paths,
        [
            ".dart_tool/flutter_gen/gen_l10n/app_localizations.dart",
            "build/app/generated.dart",
            "lib/gen/assets.gen.dart",
            "lib/generated_plugin_registrant.dart",
            "lib/l10n/app_localizations.dart",
            "lib/l10n/app_localizations_en.dart",
            "lib/models/user.freezed.dart",
            "lib/models/user.g.dart",
        ]
    );
    assert!(app.reference_ranges.is_empty() && app.reference_range_note.is_none());

    let section_only = eval_auto(
        "",
        &["pubspec.yaml", "lib/main.dart"],
        &[(
            "pubspec.yaml",
            "name: minimal_flutter\nflutter:\n  uses-material-design: true\n",
        )],
    );
    assert_eq!(section_only.active_profiles, ["flutter"]);
    assert_eq!(section_only.entry_points, ["lib/main.dart"]);
}

#[test]
fn flutter_plugin_and_package_distinguish_library_entries_from_apps() {
    let cases = [
        (
            &[
                "pubspec.yaml",
                "lib/camera_plugin.dart",
                "lib/main.dart",
                "lib/src/platform.dart",
            ][..],
            "name: camera_plugin\ndependencies:\n  flutter:\n    sdk: flutter\nflutter:\n  plugin:\n    platforms:\n      android:\n        package: com.example.camera\n",
            "lib/camera_plugin.dart",
        ),
        (
            &["pubspec.yaml", "lib/ui_kit.dart", "lib/src/button.dart"][..],
            "name: ui_kit\ndependencies:\n  flutter:\n    sdk: flutter\n",
            "lib/ui_kit.dart",
        ),
    ];
    for (paths, pubspec, expected_entry) in cases {
        let out = eval_auto("", paths, &[("pubspec.yaml", pubspec)]);
        assert_eq!(out.active_profiles, ["flutter"]);
        assert_eq!(out.entry_points, [expected_entry]);
    }
}

#[test]
fn pure_dart_package_and_monorepo_isolation_never_activate_flutter() {
    let app_pubspec = "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n";
    let empty_cases = [
        (
            "",
            &["pubspec.yaml", "lib/tool.dart", "bin/tool.dart"][..],
            vec![(
                "pubspec.yaml",
                "name: cli_tool\ndependencies:\n  http: ^1.2.0\ndev_dependencies:\n  flutter_test:\n    sdk: flutter\n  flutter_lints: ^5.0.0\n",
            )],
            FrameworkProfileSelection::Auto,
        ),
        (
            "packages/core",
            &["packages/core/pubspec.yaml", "packages/core/lib/core.dart"][..],
            vec![
                (
                    "pubspec.yaml",
                    "name: root\nworkspace:\n  - packages/core\ndependencies:\n  flutter:\n    sdk: flutter\n",
                ),
                (
                    "packages/core/pubspec.yaml",
                    "name: core\nresolution: workspace\ndependencies:\n  collection: ^1.19.0\n",
                ),
            ],
            FrameworkProfileSelection::Auto,
        ),
        (
            "",
            &["pubspec.yaml", "lib/main.dart"][..],
            vec![("pubspec.yaml", app_pubspec)],
            FrameworkProfileSelection::Disabled,
        ),
        (
            "",
            &["pubspec.yaml", "lib/main.dart"][..],
            vec![("pubspec.yaml", app_pubspec)],
            FrameworkProfileSelection::Exclude(vec!["flutter".to_string()]),
        ),
        (
            "",
            &["pubspec.yaml", "lib/main.dart"][..],
            vec![("pubspec.yaml", "dependencies: [\n")],
            FrameworkProfileSelection::Auto,
        ),
    ];
    for (root, paths, manifests, mode) in empty_cases {
        let out = DartCase {
            mode,
            root,
            paths,
            pubspecs: &manifests,
        }
        .run()
        .unwrap();
        assert!(out.active_profiles.is_empty());
        assert!(out.entry_points.is_empty());
        assert!(out.generated_paths.is_empty());
    }

    assert!(
        DartCase {
            mode: FrameworkProfileSelection::Only(vec!["jaspr".to_string()]),
            root: "",
            paths: &["lib/main.dart"],
            pubspecs: &[],
        }
        .run()
        .is_err()
    );
}

#[test]
fn flutter_fallback_import_hints_preserve_flutter_external_and_handle_generated_targets() {
    let manifest = vec![Manifest {
        path: "pubspec.yaml".to_string(),
        text: "name: my_app\ndependencies:\n  flutter:\n    sdk: flutter\n".to_string(),
    }];
    let base_paths: Vec<RepoPath> = [
        "pubspec.yaml",
        "lib/main.dart",
        "lib/models/user.dart",
        "lib/models/existing.g.dart",
    ]
    .into_iter()
    .map(|p| RepoPath::new(p).unwrap())
    .collect();

    let requests: Vec<ImportRequest> = [
        ("lib/main.dart", "package:flutter/material.dart"),
        ("lib/models/user.dart", "existing.g.dart"),
        ("lib/models/user.dart", "user.g.dart"),
        ("lib/models/user.dart", "user.freezed.dart"),
        ("lib/models/user.dart", "missing.dart"),
        (
            "lib/main.dart",
            "package:flutter_gen/gen_l10n/app_localizations.dart",
        ),
    ]
    .into_iter()
    .map(|(importer, specifier)| ImportRequest {
        importer: importer.to_string(),
        specifier: specifier.to_string(),
    })
    .collect();

    let flutter_profiles = vec![codeint_model::PackageFrameworkProfiles {
        package_root: String::new(),
        active_profiles: vec!["flutter".to_string()],
    }];
    let with_flutter = Dart
        .resolve_imports(
            &base_paths,
            manifest.clone(),
            requests.clone(),
            &flutter_profiles,
        )
        .unwrap();
    assert_eq!(with_flutter[0], Resolution::External("flutter".to_string()));
    assert_eq!(
        with_flutter[1],
        Resolution::Resolved(vec!["lib/models/existing.g.dart".to_string()])
    );
    assert_eq!(with_flutter[2], Resolution::Builtin);
    assert_eq!(with_flutter[3], Resolution::Builtin);
    assert!(matches!(with_flutter[4], Resolution::Unresolved(_)));
    assert_eq!(with_flutter[5], Resolution::Builtin);

    let without_flutter = Dart
        .resolve_imports(&base_paths, manifest.clone(), requests, &[])
        .unwrap();
    assert!(matches!(without_flutter[2], Resolution::Unresolved(_)));
    assert_eq!(
        without_flutter[5],
        Resolution::External("flutter_gen".to_string())
    );

    let with_gen_paths: Vec<RepoPath> = [
        "pubspec.yaml",
        "lib/main.dart",
        ".dart_tool/flutter_gen/gen_l10n/app_localizations.dart",
    ]
    .into_iter()
    .map(|p| RepoPath::new(p).unwrap())
    .collect();
    let gen_resolved = Dart
        .resolve_imports(
            &with_gen_paths,
            manifest,
            vec![ImportRequest {
                importer: "lib/main.dart".to_string(),
                specifier: "package:flutter_gen/gen_l10n/app_localizations.dart".to_string(),
            }],
            &flutter_profiles,
        )
        .unwrap();
    assert_eq!(
        gen_resolved,
        vec![Resolution::Resolved(vec![
            ".dart_tool/flutter_gen/gen_l10n/app_localizations.dart".to_string(),
        ])]
    );
}
