use codeint_lang_rust::{Rust, flagged_line_rules};
use codeint_model::language::{Function, ImportRequest, Resolution};
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
                name: None,
                cognitive_complexity: 0,
            },
            Function {
                line: 7,
                end_line: 9,
                decision_points: 2,
                source_lines: 3,
                name: None,
                cognitive_complexity: 0,
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
            Resolution::External(String::new()),
        ]
    );
}
