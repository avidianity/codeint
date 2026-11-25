use std::fs;
use std::path::Path;

use codeint_lang_python::Python;
use codeint_model::language::{FrameworkProfileResult, ImportRequest, Manifest, Resolution};
use codeint_model::{FrameworkProfileSelection, LanguagePlugin, RepoPath};

struct ImportCase<'a> {
    files: &'a [&'a str],
    manifests: &'a [(&'a str, &'a str)],
    importer: &'a str,
    specifier: &'a str,
    profile: &'a str,
    resolved_target: &'a str,
    external_fallback: &'a str,
}

fn py_profiles(
    root: &str,
    files: &[&str],
    manifests: &[(&str, &str)],
    selection: FrameworkProfileSelection,
) -> Result<FrameworkProfileResult, codeint_model::LanguagePluginError> {
    let repo: Vec<RepoPath> = files.iter().map(|p| RepoPath::new(*p).unwrap()).collect();
    let docs: Vec<Manifest> = manifests
        .iter()
        .map(|&(p, t)| Manifest {
            path: p.into(),
            text: t.into(),
        })
        .collect();
    Python.resolve_framework_profiles(root, &repo, docs, selection)
}

fn assert_import_fallback(case: &ImportCase<'_>) {
    let repo: Vec<RepoPath> = case
        .files
        .iter()
        .map(|p| RepoPath::new(*p).unwrap())
        .collect();
    let docs: Vec<Manifest> = case
        .manifests
        .iter()
        .map(|&(p, t)| Manifest {
            path: p.into(),
            text: t.into(),
        })
        .collect();
    let req = || {
        vec![ImportRequest {
            importer: case.importer.into(),
            specifier: case.specifier.into(),
        }]
    };
    let active = Python
        .resolve_imports(&repo, docs.clone(), req(), &[case.profile.into()])
        .unwrap();
    assert_eq!(
        active,
        vec![Resolution::Resolved(vec![case.resolved_target.into()])]
    );
    let inactive = Python.resolve_imports(&repo, docs, req(), &[]).unwrap();
    assert_eq!(
        inactive,
        vec![Resolution::External(case.external_fallback.into())]
    );
}

#[test]
fn known_framework_profiles_lists_django_fastapi_and_flask() {
    assert_eq!(
        Python.known_framework_profiles(),
        vec!["django", "fastapi", "flask"]
    );
}

#[test]
fn django_profile_detects_manifest_variants_entry_points_migrations_and_excludes_libraries() {
    let pyproject = "[project]\nname = \"mysite\"\ndependencies = [\"Django>=5.0\"]\n";
    let django_files = [
        "pyproject.toml",
        "manage.py",
        "mysite/wsgi.py",
        "mysite/asgi.py",
        "mysite/urls.py",
        "mysite/settings.py",
        "mysite/settings/prod.py",
        "blog/apps.py",
        "blog/models.py",
        "blog/models/post.py",
        "blog/admin.py",
        "blog/checks.py",
        "blog/signals.py",
        "blog/management/commands/seed.py",
        "blog/management/commands/_internal.py",
        "blog/templatetags/filters.py",
        "blog/migrations/__init__.py",
        "blog/migrations/0001_initial.py",
        "blog/migrations/0042_add_index.py",
        "blog/migrations/helpers.py",
        "blog/migrations/123_short.py",
        "blog/unused_helper.py",
        "staticfiles/bundle.py",
    ];
    let resolved = py_profiles(
        "",
        &django_files,
        &[("pyproject.toml", pyproject)],
        FrameworkProfileSelection::Auto,
    )
    .unwrap();
    assert_eq!(resolved.active_profiles, ["django"]);
    assert_eq!(
        resolved.entry_points,
        [
            "blog/admin.py",
            "blog/apps.py",
            "blog/checks.py",
            "blog/management/commands/seed.py",
            "blog/migrations/0001_initial.py",
            "blog/migrations/0042_add_index.py",
            "blog/migrations/123_short.py",
            "blog/migrations/__init__.py",
            "blog/migrations/helpers.py",
            "blog/models.py",
            "blog/models/post.py",
            "blog/signals.py",
            "blog/templatetags/filters.py",
            "manage.py",
            "mysite/asgi.py",
            "mysite/settings.py",
            "mysite/settings/prod.py",
            "mysite/urls.py",
            "mysite/wsgi.py",
        ]
    );
    assert_eq!(
        resolved.generated_paths,
        [
            "blog/migrations/0001_initial.py",
            "blog/migrations/0042_add_index.py",
            "staticfiles/bundle.py",
        ]
    );
    assert!(resolved.reference_ranges.is_empty() && resolved.reference_range_note.is_none());

    for (manifest_path, manifest_body) in [
        ("requirements.txt", "Django==5.1.2\npsycopg[binary]>=3.2\n"),
        (
            "setup.cfg",
            "[metadata]\nname = mysite\n[options]\ninstall_requires =\n    Django>=4.2\n",
        ),
        ("Pipfile", "[packages]\ndjango = \"*\"\n"),
    ] {
        let triad = py_profiles(
            "",
            &[manifest_path, "config/settings/prod.py", "config/wsgi.py"],
            &[(manifest_path, manifest_body)],
            FrameworkProfileSelection::Auto,
        )
        .unwrap();
        assert_eq!(triad.active_profiles, ["django"], "for {manifest_path}");
    }

    let library = py_profiles(
        "",
        &[
            "pyproject.toml",
            "src/django_filter/__init__.py",
            "src/django_filter/models.py",
            "tests/settings.py",
            "tests/wsgi.py",
        ],
        &[(
            "pyproject.toml",
            "[project]\nname = \"django-filter\"\ndependencies = [\"django>=4.2\"]\n",
        )],
        FrameworkProfileSelection::Auto,
    )
    .unwrap();
    assert!(library.active_profiles.is_empty());

    assert_import_fallback(&ImportCase {
        files: &[
            "backend/requirements.txt",
            "backend/manage.py",
            "backend/mysite/urls.py",
            "backend/blog/views.py",
        ],
        manifests: &[("backend/requirements.txt", "Django>=5.0\n")],
        importer: "backend/mysite/urls.py",
        specifier: "blog.views",
        profile: "django",
        resolved_target: "backend/blog/views.py",
        external_fallback: "blog",
    });
    assert_import_fallback(&ImportCase {
        files: &[
            "pyproject.toml",
            "manage.py",
            "mysite/urls.py",
            "apps/billing/service.py",
        ],
        manifests: &[("pyproject.toml", pyproject)],
        importer: "mysite/urls.py",
        specifier: "billing.service",
        profile: "django",
        resolved_target: "apps/billing/service.py",
        external_fallback: "billing",
    });
}

#[test]
fn fastapi_profile_detects_manifests_entrypoints_routers_and_rejects_starlette_only() {
    let pyproject = "[project]\nname = \"orders-api\"\ndependencies = [\"fastapi[standard]>=0.115\"]\n\n[tool.fastapi]\nentrypoint = \"svc.server:app\"\n";
    let files = [
        "pyproject.toml",
        "app/main.py",
        "app/routers/users.py",
        "svc/server.py",
        "app/unused.py",
    ];
    let resolved = py_profiles(
        "",
        &files,
        &[("pyproject.toml", pyproject)],
        FrameworkProfileSelection::Auto,
    )
    .unwrap();
    assert_eq!(resolved.active_profiles, ["fastapi"]);
    assert_eq!(
        resolved.entry_points,
        ["app/main.py", "app/routers/users.py", "svc/server.py"]
    );
    assert!(resolved.generated_paths.is_empty());

    assert_import_fallback(&ImportCase {
        files: &files,
        manifests: &[("pyproject.toml", pyproject)],
        importer: "app/main.py",
        specifier: "routers.users",
        profile: "fastapi",
        resolved_target: "app/routers/users.py",
        external_fallback: "routers",
    });

    for (neg_files, neg_manifest) in [
        (
            &["pyproject.toml", "main.py", "src/main.py"][..],
            "[project]\nname = \"asgi\"\ndependencies = [\"starlette>=0.40\", \"uvicorn>=0.30\"]\n",
        ),
        (
            &["pyproject.toml", "src/helper_pkg/auth.py"][..],
            "[project]\nname = \"helper-pkg\"\ndependencies = [\"fastapi>=0.115\"]\n",
        ),
    ] {
        let negative = py_profiles(
            "",
            neg_files,
            &[("pyproject.toml", neg_manifest)],
            FrameworkProfileSelection::Auto,
        )
        .unwrap();
        assert!(negative.active_profiles.is_empty());
    }

    let src_layout = py_profiles(
        "",
        &["requirements.txt", "src/main.py", "src/routers/health.py"],
        &[("requirements.txt", "fastapi==0.115.0\n")],
        FrameworkProfileSelection::Auto,
    )
    .unwrap();
    assert_eq!(src_layout.active_profiles, ["fastapi"]);
    assert_eq!(
        src_layout.entry_points,
        ["src/main.py", "src/routers/health.py"]
    );
}

#[test]
fn flask_profile_detects_manifests_views_blueprints_instance_and_rejects_werkzeug_only() {
    let resolved = py_profiles(
        "",
        &[
            "pyproject.toml",
            "app.py",
            "wsgi.py",
            "run.py",
            "web/views.py",
            "web/blueprints/auth.py",
            "instance/config.py",
            "web/unused.py",
        ],
        &[(
            "pyproject.toml",
            "[project]\nname = \"portal\"\ndependencies = [\"Flask>=3.0\"]\n",
        )],
        FrameworkProfileSelection::Auto,
    )
    .unwrap();
    assert_eq!(resolved.active_profiles, ["flask"]);
    assert_eq!(
        resolved.entry_points,
        [
            "app.py",
            "run.py",
            "web/blueprints/auth.py",
            "web/views.py",
            "wsgi.py",
        ]
    );
    assert_eq!(resolved.generated_paths, ["instance/config.py"]);

    let factory = py_profiles(
        "",
        &[
            "Pipfile",
            "flaskr/__init__.py",
            "flaskr/views.py",
            "flaskr/unused.py",
        ],
        &[("Pipfile", "[packages]\nFlask = \"*\"\n")],
        FrameworkProfileSelection::Auto,
    )
    .unwrap();
    assert_eq!(factory.active_profiles, ["flask"]);
    assert_eq!(
        factory.entry_points,
        ["flaskr/__init__.py", "flaskr/views.py"]
    );

    for (neg_files, neg_manifest) in [
        (
            &["pyproject.toml", "app.py", "wsgi.py"][..],
            "[project]\nname = \"wsgi\"\ndependencies = [\"werkzeug>=3.0\", \"jinja2>=3.1\", \"click>=8.1\"]\n",
        ),
        (
            &[
                "pyproject.toml",
                "src/flask_ext/__init__.py",
                "src/flask_ext/mixins.py",
            ][..],
            "[project]\nname = \"flask-ext\"\ndependencies = [\"Flask>=3.0\"]\n",
        ),
    ] {
        let negative = py_profiles(
            "",
            neg_files,
            &[("pyproject.toml", neg_manifest)],
            FrameworkProfileSelection::Auto,
        )
        .unwrap();
        assert!(negative.active_profiles.is_empty());
    }

    assert_import_fallback(&ImportCase {
        files: &[
            "backend/requirements.txt",
            "backend/app.py",
            "backend/views.py",
        ],
        manifests: &[("backend/requirements.txt", "Flask>=3.0\n")],
        importer: "backend/app.py",
        specifier: "views",
        profile: "flask",
        resolved_target: "backend/views.py",
        external_fallback: "views",
    });
}

#[test]
fn monorepo_isolation_selection_modes_and_invalid_manifests_behave_strictly() {
    let child = py_profiles(
        "packages/worker",
        &[
            "packages/worker/pyproject.toml",
            "packages/worker/manage.py",
            "packages/worker/main.py",
            "packages/worker/app.py",
        ],
        &[
            (
                "pyproject.toml",
                "[project]\nname = \"root\"\ndependencies = [\"django\", \"fastapi\", \"flask\"]\n",
            ),
            (
                "packages/worker/pyproject.toml",
                "[project]\nname = \"worker\"\n",
            ),
        ],
        FrameworkProfileSelection::Auto,
    )
    .unwrap();
    assert!(child.active_profiles.is_empty() && child.entry_points.is_empty());

    let hybrid_files = ["pyproject.toml", "main.py", "app.py"];
    let hybrid_docs = [(
        "pyproject.toml",
        "[project]\nname = \"hybrid\"\ndependencies = [\"fastapi\", \"flask\"]\n",
    )];
    let disabled = py_profiles(
        "",
        &hybrid_files,
        &hybrid_docs,
        FrameworkProfileSelection::Disabled,
    )
    .unwrap();
    assert!(disabled.active_profiles.is_empty());

    let excluded = py_profiles(
        "",
        &hybrid_files,
        &hybrid_docs,
        FrameworkProfileSelection::Exclude(vec!["flask".into()]),
    )
    .unwrap();
    assert_eq!(excluded.active_profiles, ["fastapi"]);

    let only = py_profiles(
        "",
        &["manage.py"],
        &[],
        FrameworkProfileSelection::Only(vec!["django".into()]),
    )
    .unwrap();
    assert_eq!(only.active_profiles, ["django"]);
    assert_eq!(only.entry_points, ["manage.py"]);

    assert!(
        py_profiles(
            "",
            &["main.py"],
            &[],
            FrameworkProfileSelection::Only(vec!["pyramid".into()]),
        )
        .is_err()
    );

    for (files, manifests) in [
        (
            &[".venv/pyproject.toml", "manage.py"][..],
            &[(
                ".venv/pyproject.toml",
                "[project]\ndependencies = [\"django\"]\n",
            )][..],
        ),
        (
            &["pyproject.toml", "manage.py"][..],
            &[("pyproject.toml", "[project\nbroken")][..],
        ),
    ] {
        let ignored = py_profiles("", files, manifests, FrameworkProfileSelection::Auto).unwrap();
        assert!(ignored.active_profiles.is_empty());
    }
}

#[test]
fn sibling_framework_modules_do_not_import_each_other() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/frameworks");
    let names = ["django", "fastapi", "flask"];
    for name in names {
        let content = fs::read_to_string(root.join(format!("{name}.rs"))).unwrap();
        let has_cross_import = names.iter().filter(|&&peer| peer != name).any(|peer| {
            content.contains(&format!("super::{peer}"))
                || content.contains(&format!("frameworks::{peer}"))
        });
        assert!(
            !has_cross_import,
            "{name}.rs must not import a sibling module"
        );
    }
}
