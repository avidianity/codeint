use codeint_model::language::{ImportRequest, Resolution};

use super::{ProjectResolver, package_name};

fn resolve_specifier(specifier: &str) -> Resolution {
    let paths = vec!["src/main.ts".to_owned()];
    let resolver = ProjectResolver::new(&paths, Vec::new());
    resolver.resolve(&ImportRequest {
        importer: "src/main.ts".to_owned(),
        specifier: specifier.to_owned(),
    })
}

#[test]
fn package_name_strips_query_and_fragment_suffixes() {
    assert_eq!(package_name("lodash?raw"), "lodash");
    assert_eq!(package_name("@scope/pkg?v=1"), "@scope/pkg");
    assert_eq!(package_name("@scope/pkg/sub?v=1#top"), "@scope/pkg");
    assert_eq!(package_name("react#hash"), "react");
    assert_eq!(package_name("?raw"), "");
    assert_eq!(package_name(""), "");
}

#[test]
fn external_resolutions_strip_query_and_fragment_suffixes() {
    assert_eq!(
        resolve_specifier("lodash?raw"),
        Resolution::External("lodash".to_owned())
    );
    assert_eq!(
        resolve_specifier("@scope/pkg?v=1"),
        Resolution::External("@scope/pkg".to_owned())
    );
    assert_eq!(
        resolve_specifier("react#hash"),
        Resolution::External("react".to_owned())
    );
}

#[test]
fn empty_and_query_only_specifiers_are_unresolved() {
    assert!(
        matches!(resolve_specifier(""), Resolution::Unresolved(_)),
        "empty specifier must be unresolved"
    );
    assert!(
        matches!(resolve_specifier("?raw"), Resolution::Unresolved(_)),
        "query-only specifier must be unresolved"
    );
    assert!(
        matches!(resolve_specifier("npm:"), Resolution::Unresolved(_)),
        "empty npm: specifier must be unresolved"
    );
}

#[test]
fn deno_npm_and_jsr_specifiers_strip_prefix_and_version_before_normalizing() {
    assert_eq!(package_name("npm:@medley/router"), "@medley/router");
    assert_eq!(
        package_name("npm:@medley/router@1.2.3/sub"),
        "@medley/router"
    );
    assert_eq!(package_name("npm:chalk@^5/foo"), "chalk");
    assert_eq!(package_name("npm:chalk"), "chalk");
    assert_eq!(package_name("jsr:@std/path@1.0.0/join"), "@std/path");
    assert_eq!(package_name("npm:"), "");
    assert_eq!(
        resolve_specifier("npm:@medley/router"),
        Resolution::External("@medley/router".to_owned())
    );
    assert_eq!(
        resolve_specifier("npm:@medley/router@1.2.3/sub"),
        Resolution::External("@medley/router".to_owned())
    );
    assert_eq!(
        resolve_specifier("npm:chalk@^5/foo"),
        Resolution::External("chalk".to_owned())
    );
    assert_eq!(
        resolve_specifier("jsr:@std/path@1.0.0/join"),
        Resolution::External("@std/path".to_owned())
    );
}

#[test]
fn cloudflare_and_deno_protocol_specifiers_resolve_as_builtins() {
    for specifier in [
        "cloudflare:workers",
        "cloudflare:sockets",
        "deno:kv",
        "deno:std",
    ] {
        assert_eq!(
            resolve_specifier(specifier),
            Resolution::Builtin,
            "{specifier}"
        );
    }
}
