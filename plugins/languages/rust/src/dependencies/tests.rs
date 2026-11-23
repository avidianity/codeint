use codeint_model::language::{DeclaredDep, DepKind, Manifest};

use super::{extract_declared, normalize_crate_name};

#[test]
fn crate_name_normalization_maps_hyphens_and_raw_identifiers() {
    assert_eq!(normalize_crate_name("serde-json"), "serde_json");
    assert_eq!(normalize_crate_name("proc-macro2"), "proc_macro2");
    assert_eq!(normalize_crate_name("oxc_parser"), "oxc_parser");
    assert_eq!(normalize_crate_name("r#async"), "async");
}

#[test]
fn polyglot_repo_without_root_cargo_toml_parses_nested_crate_and_skips_invalid() {
    let valid = Manifest {
        path: "native/Cargo.toml".into(),
        text: "[package]\nname = \"native-ext\"\n[dependencies]\nanyhow = \"1\"\n[target.'cfg(windows)'.dev-dependencies]\nwinapi-util = \"0.1\"\n".into(),
    };
    let broken = Manifest {
        path: "broken/Cargo.toml".into(),
        text: "[package\nname = ".into(),
    };

    assert_eq!(
        extract_declared(&[valid, broken]),
        vec![
            DeclaredDep {
                package_root: "native".into(),
                name: "anyhow".into(),
                kind: DepKind::Runtime,
            },
            DeclaredDep {
                package_root: "native".into(),
                name: "winapi_util".into(),
                kind: DepKind::Dev,
            },
        ]
    );
}
