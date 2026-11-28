use std::fs;
use std::path::Path;

use codeint_lang_ts::TypeScriptAndJavaScript;
use codeint_model::language::{FrameworkProfileResult, ImportRequest, Manifest, Resolution};
use codeint_model::{FrameworkProfileSelection, LanguagePlugin, RepoPath};

fn paths(raw: &[&str]) -> Vec<RepoPath> {
    raw.iter()
        .map(|path| RepoPath::new(*path).expect("valid repo path"))
        .collect()
}

fn resolve_auto(
    package_root: &str,
    raw_files: &[&str],
    raw_manifests: &[(&str, &str)],
) -> FrameworkProfileResult {
    let files = paths(raw_files);
    let manifests = raw_manifests
        .iter()
        .map(|(path, text)| Manifest {
            path: (*path).to_owned(),
            text: (*text).to_owned(),
        })
        .collect();
    TypeScriptAndJavaScript
        .resolve_framework_profiles(
            package_root,
            &files,
            manifests,
            FrameworkProfileSelection::Auto,
        )
        .expect("resolve framework profiles")
}

fn resolve_single_import(
    raw_files: &[&str],
    importer: &str,
    specifier: &str,
    active_profiles: &[&str],
) -> Resolution {
    let files = paths(raw_files);
    let request = vec![ImportRequest {
        importer: importer.to_owned(),
        specifier: specifier.to_owned(),
    }];
    let profiles = vec![codeint_model::PackageFrameworkProfiles {
        package_root: String::new(),
        active_profiles: active_profiles
            .iter()
            .map(|name| (*name).to_owned())
            .collect(),
    }];
    TypeScriptAndJavaScript
        .resolve_imports(&files, Vec::new(), request, &profiles)
        .expect("resolve import")
        .into_iter()
        .next()
        .expect("single resolution")
}

fn assert_alias_honors_profile(
    raw_files: &[&str],
    importer: &str,
    specifier: &str,
    profile: &str,
    expected_resolved: &str,
    expected_external: &str,
) {
    assert_eq!(
        resolve_single_import(raw_files, importer, specifier, &[profile]),
        Resolution::Resolved(vec![expected_resolved.to_owned()])
    );
    assert_eq!(
        resolve_single_import(raw_files, importer, specifier, &[]),
        Resolution::External(expected_external.to_owned())
    );
}

#[test]
fn known_framework_profiles_lists_all_nine_typescript_javascript_profiles() {
    assert_eq!(
        TypeScriptAndJavaScript.known_framework_profiles(),
        vec![
            "react",
            "vue2",
            "vue3",
            "svelte",
            "nextjs",
            "nuxt",
            "sveltekit",
            "angular",
            "nestjs",
        ]
    );
}

#[test]
fn react_profile_detects_app_conventions_excludes_peer_only_libraries_and_sets_adr_0020_ranges() {
    let app = resolve_auto(
        "",
        &[
            "package.json",
            "index.html",
            "src/main.tsx",
            "src/App.tsx",
            "src/helper.ts",
        ],
        &[(
            "package.json",
            r#"{"dependencies":{"react":"^18.3.1","react-dom":"^18.3.1"}}"#,
        )],
    );
    assert_eq!(app.active_profiles, vec!["react"]);
    assert_eq!(app.entry_points, vec!["src/main.tsx"]);
    assert!(app.generated_paths.is_empty());
    assert_panel_range(&app, "50 React repositories");
    assert!(app.reference_ranges.iter().all(|range| {
        let expected = if range.metric == "verbosity" { 2 } else { 1 };
        range.metric_version == expected
    }));
    assert_eq!(app.reference_range_note, None);

    let lib = resolve_auto(
        "",
        &[
            "package.json",
            "src/index.tsx",
            "src/main.tsx",
            "src/Button.tsx",
        ],
        &[(
            "package.json",
            r#"{"peerDependencies":{"react":"^18.0.0"}}"#,
        )],
    );
    assert!(lib.active_profiles.is_empty());
    assert!(lib.entry_points.is_empty());
    assert!(lib.reference_ranges.is_empty());
}

#[test]
fn vue2_and_vue3_profiles_separate_versions_emit_conventions_and_honor_active_profile_import_hints()
{
    let vue2 = resolve_auto(
        "",
        &[
            "package.json",
            "vue.config.js",
            "src/main.js",
            "src/App.vue",
            "src/components/Card.vue",
            "dist/bundle.js",
        ],
        &[("package.json", r#"{"dependencies":{"vue":"^2.7.16"}}"#)],
    );
    assert_eq!(vue2.active_profiles, vec!["vue2"]);
    assert_eq!(vue2.entry_points, vec!["src/main.js", "vue.config.js"]);
    assert_eq!(vue2.generated_paths, vec!["dist/bundle.js"]);
    assert_panel_range(&vue2, "50 Vue 2 repositories");

    let vue3_files = [
        "package.json",
        "vite.config.ts",
        "src/main.ts",
        "src/App.vue",
        "src/components/Card.vue",
        "auto-imports.d.ts",
        "components.d.ts",
    ];
    let vue3 = resolve_auto(
        "",
        &vue3_files,
        &[("package.json", r#"{"dependencies":{"vue":"^3.4.0"}}"#)],
    );
    assert_eq!(vue3.active_profiles, vec!["vue3"]);
    assert_eq!(vue3.entry_points, vec!["src/main.ts", "vite.config.ts"]);
    assert_eq!(
        vue3.generated_paths,
        vec!["auto-imports.d.ts", "components.d.ts"]
    );
    assert_panel_range(&vue3, "50 Vue 3 repositories");

    assert_alias_honors_profile(
        &vue3_files,
        "src/App.vue",
        "@/components/Card.vue",
        "vue3",
        "src/components/Card.vue",
        "@/components",
    );
}

#[test]
fn svelte_profile_allows_dev_dependencies_and_resolves_lib_alias_when_active() {
    let svelte_files = [
        "package.json",
        "index.html",
        "svelte.config.js",
        "vite.config.ts",
        "src/main.ts",
        "src/App.svelte",
        "src/lib/format.ts",
    ];
    let resolved = resolve_auto(
        "",
        &svelte_files,
        &[("package.json", r#"{"devDependencies":{"svelte":"^5.0.0"}}"#)],
    );
    assert_eq!(resolved.active_profiles, vec!["svelte"]);
    assert_eq!(
        resolved.entry_points,
        vec!["src/main.ts", "svelte.config.js", "vite.config.ts"]
    );
    assert!(resolved.generated_paths.is_empty());
    assert!(resolved.reference_ranges.is_empty());

    assert_alias_honors_profile(
        &svelte_files,
        "src/App.svelte",
        "$lib/format",
        "svelte",
        "src/lib/format.ts",
        "$lib",
    );
}

const HYBRID_FILES: [&str; 7] = [
    "package.json",
    "index.html",
    "vite.config.ts",
    "src/main.tsx",
    "src/App.vue",
    "src/Widget.tsx",
    "components.d.ts",
];

fn resolve_hybrid(deps_json: &str) -> FrameworkProfileResult {
    resolve_auto(
        "",
        &HYBRID_FILES,
        &[(
            "package.json",
            &format!(r#"{{"dependencies":{deps_json}}}"#),
        )],
    )
}

#[test]
fn sibling_framework_conflict_unions_conventions_and_falls_back_to_base_ranges_with_note() {
    let first = resolve_hybrid(r#"{"react":"^18.3.1","react-dom":"^18.3.1","vue":"^3.4.0"}"#);
    let second = resolve_hybrid(r#"{"vue":"^3.4.0","react-dom":"^18.3.1","react":"^18.3.1"}"#);
    assert_eq!(first.active_profiles, vec!["react", "vue3"]);
    assert_eq!(first.active_profiles, second.active_profiles);
    assert_eq!(first.entry_points, vec!["src/main.tsx", "vite.config.ts"]);
    assert_eq!(first.entry_points, second.entry_points);
    assert_eq!(first.generated_paths, vec!["components.d.ts"]);
    assert_eq!(first.generated_paths, second.generated_paths);
    assert!(first.reference_ranges.is_empty());
    assert!(second.reference_ranges.is_empty());
    assert_eq!(first.reference_range_note, second.reference_range_note);
    let note = first.reference_range_note.expect("sibling conflict note");
    assert!(note.contains("react") && note.contains("vue3"), "{note}");
}

#[test]
fn hoisted_monorepo_root_manifest_never_activates_child_package_without_local_signal() {
    let resolved = resolve_auto(
        "packages/api",
        &["packages/api/package.json", "packages/api/src/index.ts"],
        &[
            (
                "package.json",
                r#"{"private":true,"workspaces":["packages/*"],"dependencies":{"react":"^18.3.1","vue":"^3.4.0"}}"#,
            ),
            ("packages/api/package.json", r#"{"name":"@mono/api"}"#),
        ],
    );
    assert!(resolved.active_profiles.is_empty());
    assert!(resolved.entry_points.is_empty());
    assert!(resolved.reference_ranges.is_empty());
}

#[test]
fn sibling_framework_modules_do_not_import_each_other() {
    let frameworks_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/frameworks");
    let siblings = [
        "react",
        "vue2",
        "vue3",
        "svelte",
        "nextjs",
        "nuxt",
        "sveltekit",
        "angular",
        "nestjs",
    ];
    for module in siblings {
        let path = frameworks_dir.join(format!("{module}.rs"));
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        for other in siblings.iter().filter(|other| **other != module) {
            for needle in [format!("super::{other}"), format!("frameworks::{other}")] {
                assert!(
                    !source.contains(&needle),
                    "{} must not import sibling {other}",
                    path.display()
                );
            }
        }
    }
}

fn path_list(spec: &str) -> Vec<&str> {
    spec.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect()
}

fn resolve_root_pkg(files: &[&str], pkg_json: &str) -> FrameworkProfileResult {
    resolve_auto("", files, &[("package.json", pkg_json)])
}

fn assert_profile_layout(
    resolved: &FrameworkProfileResult,
    profiles: &[&str],
    entries_spec: &str,
    generated: &[&str],
) {
    assert_eq!(resolved.active_profiles, profiles);
    assert_eq!(resolved.entry_points, path_list(entries_spec));
    assert_eq!(resolved.generated_paths, generated);
}

fn assert_panel_range(resolved: &FrameworkProfileResult, panel: &str) {
    assert!(
        resolved
            .reference_ranges
            .iter()
            .any(|range| range.source.contains(panel) && range.source.contains("ADR 0035")),
        "expected {panel} ADR 0035 range in {:?}",
        resolved.reference_ranges
    );
}

fn assert_no_profiles(files: &[&str], pkg_json: &str) {
    let resolved = resolve_root_pkg(files, pkg_json);
    assert!(resolved.active_profiles.is_empty());
    assert!(resolved.entry_points.is_empty());
}

#[test]
fn nextjs_profile_detects_conventions_excludes_libraries_and_inherits_react_ranges() {
    let files = path_list(
        "package.json
        next.config.ts
        src/middleware.ts
        src/proxy.ts
        src/instrumentation.ts
        src/mdx-components.tsx
        src/app/layout.tsx
        src/app/page.tsx
        src/app/api/health/route.ts
        src/app/_components/Card.tsx
        src/pages/_app.tsx
        src/pages/api/ping.ts
        src/lib/format.ts
        components/Hero.tsx
        .next/types/routes.ts
        next-env.d.ts
        out/chunk.js",
    );
    let app = resolve_root_pkg(
        &files,
        r#"{"dependencies":{"next":"^15.0.0","react":"^19.0.0","react-dom":"^19.0.0"}}"#,
    );
    assert_profile_layout(
        &app,
        &["nextjs", "react"],
        "next.config.ts
        src/app/api/health/route.ts
        src/app/layout.tsx
        src/app/page.tsx
        src/instrumentation.ts
        src/mdx-components.tsx
        src/middleware.ts
        src/pages/_app.tsx
        src/pages/api/ping.ts
        src/proxy.ts",
        &[".next/types/routes.ts", "next-env.d.ts", "out/chunk.js"],
    );
    assert_panel_range(&app, "50 React repositories");
    assert_eq!(app.reference_range_note, None);

    for (specifier, resolved_path, external) in [
        ("@/lib/format", "src/lib/format.ts", "@/lib"),
        ("~/components/Hero", "components/Hero.tsx", "~"),
    ] {
        assert_alias_honors_profile(
            &files,
            "src/app/page.tsx",
            specifier,
            "nextjs",
            resolved_path,
            external,
        );
    }

    assert_no_profiles(
        &["package.json", "src/index.tsx", "src/app/page.tsx"],
        r#"{"peerDependencies":{"next":"^15.0.0","react":"^19.0.0"}}"#,
    );
}

#[test]
fn nuxt_profile_handles_v3_v4_conventions_aliases_virtual_modules_and_vue_ranges() {
    let v4_files = path_list(
        "package.json
        nuxt.config.ts
        app/app.vue
        app/error.vue
        app/pages/index.vue
        app/layouts/default.vue
        app/middleware/auth.ts
        app/plugins/init.ts
        app/composables/useAuth.ts
        app/utils/format.ts
        app/lib/orphan.ts
        server/api/health.ts
        shared/utils/math.ts
        .nuxt/types/imports.ts
        .output/server/index.mjs",
    );
    let v4 = resolve_root_pkg(
        &v4_files,
        r#"{"dependencies":{"nuxt":"^4.0.0","vue":"^3.5.0"}}"#,
    );
    assert_profile_layout(
        &v4,
        &["nuxt", "vue3"],
        "app/app.vue
        app/composables/useAuth.ts
        app/error.vue
        app/layouts/default.vue
        app/middleware/auth.ts
        app/pages/index.vue
        app/plugins/init.ts
        app/utils/format.ts
        nuxt.config.ts
        server/api/health.ts
        shared/utils/math.ts",
        &[".nuxt/types/imports.ts", ".output/server/index.mjs"],
    );
    assert_panel_range(&v4, "50 Vue 3 repositories");
    assert_eq!(v4.reference_range_note, None);

    for (specifier, resolved_path, external) in [
        ("~/utils/format", "app/utils/format.ts", "~"),
        ("@/utils/format", "app/utils/format.ts", "@/utils"),
        ("~~/shared/utils/math", "shared/utils/math.ts", "~~"),
        ("@@/shared/utils/math", "shared/utils/math.ts", "@@/shared"),
    ] {
        assert_alias_honors_profile(
            &v4_files,
            "app/pages/index.vue",
            specifier,
            "nuxt",
            resolved_path,
            external,
        );
    }
    for virtual_spec in ["#imports", "#components", "#app", "#build/routes"] {
        assert_eq!(
            resolve_single_import(&v4_files, "app/pages/index.vue", virtual_spec, &["nuxt"]),
            Resolution::Builtin,
            "{virtual_spec}"
        );
        assert!(
            matches!(
                resolve_single_import(&v4_files, "app/pages/index.vue", virtual_spec, &[]),
                Resolution::Unresolved(_)
            ),
            "{virtual_spec} must not resolve as builtin without nuxt profile"
        );
    }

    let v2 = resolve_root_pkg(
        &[
            "package.json",
            "nuxt.config.js",
            "pages/index.vue",
            "dist/app.js",
        ],
        r#"{"dependencies":{"nuxt":"^2.17.0","vue":"^2.7.16"}}"#,
    );
    assert_eq!(v2.active_profiles, vec!["nuxt", "vue2"]);
    assert_panel_range(&v2, "50 Vue 2 repositories");

    assert_no_profiles(
        &[
            "package.json",
            "src/module.ts",
            "playground/nuxt.config.ts",
            "playground/app.vue",
        ],
        r#"{"dependencies":{"@nuxt/kit":"^3.14.0"},"devDependencies":{"nuxt":"^3.14.0"}}"#,
    );
}

#[test]
fn sveltekit_profile_detects_routes_hooks_generated_paths_and_unions_with_svelte() {
    let files = path_list(
        "package.json
        svelte.config.js
        vite.config.ts
        src/app.html
        src/hooks.server.ts
        src/service-worker.ts
        src/routes/+layout.svelte
        src/routes/+page.svelte
        src/routes/+page.server.ts
        src/routes/about/+page@root.svelte
        src/routes/api/health/+server.ts
        src/routes/blog/helper.ts
        src/lib/format.ts
        .svelte-kit/generated/root.svelte
        build/index.js",
    );
    let resolved = resolve_root_pkg(
        &files,
        r#"{"devDependencies":{"@sveltejs/kit":"^2.8.0","svelte":"^5.0.0"}}"#,
    );
    assert_profile_layout(
        &resolved,
        &["svelte", "sveltekit"],
        "src/app.html
        src/hooks.server.ts
        src/routes/+layout.svelte
        src/routes/+page.server.ts
        src/routes/+page.svelte
        src/routes/about/+page@root.svelte
        src/routes/api/health/+server.ts
        src/service-worker.ts
        svelte.config.js
        vite.config.ts",
        &[".svelte-kit/generated/root.svelte", "build/index.js"],
    );
    assert_alias_honors_profile(
        &files,
        "src/routes/+page.svelte",
        "$lib/format",
        "sveltekit",
        "src/lib/format.ts",
        "$lib",
    );

    assert_no_profiles(
        &["package.json", "src/index.ts", "svelte.config.js"],
        r#"{"peerDependencies":{"@sveltejs/kit":"^2.0.0"}}"#,
    );
}

#[test]
fn angular_and_nestjs_profiles_detect_conventions_and_guard_peer_only_libraries() {
    let ng = resolve_root_pkg(
        &path_list(
            "package.json
            angular.json
            src/main.ts
            src/main.server.ts
            src/app/app.config.ts
            src/app/app.routes.ts
            src/app/app.module.ts
            src/app/orphan.service.ts
            .angular/cache/entry.ts
            dist/main.js",
        ),
        r#"{"dependencies":{"@angular/core":"^19.0.0"}}"#,
    );
    assert_profile_layout(
        &ng,
        &["angular"],
        "angular.json
        src/app/app.config.ts
        src/app/app.module.ts
        src/app/app.routes.ts
        src/main.server.ts
        src/main.ts",
        &[".angular/cache/entry.ts", "dist/main.js"],
    );

    let nest = resolve_root_pkg(
        &path_list(
            "package.json
            nest-cli.json
            src/main.ts
            src/app.module.ts
            src/orphan.service.ts
            dist/main.js",
        ),
        r#"{"dependencies":{"@nestjs/core":"^11.0.0"}}"#,
    );
    assert_profile_layout(
        &nest,
        &["nestjs"],
        "nest-cli.json
        src/app.module.ts
        src/main.ts",
        &["dist/main.js"],
    );

    for peer_manifest in [
        r#"{"peerDependencies":{"@angular/core":"^19.0.0"}}"#,
        r#"{"peerDependencies":{"@nestjs/core":"^11.0.0","@nestjs/common":"^11.0.0"}}"#,
    ] {
        assert_no_profiles(
            &["package.json", "src/main.ts", "src/index.ts"],
            peer_manifest,
        );
    }
}

#[test]
fn framework_detection_ignores_node_modules_and_tolerates_invalid_package_json() {
    let from_node_modules = resolve_auto(
        "",
        &["src/App.tsx", "node_modules/some-pkg/package.json"],
        &[(
            "node_modules/some-pkg/package.json",
            r#"{"dependencies":{"react":"^18.3.1","react-dom":"^18.3.1","vue":"^3.4.0"}}"#,
        )],
    );
    assert!(from_node_modules.active_profiles.is_empty());
    assert!(from_node_modules.reference_ranges.is_empty());

    let invalid_json = resolve_auto(
        "",
        &["package.json", "src/App.tsx", "src/App.vue"],
        &[("package.json", "{ not valid json")],
    );
    assert!(invalid_json.active_profiles.is_empty());
    assert!(invalid_json.reference_ranges.is_empty());
}
