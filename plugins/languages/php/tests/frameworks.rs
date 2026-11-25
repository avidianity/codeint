use std::fs;
use std::path::Path;

use codeint_lang_php::Php;
use codeint_model::language::{FrameworkProfileResult, ImportRequest, Manifest, Resolution};
use codeint_model::{FrameworkProfileSelection, LanguagePlugin, RepoPath};

fn build_inputs(
    raw_files: &[&str],
    raw_manifests: &[(&str, &str)],
) -> (Vec<RepoPath>, Vec<Manifest>) {
    let repo_files = raw_files
        .iter()
        .map(|entry| RepoPath::new(*entry).expect("valid repo path"))
        .collect();
    let manifest_list = raw_manifests
        .iter()
        .map(|&(loc, body)| Manifest {
            path: loc.to_owned(),
            text: body.to_owned(),
        })
        .collect();
    (repo_files, manifest_list)
}

fn resolve_with(
    package_root: &str,
    raw_files: &[&str],
    raw_manifests: &[(&str, &str)],
    selection: FrameworkProfileSelection,
) -> Result<FrameworkProfileResult, String> {
    let (repo_files, manifest_list) = build_inputs(raw_files, raw_manifests);
    Php.resolve_framework_profiles(package_root, &repo_files, manifest_list, selection)
        .map_err(|error| error.to_string())
}

fn resolve_auto(
    package_root: &str,
    raw_files: &[&str],
    raw_manifests: &[(&str, &str)],
) -> FrameworkProfileResult {
    resolve_with(
        package_root,
        raw_files,
        raw_manifests,
        FrameworkProfileSelection::Auto,
    )
    .expect("resolve framework profiles")
}

fn resolve_single_import(
    raw_files: &[&str],
    raw_manifests: &[(&str, &str)],
    importer: &str,
    specifier: &str,
    active_profiles: &[&str],
) -> Resolution {
    let (repo_files, manifest_list) = build_inputs(raw_files, raw_manifests);
    let request = vec![ImportRequest {
        importer: importer.to_owned(),
        specifier: specifier.to_owned(),
    }];
    let profiles: Vec<String> = active_profiles.iter().map(|&n| n.to_owned()).collect();
    Php.resolve_imports(&repo_files, manifest_list, request, &profiles)
        .expect("resolve import")
        .into_iter()
        .next()
        .expect("single resolution")
}

fn assert_profile_conventions(
    resolved: &FrameworkProfileResult,
    expected_profile: &str,
    expected_entries: &[&str],
    excluded_entries: &[&str],
    expected_generated: &[&str],
) {
    assert_eq!(resolved.active_profiles, vec![expected_profile]);
    assert!(resolved.reference_ranges.is_empty());
    assert_eq!(resolved.reference_range_note, None);
    for &entry in expected_entries {
        assert!(
            resolved.entry_points.iter().any(|path| path == entry),
            "missing {expected_profile} entry point {entry} in {:?}",
            resolved.entry_points
        );
    }
    for &excluded in excluded_entries {
        assert!(
            !resolved.entry_points.iter().any(|path| path == excluded),
            "unexpected {expected_profile} entry point {excluded}"
        );
    }
    assert_eq!(resolved.generated_paths, expected_generated);
}

fn assert_fallbacks(
    raw_files: &[&str],
    raw_manifests: &[(&str, &str)],
    profile: &str,
    cases: &[(&str, &str, &str, &str)],
) {
    for &(importer, specifier, expected_resolved, expected_external) in cases {
        assert_eq!(
            resolve_single_import(raw_files, raw_manifests, importer, specifier, &[profile]),
            Resolution::Resolved(vec![expected_resolved.to_owned()])
        );
        assert_eq!(
            resolve_single_import(raw_files, raw_manifests, importer, specifier, &[]),
            Resolution::External(expected_external.to_owned())
        );
    }
}

#[test]
fn known_framework_profiles_lists_laravel_symfony_cakephp_and_codeigniter() {
    assert_eq!(
        Php.known_framework_profiles(),
        vec!["laravel", "symfony", "cakephp", "codeigniter"]
    );
}

#[test]
fn laravel_profile_detects_conventions_generated_paths_and_fallback_import_hints() {
    let files = [
        "composer.json",
        "artisan",
        "public/index.php",
        "bootstrap/app.php",
        "bootstrap/providers.php",
        "config/app.php",
        "routes/web.php",
        "app/Console/Commands/SyncCommand.php",
        "app/Listeners/SendMail.php",
        "app/Policies/PostPolicy.php",
        "app/Models/Policies/UserPolicy.php",
        "app/Providers/AppServiceProvider.php",
        "app/Http/Controllers/UserController.php",
        "app/Services/Orphan.php",
        "database/migrations/2026_01_01_000000_create_users.php",
        "database/seeders/DatabaseSeeder.php",
        "database/factories/UserFactory.php",
        "bootstrap/cache/packages.php",
        "storage/framework/views/compiled.php",
        "public/build/app.js",
        "public/hot",
    ];
    let composer = [(
        "composer.json",
        r#"{"require":{"laravel/framework":"^12.0"}}"#,
    )];
    let resolved = resolve_auto("", &files, &composer);

    assert_profile_conventions(
        &resolved,
        "laravel",
        &[
            "artisan",
            "public/index.php",
            "bootstrap/app.php",
            "bootstrap/providers.php",
            "config/app.php",
            "routes/web.php",
            "app/Console/Commands/SyncCommand.php",
            "app/Listeners/SendMail.php",
            "app/Policies/PostPolicy.php",
            "app/Models/Policies/UserPolicy.php",
            "app/Providers/AppServiceProvider.php",
            "database/migrations/2026_01_01_000000_create_users.php",
            "database/seeders/DatabaseSeeder.php",
            "database/factories/UserFactory.php",
        ],
        &[
            "app/Http/Controllers/UserController.php",
            "app/Services/Orphan.php",
        ],
        &[
            "bootstrap/cache/packages.php",
            "public/build/app.js",
            "public/hot",
            "storage/framework/views/compiled.php",
        ],
    );

    assert_fallbacks(
        &files,
        &composer,
        "laravel",
        &[
            (
                "routes/web.php",
                "use:App\\Http\\Controllers\\UserController",
                "app/Http/Controllers/UserController.php",
                "app/http",
            ),
            (
                "database/seeders/DatabaseSeeder.php",
                "use:Database\\Factories\\UserFactory",
                "database/factories/UserFactory.php",
                "database/factories",
            ),
            (
                "routes/web.php",
                "use:Database\\Seeders\\DatabaseSeeder",
                "database/seeders/DatabaseSeeder.php",
                "database/seeders",
            ),
        ],
    );

    let custom_psr4 = [(
        "composer.json",
        r#"{"require":{"laravel/framework":"^12.0"},"autoload":{"psr-4":{"App\\":"custom/"}}}"#,
    )];
    let custom_files = [
        "composer.json",
        "routes/web.php",
        "custom/Services/Billing.php",
        "app/Services/Billing.php",
    ];
    assert_eq!(
        resolve_single_import(
            &custom_files,
            &custom_psr4,
            "routes/web.php",
            "use:App\\Services\\Billing",
            &["laravel"]
        ),
        Resolution::Resolved(vec!["custom/Services/Billing.php".to_owned()])
    );
}

#[test]
fn laravel_versus_symfony_isolation_and_standalone_illuminate_exclusion() {
    let laravel_with_symfony_components = resolve_auto(
        "",
        &[
            "composer.json",
            "composer.lock",
            "artisan",
            "bootstrap/app.php",
            "public/index.php",
            "bin/console",
            "routes/web.php",
        ],
        &[(
            "composer.json",
            r#"{"require":{"laravel/framework":"^12.0","symfony/console":"^7.2","symfony/http-foundation":"^7.2","symfony/http-kernel":"^7.2","symfony/routing":"^7.2","symfony/mailer":"^7.2","symfony/finder":"^7.2"}}"#,
        )],
    );
    assert_eq!(
        laravel_with_symfony_components.active_profiles,
        vec!["laravel"]
    );

    for (files, composer_text) in [
        (
            [
                "composer.json",
                "artisan",
                "bootstrap/app.php",
                "routes/web.php",
            ]
            .as_slice(),
            r#"{"require":{"illuminate/support":"^12.0","illuminate/database":"^12.0","symfony/console":"^7.2"}}"#,
        ),
        (
            [
                "composer.json",
                "artisan",
                "bootstrap/app.php",
                "src/Package.php",
            ]
            .as_slice(),
            r#"{"require-dev":{"laravel/framework":"^12.0"}}"#,
        ),
        (
            [
                "composer.json",
                "bin/console",
                "public/index.php",
                "config/bundles.php",
                "src/Kernel.php",
            ]
            .as_slice(),
            r#"{"require":{"symfony/console":"^7.2","symfony/http-foundation":"^7.2","symfony/routing":"^7.2"}}"#,
        ),
        (
            ["composer.json", "config/bundles.php", "src/Kernel.php"].as_slice(),
            r#"{"type":"symfony-bundle","require":{"symfony/framework-bundle":"^7.2"}}"#,
        ),
    ] {
        let resolved = resolve_auto("", files, &[("composer.json", composer_text)]);
        assert!(
            resolved.active_profiles.is_empty(),
            "expected no active profile for {composer_text}, got {:?}",
            resolved.active_profiles
        );
    }
}

#[test]
fn symfony_profile_detects_conventions_generated_paths_and_fallback_import_hints() {
    let files = [
        "composer.json",
        "public/index.php",
        "bin/console",
        "src/Kernel.php",
        "config/bundles.php",
        "config/preload.php",
        "config/routes.php",
        "config/services.php",
        "src/Controller/HomeController.php",
        "src/Command/ImportCommand.php",
        "src/EventListener/AuditListener.php",
        "src/EventSubscriber/AuthSubscriber.php",
        "src/Twig/AppExtension.php",
        "src/Security/AppAuthenticator.php",
        "src/Form/UserType.php",
        "src/DataFixtures/AppFixtures.php",
        "src/Service/Helper.php",
        "migrations/Version20260101.php",
        "var/cache/dev/Container.php",
        "var/log/dev.log",
        "public/bundles/app/app.js",
        "public/build/app.js",
    ];
    let composer = [(
        "composer.json",
        r#"{"require":{"symfony/framework-bundle":"^7.2"}}"#,
    )];
    let resolved = resolve_auto("", &files, &composer);

    assert_profile_conventions(
        &resolved,
        "symfony",
        &[
            "public/index.php",
            "bin/console",
            "src/Kernel.php",
            "config/bundles.php",
            "config/preload.php",
            "config/routes.php",
            "config/services.php",
            "src/Controller/HomeController.php",
            "src/Command/ImportCommand.php",
            "src/EventListener/AuditListener.php",
            "src/EventSubscriber/AuthSubscriber.php",
            "src/Twig/AppExtension.php",
            "src/Security/AppAuthenticator.php",
            "src/Form/UserType.php",
            "src/DataFixtures/AppFixtures.php",
            "migrations/Version20260101.php",
        ],
        &["src/Service/Helper.php"],
        &[
            "public/build/app.js",
            "public/bundles/app/app.js",
            "var/cache/dev/Container.php",
            "var/log/dev.log",
        ],
    );

    assert_fallbacks(
        &files,
        &composer,
        "symfony",
        &[
            (
                "src/Controller/HomeController.php",
                "use:App\\Service\\Helper",
                "src/Service/Helper.php",
                "app/service",
            ),
            (
                "src/Kernel.php",
                "use:DoctrineMigrations\\Version20260101",
                "migrations/Version20260101.php",
                "doctrinemigrations/version20260101",
            ),
        ],
    );

    let flex_skeleton = resolve_auto(
        "",
        &["composer.json", "symfony.lock", "src/Kernel.php"],
        &[(
            "composer.json",
            r#"{"flex-require":{"symfony/framework-bundle":"^8.0"}}"#,
        )],
    );
    assert_eq!(flex_skeleton.active_profiles, vec!["symfony"]);
}

#[test]
fn cakephp_profile_detects_conventions_generated_paths_excludes_plugins_and_components() {
    let files = [
        "composer.json",
        "webroot/index.php",
        "bin/cake",
        "bin/cake.php",
        "src/Application.php",
        "config/bootstrap.php",
        "config/bootstrap_cli.php",
        "config/routes.php",
        "config/paths.php",
        "config/app.php",
        "src/Controller/ArticlesController.php",
        "src/Controller/UnusedHelper.php",
        "src/Command/SyncCommand.php",
        "src/Model/Table/ArticlesTable.php",
        "src/Model/Entity/Article.php",
        "src/View/Helper/NavHelper.php",
        "src/View/Cell/InboxCell.php",
        "src/Mailer/UserMailer.php",
        "src/Service/Orphan.php",
        "templates/Articles/index.php",
        "config/Migrations/20260101_Initial.php",
        "config/Seeds/UsersSeed.php",
        "tmp/cache/models.php",
        "logs/error.log",
    ];
    let composer = [("composer.json", r#"{"require":{"cakephp/cakephp":"^5.1"}}"#)];
    let resolved = resolve_auto("", &files, &composer);

    assert_profile_conventions(
        &resolved,
        "cakephp",
        &[
            "webroot/index.php",
            "bin/cake",
            "bin/cake.php",
            "src/Application.php",
            "config/bootstrap.php",
            "config/bootstrap_cli.php",
            "config/routes.php",
            "config/paths.php",
            "config/app.php",
            "src/Controller/ArticlesController.php",
            "src/Command/SyncCommand.php",
            "src/Model/Table/ArticlesTable.php",
            "src/Model/Entity/Article.php",
            "src/View/Helper/NavHelper.php",
            "src/View/Cell/InboxCell.php",
            "src/Mailer/UserMailer.php",
            "templates/Articles/index.php",
            "config/Migrations/20260101_Initial.php",
            "config/Seeds/UsersSeed.php",
        ],
        &["src/Controller/UnusedHelper.php", "src/Service/Orphan.php"],
        &["logs/error.log", "tmp/cache/models.php"],
    );

    assert_fallbacks(
        &files,
        &composer,
        "cakephp",
        &[(
            "src/Controller/ArticlesController.php",
            "use:App\\Service\\Orphan",
            "src/Service/Orphan.php",
            "app/service",
        )],
    );

    for non_app_composer in [
        r#"{"type":"cakephp-plugin","require":{"cakephp/cakephp":"^5.1"}}"#,
        r#"{"require":{"cakephp/orm":"^5.1"}}"#,
    ] {
        let non_app = resolve_auto(
            "",
            &["composer.json", "src/Application.php", "bin/cake.php"],
            &[("composer.json", non_app_composer)],
        );
        assert!(non_app.active_profiles.is_empty());
    }
}

#[test]
fn codeigniter_profile_detects_composer_and_manual_installs_conventions_and_fallback_hints() {
    let files = [
        "composer.json",
        "spark",
        "public/index.php",
        "app/Config/App.php",
        "app/Config/Routes.php",
        "app/Config/Boot/production.php",
        "app/Controllers/Home.php",
        "app/Commands/Sync.php",
        "app/Filters/AuthFilter.php",
        "app/Views/welcome.php",
        "app/Database/Migrations/2026_01_01_Init.php",
        "app/Database/Seeds/MainSeeder.php",
        "app/Helpers/url_helper.php",
        "app/Models/OrphanModel.php",
        "writable/cache/page.php",
    ];
    let composer = [(
        "composer.json",
        r#"{"require":{"codeigniter4/framework":"^4.5"}}"#,
    )];
    let resolved = resolve_auto("", &files, &composer);

    assert_profile_conventions(
        &resolved,
        "codeigniter",
        &[
            "spark",
            "public/index.php",
            "app/Config/App.php",
            "app/Config/Routes.php",
            "app/Config/Boot/production.php",
            "app/Controllers/Home.php",
            "app/Commands/Sync.php",
            "app/Filters/AuthFilter.php",
            "app/Views/welcome.php",
            "app/Database/Migrations/2026_01_01_Init.php",
            "app/Database/Seeds/MainSeeder.php",
            "app/Helpers/url_helper.php",
        ],
        &["app/Models/OrphanModel.php"],
        &["writable/cache/page.php"],
    );

    assert_fallbacks(
        &files,
        &composer,
        "codeigniter",
        &[
            (
                "app/Controllers/Home.php",
                "use:App\\Models\\OrphanModel",
                "app/Models/OrphanModel.php",
                "app/models",
            ),
            (
                "app/Controllers/Home.php",
                "use:Config\\App",
                "app/Config/App.php",
                "config/app",
            ),
        ],
    );

    let manual = resolve_auto(
        "",
        &[
            "spark",
            "app/Config/Paths.php",
            "system/Boot.php",
            "app/Controllers/Home.php",
        ],
        &[],
    );
    assert_eq!(manual.active_profiles, vec!["codeigniter"]);

    let ci3 = resolve_auto(
        "",
        &["index.php", "application/controllers/Welcome.php"],
        &[],
    );
    assert!(ci3.active_profiles.is_empty());
}

#[test]
fn monorepo_isolation_vendor_exclusion_and_selection_modes() {
    let child = resolve_auto(
        "packages/shared",
        &[
            "packages/shared/composer.json",
            "packages/shared/src/Util.php",
        ],
        &[
            (
                "composer.json",
                r#"{"require":{"laravel/framework":"^12.0"},"repositories":[{"type":"path","url":"packages/*"}]}"#,
            ),
            ("packages/shared/composer.json", r#"{"name":"acme/shared"}"#),
        ],
    );
    assert!(child.active_profiles.is_empty());

    let vendored = resolve_auto(
        "",
        &[
            "vendor/acme/app/composer.json",
            "artisan",
            "bootstrap/app.php",
            "app/Services/Tool.php",
        ],
        &[(
            "vendor/acme/app/composer.json",
            r#"{"require":{"laravel/framework":"^12.0"}}"#,
        )],
    );
    assert!(vendored.active_profiles.is_empty());

    let invalid = resolve_auto(
        "",
        &["composer.json", "artisan", "bootstrap/app.php"],
        &[("composer.json", "{ not valid json")],
    );
    assert!(invalid.active_profiles.is_empty());

    let laravel_files = [
        "composer.json",
        "artisan",
        "bootstrap/app.php",
        "routes/web.php",
    ];
    let laravel_manifest = [(
        "composer.json",
        r#"{"require":{"laravel/framework":"^12.0"}}"#,
    )];
    for selection in [
        FrameworkProfileSelection::Disabled,
        FrameworkProfileSelection::Exclude(vec!["laravel".to_owned()]),
    ] {
        let suppressed = resolve_with("", &laravel_files, &laravel_manifest, selection)
            .expect("suppressed selection");
        assert!(suppressed.active_profiles.is_empty());
        assert!(suppressed.entry_points.is_empty());
    }

    let forced = resolve_with(
        "",
        &["routes/web.php"],
        &[],
        FrameworkProfileSelection::Only(vec!["laravel".to_owned()]),
    )
    .expect("forced laravel");
    assert_eq!(forced.active_profiles, vec!["laravel"]);
    assert_eq!(forced.entry_points, vec!["routes/web.php"]);

    assert!(
        resolve_with(
            "",
            &laravel_files,
            &laravel_manifest,
            FrameworkProfileSelection::Only(vec!["laravell".to_owned()]),
        )
        .is_err()
    );
}

#[test]
fn sibling_framework_modules_do_not_import_each_other() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/frameworks");
    let mut modules: Vec<(String, String)> = fs::read_dir(&dir)
        .expect("list PHP framework modules")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let stem = path.file_stem()?.to_str()?.to_owned();
            if stem == "mod" || path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                return None;
            }
            Some((stem, fs::read_to_string(&path).expect("read module source")))
        })
        .collect();
    modules.sort_by(|left, right| left.0.cmp(&right.0));
    let names: Vec<&str> = modules.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, vec!["cakephp", "codeigniter", "laravel", "symfony"]);
    for (current, contents) in &modules {
        for peer in names.iter().filter(|peer| **peer != current.as_str()) {
            assert!(
                !contents.contains(&format!("super::{peer}"))
                    && !contents.contains(&format!("frameworks::{peer}")),
                "PHP framework module {current} must stay isolated from {peer}"
            );
        }
    }
}
