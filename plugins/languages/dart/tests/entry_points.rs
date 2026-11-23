use codeint_lang_dart::Dart;
use codeint_model::{LanguagePlugin, Manifest, RepoPath};

#[test]
fn extracts_executables_default_lib_and_bin_entries_from_pubspec() {
    let mut paths = Vec::new();
    for raw in [
        "pubspec.yaml",
        "lib/toolkit.dart",
        "lib/src/internal.dart",
        "lib/secret.dart",
        "bin/run_cli.dart",
        "bin/direct.dart",
        "bin/extra.dart",
        "bin/internal/helper.dart",
        "packages/core/pubspec.yaml",
        "packages/core/lib/core_lib.dart",
        "packages/core/lib/src/impl.dart",
        "packages/core/bin/serve.dart",
    ] {
        paths.push(RepoPath::new(raw).unwrap());
    }

    let root = Manifest {
        path: "pubspec.yaml".into(),
        text: "name: toolkit\nexecutables:\n  cli: run_cli\n  direct:\n  missing_cmd: absent\n  escape: ../lib/secret\n".into(),
    };
    let core = Manifest {
        path: "packages/core/pubspec.yaml".into(),
        text: "name: core_lib\n".into(),
    };

    assert_eq!(
        Dart.entry_points(&paths, vec![root, core]),
        [
            "bin/direct.dart",
            "bin/extra.dart",
            "bin/run_cli.dart",
            "lib/toolkit.dart",
            "packages/core/bin/serve.dart",
            "packages/core/lib/core_lib.dart",
        ]
    );
}

#[test]
fn returns_empty_when_pubspec_has_no_matching_entry_points_or_is_invalid() {
    let paths = [
        RepoPath::new("pubspec.yaml").unwrap(),
        RepoPath::new("lib/helper.dart").unwrap(),
    ];
    let no_match = Dart.entry_points(
        &paths,
        vec![Manifest {
            path: "pubspec.yaml".into(),
            text: "name: app\n".into(),
        }],
    );
    let malformed = Dart.entry_points(
        &paths,
        vec![Manifest {
            path: "pubspec.yaml".into(),
            text: "executables: [\n".into(),
        }],
    );
    assert!(no_match.is_empty() && malformed.is_empty());
}
