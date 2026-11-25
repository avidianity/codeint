<?php

const DECISION_TOKENS = [
    T_IF, T_ELSEIF, T_FOR, T_FOREACH, T_WHILE, T_CASE, T_CATCH,
    T_LOGICAL_AND, T_LOGICAL_OR, T_BOOLEAN_AND, T_BOOLEAN_OR, T_COALESCE,
];
const EXPRESSION_END_TOKENS = [
    T_VARIABLE, T_STRING, T_LNUMBER, T_DNUMBER, T_CONSTANT_ENCAPSED_STRING,
    T_NAME_QUALIFIED, T_NAME_FULLY_QUALIFIED, T_NAME_RELATIVE,
];
const REQUIRE_TOKENS = [T_REQUIRE, T_REQUIRE_ONCE, T_INCLUDE, T_INCLUDE_ONCE];
const JSON_FLAGS = JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE;

function significantTokens(string $code): array
{
    $tokens = PhpToken::tokenize($code, TOKEN_PARSE);
    return array_values(array_filter($tokens, fn (PhpToken $token) => !$token->isIgnorable()));
}

function textAt(array $tokens, int $index): string
{
    return isset($tokens[$index]) ? $tokens[$index]->text : '';
}

function depthChange(PhpToken $token): int
{
    if (in_array($token->text, ['{', '(', '[', '${'], true)) {
        return 1;
    }
    return in_array($token->text, ['}', ')', ']'], true) ? -1 : 0;
}

function matchingClose(array $tokens, int $open): int
{
    $depth = 0;
    $count = count($tokens);
    for ($index = $open; $index < $count; $index++) {
        $depth += depthChange($tokens[$index]);
        if ($depth === 0) {
            return $index;
        }
    }
    return $count - 1;
}

function nextIndexOf(array $tokens, int $from, string $text): ?int
{
    $count = count($tokens);
    for ($index = $from; $index < $count; $index++) {
        if ($tokens[$index]->text === $text) {
            return $index;
        }
    }
    return null;
}

function bodyOpenAfter(array $tokens, int $from): ?int
{
    $count = count($tokens);
    for ($index = $from; $index < $count; $index++) {
        $text = $tokens[$index]->text;
        if ($text === ';') {
            return null;
        }
        if ($text === '{') {
            return $index;
        }
        $index = $text === '(' ? matchingClose($tokens, $index) : $index;
    }
    return null;
}

function namedFunctionSpan(array $tokens, int $start): ?array
{
    $paren = nextIndexOf($tokens, $start, '(');
    $afterUse = isset($tokens[$start - 1]) && $tokens[$start - 1]->id === T_USE;
    if ($paren === null || $afterUse) {
        return null;
    }
    $open = bodyOpenAfter($tokens, matchingClose($tokens, $paren) + 1);
    return $open === null ? null : [$start, $open, matchingClose($tokens, $open)];
}

function arrowExpressionEnd(array $tokens, int $arrow): int
{
    $depth = 0;
    $count = count($tokens);
    for ($index = $arrow + 1; $index < $count; $index++) {
        $change = depthChange($tokens[$index]);
        $separator = in_array($tokens[$index]->text, [';', ','], true);
        if ($depth === 0 && ($separator || $change < 0)) {
            return $index - 1;
        }
        $depth += $change;
    }
    return $count - 1;
}

function arrowFunctionSpan(array $tokens, int $start): ?array
{
    $paren = nextIndexOf($tokens, $start, '(');
    $arrow = $paren === null ? null : nextIndexOf($tokens, matchingClose($tokens, $paren), '=>');
    return $arrow === null ? null : [$start, $arrow, arrowExpressionEnd($tokens, $arrow)];
}

function functionSpans(array $tokens): array
{
    $spans = [];
    foreach ($tokens as $index => $token) {
        $span = match ($token->id) {
            T_FUNCTION => namedFunctionSpan($tokens, $index),
            T_FN => arrowFunctionSpan($tokens, $index),
            default => null,
        };
        if ($span !== null) {
            $spans[$index] = $span;
        }
    }
    return $spans;
}

function isTernary(array $tokens, int $index): bool
{
    $previous = $tokens[$index - 1] ?? null;
    if ($previous === null) {
        return false;
    }
    return in_array($previous->id, EXPRESSION_END_TOKENS, true) || in_array($previous->text, [')', ']'], true);
}

function matchArms(array $tokens, int $index): int
{
    $paren = nextIndexOf($tokens, $index, '(');
    $open = $paren === null ? null : nextIndexOf($tokens, matchingClose($tokens, $paren), '{');
    if ($open === null) {
        return 0;
    }
    $arms = 0;
    $depth = 0;
    $close = matchingClose($tokens, $open);
    for ($cursor = $open + 1; $cursor < $close; $cursor++) {
        $depth += depthChange($tokens[$cursor]);
        $arms += ($depth === 0 && $tokens[$cursor]->id === T_DOUBLE_ARROW) ? 1 : 0;
    }
    return max(0, $arms - 1);
}

function decisionWeight(array $tokens, int $index): int
{
    $token = $tokens[$index];
    if (in_array($token->id, DECISION_TOKENS, true)) {
        return 1;
    }
    if ($token->text === '?') {
        return isTernary($tokens, $index) ? 1 : 0;
    }
    return $token->id === T_MATCH ? matchArms($tokens, $index) : 0;
}

function ownDecisions(array $tokens, array $span, array $spans): int
{
    $total = 0;
    for ($index = $span[1] + 1; $index <= $span[2]; $index++) {
        if (isset($spans[$index])) {
            $index = $spans[$index][2];
            continue;
        }
        $total += decisionWeight($tokens, $index);
    }
    return $total;
}

function functions(array $tokens): array
{
    $spans = functionSpans($tokens);
    $found = [];
    foreach ($spans as $span) {
        $found[] = [$tokens[$span[0]]->line, $tokens[$span[2]]->line, ownDecisions($tokens, $span, $spans)];
    }
    return $found;
}

function statementEnd(array $tokens, int $from): int
{
    $semicolon = nextIndexOf($tokens, $from, ';');
    return $semicolon ?? count($tokens) - 1;
}

function useClauseText(array $tokens, int $from, int $end): string
{
    $text = '';
    for ($index = $from; $index < $end; $index++) {
        $token = $tokens[$index];
        $text .= match ($token->id) {
            T_AS => ' as ',
            T_FUNCTION, T_CONST => $token->text . ' ',
            default => $token->text,
        };
    }
    return $text;
}

function useItems(string $list): array
{
    $names = [];
    foreach (explode(',', $list) as $item) {
        $parts = preg_split('/\s+as\s+/i', trim($item));
        $name = trim($parts[0], "\\ ");
        if ($name !== '' && !preg_match('/^(function|const)\s/i', $name)) {
            $names[] = $name;
        }
    }
    return $names;
}

function useNames(string $clause): array
{
    if (!preg_match('/^(.*)\{(.*)\}$/s', $clause, $group)) {
        return useItems($clause);
    }
    $prefix = trim($group[1], "\\ ");
    return array_map(fn (string $item) => $prefix === '' ? $item : $prefix . '\\' . $item, useItems($group[2]));
}

function useSpecifiers(array $tokens, int $use, int $end): array
{
    $first = $tokens[$use + 1] ?? null;
    if ($first === null || in_array($first->id, [T_FUNCTION, T_CONST], true)) {
        return [];
    }
    return array_map(fn (string $name) => 'use:' . $name, useNames(useClauseText($tokens, $use + 1, $end)));
}

function isStaticRequireEnd(array $tokens, int $after, bool $paren): bool
{
    if (!$paren) {
        return textAt($tokens, $after) !== '.';
    }
    return textAt($tokens, $after) === ')' && textAt($tokens, $after + 1) !== '.';
}

function requireLiteral(array $tokens, int $at, bool $paren): ?string
{
    $literal = $tokens[$at] ?? null;
    if ($literal === null || $literal->id !== T_CONSTANT_ENCAPSED_STRING || !isStaticRequireEnd($tokens, $at + 1, $paren)) {
        return null;
    }
    $inner = substr($literal->text, 1, -1);
    return str_contains($inner, '$') ? null : $inner;
}

function requireSpecifier(array $tokens, int $index): ?string
{
    $paren = textAt($tokens, $index + 1) === '(';
    $cursor = $index + 1 + ($paren ? 1 : 0);
    $dir = isset($tokens[$cursor]) && $tokens[$cursor]->id === T_DIR && textAt($tokens, $cursor + 1) === '.';
    $inner = requireLiteral($tokens, $cursor + ($dir ? 2 : 0), $paren);
    if ($inner === null) {
        return null;
    }
    return $dir ? 'req:./' . ltrim($inner, '/') : 'req:' . $inner;
}

final class ImportScanner
{
    private array $braces = [];
    private array $found = [];

    public function __construct(private array $tokens)
    {
    }

    public function scan(): array
    {
        $count = count($this->tokens);
        for ($index = 0; $index < $count; $index++) {
            $index = $this->visit($index);
        }
        $unique = array_values(array_unique($this->found));
        sort($unique);
        return $unique;
    }

    private function visit(int $index): int
    {
        $token = $this->tokens[$index];
        $specifier = in_array($token->id, REQUIRE_TOKENS, true) ? requireSpecifier($this->tokens, $index) : null;
        if ($specifier !== null) {
            $this->found[] = $specifier;
        }
        return match ($token->id) {
            T_NAMESPACE => $this->enterNamespace($index),
            T_USE => $this->readUse($index),
            default => $this->trackBrace($index),
        };
    }

    private function enterNamespace(int $index): int
    {
        $end = $index;
        while (isset($this->tokens[$end + 1]) && !in_array($this->tokens[$end]->text, [';', '{'], true)) {
            $end++;
        }
        if ($this->tokens[$end]->text === '{') {
            $this->braces[] = true;
        }
        return $end;
    }

    private function readUse(int $index): int
    {
        $atNamespaceLevel = $this->braces === [] || end($this->braces);
        if (!$atNamespaceLevel || textAt($this->tokens, $index + 1) === '(') {
            return $index;
        }
        $end = statementEnd($this->tokens, $index);
        array_push($this->found, ...useSpecifiers($this->tokens, $index, $end));
        return $end;
    }

    private function trackBrace(int $index): int
    {
        $change = depthChange($this->tokens[$index]);
        $isBrace = in_array($this->tokens[$index]->text, ['{', '}', '${'], true);
        if ($isBrace && $change > 0) {
            $this->braces[] = false;
        }
        if ($isBrace && $change < 0) {
            array_pop($this->braces);
        }
        return $index;
    }
}

function describe(string $path): array
{
    try {
        $tokens = significantTokens((string) file_get_contents($path));
    } catch (ParseError $error) {
        return ['path' => $path, 'error' => $error->getMessage()];
    }
    return ['path' => $path, 'functions' => functions($tokens), 'imports' => (new ImportScanner($tokens))->scan()];
}

foreach (array_filter(explode("\0", (string) file_get_contents($argv[1])), fn (string $path) => $path !== '') as $path) {
    echo json_encode(describe($path), JSON_FLAGS), "\n";
}
