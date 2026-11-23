use codeint_lang_python::{Python, flagged_line_rules};
use codeint_model::language::{Function, ImportRequest, Resolution};
use codeint_model::{LanguagePlugin, LowerError, Manifest, RepoPath, SourceSet, TokenKind};

#[test]
fn python_plugin_contract_and_python2_rejection() {
    assert_eq!(Python.extensions(), ["py"]);
    let ranges = Python.reference_ranges().unwrap();
    for metric in ["erosion", "verbosity", "duplication", "import_cycles"] {
        assert!(ranges.for_metric(metric).is_some(), "missing {metric}");
    }
    let bad_cases = [
        ("a.pyi", "def f() -> int: ...\n", true),
        ("a.py", "def f():\n    s = 'unterminated\n", false),
        ("legacy.py", "print 'hello'\n", false),
        ("legacy_op.py", "if a <> b:\n    pass\n", false),
    ];
    for (path, src, expect_unsupported) in bad_cases {
        let res = Python.lower(&RepoPath::new(path).unwrap(), src);
        if expect_unsupported {
            assert!(matches!(res, Err(LowerError::UnsupportedFile { .. })));
        } else {
            assert!(matches!(res, Err(LowerError::Unparsable { .. })));
        }
    }
    let empty = Python
        .lower(&RepoPath::new("empty.py").unwrap(), "")
        .unwrap();
    assert_eq!(empty.source_set, SourceSet::Production);
    assert_eq!(empty.source_lines, 0);
}

#[test]
fn python_functions_decisions_and_source_sets() {
    let src = [
        "async def evaluate(x):",
        "    if x is not None and x > 0:",
        "        for i in range(x):",
        "            x += i",
        "    f = lambda v: v if v > 0 else 0",
        "    match x:",
        "        case 0:",
        "            return f(0)",
        "        case _:",
        "            return f(x)",
    ]
    .join("\n");
    let model = Python
        .lower(&RepoPath::new("src/app/eval.py").unwrap(), &src)
        .unwrap();
    assert_eq!(model.functions.len(), 2);
    assert_eq!(
        model.functions[0],
        Function {
            line: 1,
            end_line: 10,
            decision_points: 4,
            source_lines: 10,
            name: None,
            cognitive_complexity: 0,
        }
    );
    assert_eq!(model.functions[1].decision_points, 1);

    for (path, body, expected) in [
        ("tests/test_app.py", "x = 1\n", SourceSet::Test),
        ("pkg/conftest.py", "x = 1\n", SourceSet::Test),
        ("pkg/api_pb2.py", "x = 1\n", SourceSet::Generated),
        ("pkg/gen.py", "# @generated\nx = 1\n", SourceSet::Generated),
    ] {
        let m = Python.lower(&RepoPath::new(path).unwrap(), body).unwrap();
        assert_eq!(m.source_set, expected, "wrong source set for {path}");
    }
}

#[test]
fn python_imports_type_checking_resolution_and_workspaces() {
    let src = [
        "from __future__ import annotations",
        "from typing import TYPE_CHECKING",
        "import os",
        "from . import helpers",
        "from demo.utils import run",
        "if TYPE_CHECKING:",
        "    from demo.types import Widget",
        "def entry():",
        "    return run(helpers)",
    ]
    .join("\n");
    let model = Python
        .lower(&RepoPath::new("src/demo/main.py").unwrap(), &src)
        .unwrap();
    assert_eq!(
        model.runtime_imports,
        [".helpers", "demo.utils", "os", "typing"]
    );
    assert!(
        model
            .tokens
            .iter()
            .filter(|t| t.line <= 5 || t.line == 7)
            .all(|t| t.kind == TokenKind::ImportOrExport)
    );

    let repo: Vec<RepoPath> = [
        "pyproject.toml",
        "src/demo/__init__.py",
        "src/demo/main.py",
        "src/demo/helpers.py",
        "src/demo/utils.py",
    ]
    .into_iter()
    .map(|p| RepoPath::new(p).unwrap())
    .collect();
    let resolutions = Python.resolve_imports(
        &repo,
        vec![Manifest {
            path: "pyproject.toml".into(),
            text: "[project]\nname = \"demo\"\n".into(),
        }],
        vec![
            ImportRequest {
                importer: "src/demo/main.py".into(),
                specifier: ".helpers".into(),
            },
            ImportRequest {
                importer: "src/demo/main.py".into(),
                specifier: "demo.utils".into(),
            },
            ImportRequest {
                importer: "src/demo/main.py".into(),
                specifier: "os".into(),
            },
            ImportRequest {
                importer: "src/demo/main.py".into(),
                specifier: ".missing".into(),
            },
        ],
    );
    assert_eq!(
        resolutions[0],
        Resolution::Resolved("src/demo/helpers.py".into())
    );
    assert_eq!(
        resolutions[1],
        Resolution::Resolved("src/demo/utils.py".into())
    );
    assert_eq!(resolutions[2], Resolution::External(String::new()));
    assert!(matches!(resolutions[3], Resolution::Unresolved(_)));

    let packages = Python
        .workspace_packages(vec![
            Manifest {
                path: "pyproject.toml".into(),
                text: "[tool.uv.workspace]\nmembers = [\"packages/*\"]\nexclude = [\"packages/ignored\"]\n".into(),
            },
            Manifest {
                path: "packages/core/pyproject.toml".into(),
                text: "[project]\nname = \"core\"\n".into(),
            },
            Manifest {
                path: "packages/ignored/pyproject.toml".into(),
                text: "[project]\nname = \"ignored\"\n".into(),
            },
        ])
        .unwrap();
    assert_eq!(packages, [RepoPath::new("packages/core").unwrap()]);
}

#[test]
fn python_verbosity_rules() {
    let mut ids: Vec<_> = flagged_line_rules().collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        [
            "conditional-yields-boolean",
            "else-after-return",
            "except-only-reraises",
            "if-else-returns-boolean",
            "trailing-bare-return",
        ]
    );
    let cases = [
        (
            "else-after-return",
            "def f(x):\n    if x > 0:\n        return 1\n    else:\n        return 2\n",
            "def f(x):\n    if x > 0:\n        y = 1\n    else:\n        y = 2\n    return y\n",
        ),
        (
            "if-else-returns-boolean",
            "def f(x):\n    if x > 0:\n        return True\n    else:\n        return False\n",
            "def f(x):\n    if x > 0:\n        return 1\n    else:\n        return 0\n",
        ),
        (
            "conditional-yields-boolean",
            "def f(x):\n    return True if x > 0 else False\n",
            "def f(x):\n    return 'a' if x > 0 else 'b'\n",
        ),
        (
            "except-only-reraises",
            "def f():\n    try:\n        work()\n    except ValueError as err:\n        raise err\n",
            "def f():\n    try:\n        work()\n    except ValueError as err:\n        log(err)\n        raise\n",
        ),
        (
            "trailing-bare-return",
            "def f():\n    work()\n    return None\n",
            "def f(x):\n    if x:\n        return\n    work()\n",
        ),
    ];
    for (rule, bad, good) in cases {
        let bad_model = Python.lower(&RepoPath::new("a.py").unwrap(), bad).unwrap();
        let good_model = Python.lower(&RepoPath::new("b.py").unwrap(), good).unwrap();
        assert!(bad_model.flagged_lines.iter().any(|l| l.rule == rule));
        assert!(!good_model.flagged_lines.iter().any(|l| l.rule == rule));
    }
}
