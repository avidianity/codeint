use codeint_lang_rust::Rust;
use codeint_model::{LanguagePlugin, Manifest, RepoPath};

#[test]
fn extracts_explicit_and_default_lib_and_bin_targets_from_cargo_manifests() {
    let files: Vec<RepoPath> = [
        "Cargo.toml",
        "src/lib.rs",
        "crates/core/Cargo.toml",
        "crates/core/src/lib.rs",
        "crates/core/src/main.rs",
        "crates/core/src/helper.rs",
        "crates/core/src/bin/tool.rs",
        "crates/core/src/bin/worker/main.rs",
        "crates/core/src/bin/worker/extra.rs",
        "crates/custom/Cargo.toml",
        "crates/custom/src/custom_lib.rs",
        "crates/custom/src/cli_main.rs",
        "crates/custom/src/bin/named.rs",
        "crates/disabled/Cargo.toml",
        "crates/disabled/src/lib.rs",
        "crates/disabled/src/main.rs",
    ]
    .into_iter()
    .map(|raw| RepoPath::new(raw).expect("valid repo path"))
    .collect();

    let manifests = [
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
        ("crates/core/Cargo.toml", "[package]\nname = \"core\"\n"),
        (
            "crates/custom/Cargo.toml",
            "[package]\nname = \"custom\"\nautobins = false\n[lib]\npath = \"src/custom_lib.rs\"\n[[bin]]\nname = \"cli\"\npath = \"src/cli_main.rs\"\n[[bin]]\nname = \"named\"\n[[bin]]\nname = \"missing\"\npath = \"src/missing.rs\"\n",
        ),
        (
            "crates/disabled/Cargo.toml",
            "[package]\nname = \"disabled\"\nautolib = false\nautobins = false\n",
        ),
    ]
    .into_iter()
    .map(|(path, text)| Manifest {
        path: path.to_owned(),
        text: text.to_owned(),
    })
    .collect();

    let found: Vec<String> = Rust
        .entry_points(&files, manifests)
        .expect("valid entry points")
        .into_iter()
        .map(|path| path.as_str().to_owned())
        .collect();
    assert_eq!(
        found,
        [
            "crates/core/src/bin/tool.rs",
            "crates/core/src/bin/worker/main.rs",
            "crates/core/src/lib.rs",
            "crates/core/src/main.rs",
            "crates/custom/src/bin/named.rs",
            "crates/custom/src/cli_main.rs",
            "crates/custom/src/custom_lib.rs",
        ]
    );
}

#[test]
fn handles_root_crate_empty_targets_and_invalid_manifests() {
    let root_files = [
        RepoPath::new("Cargo.toml").unwrap(),
        RepoPath::new("src/lib.rs").unwrap(),
        RepoPath::new("src/main.rs").unwrap(),
        RepoPath::new("src/bin/extra.rs").unwrap(),
    ];
    let root_manifest = Manifest {
        path: "Cargo.toml".into(),
        text: "[package]\nname = \"root_app\"\nautobins = false\n[[bin]]\nname = \"root_app\"\n[[bin]]\nname = \"escape\"\npath = \"../outside.rs\"\n[[bin]]\nname = \"abs\"\npath = \"/etc/passwd\"\n".into(),
    };
    let root_found: Vec<String> = Rust
        .entry_points(&root_files, vec![root_manifest])
        .expect("valid entry points")
        .into_iter()
        .map(|path| path.as_str().to_owned())
        .collect();
    assert_eq!(root_found, ["src/lib.rs", "src/main.rs"]);

    let no_targets = [
        RepoPath::new("Cargo.toml").unwrap(),
        RepoPath::new("src/helper.rs").unwrap(),
    ];
    for text in [
        "[workspace]\nmembers = [\"crates/*\"]\n",
        "[package]\nname = \"empty\"\n",
        "[package\nname =",
    ] {
        let entry = Manifest {
            path: "Cargo.toml".into(),
            text: text.into(),
        };
        assert!(
            Rust.entry_points(&no_targets, vec![entry])
                .expect("valid empty result")
                .is_empty()
        );
    }
}

fn cargo_entries(paths: &[&str], manifest_path: &str, text: &str) -> Vec<String> {
    let repo: Vec<RepoPath> = paths
        .iter()
        .map(|raw| RepoPath::new(*raw).unwrap())
        .collect();
    Rust.entry_points(
        &repo,
        vec![Manifest {
            path: manifest_path.into(),
            text: text.into(),
        }],
    )
    .unwrap()
    .into_iter()
    .map(|entry| entry.as_str().to_owned())
    .collect()
}

#[test]
fn rejects_parent_traversal_in_nested_cargo_paths_and_bin_names() {
    let entries = cargo_entries(
        &[
            "crates/app/Cargo.toml",
            "crates/app/src/main.rs",
            "crates/app/src/secret.rs",
            "crates/app/src/bin/sub/tool.rs",
            "crates/other/src/lib.rs",
            "crates/other/src/main.rs",
        ],
        "crates/app/Cargo.toml",
        "[package]\nname = \"app\"\nautolib = false\nautobins = false\n[lib]\npath = \"../other/src/lib.rs\"\n[[bin]]\nname = \"escape_path\"\npath = \"../other/src/main.rs\"\n[[bin]]\nname = \"../secret\"\n[[bin]]\nname = \"../../other/src/main\"\n[[bin]]\nname = \"sub/tool\"\n[[bin]]\nname = \"..\"\n",
    );
    assert!(entries.is_empty());
}

#[test]
fn includes_default_lib_when_autolib_is_false_and_lib_table_has_no_path() {
    let entries = cargo_entries(
        &[
            "crates/explicit_lib/Cargo.toml",
            "crates/explicit_lib/src/lib.rs",
            "crates/explicit_lib/src/main.rs",
        ],
        "crates/explicit_lib/Cargo.toml",
        "[package]\nname = \"explicit_lib\"\nautolib = false\nautobins = false\n[lib]\ncrate-type = [\"cdylib\"]\n",
    );
    assert_eq!(entries, ["crates/explicit_lib/src/lib.rs"]);
}

#[test]
fn extracts_build_scripts_custom_bin_paths_and_implicit_lib_alongside_bin() {
    let ripgrep_entries = cargo_entries(
        &[
            "Cargo.toml",
            "build.rs",
            "crates/core/main.rs",
            "crates/core/flags/mod.rs",
        ],
        "Cargo.toml",
        "[package]\nname = \"ripgrep\"\nbuild = \"build.rs\"\n[[bin]]\nname = \"rg\"\npath = \"crates/core/main.rs\"\n",
    );
    assert_eq!(ripgrep_entries, ["build.rs", "crates/core/main.rs"]);

    let implicit_build_and_lib = cargo_entries(
        &[
            "crates/cli/Cargo.toml",
            "crates/cli/build.rs",
            "crates/cli/src/lib.rs",
            "crates/cli/src/main.rs",
        ],
        "crates/cli/Cargo.toml",
        "[package]\nname = \"codeint-cli\"\n[[bin]]\nname = \"codeint\"\npath = \"src/main.rs\"\n",
    );
    assert_eq!(
        implicit_build_and_lib,
        [
            "crates/cli/build.rs",
            "crates/cli/src/lib.rs",
            "crates/cli/src/main.rs",
        ]
    );

    let disabled_build = cargo_entries(
        &["Cargo.toml", "build.rs", "src/lib.rs"],
        "Cargo.toml",
        "[package]\nname = \"no_build\"\nbuild = false\n",
    );
    assert_eq!(disabled_build, ["src/lib.rs"]);

    let escaped_build = cargo_entries(
        &["crates/app/Cargo.toml", "build.rs", "crates/app/src/lib.rs"],
        "crates/app/Cargo.toml",
        "[package]\nname = \"app\"\nbuild = \"../build.rs\"\n",
    );
    assert_eq!(escaped_build, ["crates/app/src/lib.rs"]);
}
