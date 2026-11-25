import ast
import json
import sys

FUNCTION_NODES = (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda)
SINGLE_DECISIONS = (
    ast.If,
    ast.IfExp,
    ast.For,
    ast.AsyncFor,
    ast.While,
    ast.ExceptHandler,
)


def is_bare_wildcard(case):
    pattern = case.pattern
    plain = isinstance(pattern, ast.MatchAs) and pattern.pattern is None
    return plain and pattern.name is None and case.guard is None


def case_weight(case):
    if is_bare_wildcard(case):
        return 0
    return 1 + int(case.guard is not None)


def decision_weight(node):
    if isinstance(node, SINGLE_DECISIONS):
        return 1
    if isinstance(node, ast.comprehension):
        return 1 + len(node.ifs)
    if isinstance(node, ast.BoolOp):
        return len(node.values) - 1
    if isinstance(node, ast.match_case):
        return case_weight(node)
    return 0


def own_decisions(function):
    total = 0
    pending = list(ast.iter_child_nodes(function))
    while pending:
        node = pending.pop()
        if isinstance(node, FUNCTION_NODES):
            continue
        total += decision_weight(node)
        pending.extend(ast.iter_child_nodes(node))
    return total


def functions(tree):
    return [
        [node.lineno, node.end_lineno, own_decisions(node)]
        for node in ast.walk(tree)
        if isinstance(node, FUNCTION_NODES)
    ]


def is_type_checking_guard(node):
    test = node.test
    named = isinstance(test, ast.Name) and test.id == "TYPE_CHECKING"
    attribute = isinstance(test, ast.Attribute) and test.attr == "TYPE_CHECKING"
    return named or attribute


def from_import_names(node):
    dots = "." * node.level
    if node.level == 0 and node.module == "__future__":
        return []
    if node.module:
        return [dots + node.module]
    return [dots if alias.name == "*" else dots + alias.name for alias in node.names]


def import_names(node):
    if isinstance(node, ast.Import):
        return [alias.name for alias in node.names]
    if isinstance(node, ast.ImportFrom):
        return from_import_names(node)
    return []


def imports(tree):
    found = set()
    pending = [tree]
    while pending:
        node = pending.pop()
        if isinstance(node, ast.If) and is_type_checking_guard(node):
            pending.extend(node.orelse)
            continue
        found.update(import_names(node))
        pending.extend(ast.iter_child_nodes(node))
    return sorted(found)


def describe(path):
    try:
        with open(path, encoding="utf-8") as handle:
            tree = ast.parse(handle.read(), filename=path)
    except (SyntaxError, UnicodeDecodeError, ValueError) as error:
        return {"path": path, "error": str(error)}
    return {"path": path, "functions": functions(tree), "imports": imports(tree)}


def main():
    with open(sys.argv[1], encoding="utf-8") as listing:
        paths = [line.rstrip("\n") for line in listing if line.strip()]
    for path in paths:
        print(json.dumps(describe(path)))


main()
