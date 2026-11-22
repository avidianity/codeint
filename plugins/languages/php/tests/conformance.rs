use codeint_lang_php::{Php, flagged_line_rules};
use codeint_model::language::{Function, ImportRequest, Resolution};
use codeint_model::{
    FileModel, LanguagePlugin, LowerError, Manifest, RepoPath, SourceSet, TokenKind,
};

fn plugin() -> &'static dyn LanguagePlugin {
    &Php
}

fn lower(path: &str, text: &str) -> Result<FileModel, LowerError> {
    plugin().lower(&RepoPath::new(path).expect("valid path"), text)
}

#[test]
fn declares_php_extension_and_empty_reference_ranges() {
    assert_eq!(plugin().extensions(), vec!["php"]);
    let ranges = plugin().reference_ranges().expect("valid");
    for metric in ["erosion", "verbosity", "duplication", "import_cycles"] {
        assert!(ranges.for_metric(metric).is_none(), "{metric}");
    }
}

#[test]
fn rejects_non_php_and_unparsable_files() {
    assert!(matches!(
        lower("src/app.rs", "fn main() {}\n"),
        Err(LowerError::UnsupportedFile { .. })
    ));
    assert!(matches!(
        lower("src/broken.php", "<?php\nfunction f() {\n    $s = 'unterminated;\n}\n"),
        Err(LowerError::Unparsable { message }) if message.contains("line 3")
    ));
    assert!(matches!(
        lower("src/unclosed.php", "<?php\nfunction f() {\n    return 1;\n"),
        Err(LowerError::Unparsable { message }) if message.contains("unclosed")
    ));
}

#[test]
fn empty_file_produces_empty_model() {
    let model = lower("src/empty.php", "").expect("empty");
    assert_eq!(model.source_set, SourceSet::Production);
    assert_eq!(model.source_lines, 0);
    assert!(model.functions.is_empty());
    assert!(model.tokens.is_empty());
    assert!(model.flagged_lines.is_empty());
    assert!(model.runtime_imports.is_empty());
}

#[test]
fn counts_decision_points_across_php_control_flow_and_arrow_functions() {
    let source = concat!(
        "<?php\n",
        "function evaluate(int $x, ?int $y): int {\n",
        "    if ($x > 0 && $x < 10) {\n",
        "        foreach ([1, 2] as $item) {\n",
        "            $x += $item;\n",
        "        }\n",
        "    } elseif ($x < 0 || $y === null) {\n",
        "        $x = $y ?? 0;\n",
        "    }\n",
        "    $mapper = fn($v) => $v > 0 ? $v : 0;\n",
        "    return match ($x) {\n",
        "        1 => $mapper(1),\n",
        "        2 => $mapper(2),\n",
        "        default => 0,\n",
        "    };\n",
        "}\n",
    );
    let model = lower("src/logic.php", source).expect("lower");
    assert_eq!(
        model.functions,
        vec![
            Function {
                line: 2,
                end_line: 16,
                decision_points: 8,
                source_lines: 15,
            },
            Function {
                line: 10,
                end_line: 10,
                decision_points: 1,
                source_lines: 1,
            },
        ]
    );
}

#[test]
fn marks_namespace_and_use_as_import_or_export_and_normalizes_crlf() {
    let lf = "<?php\nnamespace App\\Services;\nuse App\\Models\\{User, Order as Ord};\nclass Svc {\n    public function run(): int { return 1; }\n}\n";
    let crlf = lf.replace('\n', "\r\n");
    let model_lf = lower("app/Services/Svc.php", lf).expect("lf");
    let model_crlf = lower("app/Services/Svc.php", &crlf).expect("crlf");
    assert_eq!(model_lf, model_crlf);
    assert_eq!(
        model_lf.runtime_imports,
        vec!["use:App\\Models\\Order", "use:App\\Models\\User"]
    );
    assert!(
        model_lf
            .tokens
            .iter()
            .filter(|t| t.line == 2 || t.line == 3)
            .all(|t| t.kind == TokenKind::ImportOrExport)
    );
}

#[test]
fn flagged_line_rules_catch_verbose_patterns_without_false_positives() {
    let mut declared: Vec<&str> = flagged_line_rules().collect();
    declared.sort();
    assert_eq!(
        declared,
        vec![
            "catch-only-rethrows",
            "constructor-only-forwards",
            "else-after-return",
            "if-else-returns-boolean",
            "ternary-yields-boolean",
            "trailing-bare-return",
        ]
    );

    let cases: [(&str, &str, &str); 6] = [
        (
            "else-after-return",
            "<?php\nfunction f($x) {\n    if ($x > 0) {\n        return 1;\n    } else {\n        return 2;\n    }\n}\n",
            "<?php\nfunction f($x) {\n    if ($x > 0) {\n        $y = 1;\n    } else {\n        $y = 2;\n    }\n    return $y;\n}\n",
        ),
        (
            "if-else-returns-boolean",
            "<?php\nfunction f($x) {\n    if ($x > 0) {\n        return true;\n    } else {\n        return false;\n    }\n}\n",
            "<?php\nfunction f($x) {\n    if ($x > 0) {\n        return 1;\n    } else {\n        return 0;\n    }\n}\n",
        ),
        (
            "catch-only-rethrows",
            "<?php\nfunction f() {\n    try { work(); } catch (Exception $e) { throw $e; }\n}\n",
            "<?php\nfunction f() {\n    try { work(); } catch (Exception $e) { log($e); throw $e; }\n}\n",
        ),
        (
            "ternary-yields-boolean",
            "<?php\nfunction f($x) {\n    return $x > 0 ? true : false;\n}\n",
            "<?php\nfunction f($x) {\n    return $x > 0 ? 'yes' : 'no';\n}\n",
        ),
        (
            "trailing-bare-return",
            "<?php\nfunction f() {\n    work();\n    return;\n}\n",
            "<?php\nfunction f($x) {\n    if ($x) { return; }\n    work();\n}\n",
        ),
        (
            "constructor-only-forwards",
            "<?php\nclass Child extends Base {\n    public function __construct($a, $b) {\n        parent::__construct($a, $b);\n    }\n}\n",
            "<?php\nclass Child extends Base {\n    public function __construct(public int $a) {\n        parent::__construct($a);\n    }\n}\n",
        ),
    ];

    for (rule, bad_src, good_src) in cases {
        let bad = lower("src/bad.php", bad_src).expect("bad lowers");
        assert!(
            bad.flagged_lines.iter().any(|l| l.rule == rule),
            "expected {rule} to flag:\n{bad_src}"
        );
        let good = lower("src/good.php", good_src).expect("good lowers");
        assert!(
            !good.flagged_lines.iter().any(|l| l.rule == rule),
            "did not expect {rule} to flag:\n{good_src}"
        );
    }
}

#[test]
fn resolves_psr4_use_and_static_require_imports() {
    let paths: Vec<RepoPath> = [
        "composer.json",
        "app/Services/OrderService.php",
        "app/Models/Order.php",
        "app/helpers.php",
    ]
    .into_iter()
    .map(|p| RepoPath::new(p).expect("valid"))
    .collect();
    let manifests = vec![Manifest {
        path: "composer.json".to_owned(),
        text: r#"{"autoload": {"psr-4": {"App\\": "app/"}}}"#.to_owned(),
    }];
    let requests = vec![
        ImportRequest {
            importer: "app/Services/OrderService.php".to_owned(),
            specifier: "use:App\\Models\\Order".to_owned(),
        },
        ImportRequest {
            importer: "app/Services/OrderService.php".to_owned(),
            specifier: "req:../helpers.php".to_owned(),
        },
        ImportRequest {
            importer: "app/Services/OrderService.php".to_owned(),
            specifier: "use:Illuminate\\Support\\Collection".to_owned(),
        },
        ImportRequest {
            importer: "app/Services/OrderService.php".to_owned(),
            specifier: "req:./missing.php".to_owned(),
        },
    ];

    let resolutions = plugin().resolve_imports(&paths, manifests, requests);
    assert_eq!(
        resolutions[0],
        Resolution::Resolved("app/Models/Order.php".to_owned())
    );
    assert_eq!(
        resolutions[1],
        Resolution::Resolved("app/helpers.php".to_owned())
    );
    assert_eq!(resolutions[2], Resolution::External);
    assert!(matches!(resolutions[3], Resolution::Unresolved(_)));
}
