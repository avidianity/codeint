use std::fs;
use std::process::{Command, Output};

use serde_json::{Value, json};
use tempfile::TempDir;

struct PhpFixture {
    temp: TempDir,
}

impl PhpFixture {
    fn create(files: &[(&str, &str)]) -> Self {
        let fixture = Self {
            temp: tempfile::tempdir().expect("tempdir"),
        };
        for &(rel, body) in files {
            fixture.write_file(rel, body);
        }
        let status = Command::new("git")
            .args(["init", "-q", "-b", "main"])
            .current_dir(fixture.temp.path())
            .envs([
                ("GIT_CONFIG_GLOBAL", "/dev/null"),
                ("GIT_CONFIG_NOSYSTEM", "1"),
            ])
            .status()
            .expect("git init");
        assert!(status.success(), "git init failed");
        fixture
    }

    fn write_file(&self, rel: &str, body: &str) {
        let path = self.temp.path().join(rel);
        let dir = path.parent().expect("file parent");
        if !dir.exists() {
            fs::create_dir_all(dir).expect("mkdir");
        }
        fs::write(&path, body).expect("write");
    }

    fn check_json(&self) -> Value {
        let output = self.run(&["check", "--format", "json"]);
        assert!(
            output.status.success(),
            "codeint check failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("parse check json")
    }

    fn run(&self, args: &[&str]) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_codeint"));
        cmd.current_dir(self.temp.path()).args(args);
        cmd.output().expect("run codeint")
    }

    fn write_config(&self, body: &str) {
        fs::write(self.temp.path().join("codeint.json"), body).expect("write config");
    }
}

fn orphan_list(report: &Value) -> Vec<&str> {
    report
        .pointer("/source_sets/production/orphan_files/findings")
        .and_then(Value::as_array)
        .expect("orphan findings")
        .iter()
        .filter_map(|finding| finding["path"].as_str())
        .collect()
}

fn assert_single_framework_report(
    report: &Value,
    expected_profile: &str,
    expected_generated: u64,
    expected_orphan: &str,
) {
    assert_eq!(report["framework_profiles"], json!([expected_profile]));
    assert!(report.get("reference_range_note").is_none());
    assert_eq!(
        report
            .pointer("/source_sets/production/import_cycles/unresolved_imports/count")
            .and_then(Value::as_u64),
        Some(0),
        "unexpected unresolved imports: {report}"
    );
    assert_eq!(
        report.pointer("/source_sets/generated/files"),
        Some(&json!(expected_generated)),
        "expected {expected_generated} generated files: {report}"
    );
    assert_eq!(orphan_list(report), vec![expected_orphan]);
}

fn assert_pkg_profiles(report: &Value, pkg_path: &str, expected: Option<&[&str]>) {
    let packages = report
        .pointer("/source_sets/production/packages")
        .and_then(Value::as_array)
        .expect("production packages");
    let found = packages
        .iter()
        .find(|entry| entry["path"] == pkg_path)
        .unwrap_or_else(|| panic!("package {pkg_path} missing from {report}"));
    match expected {
        Some(profiles) => assert_eq!(found["framework_profiles"], json!(profiles)),
        None => assert!(found.get("framework_profiles").is_none(), "{found}"),
    }
}

#[test]
fn laravel_end_to_end_profiles_conventions_generated_fallback_and_symfony_isolation() {
    let fixture = PhpFixture::create(&[
        (
            "composer.json",
            r#"{"name":"acme/laravel-app","require":{"laravel/framework":"^12.0","symfony/console":"^7.2","symfony/http-foundation":"^7.2","symfony/routing":"^7.2"}}"#,
        ),
        (
            "composer.lock",
            r#"{"packages":[{"name":"laravel/framework"},{"name":"symfony/console"},{"name":"symfony/http-foundation"}]}"#,
        ),
        ("artisan", "#!/usr/bin/env php\n<?php\n"),
        (
            "public/index.php",
            "<?php\nfunction boot(): int { return 1; }\n",
        ),
        (
            "bootstrap/app.php",
            "<?php\nfunction createApp(): int { return 1; }\n",
        ),
        (
            "routes/web.php",
            "<?php\nuse App\\Http\\Controllers\\UserController;\nfunction routes(): UserController { return new UserController(); }\n",
        ),
        (
            "app/Http/Controllers/UserController.php",
            "<?php\nnamespace App\\Http\\Controllers;\nclass UserController { public function index(): int { return 1; } }\n",
        ),
        (
            "app/Providers/AppServiceProvider.php",
            "<?php\nnamespace App\\Providers;\nclass AppServiceProvider { public function register(): void {} }\n",
        ),
        (
            "database/seeders/DatabaseSeeder.php",
            "<?php\nnamespace Database\\Seeders;\nuse Database\\Factories\\UserFactory;\nclass DatabaseSeeder { public function run(): UserFactory { return new UserFactory(); } }\n",
        ),
        (
            "database/factories/UserFactory.php",
            "<?php\nnamespace Database\\Factories;\nclass UserFactory { public function make(): int { return 1; } }\n",
        ),
        (
            "app/Services/OrphanService.php",
            "<?php\nnamespace App\\Services;\nclass OrphanService { public function run(): int { return 1; } }\n",
        ),
        (
            "bootstrap/cache/services.php",
            "<?php\nfunction cachedServices(): array { return []; }\n",
        ),
        (
            "storage/framework/views/compiled.php",
            "<?php\nfunction compiledView(): string { return 'ok'; }\n",
        ),
    ]);

    assert_single_framework_report(
        &fixture.check_json(),
        "laravel",
        2,
        "app/Services/OrphanService.php",
    );

    fixture
        .write_config(r#"{ "framework_profiles": false, "entry_points": ["public/index.php"] }"#);
    let disabled = fixture.check_json();
    assert!(disabled.get("framework_profiles").is_none());
    assert!(disabled.pointer("/source_sets/generated").is_none());
    assert!(orphan_list(&disabled).contains(&"app/Providers/AppServiceProvider.php"));
    let unlisted = disabled["dependencies"]["unlisted_dependencies"]["findings"]
        .as_array()
        .expect("unlisted");
    assert!(
        unlisted.iter().any(|item| item["dependency"] == "app/http"),
        "expected App\\ fallback hint to be disabled: {disabled}"
    );
}

struct FrameworkCase {
    profile: &'static str,
    generated: u64,
    orphan: &'static str,
    files: &'static [(&'static str, &'static str)],
}

#[test]
fn symfony_cakephp_and_codeigniter_end_to_end_profiles_conventions_and_generated_paths() {
    let cases = [
        FrameworkCase {
            profile: "symfony",
            generated: 1,
            orphan: "src/Service/OrphanService.php",
            files: &[
                (
                    "composer.json",
                    r#"{"name":"acme/symfony-app","require":{"symfony/framework-bundle":"^7.2"}}"#,
                ),
                ("bin/console", "#!/usr/bin/env php\n"),
                (
                    "config/bundles.php",
                    "<?php\nfunction bundles(): array { return []; }\n",
                ),
                (
                    "src/Kernel.php",
                    "<?php\nnamespace App;\nclass Kernel { public function boot(): void {} }\n",
                ),
                (
                    "src/Controller/HomeController.php",
                    "<?php\nnamespace App\\Controller;\nuse App\\Service\\GreetingService;\nclass HomeController { public function show(): GreetingService { return new GreetingService(); } }\n",
                ),
                (
                    "src/Service/GreetingService.php",
                    "<?php\nnamespace App\\Service;\nclass GreetingService { public function hi(): string { return 'hi'; } }\n",
                ),
                (
                    "src/Service/OrphanService.php",
                    "<?php\nnamespace App\\Service;\nclass OrphanService { public function unused(): int { return 0; } }\n",
                ),
                (
                    "var/cache/dev/Container.php",
                    "<?php\nfunction cached(): int { return 1; }\n",
                ),
            ],
        },
        FrameworkCase {
            profile: "cakephp",
            generated: 1,
            orphan: "src/Service/OrphanService.php",
            files: &[
                (
                    "composer.json",
                    r#"{"name":"acme/cake-app","require":{"cakephp/cakephp":"^5.1"}}"#,
                ),
                ("bin/cake.php", "<?php\nfunction cli(): int { return 0; }\n"),
                (
                    "src/Application.php",
                    "<?php\nnamespace App;\nclass Application { public function bootstrap(): void {} }\n",
                ),
                (
                    "config/routes.php",
                    "<?php\nfunction cakeRoutes(): void {}\n",
                ),
                (
                    "src/Controller/ArticlesController.php",
                    "<?php\nnamespace App\\Controller;\nuse App\\Service\\Slugger;\nclass ArticlesController { public function view(): Slugger { return new Slugger(); } }\n",
                ),
                (
                    "src/Service/Slugger.php",
                    "<?php\nnamespace App\\Service;\nclass Slugger { public function slug(): string { return 'a'; } }\n",
                ),
                (
                    "src/Service/OrphanService.php",
                    "<?php\nnamespace App\\Service;\nclass OrphanService { public function dead(): int { return 0; } }\n",
                ),
                (
                    "tmp/cache/models.php",
                    "<?php\nfunction cachedModels(): array { return []; }\n",
                ),
            ],
        },
        FrameworkCase {
            profile: "codeigniter",
            generated: 1,
            orphan: "app/Models/OrphanModel.php",
            files: &[
                (
                    "composer.json",
                    r#"{"name":"acme/ci-app","require":{"codeigniter4/framework":"^4.5"}}"#,
                ),
                ("spark", "#!/usr/bin/env php\n"),
                (
                    "public/index.php",
                    "<?php\nfunction start(): int { return 1; }\n",
                ),
                (
                    "app/Config/App.php",
                    "<?php\nnamespace Config;\nclass App { public string $baseUrl = 'http://localhost'; }\n",
                ),
                (
                    "app/Controllers/Home.php",
                    "<?php\nnamespace App\\Controllers;\nuse Config\\App;\nuse App\\Models\\UserModel;\nclass Home { public function index(): UserModel { $c = new App(); return new UserModel($c); } }\n",
                ),
                (
                    "app/Models/UserModel.php",
                    "<?php\nnamespace App\\Models;\nclass UserModel { public function __construct(public object $cfg) {} }\n",
                ),
                (
                    "app/Models/OrphanModel.php",
                    "<?php\nnamespace App\\Models;\nclass OrphanModel { public function dead(): int { return 0; } }\n",
                ),
                (
                    "writable/cache/page.php",
                    "<?php\nfunction cachedPage(): string { return 'ok'; }\n",
                ),
            ],
        },
    ];
    for case in cases {
        let fixture = PhpFixture::create(case.files);
        assert_single_framework_report(
            &fixture.check_json(),
            case.profile,
            case.generated,
            case.orphan,
        );
    }
}

#[test]
fn php_monorepo_isolation_standalone_illuminate_and_config_overrides() {
    let fixture = PhpFixture::create(&[(
        "composer.json",
        r#"{"name":"acme/monorepo","repositories":[{"type":"path","url":"packages/*"}]}"#,
    )]);
    fixture.write_file(
        "packages/laravel-app/composer.json",
        r#"{"name":"acme/laravel-app","require":{"laravel/framework":"^12.0","symfony/console":"^7.2"}}"#,
    );
    fixture.write_file("packages/laravel-app/artisan", "#!/usr/bin/env php\n");
    fixture.write_file(
        "packages/laravel-app/routes/web.php",
        "<?php\nfunction web(): int { return 1; }\n",
    );
    fixture.write_file(
        "packages/symfony-app/composer.json",
        r#"{"name":"acme/symfony-app","require":{"symfony/framework-bundle":"^7.2"}}"#,
    );
    fixture.write_file(
        "packages/symfony-app/config/bundles.php",
        "<?php\nfunction bundles(): array { return []; }\n",
    );
    fixture.write_file(
        "packages/symfony-app/src/Kernel.php",
        "<?php\nnamespace App;\nclass Kernel { public function boot(): void {} }\n",
    );
    fixture.write_file(
        "packages/illuminate-lib/composer.json",
        r#"{"name":"acme/illuminate-lib","require":{"illuminate/support":"^12.0","symfony/console":"^7.2"}}"#,
    );
    fixture.write_file(
        "packages/illuminate-lib/src/CollectionHelper.php",
        "<?php\nnamespace Acme\\Lib;\nclass CollectionHelper { public function count(): int { return 1; } }\n",
    );

    let initial = fixture.check_json();
    assert_eq!(initial["framework_profiles"], json!(["laravel", "symfony"]));
    assert_pkg_profiles(&initial, "packages/laravel-app", Some(&["laravel"]));
    assert_pkg_profiles(&initial, "packages/symfony-app", Some(&["symfony"]));
    assert_pkg_profiles(&initial, "packages/illuminate-lib", None);

    fixture.write_config(
        r#"{ "framework_profiles": { "exclude": ["symfony"] }, "packages": { "packages/laravel-app": { "framework_profiles": false }, "packages/symfony-app": { "framework_profiles": ["symfony"] } } }"#,
    );
    let overridden = fixture.check_json();
    assert_eq!(overridden["framework_profiles"], json!(["symfony"]));
    assert_pkg_profiles(&overridden, "packages/laravel-app", None);
    assert_pkg_profiles(&overridden, "packages/symfony-app", Some(&["symfony"]));

    fixture.write_config(r#"{ "framework_profiles": ["laravell"] }"#);
    let bad = fixture.run(&["check"]);
    assert_eq!(bad.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&bad.stderr);
    assert!(
        stderr.contains("laravell") && stderr.contains("laravel"),
        "{stderr}"
    );
}
