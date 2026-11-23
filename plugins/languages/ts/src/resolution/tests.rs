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
}
