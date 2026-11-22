use codeint_lang_php::{Php, flagged_line_rules};
use codeint_model::language::{Function, ImportRequest, Resolution};
use codeint_model::{LanguagePlugin, LowerError, Manifest, RepoPath, SourceSet, TokenKind};

fn lower_php(path: &str, text: &str) -> Result<codeint_model::FileModel, LowerError> {
    Php.lower(&RepoPath::new(path).expect("valid repo path"), text)
}

#[test]
fn php_extension_and_reference_ranges_match_contract() {
    assert_eq!(Php.extensions(), ["php"]);
    let ranges = Php.reference_ranges().expect("reference ranges");
    assert!(ranges.for_metric("erosion").is_none());
    assert!(ranges.for_metric("verbosity").is_none());
    assert!(ranges.for_metric("duplication").is_none());
    assert!(ranges.for_metric("import_cycles").is_none());
}

#[test]
fn php_lowering_handles_invalid_and_empty_inputs() {
    assert!(matches!(
        lower_php("src/app.rs", "fn main() {}\n"),
        Err(LowerError::UnsupportedFile { .. })
    ));
    let bad_str = lower_php(
        "src/broken.php",
        "<?php\nfunction f() {\n    $s = 'oops;\n}\n",
    );
    assert!(
        matches!(bad_str, Err(LowerError::Unparsable { message }) if message.contains("line 3"))
    );
    let unclosed = lower_php("src/unclosed.php", "<?php\nfunction f() {\n    return 1;\n");
    assert!(
        matches!(unclosed, Err(LowerError::Unparsable { message }) if message.contains("unclosed"))
    );

    let empty = lower_php("src/empty.php", "").expect("empty php");
    assert_eq!(
        (
            empty.source_set,
            empty.source_lines,
            empty.functions.len(),
            empty.tokens.len(),
            empty.flagged_lines.len(),
            empty.runtime_imports.len(),
        ),
        (SourceSet::Production, 0, 0, 0, 0, 0)
    );
}

#[test]
fn php_decision_points_cover_control_flow_match_and_arrow_functions() {
    let source = "<?php\n\
        function evaluate(int $x, ?int $y): int {\n\
            if ($x > 0 && $x < 10) {\n\
                foreach ([1, 2] as $item) {\n\
                    $x += $item;\n\
                }\n\
            } elseif ($x < 0 || $y === null) {\n\
                $x = $y ?? 0;\n\
            }\n\
            $mapper = fn($v) => $v > 0 ? $v : 0;\n\
            return match ($x) {\n\
                1 => $mapper(1),\n\
                2 => $mapper(2),\n\
                default => 0,\n\
            };\n\
        }\n";
    let model = lower_php("src/logic.php", source).expect("lower logic");
    assert_eq!(
        model.functions,
        [
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
fn php_imports_and_crlf_normalization() {
    let lf = "<?php\nnamespace App\\Services;\nuse App\\Models\\{User, Order as Ord};\nclass Svc {\n    public function run(): int { return 1; }\n}\n";
    let model = lower_php("app/Services/Svc.php", lf).expect("lf");
    assert_eq!(
        model,
        lower_php("app/Services/Svc.php", &lf.replace('\n', "\r\n")).expect("crlf")
    );
    assert_eq!(
        model.runtime_imports,
        ["use:App\\Models\\Order", "use:App\\Models\\User"]
    );
    let import_lines_ok = model
        .tokens
        .iter()
        .filter(|t| matches!(t.line, 2 | 3))
        .all(|t| t.kind == TokenKind::ImportOrExport);
    assert!(import_lines_ok);
}

fn assert_rule_pair(rule: &str, bad: &str, good: &str) {
    let bad_flags = lower_php("src/bad.php", bad).expect("bad").flagged_lines;
    assert!(bad_flags.iter().any(|l| l.rule == rule), "missed {rule}");
    let good_flags = lower_php("src/good.php", good).expect("good").flagged_lines;
    assert!(
        !good_flags.iter().any(|l| l.rule == rule),
        "false positive {rule}"
    );
}

#[test]
fn php_verbosity_rules_flag_only_verbose_constructs() {
    let mut rules: Vec<&str> = flagged_line_rules().collect();
    rules.sort_unstable();
    assert_eq!(
        rules,
        [
            "catch-only-rethrows",
            "constructor-only-forwards",
            "else-after-return",
            "if-else-returns-boolean",
            "ternary-yields-boolean",
            "trailing-bare-return",
        ]
    );
    assert_rule_pair(
        "else-after-return",
        "<?php\nfunction f($x) { if ($x > 0) { return 1; } else { return 2; } }\n",
        "<?php\nfunction f($x) { if ($x > 0) { $y = 1; } else { $y = 2; } return $y; }\n",
    );
    assert_rule_pair(
        "if-else-returns-boolean",
        "<?php\nfunction f($x) { if ($x > 0) { return true; } else { return false; } }\n",
        "<?php\nfunction f($x) { if ($x > 0) { return 1; } else { return 0; } }\n",
    );
    assert_rule_pair(
        "catch-only-rethrows",
        "<?php\nfunction f() { try { work(); } catch (Exception $e) { throw $e; } }\n",
        "<?php\nfunction f() { try { work(); } catch (Exception $e) { log($e); throw $e; } }\n",
    );
    assert_rule_pair(
        "ternary-yields-boolean",
        "<?php\nfunction f($x) { return $x > 0 ? true : false; }\n",
        "<?php\nfunction f($x) { return $x > 0 ? 'yes' : 'no'; }\n",
    );
    assert_rule_pair(
        "trailing-bare-return",
        "<?php\nfunction f() { work(); return; }\n",
        "<?php\nfunction f($x) { if ($x) { return; } work(); }\n",
    );
    assert_rule_pair(
        "constructor-only-forwards",
        "<?php\nclass C extends B { public function __construct($a, $b) { parent::__construct($a, $b); } }\n",
        "<?php\nclass C extends B { public function __construct(public int $a) { parent::__construct($a); } }\n",
    );
}

#[test]
fn php_resolver_handles_psr4_and_static_require() {
    let repo: Vec<RepoPath> = [
        "composer.json",
        "app/Services/OrderService.php",
        "app/Models/Order.php",
        "app/helpers.php",
    ]
    .into_iter()
    .map(|p| RepoPath::new(p).expect("path"))
    .collect();
    let manifests = vec![Manifest {
        path: "composer.json".to_owned(),
        text: r#"{"autoload": {"psr-4": {"App\\": "app/"}}}"#.to_owned(),
    }];
    let req = |spec: &str| ImportRequest {
        importer: "app/Services/OrderService.php".to_owned(),
        specifier: spec.to_owned(),
    };
    let out = Php.resolve_imports(
        &repo,
        manifests,
        vec![
            req("use:App\\Models\\Order"),
            req("req:../helpers.php"),
            req("use:Illuminate\\Support\\Collection"),
            req("req:./missing.php"),
        ],
    );
    assert_eq!(
        out[0],
        Resolution::Resolved("app/Models/Order.php".to_owned())
    );
    assert_eq!(out[1], Resolution::Resolved("app/helpers.php".to_owned()));
    assert_eq!(out[2], Resolution::External);
    assert!(matches!(out[3], Resolution::Unresolved(_)));
}
