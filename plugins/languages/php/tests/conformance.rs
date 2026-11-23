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

fn resolve_single_manifest_php(
    paths: &[&str],
    composer_json: &str,
    importer: &str,
    specs: &[&str],
) -> Vec<Resolution> {
    let repo: Vec<RepoPath> = paths
        .iter()
        .map(|p| RepoPath::new(*p).expect("path"))
        .collect();
    let manifests = vec![Manifest {
        path: "composer.json".to_owned(),
        text: composer_json.to_owned(),
    }];
    let requests = specs
        .iter()
        .map(|spec| ImportRequest {
            importer: importer.to_owned(),
            specifier: (*spec).to_owned(),
        })
        .collect();
    Php.resolve_imports(&repo, manifests, requests)
}

#[test]
fn php_resolver_handles_psr4_and_static_require() {
    let out = resolve_single_manifest_php(
        &[
            "composer.json",
            "app/Services/OrderService.php",
            "app/Models/Order.php",
            "app/helpers.php",
        ],
        r#"{"autoload": {"psr-4": {"App\\": "app/"}}}"#,
        "app/Services/OrderService.php",
        &[
            "use:App\\Models\\Order",
            "req:../helpers.php",
            "use:Illuminate\\Support\\Collection",
            "req:./missing.php",
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
fn php_internal_psr4_namespace_import_does_not_fall_through_to_external() {
    let out = resolve_single_manifest_php(
        &["composer.json", "src/Service.php", "src/Models/User.php"],
        r#"{"name": "acme/shop", "autoload": {"psr-4": {"Acme\\Shop\\": "src/"}}}"#,
        "src/Service.php",
        &[
            "use:Acme\\Shop\\Models",
            "use:Acme\\Shop",
            "use:Acme\\Shop\\Models\\User",
            "use:Acme\\HttpClient\\Client",
        ],
    );
    assert!(matches!(out[0], Resolution::Unresolved(_)));
    assert!(matches!(out[1], Resolution::Unresolved(_)));
    assert_eq!(out[2], Resolution::Resolved("src/Models/User.php".into()));
    assert_eq!(out[3], Resolution::External("acme/http-client".into()));
}

#[test]
fn php_builtins_composer_normalization_and_declared_dependencies() {
    const ROOT_COMPOSER: &str = r#"{
  "name": "acme/root",
  "repositories": [
    { "type": "path", "url": "packages/*" }
  ],
  "require": {
    "php": "^8.3",
    "ext-json": "*",
    "lib-openssl": "*",
    "composer-runtime-api": "^2.2",
    "monolog/monolog": "^3.6",
    "Illuminate/Support": "^11.0",
    "acme/http-client": "^1.0"
  },
  "require-dev": {
    "phpunit/phpunit": "^11.0",
    "ext-xdebug": "*"
  },
  "suggest": {
    "symfony/cache": "Optional PSR-6 cache",
    "ext-redis": "Optional Redis extension"
  }
}
"#;
    const AUTH_COMPOSER: &str = r#"{
  "name": "acme/auth",
  "autoload": {
    "psr-4": {
      "Acme\\Auth\\": "src/"
    }
  },
  "require": {
    "psr/log": "^3.0"
  }
}
"#;
    const BILLING_COMPOSER: &str = r#"{
  "name": "acme/billing",
  "autoload": {
    "psr-4": {
      "Acme\\Billing\\": "src/"
    }
  },
  "require": {
    "acme/auth": "*",
    "symfony/http-foundation": "^7.0"
  },
  "require-dev": {
    "phpstan/phpstan": "^1.12"
  }
}
"#;
    const INVOICE_SERVICE: &str = r#"<?php
namespace Acme\Billing;

use DateTimeImmutable;
use RuntimeException;
use Random\Randomizer;
use Acme\Auth\Token;
use Monolog\Logger;
use Illuminate\Support\Collection;
use Acme\HttpClient\Client;
use Symfony\Component\HttpFoundation\Request;
use Unlisted\CustomPkg\Helper;

final class InvoiceService {
    public function issue(Token $token, Request $request): string {
        if ($token->value() === '') {
            throw new RuntimeException('empty');
        }
        return (new DateTimeImmutable())->format('c') . get_class($request);
    }
}
"#;
    let manifests: Vec<Manifest> = [
        ("composer.json", ROOT_COMPOSER),
        ("packages/auth/composer.json", AUTH_COMPOSER),
        ("packages/billing/composer.json", BILLING_COMPOSER),
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
    let lowered = lower_php(importer, INVOICE_SERVICE).unwrap();
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

    let deps = Php.declared_dependencies(manifests).unwrap();
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

#[test]
fn php_psr4_subpackage_collision_and_global_single_segment_uses_resolve_accurately() {
    let out = resolve_single_manifest_php(
        &[
            "composer.json",
            "src/Client.php",
            "src/Handler/CurlHandler.php",
        ],
        r#"{
            "name": "guzzlehttp/guzzle",
            "autoload": { "psr-4": { "GuzzleHttp\\": "src/" } },
            "require": {
                "guzzlehttp/psr7": "^2.7",
                "guzzlehttp/promises": "^2.0"
            }
        }"#,
        "src/Client.php",
        &[
            "use:GuzzleHttp\\Psr7\\Request",
            "use:GuzzleHttp\\Promise\\PromiseInterface",
            "use:GuzzleHttp\\Handler",
            "use:CarbonTimezoneTrait",
            "use:SubCarbon",
        ],
    );
    assert_eq!(out[0], Resolution::External("guzzlehttp/psr7".into()));
    assert_eq!(out[1], Resolution::External("guzzlehttp/promises".into()));
    assert!(matches!(out[2], Resolution::Unresolved(_)));
    assert_eq!(out[3], Resolution::Builtin);
    assert_eq!(out[4], Resolution::Builtin);
}

#[test]
fn php_multi_segment_composer_namespaces_and_runtime_builtins_resolve_cleanly() {
    let out = resolve_single_manifest_php(
        &["composer.json", "src/Client.php"],
        r#"{
            "name": "acme/client",
            "autoload": { "psr-4": { "Acme\\Client\\": "src/" } },
            "require": {
                "psr/http-message": "^2.0",
                "psr/http-client": "^1.0",
                "psr/http-factory": "^1.0",
                "symfony/translation": "^7.0",
                "symfony/translation-contracts": "^3.0",
                "composer/semver": "^3.4"
            },
            "require-dev": {
                "friendsofphp/php-cs-fixer": "^3.0"
            }
        }"#,
        "src/Client.php",
        &[
            "use:Psr\\Http\\Message\\RequestInterface",
            "use:Psr\\Http\\Client\\ClientInterface",
            "use:Psr\\Http\\Factory\\RequestFactoryInterface",
            "use:Psr\\Http\\Message\\StreamFactoryInterface",
            "use:Symfony\\Component\\Translation\\Translator",
            "use:Symfony\\Contracts\\Translation\\TranslatorInterface",
            "use:PhpCsFixer\\Config",
            "use:Composer\\InstalledVersions",
            "use:Composer\\Autoload\\ClassLoader",
            "use:Composer\\Semver\\Semver",
            "use:Pdo\\Sqlite",
            "use:Random\\Randomizer",
            "use:Xml\\Parser\\Reader",
            "use:OpenSSL\\Wrapper\\Cipher",
            "use:Vendor\\Unlisted\\Thing",
        ],
    );
    assert_eq!(
        out,
        vec![
            Resolution::External("psr/http-message".into()),
            Resolution::External("psr/http-client".into()),
            Resolution::External("psr/http-factory".into()),
            Resolution::External("psr/http-factory".into()),
            Resolution::External("symfony/translation".into()),
            Resolution::External("symfony/translation-contracts".into()),
            Resolution::External("friendsofphp/php-cs-fixer".into()),
            Resolution::Builtin,
            Resolution::Builtin,
            Resolution::External("composer/semver".into()),
            Resolution::Builtin,
            Resolution::Builtin,
            Resolution::External("xml/parser".into()),
            Resolution::External("openssl/wrapper".into()),
            Resolution::External("vendor/unlisted".into()),
        ]
    );
}
