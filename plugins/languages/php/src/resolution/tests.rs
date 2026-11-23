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
    ] {
        assert!(is_builtin_symbol(sym), "expected PHP builtin {sym}");
    }
    for ext in [
        "Carbon",
        "Illuminate\\Support\\Collection",
        "Monolog\\Logger",
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
