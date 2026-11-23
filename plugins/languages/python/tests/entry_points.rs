use codeint_lang_python::Python;
use codeint_model::{LanguagePlugin, Manifest, RepoPath};

#[test]
fn extracts_project_scripts_gui_scripts_entry_points_and_declared_packages() {
    let snapshot: Vec<RepoPath> = [
        "pyproject.toml",
        "src/demo/cli.py",
        "src/demo/pkg/__init__.py",
        "demo/gui.py",
        "demo/plugin.py",
        "helper_pkg/__init__.py",
        "packages/worker/pyproject.toml",
        "packages/worker/src/worker/__init__.py",
        "packages/worker/src/worker/__main__.py",
        "packages/worker/src/worker/entry.py",
        "packages/worker/src/worker/jobs.py",
        "packages/poetry_app/pyproject.toml",
        "packages/poetry_app/src/papp/__init__.py",
    ]
    .iter()
    .map(|item| RepoPath::new(*item).unwrap())
    .collect();

    let manifests = vec![
        Manifest {
            path: "pyproject.toml".to_owned(),
            text: "[project]\nname = \"demo\"\n[project.scripts]\napp-cli = \"demo.cli:main\"\npkg-cmd = \"demo.pkg:run [extra]\"\nmissing-cmd = \"demo.absent:run\"\n[project.gui-scripts]\napp-gui = \"demo.gui:launch\"\n[project.entry-points.\"demo.plugins\"]\next = \"demo.plugin:Plugin\"\n[tool.setuptools]\npackages = [\"helper_pkg\"]\n".to_owned(),
        },
        Manifest {
            path: "packages/worker/pyproject.toml".to_owned(),
            text: "[project]\nname = \"worker\"\n[tool.poetry.scripts]\nrun-worker = \"worker.entry:start\"\njob-runner = { callable = \"worker.jobs:main\" }\n[tool.hatch.build.targets.wheel]\npackages = [\"src/worker\"]\n".to_owned(),
        },
        Manifest {
            path: "packages/poetry_app/pyproject.toml".to_owned(),
            text: "[tool.poetry]\nname = \"papp\"\npackages = [{ include = \"papp\", from = \"src\" }]\n".to_owned(),
        },
    ];

    let found: Vec<String> = Python
        .entry_points(&snapshot, manifests)
        .expect("valid entry points")
        .into_iter()
        .map(|path| path.as_str().to_owned())
        .collect();
    assert_eq!(
        found,
        [
            "demo/gui.py",
            "demo/plugin.py",
            "helper_pkg/__init__.py",
            "packages/poetry_app/src/papp/__init__.py",
            "packages/worker/src/worker/__init__.py",
            "packages/worker/src/worker/__main__.py",
            "packages/worker/src/worker/entry.py",
            "packages/worker/src/worker/jobs.py",
            "src/demo/cli.py",
            "src/demo/pkg/__init__.py",
        ]
    );
}

#[test]
fn returns_empty_when_pyproject_declares_no_entry_points_or_is_invalid() {
    let snapshot = [
        RepoPath::new("pyproject.toml").unwrap(),
        RepoPath::new("webapp/__init__.py").unwrap(),
        RepoPath::new("webapp/views.py").unwrap(),
    ];
    let plain = Manifest {
        path: "pyproject.toml".to_owned(),
        text: "[project]\nname = \"webapp\"\n".to_owned(),
    };
    let broken = Manifest {
        path: "pyproject.toml".to_owned(),
        text: "[project\nname =".to_owned(),
    };
    assert_eq!(
        Python
            .entry_points(&snapshot, vec![plain])
            .expect("valid empty result"),
        Vec::<RepoPath>::new()
    );
    assert_eq!(
        Python
            .entry_points(&snapshot, vec![broken])
            .expect("valid empty result"),
        Vec::<RepoPath>::new()
    );
}

#[test]
fn rejects_parent_traversal_in_nested_pyproject_entries() {
    let repo = [
        "packages/app/pyproject.toml",
        "packages/app/src/app/__init__.py",
        "packages/other/secret/__init__.py",
        "packages/other/secret.py",
    ]
    .map(|item| RepoPath::new(item).unwrap());
    let pyproject = Manifest {
        path: "packages/app/pyproject.toml".to_owned(),
        text: "[project]\nname = \"app\"\n[project.scripts]\nescape = \"../other.secret:main\"\ndotted = \"..other.secret:main\"\n[tool.setuptools]\npackages = [\"../other/secret\"]\n[tool.hatch.build.targets.wheel]\npackages = [\"src/../../other/secret\"]\n[tool.poetry]\npackages = [{ include = \"../other/secret\" }, { include = \"secret\", from = \"../other\" }, { include = \"app\", from = \"src\" }]\n".to_owned(),
    };
    assert_eq!(
        Python.entry_points(&repo, vec![pyproject]).unwrap(),
        [RepoPath::new("packages/app/src/app/__init__.py").unwrap()]
    );
}
