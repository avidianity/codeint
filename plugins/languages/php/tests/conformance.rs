use codeint_lang_php::{Php, flagged_line_rules};
use codeint_model::language::{DepKind, Function, ImportRequest, Resolution};
use codeint_model::{LanguagePlugin, LowerError, Manifest, RepoPath, SourceSet, TokenKind};

fn lower_php(path: &str, text: &str) -> Result<codeint_model::FileModel, LowerError> {
    Php.lower(&RepoPath::new(path).expect("valid repo path"), text)
}

#[test]
fn php_extension_and_reference_ranges_match_contract() {
    assert_eq!(Php.extensions(), ["php"]);
    let ranges = Php.reference_ranges().expect("reference ranges");
    assert!(ranges.for_metric("erosion").is_some());
    assert!(ranges.for_metric("verbosity").is_some());
    assert!(ranges.for_metric("duplication").is_some());
    assert!(ranges.for_metric("import_cycles").is_some());
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
                name: Some("evaluate".to_owned()),
                cognitive_complexity: 7,
            },
            Function {
                line: 10,
                end_line: 10,
                decision_points: 1,
                source_lines: 1,
                name: None,
                cognitive_complexity: 1,
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
    assert_eq!(out[2], Resolution::External("illuminate/support".into()));
    assert!(matches!(out[3], Resolution::Unresolved(_)));
}

#[test]
fn php_builtins_composer_normalization_and_declared_dependencies() {
    let manifests: Vec<Manifest> = [
        (
            "composer.json",
            include_str!("fixtures/workspace/composer.json"),
        ),
        (
            "packages/auth/composer.json",
            include_str!("fixtures/workspace/packages/auth/composer.json"),
        ),
        (
            "packages/billing/composer.json",
            include_str!("fixtures/workspace/packages/billing/composer.json"),
        ),
        (
            "vendor/ignored/pkg/composer.json",
            r#"{"name":"ignored/pkg","require":{"foo/bar":"^1.0"}}"#,
        ),
    ]
    .into_iter()
    .map(|(path, text)| Manifest {
        path: path.to_owned(),
        text: text.to_owned(),
    })
    .collect();
    let importer = "packages/billing/src/InvoiceService.php";
    let lowered = lower_php(
        importer,
        include_str!("fixtures/workspace/packages/billing/src/InvoiceService.php"),
    )
    .unwrap();
    let repo: Vec<RepoPath> = [
        "composer.json",
        "packages/auth/composer.json",
        "packages/auth/src/Token.php",
        "packages/billing/composer.json",
        importer,
    ]
    .into_iter()
    .map(|p| RepoPath::new(p).unwrap())
    .collect();
    let resolved = Php.resolve_imports(
        &repo,
        manifests.clone(),
        lowered
            .runtime_imports
            .iter()
            .map(|specifier| ImportRequest {
                importer: importer.to_owned(),
                specifier: specifier.clone(),
            })
            .collect(),
    );
    let by_spec: std::collections::BTreeMap<&str, Resolution> = lowered
        .runtime_imports
        .iter()
        .map(String::as_str)
        .zip(resolved)
        .collect();
    assert_eq!(
        by_spec["use:Acme\\Auth\\Token"],
        Resolution::Resolved("packages/auth/src/Token.php".into())
    );
    assert_eq!(by_spec["use:DateTimeImmutable"], Resolution::Builtin);
    assert_eq!(by_spec["use:Random\\Randomizer"], Resolution::Builtin);
    assert_eq!(by_spec["use:RuntimeException"], Resolution::Builtin);
    assert_eq!(
        by_spec["use:Acme\\HttpClient\\Client"],
        Resolution::External("acme/http-client".into())
    );
    assert_eq!(
        by_spec["use:Illuminate\\Support\\Collection"],
        Resolution::External("illuminate/support".into())
    );
    assert_eq!(
        by_spec["use:Monolog\\Logger"],
        Resolution::External("monolog/monolog".into())
    );
    assert_eq!(
        by_spec["use:Symfony\\Component\\HttpFoundation\\Request"],
        Resolution::External("symfony/http-foundation".into())
    );
    assert_eq!(
        by_spec["use:Unlisted\\CustomPkg\\Helper"],
        Resolution::External("unlisted/custom-pkg".into())
    );

    let deps = Php.declared_dependencies(manifests);
    let rows: Vec<(&str, &str, DepKind)> = deps
        .iter()
        .map(|d| (d.package_root.as_str(), d.name.as_str(), d.kind))
        .collect();
    assert_eq!(
        rows,
        [
            ("", "acme/http-client", DepKind::Runtime),
            ("", "illuminate/support", DepKind::Runtime),
            ("", "monolog/monolog", DepKind::Runtime),
            ("", "phpunit/phpunit", DepKind::Dev),
            ("", "symfony/cache", DepKind::Optional),
            ("packages/auth", "psr/log", DepKind::Runtime),
            (
                "packages/billing",
                "symfony/http-foundation",
                DepKind::Runtime
            ),
            ("packages/billing", "phpstan/phpstan", DepKind::Dev),
        ]
    );
}

#[test]
fn php_function_names_cover_functions_methods_refs_and_anonymous_closures() {
    let source = "<?php\n\
        function freeHelper(int $x): int { return $x; }\n\
        function &byRef(array &$items): array { return $items; }\n\
        class OrderService {\n\
            public function __construct(private int $id) {}\n\
            public static function create(): self { return new self(1); }\n\
            public function process(array $items): array {\n\
                $doubled = array_map(function ($v) { return $v * 2; }, $items);\n\
                return array_filter($doubled, fn($v) => $v > 0);\n\
            }\n\
        }\n";
    let model = lower_php("src/Names.php", source).expect("lower names");
    assert_eq!(model.functions.len(), 7);
    assert_eq!(model.functions[0].name.as_deref(), Some("freeHelper"));
    assert_eq!(model.functions[1].name.as_deref(), Some("byRef"));
    assert_eq!(model.functions[2].name.as_deref(), Some("__construct"));
    assert_eq!(model.functions[3].name.as_deref(), Some("create"));
    assert_eq!(model.functions[4].name.as_deref(), Some("process"));
    assert_eq!(model.functions[5].name, None);
    assert_eq!(model.functions[6].name, None);
}

#[test]
fn php_cognitive_complexity_distinguishes_flat_vs_nested_and_isolates_closures() {
    let source = "<?php\n\
        function flatSwitch(int $x): int {\n\
            switch ($x) {\n\
                case 1: return 10;\n\
                case 2: return 20;\n\
                case 3: return 30;\n\
                case 4: return 40;\n\
                default: return 0;\n\
            }\n\
        }\n\
        function flatIfs(bool $a, bool $b, bool $c, bool $d): int {\n\
            $n = 0;\n\
            if ($a) { $n += 1; }\n\
            if ($b) { $n += 1; }\n\
            if ($c) { $n += 1; }\n\
            if ($d) { $n += 1; }\n\
            return $n;\n\
        }\n\
        function deeplyNested(bool $a, bool $b, bool $c, bool $d): int {\n\
            if ($a) {\n\
                if ($b) {\n\
                    if ($c) {\n\
                        if ($d) { return 1; }\n\
                    }\n\
                }\n\
            }\n\
            return 0;\n\
        }\n\
        function methodCallsNotControlFlow(object $router): int {\n\
            return $router->match(1) + $router->for(2);\n\
        }\n\
        function complex(bool $a, bool $b, bool $c, array $items): int {\n\
            if ($a && $b || $c) {\n\
                foreach ($items as $item) {\n\
                    if ($item < 0) {\n\
                        break 2;\n\
                    } elseif ($item === 0) {\n\
                        continue;\n\
                    } else {\n\
                        try {\n\
                            $cb = function ($v) {\n\
                                if ($v > 0) {\n\
                                    if ($v > 1) { return $v; }\n\
                                }\n\
                                return 0;\n\
                            };\n\
                            return $cb($item);\n\
                        } catch (Exception $e) {\n\
                            return complex(false, $b, $c, []);\n\
                        }\n\
                    }\n\
                }\n\
            }\n\
            return 0;\n\
        }\n";
    let fns = lower_php("src/Cognitive.php", source)
        .expect("lower cognitive")
        .functions;
    assert_eq!(fns.len(), 6);
    assert_eq!(
        (fns[0].name.as_deref(), fns[0].cognitive_complexity),
        (Some("flatSwitch"), 1)
    );
    assert_eq!(
        (fns[1].name.as_deref(), fns[1].cognitive_complexity),
        (Some("flatIfs"), 4)
    );
    assert_eq!(
        (fns[2].name.as_deref(), fns[2].cognitive_complexity),
        (Some("deeplyNested"), 10)
    );
    assert_eq!(
        (fns[3].name.as_deref(), fns[3].cognitive_complexity),
        (Some("methodCallsNotControlFlow"), 0)
    );
    assert_eq!(
        (fns[4].name.as_deref(), fns[4].cognitive_complexity),
        (Some("complex"), 16)
    );
    assert_eq!(
        (fns[5].name.as_deref(), fns[5].cognitive_complexity),
        (None, 3)
    );
}
