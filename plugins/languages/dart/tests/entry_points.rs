use codeint_lang_dart::Dart;
use codeint_model::{LanguagePlugin, Manifest, RepoPath};

#[test]
fn extracts_executables_public_lib_bin_and_hook_entries_from_pubspec() {
    let mut paths = Vec::new();
    for raw in [
        "pubspec.yaml",
        "lib/toolkit.dart",
        "lib/browser.dart",
        "lib/src/internal.dart",
        "lib/src/secret.dart",
        "bin/run_cli.dart",
        "bin/direct.dart",
        "bin/extra.dart",
        "bin/internal/helper.dart",
        "hook/build.dart",
        "hook/link.dart",
        "hook/helper.dart",
        "packages/core/pubspec.yaml",
        "packages/core/lib/core_lib.dart",
        "packages/core/lib/web.dart",
        "packages/core/lib/src/impl.dart",
        "packages/core/bin/serve.dart",
        "packages/core/hook/build.dart",
    ] {
        paths.push(RepoPath::new(raw).unwrap());
    }

    let root = Manifest {
        path: "pubspec.yaml".into(),
        text: "name: toolkit\nexecutables:\n  cli: run_cli\n  direct:\n  missing_cmd: absent\n  escape: ../lib/src/secret\n".into(),
    };
    let core = Manifest {
        path: "packages/core/pubspec.yaml".into(),
        text: "name: core_lib\n".into(),
    };

    let found: Vec<String> = Dart
        .entry_points(&paths, vec![root, core])
        .expect("valid entry points")
        .into_iter()
        .map(|path| path.as_str().to_owned())
        .collect();
    assert_eq!(
        found,
        [
            "bin/direct.dart",
            "bin/extra.dart",
            "bin/run_cli.dart",
            "hook/build.dart",
            "hook/link.dart",
            "lib/browser.dart",
            "lib/toolkit.dart",
            "packages/core/bin/serve.dart",
            "packages/core/hook/build.dart",
            "packages/core/lib/core_lib.dart",
            "packages/core/lib/web.dart",
        ]
    );
}

#[test]
fn returns_empty_when_pubspec_has_no_matching_entry_points_or_is_invalid() {
    let private_only = [
        RepoPath::new("pubspec.yaml").unwrap(),
        RepoPath::new("lib/src/helper.dart").unwrap(),
    ];
    let no_match = Dart
        .entry_points(
            &private_only,
            vec![Manifest {
                path: "pubspec.yaml".into(),
                text: "name: app\n".into(),
            }],
        )
        .expect("valid empty result");
    let with_public_lib = [
        RepoPath::new("pubspec.yaml").unwrap(),
        RepoPath::new("lib/helper.dart").unwrap(),
    ];
    let malformed = Dart
        .entry_points(
            &with_public_lib,
            vec![Manifest {
                path: "pubspec.yaml".into(),
                text: "executables: [\n".into(),
            }],
        )
        .expect("valid empty result");
    assert!(no_match.is_empty() && malformed.is_empty());
}
