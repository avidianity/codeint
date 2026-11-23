use codeint_model::language::{ImportRequest, Manifest, Resolution};

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

#[test]
fn malformed_registry_specifiers_are_unresolved() {
    for specifier in [
        "npm:@",
        "jsr:@",
        "npm:@scope",
        "npm:@scope/",
        "npm:@/",
        "npm:@/pkg",
        "npm:./local",
        "npm:../local",
        "npm:.",
        "npm:..",
        "npm:/abs",
        "jsr:@scope",
        "jsr:@/pkg",
    ] {
        assert_eq!(package_name(specifier), "", "{specifier}");
        assert!(
            matches!(resolve_specifier(specifier), Resolution::Unresolved(_)),
            "{specifier} must be unresolved"
        );
    }
}

#[test]
fn framework_virtual_modules_resolve_as_builtins() {
    for specifier in [
        "virtual:pwa-register",
        "virtual:uno.css?inline",
        "$app",
        "$app/stores",
        "$app/navigation",
        "$env/static/private",
        "$env/dynamic/public",
        "$service-worker",
        "$service-worker?url",
    ] {
        assert_eq!(
            resolve_specifier(specifier),
            Resolution::Builtin,
            "{specifier}"
        );
    }
    for non_virtual in ["$apple", "$env", "$service-worker-extra"] {
        assert_eq!(
            resolve_specifier(non_virtual),
            Resolution::External(non_virtual.to_owned()),
            "{non_virtual}"
        );
    }
}

fn manifest(path: &str, text: &str) -> Manifest {
    Manifest {
        path: path.to_owned(),
        text: text.to_owned(),
    }
}

#[test]
fn workspace_members_inherit_hoisted_root_dependencies_without_hiding_member_or_unlisted_deps() {
    let paths: Vec<String> = [
        "package.json",
        "scripts/release.ts",
        "packages/app/package.json",
        "packages/app/src/index.ts",
        "packages/app/src/index.test.ts",
        "packages/retired/package.json",
        "packages/retired/src/index.ts",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let manifests = vec![
        manifest(
            "package.json",
            r#"{"name":"mono","private":true,"workspaces":["packages/*","!packages/retired"],"devDependencies":{"vitest":"^2.0.0","vite":"^6.0.0","zod":"^3.0.0"}}"#,
        ),
        manifest(
            "packages/app/package.json",
            r#"{"name":"@mono/app","dependencies":{"zod":"^3.0.0"}}"#,
        ),
        manifest(
            "packages/retired/package.json",
            r#"{"name":"@mono/retired","dependencies":{}}"#,
        ),
    ];
    let resolver = ProjectResolver::new(&paths, manifests);
    let resolve = |importer: &str, specifier: &str| {
        resolver.resolve(&ImportRequest {
            importer: importer.to_owned(),
            specifier: specifier.to_owned(),
        })
    };

    let ext = |name: &str| Resolution::External(name.to_owned());
    let cases = [
        (
            "packages/app/src/index.test.ts",
            "vitest",
            Resolution::Builtin,
        ),
        (
            "packages/app/src/index.ts",
            "vite/client",
            Resolution::Builtin,
        ),
        ("packages/app/src/index.ts", "zod", ext("zod")),
        ("packages/app/src/index.ts", "axios", ext("axios")),
        ("scripts/release.ts", "vitest", ext("vitest")),
        ("packages/retired/src/index.ts", "vitest", ext("vitest")),
    ];
    for (importer, specifier, expected) in cases {
        assert_eq!(
            resolve(importer, specifier),
            expected,
            "{importer} -> {specifier}"
        );
    }
}
