use std::fs;
use std::process::Command;

use serde_json::{Value, json};
use tempfile::TempDir;

struct FixtureRepo {
    dir: TempDir,
}

impl FixtureRepo {
    fn new(entries: &[(&str, &str)]) -> Self {
        let repo = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        for &(rel, body) in entries {
            repo.write_file(rel, body);
        }
        let ok = Command::new("git")
            .args(["init", "-q", "-b", "main"])
            .current_dir(repo.dir.path())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .status()
            .unwrap()
            .success();
        assert!(ok);
        repo
    }

    fn write_under(&self, prefix: &str, entries: &[(&str, &str)]) {
        for &(rel, body) in entries {
            self.write_file(&format!("{prefix}/{rel}"), body);
        }
    }

    fn write_file(&self, rel: &str, body: &str) {
        let dest = self.dir.path().join(rel);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::write(dest, body).unwrap();
    }

    fn check(&self) -> Value {
        let out = Command::new(env!("CARGO_BIN_EXE_codeint"))
            .current_dir(self.dir.path())
            .args(["check", "--format", "json"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "check stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
}

fn orphan_list(report: &Value) -> Vec<&str> {
    report["source_sets"]["production"]["orphan_files"]["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["path"].as_str())
        .collect()
}

fn unlisted_list(report: &Value) -> Vec<&str> {
    report["dependencies"]["unlisted_dependencies"]["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["dependency"].as_str())
        .collect()
}

fn unresolved_imports(report: &Value) -> u64 {
    report["source_sets"]["production"]["import_cycles"]["unresolved_imports"]["count"]
        .as_u64()
        .unwrap()
}

fn package_profiles<'a>(report: &'a Value, pkg_path: &str) -> Option<&'a Value> {
    report["source_sets"]["production"]["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["path"] == pkg_path)
        .and_then(|entry| entry.get("framework_profiles"))
}

fn assert_source_counts(report: &Value, generated: Option<u64>, vendored: u64) {
    assert_eq!(
        report
            .pointer("/source_sets/generated/files")
            .and_then(Value::as_u64),
        generated
    );
    assert_eq!(
        report
            .pointer("/source_sets/vendored/files")
            .and_then(Value::as_u64),
        Some(vendored)
    );
}

#[test]
fn django_end_to_end_entry_points_generated_priority_fallback_imports_and_overrides() {
    let repo = FixtureRepo::new(&[
        (
            "codeint.json",
            r#"{ "source_sets": { "production": ["blog/migrations/0002_handwritten.py"] } }"#,
        ),
        (
            "pyproject.toml",
            "[project]\nname = \"mysite\"\ndependencies = [\"Django>=5.0\"]\n",
        ),
        (
            "manage.py",
            "import django\ndef main():\n    return django\n",
        ),
        ("mysite/settings.py", "DEBUG = False\n"),
        (
            "mysite/urls.py",
            "from blog.views import home\nurlpatterns = [home]\n",
        ),
        ("mysite/wsgi.py", "application = None\n"),
        ("blog/apps.py", "class BlogConfig:\n    name = 'blog'\n"),
        ("blog/models.py", "class Post:\n    title = ''\n"),
        ("blog/admin.py", "def register():\n    return True\n"),
        (
            "blog/views.py",
            "from billing.service import charge\ndef home():\n    return charge()\n",
        ),
        (
            "blog/management/commands/seed.py",
            "def handle():\n    return 1\n",
        ),
        ("blog/migrations/__init__.py", "MIGRATIONS = True\n"),
        (
            "blog/migrations/0001_initial.py",
            "class Migration:\n    operations = []\n",
        ),
        (
            "blog/migrations/0002_handwritten.py",
            "def backfill():\n    return 1\n",
        ),
        (
            "vendor/ext/migrations/0001_vendored.py",
            "def third_party():\n    return 0\n",
        ),
        ("apps/billing/service.py", "def charge():\n    return 42\n"),
        ("blog/dead_orphan.py", "def unused():\n    return 99\n"),
    ]);

    let report = repo.check();
    assert_eq!(report["framework_profiles"], json!(["django"]));
    assert_eq!(orphan_list(&report), ["blog/dead_orphan.py"]);
    assert_eq!(unresolved_imports(&report), 0);
    assert!(!unlisted_list(&report).contains(&"billing"));
    assert_source_counts(&report, Some(1), 1);

    repo.write_file(
        "codeint.json",
        r#"{ "framework_profiles": false, "source_sets": { "production": ["blog/migrations/0002_handwritten.py"] } }"#,
    );
    let disabled = repo.check();
    assert!(disabled.get("framework_profiles").is_none());
    assert_source_counts(&disabled, None, 1);
    assert!(unlisted_list(&disabled).contains(&"billing"));
}

#[test]
fn fastapi_and_flask_uv_monorepo_isolation_and_per_package_overrides() {
    let repo = FixtureRepo::new(&[(
        "pyproject.toml",
        "[project]\nname = \"workspace\"\n[tool.uv.workspace]\nmembers = [\"services/*\"]\n",
    )]);
    repo.write_under(
        "services/api",
        &[
            (
                "pyproject.toml",
                "[project]\nname = \"orders-api\"\ndependencies = [\"fastapi[standard]>=0.115\"]\n",
            ),
            (
                "app/main.py",
                "import fastapi\nfrom routers.items import list_items\ndef start():\n    return (fastapi, list_items())\n",
            ),
            ("app/routers/items.py", "def list_items():\n    return []\n"),
            ("app/dead_api.py", "def dead_api():\n    return 0\n"),
        ],
    );
    repo.write_under(
        "services/web",
        &[
            (
                "pyproject.toml",
                "[project]\nname = \"portal\"\ndependencies = [\"Flask>=3.0\"]\n",
            ),
            ("app.py", "import flask\ndef create():\n    return flask\n"),
            ("views.py", "def index():\n    return 'ok'\n"),
            ("blueprints/auth.py", "def login():\n    return True\n"),
            ("instance/config.py", "SECRET = 'dev'\n"),
            ("helpers/dead_web.py", "def dead_web():\n    return 0\n"),
        ],
    );
    repo.write_under(
        "services/plain",
        &[
            ("pyproject.toml", "[project]\nname = \"plain-lib\"\n"),
            ("plain_lib/__init__.py", "def value():\n    return 1\n"),
        ],
    );

    let report = repo.check();
    assert_eq!(report["framework_profiles"], json!(["fastapi", "flask"]));
    assert_eq!(
        package_profiles(&report, "services/api"),
        Some(&json!(["fastapi"]))
    );
    assert_eq!(
        package_profiles(&report, "services/web"),
        Some(&json!(["flask"]))
    );
    assert_eq!(package_profiles(&report, "services/plain"), None);
    assert_eq!(
        orphan_list(&report),
        [
            "services/api/app/dead_api.py",
            "services/web/helpers/dead_web.py",
        ]
    );
    assert_eq!(
        report.pointer("/source_sets/generated/files"),
        Some(&json!(1))
    );
    assert!(!unlisted_list(&report).contains(&"routers"));

    repo.write_file(
        "codeint.json",
        r#"{ "framework_profiles": { "exclude": ["flask"] } }"#,
    );
    let excluded = repo.check();
    assert_eq!(excluded["framework_profiles"], json!(["fastapi"]));
    assert_eq!(package_profiles(&excluded, "services/web"), None);

    repo.write_file(
        "codeint.json",
        r#"{ "packages": { "services/api": { "framework_profiles": false } } }"#,
    );
    let per_pkg = repo.check();
    assert_eq!(per_pkg["framework_profiles"], json!(["flask"]));
    assert_eq!(package_profiles(&per_pkg, "services/api"), None);
}

#[test]
fn flutter_end_to_end_app_vs_dart_package_generated_priority_fallback_imports_and_overrides() {
    let repo = FixtureRepo::new(&[
        (
            "codeint.json",
            r#"{ "source_sets": { "production": ["apps/mobile/lib/l10n/app_localizations_override.dart"] } }"#,
        ),
        (
            "pubspec.yaml",
            "name: mono\nworkspace:\n  - apps/mobile\n  - packages/core\n",
        ),
    ]);
    repo.write_under(
        "apps/mobile",
        &[
            (
                "pubspec.yaml",
                "name: mobile_app\nresolution: workspace\ndependencies:\n  flutter:\n    sdk: flutter\n  core:\n    path: ../../packages/core\nflutter:\n  uses-material-design: true\n",
            ),
            (
                "lib/main.dart",
                "import 'package:flutter/material.dart';\nimport 'package:flutter_gen/gen_l10n/app_localizations.dart';\nimport 'package:core/core.dart';\nimport 'models/user.dart';\nint runAppEntry() => coreValue() + userScore();\n",
            ),
            (
                "lib/main_staging.dart",
                "import 'main.dart';\nint runStaging() => runAppEntry();\n",
            ),
            (
                "lib/models/user.dart",
                "part 'user.g.dart';\nint userScore() => 7;\n",
            ),
            (
                "lib/features/unused_screen.dart",
                "int deadScreen() => 0;\n",
            ),
            (
                "lib/generated_plugin_registrant.dart",
                "void registerPlugins() {}\n",
            ),
            (
                "lib/gen/assets.gen.dart",
                "String logoPath() => 'logo.png';\n",
            ),
            (
                "lib/l10n/app_localizations.dart",
                "String appTitle() => 'Demo';\n",
            ),
            (
                "lib/l10n/app_localizations_override.dart",
                "String customTitle() => 'Override';\n",
            ),
            ("vendor/gen/vendored.gen.dart", "int vendoredHelper() => 1;\n"),
            (
                "integration_test/app_test.dart",
                "int runIntegration() => 1;\n",
            ),
        ],
    );
    repo.write_under(
        "packages/core",
        &[
            ("pubspec.yaml", "name: core\nresolution: workspace\n"),
            ("lib/core.dart", "int coreValue() => 3;\n"),
        ],
    );

    let report = repo.check();
    assert_eq!(report["framework_profiles"], json!(["flutter"]));
    assert_eq!(
        package_profiles(&report, "apps/mobile"),
        Some(&json!(["flutter"]))
    );
    assert_eq!(package_profiles(&report, "packages/core"), None);
    assert_eq!(
        orphan_list(&report),
        [
            "apps/mobile/lib/features/unused_screen.dart",
            "apps/mobile/lib/l10n/app_localizations_override.dart",
        ]
    );
    assert_eq!(unresolved_imports(&report), 0);
    assert!(!unlisted_list(&report).contains(&"flutter_gen"));
    assert!(!unlisted_list(&report).contains(&"flutter"));
    assert_source_counts(&report, Some(3), 1);
    assert_eq!(report.pointer("/source_sets/test/files"), Some(&json!(1)));

    repo.write_file(
        "codeint.json",
        r#"{ "framework_profiles": false, "source_sets": { "production": ["apps/mobile/lib/l10n/app_localizations_override.dart"] } }"#,
    );
    let disabled = repo.check();
    assert!(disabled.get("framework_profiles").is_none());
    assert_source_counts(&disabled, None, 1);
    assert_eq!(unresolved_imports(&disabled), 1);
    assert!(unlisted_list(&disabled).contains(&"flutter_gen"));
}
