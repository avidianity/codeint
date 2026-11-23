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
