use codeint_lang_php::Php;
use codeint_model::{LanguagePlugin, RepoPath, TokenKind};

fn lower_php(path: &str, text: &str) -> codeint_model::FileModel {
    Php.lower(&RepoPath::new(path).expect("valid repo path"), text, None)
        .expect("lower php")
}

fn fn_summary(model: &codeint_model::FileModel) -> Vec<(Option<&str>, u32)> {
    model
        .functions
        .iter()
        .map(|f| (f.name.as_deref(), f.decision_points))
        .collect()
}

#[test]
fn nested_double_quote_interpolation_and_backtick_literals_lex_cleanly() {
    let source = r#"<?php
function buildQuery(object $db, string $val, int $wait): string {
    $sql = "wait for ({$db->quote("{$val} (s")}, done)";
    $out = `echo "(ok)" 'arg' #comment {$db->quote("x")}`;
    return $sql . $out;
}
"#;
    let model = lower_php("src/Query.php", source);
    assert_eq!(fn_summary(&model), [(Some("buildQuery"), 0)]);
}

#[test]
fn halt_compiler_and_case_insensitive_open_tags_lex_cleanly() {
    let halt_source = r#"<?php
function stub(): int {
    return 1;
}
__halt_compiler();
raw payload { unclosed ( ' " /* never closed
"#;
    let halt_model = lower_php("src/Stub.php", halt_source);
    assert_eq!(halt_model.functions.len(), 1);
    assert_eq!(halt_model.functions[0].name.as_deref(), Some("stub"));

    let tags_source = r#"<?XML version="1.0"?>
<?PHP
function render(): string {
    return 'ok';
}
"#;
    let tags_model = lower_php("src/Template.php", tags_source);
    assert_eq!(tags_model.functions.len(), 1);
    assert_eq!(tags_model.functions[0].name.as_deref(), Some("render"));
    assert_eq!(tags_model.functions[0].line, 3);
    assert!(tags_model.tokens.iter().all(|t| t.line >= 3));
}

#[test]
fn semi_reserved_member_names_method_names_and_named_args_do_not_corrupt_functions_or_decisions() {
    let source = r#"<?php
class Builder {
    public const IF = 1;
    public const OR = 2;
    public function function(string $name): string {
        return $name;
    }
    public function fn(int $x): int {
        return $x;
    }
    public function match(int $x): int {
        $a = $this->case;
        $b = Builder::for($x);
        $c = $this?->if($a, $b);
        $d = dispatch(for: 1, match: 2, case: 3, if: 4, and: 5, or: 6);
        return $a + $b + $c + $d + self::IF + self::OR;
    }
}
"#;
    let model = lower_php("src/Builder.php", source);
    let names: Vec<Option<&str>> = model.functions.iter().map(|f| f.name.as_deref()).collect();
    assert_eq!(names, [Some("function"), Some("fn"), Some("match")]);
    assert!(model.functions.iter().all(|f| f.decision_points == 0));
}

#[test]
fn ternary_detection_handles_member_keywords_self_elvis_attributes_and_asymmetric_visibility() {
    let source = r#"<?php
function positives(object $node, int $flags, int $i): array {
    return [
        $flags & Flags::FINAL ? 'final' : '',
        $node->default ? 1 : 0,
        $node instanceof self ? 1 : 0,
        $node instanceof static ? 1 : 0,
        $node->global ?: [],
        TRUE ? 1 : 0,
        $i++ ? 1 : 0,
    ];
}
function negatives(object $container): object {
    $container->call(function (#[Inject('k')] ?string $val): ?int {
        return null;
    });
    return new class {
        public static ?int $count = null;
        public private(set) ?string $label = null;
    };
}
"#;
    let model = lower_php("src/Ternary.php", source);
    assert_eq!(
        fn_summary(&model),
        [(Some("positives"), 7), (Some("negatives"), 0), (None, 0)]
    );
}

#[test]
fn imports_handle_member_include_dynamic_require_relative_ns_bracketed_ns_and_mixed_group_use() {
    let source = r#"<?php
namespace Alpha {
    use Acme\Pkg\{One, function helperFn, const FLAG, Two as AliasTwo};
    $compiler->include('view.header');
    Loader::require('ignored.php');
    require __DIR__ . '/' . $dynamic . '.php';
    include 'prefix/' . $suffix;
    require_once __dir__ . '/static_helper.php';
    $val = namespace\Local::run();
    $cls = $node->class;
}
namespace Beta {
    use Acme\Other\Three;
}
"#;
    let model = lower_php("src/Imports.php", source);
    assert_eq!(
        model.runtime_imports,
        [
            "req:./static_helper.php",
            "use:Acme\\Other\\Three",
            "use:Acme\\Pkg\\One",
            "use:Acme\\Pkg\\Two",
        ]
    );
    let relative_ns_tokens_marked = model
        .tokens
        .iter()
        .filter(|t| t.line == 9)
        .any(|t| t.kind == TokenKind::ImportOrExport);
    assert!(!relative_ns_tokens_marked);
}

#[test]
fn flagged_line_rules_avoid_false_positives_on_catch_chains_finally_and_semantic_constructors() {
    let clean = r#"<?php
class Clean extends Base {
    private function __construct($a) { parent::__construct($a); }
}
class ProtectedCtor extends Base {
    protected function __construct($a) { parent::__construct($a); }
}
class FinalCtor extends Base {
    final public function __construct($a) { parent::__construct($a); }
}
class DefaultCtor extends Base {
    public function __construct(int $code = 400) { parent::__construct($code); }
}
class AttributedCtor extends Base {
    #[Marker]
    public function __construct($a) { parent::__construct($a); }
}
class AttributedParamCtor extends Base {
    public function __construct(#[Marker] $a) { parent::__construct($a); }
}
class TypeNarrowingCtor extends Base {
    public function __construct(SpecificType $a) { parent::__construct($a); }
}
function safeCatches(): void {
    try {
        work();
    } catch (DomainException $e) {
        throw $e;
    } catch (Throwable $t) {
        report($t);
    }
    try {
        work();
    } catch (Throwable $e) {
        throw $e;
    } finally {
        cleanup();
    }
}
"#;
    let model = lower_php("src/CleanRules.php", clean);
    assert!(
        model.flagged_lines.is_empty(),
        "unexpected flagged lines: {:?}",
        model.flagged_lines
    );
}

#[test]
fn planted_defect_suite_achieves_full_recall_across_all_six_rules_with_modern_php_syntax() {
    let source = r#"<?php
declare(strict_types=1);
enum Status: string { case Open = 'open'; case Closed = 'closed'; }
readonly class RedundantCtor extends Base {
    public function __construct($a, $b) {
        parent::__construct($a, $b);
    }
}
#[Route('/api')]
function modernDefects((Countable&Iterator)|null $items, int $x): bool {
    $label = <<<'TXT'
        if ($x) { return true; } else { return false; }
    TXT;
    $tag = match ($x) { 1 => $items?->count(), default => 0 };
    $fn = fn(int $n): int => $n + 1;
    try {
        $fn($tag);
    } catch (RuntimeException $e) {
        throw $e;
    }
    $flag = $x > 0 ? true : false;
    if ($flag) {
        return true;
    } else {
        return false;
    }
}
function extraDefects(int $x): void {
    if ($x < 0) {
        throw new InvalidArgumentException('neg');
    } else {
        logValue($x);
    }
    return;
}
"#;
    let model = lower_php("src/PlantedDefects.php", source);
    let mut rules: Vec<&str> = model
        .flagged_lines
        .iter()
        .map(|f| f.rule.as_str())
        .collect();
    rules.sort_unstable();
    rules.dedup();
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
    let flagged_heredoc = model
        .flagged_lines
        .iter()
        .any(|f| matches!(f.line, 11..=13));
    assert!(!flagged_heredoc);
}
