use codeint_lang_php::Php;
use codeint_model::{LanguagePlugin, Manifest, RepoPath};

#[test]
fn extracts_bin_entry_points_from_composer_manifests() {
    let manifest = |path: &str, text: &str| Manifest {
        path: path.into(),
        text: text.into(),
    };
    let repo_paths = [
        "composer.json",
        "bin/console",
        "bin/worker.php",
        "src/Helper.php",
        "packages/cli/composer.json",
        "packages/cli/scripts/run.php",
        "packages/tool/composer.json",
        "packages/tool/tool.php",
    ]
    .map(|path| RepoPath::new(path).unwrap());

    let manifests = vec![
        manifest(
            "composer.json",
            r#"{"name": "acme/app", "bin": ["bin/console", "./bin/worker.php", "bin/missing.php", "../outside.php", "/abs.php", "bin/console"]}"#,
        ),
        manifest(
            "packages/cli/composer.json",
            r#"{"name": "acme/cli", "bin": ["scripts/run.php"]}"#,
        ),
        manifest(
            "packages/tool/composer.json",
            r#"{"name": "acme/tool", "bin": "tool.php"}"#,
        ),
    ];

    let found: Vec<String> = Php
        .entry_points(&repo_paths, manifests)
        .expect("valid entry points")
        .into_iter()
        .map(|path| path.as_str().to_owned())
        .collect();
    assert_eq!(
        found,
        [
            "bin/console",
            "bin/worker.php",
            "packages/cli/scripts/run.php",
            "packages/tool/tool.php",
        ]
    );
}

#[test]
fn returns_empty_when_composer_manifest_has_no_bin_or_is_invalid() {
    let paths = [
        RepoPath::new("composer.json").unwrap(),
        RepoPath::new("src/App.php").unwrap(),
    ];
    let psr4_only = r#"{"name": "acme/lib", "autoload": {"psr-4": {"Acme\\": "src/"}}}"#;
    let empty_count = [psr4_only, "{invalid"]
        .into_iter()
        .map(|body| {
            Php.entry_points(
                &paths,
                vec![Manifest {
                    path: "composer.json".into(),
                    text: body.into(),
                }],
            )
            .expect("valid empty result")
            .len()
        })
        .sum::<usize>();
    assert_eq!(empty_count, 0);
}

#[test]
fn rejects_parent_traversal_in_nested_composer_bin() {
    let files = [
        RepoPath::new("packages/cli/composer.json").unwrap(),
        RepoPath::new("packages/cli/bin/run.php").unwrap(),
        RepoPath::new("packages/other/secret.php").unwrap(),
    ];
    let entries = Php
        .entry_points(
            &files,
            vec![Manifest {
                path: "packages/cli/composer.json".into(),
                text: r#"{"name": "acme/cli", "bin": ["../other/secret.php", "./../other/secret.php", "bin/../../other/secret.php", "bin/run.php"]}"#.into(),
            }],
        )
        .expect("entry points");
    assert_eq!(
        entries,
        [RepoPath::new("packages/cli/bin/run.php").unwrap()]
    );
}
