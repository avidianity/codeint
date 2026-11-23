use codeint_model::language::{DepKind, Manifest};

use super::{extract_declared, normalize_composer_name};

#[test]
fn composer_name_normalization_excludes_platform_requirements() {
    assert_eq!(
        normalize_composer_name("Illuminate/Support"),
        Some("illuminate/support".into())
    );
    for platform in [
        "php",
        "ext-json",
        "lib-openssl",
        "composer-plugin-api",
        "a/b/c",
    ] {
        assert_eq!(normalize_composer_name(platform), None);
    }
}

#[test]
fn composer_extraction_skips_invalid_json_and_vendor_manifests() {
    let root = Manifest {
        path: "composer.json".into(),
        text: r#"{"name":"acme/app","require":{"php":"^8.3","GuzzleHttp/Guzzle":"^7.0"},"suggest":{"ext-curl":"curl","Monolog/Monolog":"logs"}}"#.into(),
    };
    let broken = Manifest {
        path: "packages/broken/composer.json".into(),
        text: "{broken".into(),
    };
    let vendored = Manifest {
        path: "vendor/foo/bar/composer.json".into(),
        text: r#"{"require":{"ignored/dep":"^1.0"}}"#.into(),
    };
    let deps = extract_declared(&[root, broken, vendored]);
    assert_eq!(deps.len(), 2);
    assert_eq!(
        (deps[0].name.as_str(), deps[0].kind),
        ("guzzlehttp/guzzle", DepKind::Runtime)
    );
    assert_eq!(
        (deps[1].name.as_str(), deps[1].kind),
        ("monolog/monolog", DepKind::Optional)
    );
}
