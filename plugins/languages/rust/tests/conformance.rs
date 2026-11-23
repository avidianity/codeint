use codeint_lang_rust::{Rust, flagged_line_rules};
use codeint_model::language::{DeclaredDep, DepKind, Function, ImportRequest, Resolution};
use codeint_model::{
    FileModel, LanguagePlugin, LowerError, Manifest, RepoPath, SourceSet, TokenKind,
};

fn plugin() -> &'static dyn LanguagePlugin {
    &Rust
}

fn lower(path: &str, text: &str) -> Result<FileModel, LowerError> {
    plugin().lower(&RepoPath::new(path).expect("valid path"), text)
}

#[test]
fn declares_rs_extension_and_calibrated_reference_ranges() {
    assert_eq!(plugin().extensions(), vec!["rs"]);
    let ranges = plugin().reference_ranges().expect("valid");
    for metric in ["erosion", "verbosity", "duplication", "import_cycles"] {
        assert!(ranges.for_metric(metric).is_some(), "{metric}");
    }
}

#[test]
fn rejects_non_rust_and_unparsable_files() {
    assert!(matches!(
        lower("src/app.ts", "export const x = 1;\n"),
        Err(LowerError::UnsupportedFile { .. })
    ));
    assert!(matches!(
        lower("src/broken.rs", "fn broken( {\n"),
        Err(LowerError::Unparsable { message }) if message.contains("line 1")
    ));
}

#[test]
fn empty_file_produces_empty_model() {
    let model = lower("src/empty.rs", "").expect("empty");
    assert_eq!(model.source_set, SourceSet::Production);
    assert_eq!(model.source_lines, 0);
    assert!(model.functions.is_empty());
    assert!(model.tokens.is_empty());
    assert!(model.flagged_lines.is_empty());
    assert!(model.runtime_imports.is_empty());
}

#[test]
fn counts_decision_points_and_nested_functions_separately() {
    let source = concat!(
        "fn outer(x: i32, y: Option<i32>) -> Result<i32, i32> {\n",
        "    if x > 0 && x < 10 {\n",
        "        for i in 0..x {\n",
        "            let _ = i;\n",
        "        }\n",
        "    }\n",
        "    fn inner(a: bool) -> i32 {\n",
        "        if a || !a { 1 } else { 0 }\n",
        "    }\n",
        "    let v = y.ok_or(1)?;\n",
        "    match v {\n",
        "        n if n > 5 => Ok(inner(true)),\n",
        "        0 => Ok(0),\n",
        "        _ => Err(1),\n",
        "    }\n",
        "}\n",
    );
    let model = lower("src/logic.rs", source).expect("lower");
    assert_eq!(
        model.functions,
        vec![
            Function {
                line: 1,
                end_line: 16,
                decision_points: 7,
                source_lines: 16,
                name: Some("outer".to_owned()),
                cognitive_complexity: 5,
            },
            Function {
                line: 7,
                end_line: 9,
                decision_points: 2,
                source_lines: 3,
                name: Some("inner".to_owned()),
                cognitive_complexity: 3,
            },
        ]
    );
}

#[test]
fn excludes_inline_cfg_test_blocks_from_production_files() {
    let source = concat!(
        "use crate::helper;\n",
        "\n",
        "pub fn add(a: i32, b: i32) -> i32 {\n",
        "    helper::sum(a, b)\n",
        "}\n",
        "\n",
        "#[cfg(test)]\n",
        "mod tests {\n",
        "    use crate::test_only;\n",
        "    #[test]\n",
        "    fn check_add() {\n",
        "        if true { return; } else { let _ = test_only::run(); }\n",
        "    }\n",
        "}\n",
    );
    let model = lower("src/math.rs", source).expect("lower");
    assert_eq!(model.source_set, SourceSet::Production);
    assert_eq!(model.source_lines, 4);
    assert_eq!(model.functions.len(), 1);
    assert_eq!(model.functions[0].line, 3);
    assert!(model.flagged_lines.is_empty());
    assert_eq!(model.runtime_imports, vec!["use:crate::helper"]);

    let test_file = lower("tests/math.rs", source).expect("lower test file");
    assert_eq!(test_file.source_set, SourceSet::Test);
    assert_eq!(test_file.functions.len(), 2);
}

#[test]
fn marks_use_declarations_as_import_or_export_and_normalizes_crlf() {
    let lf = "use std::fmt;\nfn greet(name: &str) -> usize {\n    name.len()\n}\n";
    let crlf = lf.replace('\n', "\r\n");
    let model_lf = lower("src/greet.rs", lf).expect("lf");
    let model_crlf = lower("src/greet.rs", &crlf).expect("crlf");
    assert_eq!(model_lf, model_crlf);
    assert!(
        model_lf
            .tokens
            .iter()
            .filter(|t| t.line == 1)
            .all(|t| t.kind == TokenKind::ImportOrExport)
    );
    assert!(
        model_lf
            .tokens
            .iter()
            .any(|t| t.line == 2 && t.kind == TokenKind::Identifier)
    );
}

#[test]
fn flagged_line_rules_catch_verbose_patterns_without_false_positives() {
    let mut declared: Vec<&str> = flagged_line_rules().collect();
    declared.sort();
    assert_eq!(
        declared,
        vec![
            "else-after-return",
            "if-else-returns-boolean",
            "match-only-propagates",
            "match-yields-boolean",
            "redundant-closure",
            "trailing-return",
        ]
    );

    let cases: [(&str, &str, &str); 6] = [
        (
            "else-after-return",
            "fn f(x: i32) -> i32 {\n    if x > 0 {\n        return 1;\n    } else {\n        2\n    }\n}\n",
            "fn f(x: i32) -> i32 {\n    if x > 0 {\n        1\n    } else {\n        2\n    }\n}\n",
        ),
        (
            "if-else-returns-boolean",
            "fn f(x: i32) -> bool {\n    if x > 0 { true } else { false }\n}\n",
            "fn f(x: Option<i32>) -> bool {\n    if let Some(_) = x { true } else { false }\n}\n",
        ),
        (
            "trailing-return",
            "fn f(x: i32) -> i32 {\n    return x + 1;\n}\n",
            "fn f(x: i32) -> i32 {\n    if x == 0 { return 0; }\n    x + 1\n}\n",
        ),
        (
            "match-only-propagates",
            "fn f(r: Result<i32, i32>) -> Result<i32, i32> {\n    let v = match r {\n        Ok(v) => v,\n        Err(e) => return Err(e),\n    };\n    Ok(v)\n}\n",
            "fn f(r: Result<i32, i32>) -> Result<i32, i32> {\n    let v = match r {\n        Ok(v) => v + 1,\n        Err(e) => return Err(e),\n    };\n    Ok(v)\n}\n",
        ),
        (
            "match-yields-boolean",
            "fn f(b: bool) -> bool {\n    match b {\n        true => false,\n        false => true,\n    }\n}\n",
            "fn f(x: i32) -> bool {\n    match x {\n        1 => true,\n        _ => false,\n    }\n}\n",
        ),
        (
            "redundant-closure",
            "fn g(x: i32) -> i32 { x }\nfn f(v: Option<i32>) -> Option<i32> {\n    v.map(|x| g(x))\n}\n",
            "fn g(x: i32, y: i32) -> i32 { x + y }\nfn f(v: Option<i32>) -> Option<i32> {\n    v.map(|x| g(x, 1))\n}\n",
        ),
    ];

    for (rule, flagged_src, clean_src) in cases {
        let bad = lower("src/bad.rs", flagged_src).expect("flagged lowers");
        assert!(
            bad.flagged_lines.iter().any(|line| line.rule == rule),
            "expected rule {rule} to flag:\n{flagged_src}"
        );
        let good = lower("src/good.rs", clean_src).expect("clean lowers");
        assert!(
            !good.flagged_lines.iter().any(|line| line.rule == rule),
            "did not expect rule {rule} to flag:\n{clean_src}"
        );
    }
}

#[test]
fn resolves_intra_crate_mod_and_use_imports_and_marks_external_crates() {
    let paths: Vec<RepoPath> = [
        "Cargo.toml",
        "src/lib.rs",
        "src/alpha.rs",
        "src/beta/mod.rs",
        "src/beta/child.rs",
    ]
    .into_iter()
    .map(|p| RepoPath::new(p).expect("valid"))
    .collect();
    let manifests = vec![Manifest {
        path: "Cargo.toml".to_owned(),
        text: "[package]\nname = \"demo\"\n".to_owned(),
    }];
    let requests = vec![
        ImportRequest {
            importer: "src/lib.rs".to_owned(),
            specifier: "mod:alpha".to_owned(),
        },
        ImportRequest {
            importer: "src/lib.rs".to_owned(),
            specifier: "mod:beta".to_owned(),
        },
        ImportRequest {
            importer: "src/alpha.rs".to_owned(),
            specifier: "use:crate::beta::child::run".to_owned(),
        },
        ImportRequest {
            importer: "src/beta/child.rs".to_owned(),
            specifier: "use:super::super::alpha::Value".to_owned(),
        },
        ImportRequest {
            importer: "src/alpha.rs".to_owned(),
            specifier: "use:std::collections::HashMap".to_owned(),
        },
        ImportRequest {
            importer: "src/alpha.rs".to_owned(),
            specifier: "use:serde::Deserialize".to_owned(),
        },
    ];

    let resolutions = plugin().resolve_imports(&paths, manifests, requests);
    assert_eq!(
        resolutions,
        vec![
            Resolution::Resolved("src/alpha.rs".to_owned()),
            Resolution::Resolved("src/beta/mod.rs".to_owned()),
            Resolution::Resolved("src/beta/child.rs".to_owned()),
            Resolution::Resolved("src/alpha.rs".to_owned()),
            Resolution::Builtin,
            Resolution::External("serde".to_owned()),
        ]
    );
}

#[test]
fn named_functions_impl_methods_and_trait_defaults_report_expected_names() {
    let source = concat!(
        "fn free_fn() -> i32 { 1 }\n",
        "struct Widget;\n",
        "impl Widget {\n",
        "    fn compute(&self) -> i32 { 2 }\n",
        "}\n",
        "trait Runner {\n",
        "    fn required(&self);\n",
        "    fn default_run(&self) -> i32 { 3 }\n",
        "}\n",
    );
    let model = lower("src/names.rs", source).expect("lower");
    let names: Vec<Option<String>> = model.functions.into_iter().map(|f| f.name).collect();
    assert_eq!(
        names,
        vec![
            Some("free_fn".to_owned()),
            Some("compute".to_owned()),
            Some("default_run".to_owned()),
        ]
    );
}

#[test]
fn cognitive_complexity_distinguishes_flat_match_from_nested_control_flow_and_isolates_nested_fn() {
    let source = r#"
fn flat_match(x: i32) -> i32 {
    match x {
        1 => 10,
        2 => 20,
        3 => 30,
        4 => 40,
        _ => 0,
    }
}
fn deeply_nested(a: bool, b: bool, c: bool, d: bool) -> i32 {
    if a {
        if b {
            if c {
                if d {
                    return 1;
                }
            }
        }
    }
    0
}
fn outer(items: &[i32], a: bool, b: bool, c: bool) -> i32 {
    if a && b || c {
        'outer: for &item in items {
            if item < 0 {
                break 'outer;
            } else if item == 0 {
                continue 'outer;
            } else {
                fn inner(v: i32) -> i32 {
                    if v > 0 {
                        if v > 1 {
                            return v;
                        }
                    }
                    0
                }
                let _ = items.iter().filter(|&&x| {
                    if x > 5 {
                        true
                    } else {
                        false
                    }
                }).count();
                return inner(item) + outer(&[], false, b, c);
            }
        }
    }
    0
}
"#;
    let model = lower("src/cognitive.rs", source).expect("lower");
    let summary: Vec<(Option<&str>, u32)> = model
        .functions
        .iter()
        .map(|f| (f.name.as_deref(), f.cognitive_complexity))
        .collect();
    assert_eq!(
        summary,
        vec![
            (Some("flat_match"), 1),
            (Some("deeply_nested"), 10),
            (Some("outer"), 19),
            (Some("inner"), 3),
        ]
    );
}

fn assert_rust_declared_deps(manifests: &[(&str, &str)], expected: &[(&str, &str, DepKind)]) {
    let input: Vec<Manifest> = manifests
        .iter()
        .map(|(path, text)| Manifest {
            path: (*path).to_owned(),
            text: (*text).to_owned(),
        })
        .collect();
    let actual = plugin()
        .declared_dependencies(input)
        .expect("declared dependencies");
    let expected_deps: Vec<DeclaredDep> = expected
        .iter()
        .map(|(package_root, name, kind)| DeclaredDep {
            package_root: (*package_root).to_owned(),
            name: (*name).to_owned(),
            kind: *kind,
        })
        .collect();
    assert_eq!(actual, expected_deps);
}

#[test]
fn rust_builtins_external_crate_normalization_and_workspace_imports() {
    let lowered = lower(
        "crates/app/src/main.rs",
        "extern crate alloc;\nextern crate serde_json;\nuse proc_macro2::Span;\n",
    )
    .expect("extern crate lowers");
    assert_eq!(
        lowered.runtime_imports,
        vec!["use:alloc", "use:proc_macro2::Span", "use:serde_json"]
    );

    let paths: Vec<RepoPath> = [
        "Cargo.toml",
        "crates/core/Cargo.toml",
        "crates/core/src/lib.rs",
        "crates/app/Cargo.toml",
        "crates/app/src/lib.rs",
        "crates/app/src/rules.rs",
        "crates/app/src/main.rs",
    ]
    .into_iter()
    .map(|p| RepoPath::new(p).expect("valid"))
    .collect();
    let manifests: Vec<Manifest> = [
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
        (
            "crates/core/Cargo.toml",
            "[package]\nname = \"demo-core\"\n",
        ),
        ("crates/app/Cargo.toml", "[package]\nname = \"demo-app\"\n"),
    ]
    .into_iter()
    .map(|(path, text)| Manifest {
        path: path.to_owned(),
        text: text.to_owned(),
    })
    .collect();
    let requests: Vec<ImportRequest> = [
        ("crates/app/src/main.rs", "use:std::collections::HashMap"),
        ("crates/app/src/main.rs", "use:core::fmt"),
        ("crates/app/src/main.rs", "use:alloc::vec::Vec"),
        ("crates/app/src/main.rs", "use:proc_macro::TokenStream"),
        ("crates/app/src/main.rs", "use:test::Bencher"),
        ("crates/app/src/main.rs", "use:serde_json::Value"),
        ("crates/app/src/main.rs", "use:proc_macro2::Span"),
        ("crates/app/src/main.rs", "use:r#async::Task"),
        ("crates/app/src/main.rs", "use:demo_core::Model"),
        ("crates/app/src/lib.rs", "use:rules::flag"),
        ("crates/core/src/lib.rs", "use:demo_core::SelfRef"),
    ]
    .into_iter()
    .map(|(importer, specifier)| ImportRequest {
        importer: importer.to_owned(),
        specifier: specifier.to_owned(),
    })
    .collect();

    assert_eq!(
        plugin().resolve_imports(&paths, manifests, requests),
        vec![
            Resolution::Builtin,
            Resolution::Builtin,
            Resolution::Builtin,
            Resolution::Builtin,
            Resolution::Builtin,
            Resolution::External("serde_json".to_owned()),
            Resolution::External("proc_macro2".to_owned()),
            Resolution::External("async".to_owned()),
            Resolution::Resolved("crates/core/src/lib.rs".to_owned()),
            Resolution::Resolved("crates/app/src/rules.rs".to_owned()),
            Resolution::Builtin,
        ]
    );
}

#[test]
fn single_crate_cargo_toml_parses_runtime_dev_build_optional_and_target_dependencies() {
    assert_rust_declared_deps(
        &[
            (
                "Cargo.toml",
                r#"
[package]
name = "solo-app"

[dependencies]
serde-json = "1.0"
tracing = { version = "0.1", optional = true }
my-alias = { package = "actual-pkg", version = "1.0" }
local-helper = { path = "../helper" }

[dev-dependencies]
insta = "1.40"
assert-cmd = "2.0"

[build-dependencies]
cc = "1.0"

[target.'cfg(unix)'.dependencies]
nix = "0.29"
libc = { version = "0.2", optional = true }

[target.'cfg(unix)'.build-dependencies]
pkg-config = "0.3"
"#,
            ),
            (
                "target/debug/Cargo.toml",
                "[package]\nname = \"built\"\n[dependencies]\nignored = \"1\"\n",
            ),
            (
                "tests/fixtures/sample/Cargo.toml",
                "[package]\nname = \"fixture\"\n[dependencies]\nfixture-dep = \"1\"\n",
            ),
        ],
        &[
            ("", "cc", DepKind::Runtime),
            ("", "my_alias", DepKind::Runtime),
            ("", "nix", DepKind::Runtime),
            ("", "pkg_config", DepKind::Runtime),
            ("", "serde_json", DepKind::Runtime),
            ("", "assert_cmd", DepKind::Dev),
            ("", "insta", DepKind::Dev),
            ("", "libc", DepKind::Optional),
            ("", "tracing", DepKind::Optional),
            ("tests/fixtures/sample", "fixture_dep", DepKind::Runtime),
        ],
    );
}

#[test]
fn workspace_cargo_tomls_emit_per_member_deps_and_exclude_local_crates() {
    assert_rust_declared_deps(
        &[
            (
                "Cargo.toml",
                r#"
[workspace]
members = ["crates/*"]
exclude = ["crates/retired"]

[workspace.dependencies]
demo-core = { path = "crates/core" }
serde = "1.0"
proc-macro2 = "1.0"
"#,
            ),
            (
                "crates/core/Cargo.toml",
                r#"
[package]
name = "demo-core"

[dependencies]
serde = { workspace = true }
"#,
            ),
            (
                "crates/app/Cargo.toml",
                r#"
[package]
name = "demo-app"

[dependencies]
demo-core = { workspace = true }
proc-macro2 = { workspace = true, optional = true }

[dev-dependencies]
tempfile = "3.0"
"#,
            ),
            (
                "crates/retired/Cargo.toml",
                r#"
[package]
name = "retired"

[dependencies]
old-dep = "1.0"
"#,
            ),
        ],
        &[
            ("crates/app", "tempfile", DepKind::Dev),
            ("crates/app", "proc_macro2", DepKind::Optional),
            ("crates/core", "serde", DepKind::Runtime),
            ("crates/retired", "old_dep", DepKind::Runtime),
        ],
    );
}

#[test]
fn value_namespace_items_do_not_suppress_external_use_imports() {
    let source = concat!(
        "use serde::Serialize;\n",
        "use regex::Regex;\n",
        "use anyhow::Result;\n",
        "use tokio::runtime;\n",
        "pub fn serde() {}\n",
        "pub fn regex() {}\n",
        "pub const anyhow: u8 = 0;\n",
        "pub static tokio: u8 = 0;\n",
    );
    let lowered = lower("src/lib.rs", source).expect("lower");
    assert_eq!(
        lowered.runtime_imports,
        vec![
            "use:anyhow::Result",
            "use:regex::Regex",
            "use:serde::Serialize",
            "use:tokio::runtime",
        ]
    );

    let paths: Vec<RepoPath> = ["Cargo.toml", "src/lib.rs"]
        .into_iter()
        .map(|p| RepoPath::new(p).expect("valid"))
        .collect();
    let manifests = vec![Manifest {
        path: "Cargo.toml".to_owned(),
        text: "[package]\nname = \"demo\"\n[dependencies]\nregex = \"1\"\n".to_owned(),
    }];
    let declared = plugin()
        .declared_dependencies(manifests.clone())
        .expect("declared");
    assert_eq!(
        declared,
        vec![DeclaredDep {
            package_root: String::new(),
            name: "regex".to_owned(),
            kind: DepKind::Runtime,
        }]
    );
    let requests: Vec<ImportRequest> = lowered
        .runtime_imports
        .iter()
        .map(|specifier| ImportRequest {
            importer: "src/lib.rs".to_owned(),
            specifier: specifier.clone(),
        })
        .collect();
    assert_eq!(
        plugin().resolve_imports(&paths, manifests, requests),
        vec![
            Resolution::External("anyhow".to_owned()),
            Resolution::External("regex".to_owned()),
            Resolution::External("serde".to_owned()),
            Resolution::External("tokio".to_owned()),
        ]
    );
}

#[test]
fn local_uses_inline_mods_bindgen_exports_pascalcase_and_nested_workspaces_are_not_external() {
    let source = concat!(
        "use serde::Serialize;\n",
        "use crate::errors::{CliError, AgentError};\n",
        "mod bindings {\n",
        "    wit_bindgen::generate!({ world: \"plugin\" });\n",
        "}\n",
        "pub use bindings::exports::codeint::plugin::language;\n",
        "pub use language::Token;\n",
        "wit_bindgen::generate!({ world: \"guest\" });\n",
        "use exports::codeint::plugin::language::Guest;\n",
        "enum LocalKind { A, B }\n",
        "fn handle() {\n",
        "    use CliError::*;\n",
        "    use AgentError as E;\n",
        "    use LocalKind::A;\n",
        "}\n",
    );
    let lowered = lower("crates/app/src/lib.rs", source).expect("lower");
    assert_eq!(
        lowered.runtime_imports,
        vec![
            "path:wit_bindgen",
            "use:crate::errors::AgentError",
            "use:crate::errors::CliError",
            "use:serde::Serialize",
        ]
    );

    assert_rust_resolutions(
        &[
            "Cargo.toml",
            "crates/app/Cargo.toml",
            "crates/app/src/lib.rs",
            "crates/app/tests/fixtures/probes/Cargo.toml",
            "crates/app/tests/fixtures/probes/probe-runtime/Cargo.toml",
            "crates/app/tests/fixtures/probes/probe-runtime/src/lib.rs",
            "crates/app/tests/fixtures/probes/clock/Cargo.toml",
            "crates/app/tests/fixtures/probes/clock/src/lib.rs",
        ],
        &[
            ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
            ("crates/app/Cargo.toml", "[package]\nname = \"demo-app\"\n"),
            (
                "crates/app/tests/fixtures/probes/Cargo.toml",
                "[workspace]\nmembers = [\"probe-runtime\", \"clock\"]\n",
            ),
            (
                "crates/app/tests/fixtures/probes/probe-runtime/Cargo.toml",
                "[package]\nname = \"probe-runtime\"\n",
            ),
            (
                "crates/app/tests/fixtures/probes/clock/Cargo.toml",
                "[package]\nname = \"clock\"\n[dependencies]\nprobe-runtime = { path = \"../probe-runtime\" }\n",
            ),
        ],
        &[
            (
                "crates/app/src/lib.rs",
                "use:TokenKind::Identifier",
                Resolution::Builtin,
            ),
            (
                "crates/app/src/lib.rs",
                "use:serde::Serialize",
                Resolution::External("serde".to_owned()),
            ),
            (
                "crates/app/src/lib.rs",
                "use:probe_runtime::run",
                Resolution::External("probe_runtime".to_owned()),
            ),
            (
                "crates/app/src/lib.rs",
                "path:wit_bindgen",
                Resolution::Builtin,
            ),
            (
                "crates/app/tests/fixtures/probes/clock/src/lib.rs",
                "use:probe_runtime",
                Resolution::Resolved(
                    "crates/app/tests/fixtures/probes/probe-runtime/src/lib.rs".to_owned(),
                ),
            ),
        ],
    );
}

fn assert_rust_resolutions(
    paths: &[&str],
    manifests: &[(&str, &str)],
    cases: &[(&str, &str, Resolution)],
) {
    let mut repo = Vec::with_capacity(paths.len());
    for raw in paths {
        repo.push(RepoPath::new(*raw).expect("valid path"));
    }
    let mut manifest_list = Vec::with_capacity(manifests.len());
    for (path, text) in manifests {
        manifest_list.push(Manifest {
            path: (*path).to_owned(),
            text: (*text).to_owned(),
        });
    }
    let mut requests = Vec::with_capacity(cases.len());
    let mut expected = Vec::with_capacity(cases.len());
    for (importer, specifier, outcome) in cases {
        requests.push(ImportRequest {
            importer: (*importer).to_owned(),
            specifier: (*specifier).to_owned(),
        });
        expected.push(outcome.clone());
    }
    assert_eq!(
        plugin().resolve_imports(&repo, manifest_list, requests),
        expected
    );
}

#[test]
fn emits_and_resolves_attributed_external_mods_path_attrs_and_skips_cfg_test() {
    let source = concat!(
        "#[macro_use]\nmod error;\n",
        "#[cfg(unix)]\nmod unix;\n",
        "#[allow(dead_code)]\nmod helper;\n",
        "#[path = \"custom/alt.rs\"]\nmod alt;\n",
        "#[cfg(test)]\nmod tests;\n",
    );
    let lowered = lower("src/main.rs", source).expect("lower");
    assert_eq!(
        lowered.runtime_imports,
        [
            "mod-path:custom/alt.rs",
            "mod:error",
            "mod:helper",
            "mod:unix",
        ]
    );
    assert_rust_resolutions(
        &[
            "Cargo.toml",
            "src/main.rs",
            "src/error.rs",
            "src/unix/mod.rs",
            "src/helper.rs",
            "src/custom/alt.rs",
            "src/tests.rs",
        ],
        &[("Cargo.toml", "[package]\nname = \"fd\"\n")],
        &[
            (
                "src/main.rs",
                "mod-path:custom/alt.rs",
                Resolution::Resolved("src/custom/alt.rs".to_owned()),
            ),
            (
                "src/main.rs",
                "mod:error",
                Resolution::Resolved("src/error.rs".to_owned()),
            ),
            (
                "src/main.rs",
                "mod:helper",
                Resolution::Resolved("src/helper.rs".to_owned()),
            ),
            (
                "src/main.rs",
                "mod:unix",
                Resolution::Resolved("src/unix/mod.rs".to_owned()),
            ),
        ],
    );
}

#[test]
fn resolves_custom_bin_crate_root_bin_to_lib_and_qualified_crate_paths() {
    let main_src = concat!(
        "use std::process::ExitCode;\n",
        "fn main() -> ExitCode {\n",
        "    ctrlc::set_handler(|| {}).ok();\n",
        "    log::trace!(\"starting\");\n",
        "    println!(\"{}\", toml::to_string(&1).unwrap());\n",
        "    unknown_crate::call();\n",
        "    codeint_cli::run_cli()\n",
        "}\n",
    );
    let lib_src = concat!(
        "mod check;\n",
        "#[derive(Debug, thiserror::Error)]\n",
        "#[cfg_attr(feature = \"extra\", derive(serde::Serialize))]\n",
        "#[cfg_attr(test, derive(dev_only::ShouldSkip))]\n",
        "pub enum ErrKind { #[error(\"bad\")] Bad }\n",
        "pub fn parse(raw: &str) -> Result<serde_json::Value, serde_json::Error> {\n",
        "    check::ping();\n",
        "    serde_json::from_str(raw)\n",
        "}\n",
        "pub fn run_cli() -> std::process::ExitCode { std::process::ExitCode::SUCCESS }\n",
    );
    assert_eq!(
        lower("crates/cli/src/main.rs", main_src)
            .expect("main")
            .runtime_imports,
        [
            "path:codeint_cli",
            "path:ctrlc",
            "path:log",
            "path:toml",
            "path:unknown_crate",
            "use:std::process::ExitCode",
        ]
    );
    assert_eq!(
        lower("crates/cli/src/lib.rs", lib_src)
            .expect("lib")
            .runtime_imports,
        [
            "mod:check",
            "path:serde",
            "path:serde_json",
            "path:thiserror",
        ]
    );
    assert_rust_resolutions(
        &[
            "Cargo.toml",
            "crates/cli/Cargo.toml",
            "crates/cli/src/main.rs",
            "crates/cli/src/lib.rs",
            "crates/cli/src/check.rs",
            "crates/core/main.rs",
            "crates/core/flags/mod.rs",
            "crates/core/flags/hiargs.rs",
            "crates/core/haystack.rs",
        ],
        &[
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"crates/cli\"]\n[package]\nname = \"ripgrep\"\nbuild = \"build.rs\"\n[[bin]]\nname = \"rg\"\npath = \"crates/core/main.rs\"\n",
            ),
            (
                "crates/cli/Cargo.toml",
                "[package]\nname = \"codeint-cli\"\n[[bin]]\nname = \"codeint\"\npath = \"src/main.rs\"\n[dependencies]\nctrlc = \"3\"\nlog = \"0.4\"\nserde = \"1\"\nserde_json = \"1\"\nthiserror = \"2\"\ntoml = \"1\"\n",
            ),
        ],
        &[
            (
                "crates/cli/src/main.rs",
                "path:codeint_cli",
                Resolution::Resolved("crates/cli/src/lib.rs".to_owned()),
            ),
            (
                "crates/cli/src/lib.rs",
                "path:codeint_cli",
                Resolution::Builtin,
            ),
            (
                "crates/cli/src/main.rs",
                "path:ctrlc",
                Resolution::External("ctrlc".to_owned()),
            ),
            (
                "crates/cli/src/main.rs",
                "path:log",
                Resolution::External("log".to_owned()),
            ),
            (
                "crates/cli/src/main.rs",
                "path:toml",
                Resolution::External("toml".to_owned()),
            ),
            (
                "crates/cli/src/main.rs",
                "path:unknown_crate",
                Resolution::Builtin,
            ),
            (
                "crates/cli/src/lib.rs",
                "path:thiserror",
                Resolution::External("thiserror".to_owned()),
            ),
            (
                "crates/cli/src/lib.rs",
                "path:serde_json",
                Resolution::External("serde_json".to_owned()),
            ),
            (
                "crates/core/main.rs",
                "mod:flags",
                Resolution::Resolved("crates/core/flags/mod.rs".to_owned()),
            ),
            (
                "crates/core/flags/hiargs.rs",
                "use:crate::haystack::Read",
                Resolution::Resolved("crates/core/haystack.rs".to_owned()),
            ),
        ],
    );
}
