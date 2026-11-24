use codeint_lang_python::{Python, flagged_line_rules};
use codeint_model::language::{DeclaredDep, DepKind, Function, ImportRequest, Resolution};
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
            name: Some("evaluate".to_owned()),
            cognitive_complexity: 5,
            coverage: None,
        }
    );
    assert_eq!(model.functions[1].decision_points, 1);
    assert_eq!(model.functions[1].name, None);
    assert_eq!(model.functions[1].cognitive_complexity, 1);

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
                specifier: ".Flask".into(),
            },
            ImportRequest {
                importer: "src/demo/main.py".into(),
                specifier: "..missing".into(),
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
    assert_eq!(resolutions[2], Resolution::Builtin);
    assert_eq!(
        resolutions[3],
        Resolution::Resolved("src/demo/__init__.py".into())
    );
    assert!(matches!(resolutions[4], Resolution::Unresolved(_)));

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

#[test]
fn python_stdlib_pep503_external_normalization_and_declared_dependencies() {
    const ROOT_PYPROJECT: &str = r#"[project]
name = "acme-root"
dependencies = [
    "Requests>=2.31.0",
    "Pydantic_Settings[dotenv]>=2.4 ; python_version >= '3.10'",
]

[project.optional-dependencies]
postgres = ["Psycopg_Pool>=3.2"]

[dependency-groups]
dev = [
    { include-group = "lint" },
    "pytest>=8.3",
]
lint = ["ruff>=0.7"]

[tool.uv.workspace]
members = ["packages/*"]
"#;
    const CORE_PYPROJECT: &str = r#"[project]
name = "acme_core"
dependencies = [
    "acme-root",
    "zope.interface>=7.0",
]

[tool.poetry.dependencies]
python = "^3.12"
httpx = "^0.27"
orjson = { version = "^3.10", optional = true }

[tool.poetry.group.dev.dependencies]
mypy = "^1.11"
"#;
    const CLIENT_PY: &str = r#"import os.path
from collections.abc import Mapping
import tomllib
from pydantic_settings.sources import EnvSettingsSource
import httpx
from zope.interface import Interface


def fetch(cfg: Mapping[str, str]) -> str:
    return os.path.join(cfg.get("root", "."), str(EnvSettingsSource), str(httpx), str(Interface), str(tomllib))
"#;
    let client_path = "packages/core/src/acme_core/client.py";
    let model = Python
        .lower(&RepoPath::new(client_path).unwrap(), CLIENT_PY)
        .unwrap();
    let repo: Vec<RepoPath> = [
        "pyproject.toml",
        "packages/core/pyproject.toml",
        "packages/core/src/acme_core/__init__.py",
        client_path,
    ]
    .map(|p| RepoPath::new(p).unwrap())
    .into();
    let manifests = vec![
        Manifest {
            path: "pyproject.toml".into(),
            text: ROOT_PYPROJECT.into(),
        },
        Manifest {
            path: "packages/core/pyproject.toml".into(),
            text: CORE_PYPROJECT.into(),
        },
    ];
    let requests = model
        .runtime_imports
        .iter()
        .map(|spec| ImportRequest {
            importer: client_path.into(),
            specifier: spec.clone(),
        })
        .collect();
    let resolutions = Python.resolve_imports(&repo, manifests.clone(), requests);
    let expected = [
        ("collections.abc", Resolution::Builtin),
        ("httpx", Resolution::External("httpx".into())),
        ("os.path", Resolution::Builtin),
        (
            "pydantic_settings.sources",
            Resolution::External("pydantic-settings".into()),
        ),
        ("tomllib", Resolution::Builtin),
        ("zope.interface", Resolution::External("zope".into())),
    ];
    for ((spec, actual), (want_spec, want_res)) in
        model.runtime_imports.iter().zip(resolutions).zip(expected)
    {
        assert_eq!((spec.as_str(), actual), (want_spec, want_res));
    }

    let deps = Python.declared_dependencies(manifests).unwrap();
    let expected_deps = [
        ("", "pydantic-settings", DepKind::Runtime),
        ("", "requests", DepKind::Runtime),
        ("", "pytest", DepKind::Dev),
        ("", "ruff", DepKind::Dev),
        ("", "psycopg-pool", DepKind::Optional),
        ("packages/core", "httpx", DepKind::Runtime),
        ("packages/core", "zope-interface", DepKind::Runtime),
        ("packages/core", "mypy", DepKind::Dev),
        ("packages/core", "orjson", DepKind::Optional),
    ]
    .map(|(root, name, kind)| DeclaredDep {
        package_root: root.to_owned(),
        name: name.to_owned(),
        kind,
    });
    assert_eq!(deps, expected_deps);
}

#[test]
fn python_function_names_cover_defs_async_methods_nested_and_anonymous_lambdas() {
    let src = [
        "def top_level(x):",
        "    return x + 1",
        "async def fetch_data(url):",
        "    return url",
        "class Service:",
        "    def __init__(self, client):",
        "        self.client = client",
        "    async def process(self, items):",
        "        def helper(v):",
        "            return v * 2",
        "        return list(map(lambda item: helper(item), items))",
    ]
    .join("\n");
    let model = Python
        .lower(&RepoPath::new("src/names.py").unwrap(), &src)
        .unwrap();
    let extracted: Vec<(u32, Option<&str>)> = model
        .functions
        .iter()
        .map(|f| (f.line, f.name.as_deref()))
        .collect();
    assert_eq!(
        extracted,
        [
            (1, Some("top_level")),
            (3, Some("fetch_data")),
            (6, Some("__init__")),
            (8, Some("process")),
            (9, Some("helper")),
            (11, None),
        ]
    );
}

#[test]
fn python_cognitive_complexity_distinguishes_flat_vs_nested_and_isolates_inner_functions() {
    let src = [
        "def flat_match(x):",
        "    match x:",
        "        case 1:",
        "            return 10",
        "        case 2:",
        "            return 20",
        "        case 3:",
        "            return 30",
        "        case 4:",
        "            return 40",
        "        case _:",
        "            return 0",
        "def flat_ifs(a, b, c, d):",
        "    n = 0",
        "    if a:",
        "        n += 1",
        "    if b:",
        "        n += 1",
        "    if c:",
        "        n += 1",
        "    if d:",
        "        n += 1",
        "    return n",
        "def deeply_nested(a, b, c, d):",
        "    if a:",
        "        if b:",
        "            if c:",
        "                if d:",
        "                    return 1",
        "    return 0",
        "def sibling_ternaries(a, b):",
        "    return (1 if a else 2) + (3 if b else 4)",
        "def outer(items, a, b, c):",
        "    if a and b or c:",
        "        for item in items:",
        "            if item < 0:",
        "                return -1",
        "            elif item == 0:",
        "                continue",
        "            else:",
        "                try:",
        "                    def inner(v):",
        "                        if v > 0:",
        "                            if v > 1:",
        "                                return v",
        "                        return 0",
        "                    fn = lambda z: z if z > 0 else 0",
        "                    return inner(fn(item))",
        "                except ValueError:",
        "                    return outer([], False, b, c)",
        "    return 0",
    ]
    .join("\n");
    let model = Python
        .lower(&RepoPath::new("src/cognitive.py").unwrap(), &src)
        .unwrap();
    let expected = [
        (Some("flat_match"), 4u32, 1u32),
        (Some("flat_ifs"), 4, 4),
        (Some("deeply_nested"), 4, 10),
        (Some("sibling_ternaries"), 2, 2),
        (Some("outer"), 7, 15),
        (Some("inner"), 2, 3),
        (None, 1, 1),
    ];
    assert_eq!(model.functions.len(), expected.len());
    for (func, (name, dp, cog)) in model.functions.iter().zip(expected) {
        assert_eq!(
            (
                func.name.as_deref(),
                func.decision_points,
                func.cognitive_complexity
            ),
            (name, dp, cog)
        );
    }
}

#[test]
fn relative_symbol_imports_fall_back_to_package_init_without_hiding_missing_submodules() {
    let cli_src = "from . import Flask\nfrom . import *\ndef run():\n    return Flask\n";
    let lowered = Python
        .lower(&RepoPath::new("src/flask/cli.py").unwrap(), cli_src)
        .unwrap();
    assert_eq!(lowered.runtime_imports, [".", ".Flask"]);

    let repo: Vec<RepoPath> = [
        "pyproject.toml",
        "src/flask/__init__.py",
        "src/flask/cli.py",
        "src/flask/helpers.py",
        "src/flask/json/__init__.py",
        "src/flask/json/provider.py",
    ]
    .into_iter()
    .map(|p| RepoPath::new(p).unwrap())
    .collect();
    let manifests = vec![Manifest {
        path: "pyproject.toml".into(),
        text: "[project]\nname = \"Flask\"\n".into(),
    }];
    let requests: Vec<ImportRequest> = [
        ("src/flask/cli.py", ".Flask"),
        ("src/flask/cli.py", "."),
        ("src/flask/cli.py", ".helpers"),
        ("src/flask/json/provider.py", "..current_app"),
        ("src/flask/cli.py", ".sub.missing"),
        ("src/flask/__init__.py", ".missing"),
    ]
    .into_iter()
    .map(|(importer, specifier)| ImportRequest {
        importer: importer.to_owned(),
        specifier: specifier.to_owned(),
    })
    .collect();

    let resolutions = Python.resolve_imports(&repo, manifests, requests);
    assert_eq!(
        resolutions[0],
        Resolution::Resolved("src/flask/__init__.py".into())
    );
    assert_eq!(
        resolutions[1],
        Resolution::Resolved("src/flask/__init__.py".into())
    );
    assert_eq!(
        resolutions[2],
        Resolution::Resolved("src/flask/helpers.py".into())
    );
    assert_eq!(
        resolutions[3],
        Resolution::Resolved("src/flask/__init__.py".into())
    );
    assert!(matches!(resolutions[4], Resolution::Unresolved(_)));
    assert!(matches!(resolutions[5], Resolution::Unresolved(_)));
}
