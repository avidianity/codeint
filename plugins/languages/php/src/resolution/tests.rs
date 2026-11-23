use std::collections::HashSet;

use codeint_model::language::{ImportRequest, Manifest, Resolution};

use super::{normalize_external_namespace, resolve_batch};
use crate::builtins::{builtin_tables, is_builtin_symbol};

#[test]
fn php_builtin_tables_are_strictly_sorted_and_case_insensitive() {
    let (namespaces, globals) = builtin_tables();
    for pair in namespaces.windows(2) {
        assert!(pair[0] < pair[1], "unsorted PHP namespaces: {pair:?}");
    }
    for pair in globals.windows(2) {
        assert!(pair[0] < pair[1], "unsorted PHP globals: {pair:?}");
    }
    for sym in [
        "DateTime",
        "datetimeimmutable",
        "PDO",
        "Exception",
        "SplFileInfo",
        "Random\\Randomizer",
        "Dom\\HTMLDocument",
        "Pdo\\Mysql",
        "Openssl\\Session",
        "BcMath\\Number",
        "Uri\\Rfc3986\\Uri",
        "Intl\\Locale",
        "Sqlite\\Connection",
        "Shmop\\Shmop",
        "XML\\Parser",
        "Composer\\InstalledVersions",
        "Composer\\Autoload\\ClassLoader",
    ] {
        assert!(is_builtin_symbol(sym), "expected PHP builtin {sym}");
    }
    for ext in [
        "Carbon",
        "Illuminate\\Support\\Collection",
        "Monolog\\Logger",
        "Composer\\Semver\\VersionParser",
        "Composer\\CaBundle\\CaBundle",
    ] {
        assert!(!is_builtin_symbol(ext), "unexpected PHP builtin {ext}");
    }
}

#[test]
fn normalizes_composer_namespaces_and_vendor_requires() {
    let empty = HashSet::new();
    assert_eq!(
        normalize_external_namespace("Doctrine\\ORM\\EntityManager", &empty),
        "doctrine/orm"
    );
    assert_eq!(
        normalize_external_namespace("Acme\\HTTPClient\\Pool", &empty),
        "acme/http-client"
    );
    assert_eq!(
        normalize_external_namespace("GuzzleHttp\\Psr7\\Request", &empty),
        "guzzlehttp/psr7"
    );

    let files = vec!["src/index.php".into()];
    let manifests = vec![Manifest {
        path: "composer.json".into(),
        text: r#"{"require":{"phpunit/phpunit":"^11.0"}}"#.into(),
    }];
    let reqs = [
        "req:../vendor/autoload.php",
        "req:../vendor/monolog/monolog/src/Monolog/Logger.php",
        "use:PHPUnit\\Framework\\TestCase",
        "use:PDO",
    ]
    .into_iter()
    .map(|spec| ImportRequest {
        importer: "src/index.php".into(),
        specifier: spec.into(),
    })
    .collect::<Vec<_>>();
    assert_eq!(
        resolve_batch(&files, &manifests, &reqs),
        vec![
            Resolution::Builtin,
            Resolution::External("monolog/monolog".into()),
            Resolution::External("phpunit/phpunit".into()),
            Resolution::Builtin,
        ]
    );
}

#[test]
fn psr4_collision_resolves_declared_composer_subpackages_and_globals_are_never_external() {
    let files = vec![
        "composer.json".into(),
        "src/Client.php".into(),
        "src/Handler/CurlHandler.php".into(),
    ];
    let manifests = vec![Manifest {
        path: "composer.json".into(),
        text: r#"{
            "name": "guzzlehttp/guzzle",
            "autoload": { "psr-4": { "GuzzleHttp\\": "src/" } },
            "require": {
                "guzzlehttp/psr7": "^2.7",
                "guzzlehttp/promises": "^2.0"
            }
        }"#
        .into(),
    }];
    let check = |spec: &str| {
        resolve_batch(
            &files,
            &manifests,
            &[ImportRequest {
                importer: "src/Client.php".into(),
                specifier: spec.into(),
            }],
        )
        .remove(0)
    };
    assert_eq!(check("use:GuzzleHttp\\Client"), Resolution::Builtin);
    assert!(matches!(
        check("use:GuzzleHttp\\Handler"),
        Resolution::Unresolved(_)
    ));
    assert_eq!(
        check("use:GuzzleHttp\\Psr7\\Request"),
        Resolution::External("guzzlehttp/psr7".into())
    );
    assert_eq!(
        check("use:GuzzleHttp\\Psr7"),
        Resolution::External("guzzlehttp/psr7".into())
    );
    assert_eq!(
        check("use:GuzzleHttp\\Promise\\Create"),
        Resolution::External("guzzlehttp/promises".into())
    );
    assert!(matches!(
        check("use:GuzzleHttp\\MissingInternal"),
        Resolution::Unresolved(_)
    ));
    assert_eq!(check("use:CarbonTimezoneTrait"), Resolution::Builtin);
    assert_eq!(check("use:SubCarbon"), Resolution::Builtin);
}

#[test]
fn multi_segment_declared_matching_handles_psr_symfony_and_unique_package_names() {
    let declared: HashSet<String> = [
        "psr/http-message",
        "psr/http-client",
        "psr/http-factory",
        "psr/http-server-handler",
        "psr/log",
        "symfony/translation",
        "symfony/translation-contracts",
        "symfony/clock",
        "symfony/twig-bridge",
        "symfony/ux-turbo",
        "symfony/security-core",
        "friendsofphp/php-cs-fixer",
        "league/flysystem",
        "league/flysystem-aws-s3-v3",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();

    let cases = [
        ("Psr\\Http\\Message\\RequestInterface", "psr/http-message"),
        ("Psr\\Http\\Client\\ClientInterface", "psr/http-client"),
        (
            "Psr\\Http\\Factory\\StreamFactoryInterface",
            "psr/http-factory",
        ),
        (
            "Psr\\Http\\Message\\StreamFactoryInterface",
            "psr/http-factory",
        ),
        (
            "Psr\\Http\\Server\\RequestHandlerInterface",
            "psr/http-server-handler",
        ),
        ("Psr\\Log\\LoggerInterface", "psr/log"),
        (
            "Symfony\\Component\\Translation\\Translator",
            "symfony/translation",
        ),
        ("Symfony\\Component\\Clock\\NativeClock", "symfony/clock"),
        (
            "Symfony\\Contracts\\Translation\\TranslatorInterface",
            "symfony/translation-contracts",
        ),
        (
            "Symfony\\Bridge\\Twig\\Extension\\AssetExtension",
            "symfony/twig-bridge",
        ),
        ("Symfony\\UX\\Turbo\\TurboBundle", "symfony/ux-turbo"),
        (
            "Symfony\\Component\\Security\\Core\\User\\UserInterface",
            "symfony/security-core",
        ),
        ("PhpCsFixer\\Config", "friendsofphp/php-cs-fixer"),
        (
            "League\\Flysystem\\AwsS3V3\\AwsS3V3Adapter",
            "league/flysystem-aws-s3-v3",
        ),
        ("League\\Flysystem\\Filesystem", "league/flysystem"),
        ("Vendor\\Unlisted\\Thing", "vendor/unlisted"),
    ];
    for (ns, expected) in cases {
        assert_eq!(
            normalize_external_namespace(ns, &declared),
            expected,
            "namespace {ns}"
        );
    }
}

#[test]
fn ambiguous_declared_package_names_do_not_guess_vendor() {
    let ambiguous: HashSet<String> = ["friendsofphp/php-cs-fixer", "acme/php-cs-fixer"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        normalize_external_namespace("PhpCsFixer\\Config", &ambiguous),
        "phpcsfixer/config"
    );
}
