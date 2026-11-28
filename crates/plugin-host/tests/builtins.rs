use codeint_model::{
    FileScopeArtifact, FrameworkProfileSelection, Manifest, RepoPath, SummaryField,
};
use codeint_plugin_host::{
    TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME, built_in_language_descriptors,
    built_in_language_flagged_line_rules, built_in_language_framework_profiles,
    built_in_language_hazard_rules, built_in_language_names, built_in_language_plugin,
    built_in_language_plugins,
};

#[test]
fn built_in_language_plugins_register_all_eight_languages_with_descriptors_and_hazard_rules() {
    let expected_names = [
        "typescript-javascript",
        "rust",
        "php",
        "dart",
        "python",
        "go",
        "sql",
        "shell",
    ];
    let names = built_in_language_names();
    assert_eq!(names, expected_names);

    let plugins = built_in_language_plugins();
    assert_eq!(plugins.len(), 8);
    assert!(built_in_language_plugin(TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME).is_some());
    assert!(built_in_language_plugin("sql").is_some());
    assert!(built_in_language_plugin("unknown-language").is_none());

    let descriptors = built_in_language_descriptors();
    assert_eq!(descriptors.len(), 8);
    for (plugin, descriptor) in plugins.iter().zip(&descriptors) {
        assert_eq!(plugin.name, descriptor.name);
        assert_eq!(plugin.plugin.extensions(), descriptor.extensions);
        assert!(!descriptor.extensions.is_empty());
    }

    let hazard_rules = built_in_language_hazard_rules();
    let sql_rules = hazard_rules
        .get("sql")
        .expect("sql built-in plugin exposes hazard rules");
    assert!(
        sql_rules
            .iter()
            .any(|rule| rule == "filter-on-unindexed-column")
    );
    let shell_rules = hazard_rules
        .get("shell")
        .expect("shell built-in plugin exposes hazard rules");
    assert!(shell_rules.iter().any(|rule| rule == "missing-strict-mode"));
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.name == "shell"
                && descriptor.extensions == ["sh", "bash", "zsh"])
    );
    assert!(!hazard_rules.contains_key(TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME));
}

#[test]
fn every_built_in_language_plugin_declares_flagged_line_rules() {
    let rules = built_in_language_flagged_line_rules();
    let names: Vec<&str> = rules.keys().map(String::as_str).collect();
    let mut expected = built_in_language_names();
    expected.sort_unstable();
    assert_eq!(names, expected);
    assert!(rules.values().all(|names| !names.is_empty()));
    assert!(rules["go"].iter().any(|rule| rule == "redundant-break"));
}

#[test]
fn built_in_sql_plugin_exports_dialects_and_runs_scope_analysis_through_language_plugin_trait() {
    let sql = built_in_language_plugin("sql").expect("built-in sql plugin");
    assert_eq!(
        sql.plugin.known_dialects(),
        vec!["postgresql", "mysql", "sqlite", "duckdb", "generic"]
    );
    assert!(sql.plugin.has_scope_analysis());

    let migration_path = RepoPath::new("migrations/001_init.sql").expect("valid path");
    let down_path = RepoPath::new("migrations/001_init.down.sql").expect("valid path");
    let query_path = RepoPath::new("queries/get_users.sql").expect("valid path");

    let migration = sql
        .plugin
        .lower(
            &migration_path,
            "CREATE TABLE users (id INT PRIMARY KEY, email TEXT);\n",
            Some("postgresql"),
        )
        .expect("migration lowers");
    let down = sql
        .plugin
        .lower(&down_path, "DROP TABLE users;\n", Some("postgresql"))
        .expect("down migration lowers");
    let query = sql
        .plugin
        .lower(
            &query_path,
            "SELECT id FROM users WHERE email = 'a@b.com';\n",
            Some("postgresql"),
        )
        .expect("query lowers");

    let artifacts = vec![
        FileScopeArtifact {
            path: migration_path.to_string(),
            source_set: migration.source_set,
            artifact: migration.scope_artifact.expect("migration artifact"),
        },
        FileScopeArtifact {
            path: down_path.to_string(),
            source_set: down.source_set,
            artifact: down.scope_artifact.expect("down artifact"),
        },
        FileScopeArtifact {
            path: query_path.to_string(),
            source_set: query.source_set,
            artifact: query.scope_artifact.expect("query artifact"),
        },
    ];

    let analysis = sql
        .plugin
        .analyze_scope("", Some("postgresql"), artifacts)
        .expect("scope analysis succeeds");
    assert_eq!(analysis.summaries.len(), 1);
    assert_eq!(analysis.summaries[0].kind, "sql-schema");
    assert_eq!(
        analysis.summaries[0].fields,
        vec![
            SummaryField {
                key: "dialect".to_owned(),
                value: "postgresql".to_owned(),
            },
            SummaryField {
                key: "tables".to_owned(),
                value: "1".to_owned(),
            },
        ]
    );

    let query_hazards = analysis
        .extra_hazards
        .iter()
        .find(|entry| entry.path == query_path.as_str())
        .expect("query hazards present");
    assert!(
        query_hazards
            .hazard_lines
            .iter()
            .any(|hazard| hazard.rule == "filter-on-unindexed-column"),
        "expected cross-file schema hazard, got {:?}",
        query_hazards.hazard_lines
    );
}

#[test]
fn built_in_typescript_plugin_resolves_framework_profiles_and_sibling_fallback() {
    let profiles_by_language = built_in_language_framework_profiles();
    assert_eq!(
        profiles_by_language.get(TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME),
        Some(&vec![
            "react".to_owned(),
            "vue2".to_owned(),
            "vue3".to_owned(),
            "svelte".to_owned(),
            "nextjs".to_owned(),
            "nuxt".to_owned(),
            "sveltekit".to_owned(),
            "angular".to_owned(),
            "nestjs".to_owned(),
        ])
    );

    let ts_plugin = built_in_language_plugin(TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME)
        .expect("built-in typescript-javascript plugin");
    let rust_plugin = built_in_language_plugin("rust").expect("built-in rust plugin");

    let react_manifest = vec![Manifest {
        path: "package.json".to_owned(),
        text: r#"{"dependencies":{"react":"^18.3.1"}}"#.to_owned(),
    }];
    let vue_manifest = vec![Manifest {
        path: "package.json".to_owned(),
        text: r#"{"dependencies":{"vue":"^3.4.0"}}"#.to_owned(),
    }];
    let hybrid_manifest = vec![Manifest {
        path: "package.json".to_owned(),
        text: r#"{"dependencies":{"react":"^18.3.1","vue":"^3.4.0"}}"#.to_owned(),
    }];

    let tsx_path = RepoPath::new("src/App.tsx").expect("valid path");
    let vue_path = RepoPath::new("src/App.vue").expect("valid path");

    let assert_profile_adr =
        |path: &RepoPath, manifests: Vec<Manifest>, expected_profile: &str, expected_adr: &str| {
            let resolved = ts_plugin
                .plugin
                .resolve_framework_profiles(
                    "",
                    std::slice::from_ref(path),
                    manifests,
                    FrameworkProfileSelection::Auto,
                )
                .expect("framework profile");
            assert_eq!(resolved.active_profiles, vec![expected_profile]);
            assert!(
                resolved
                    .reference_ranges
                    .iter()
                    .find(|range| range.metric == "erosion")
                    .expect("erosion range")
                    .source
                    .contains(expected_adr)
            );
        };

    assert_profile_adr(
        &tsx_path,
        react_manifest.clone(),
        "react",
        "50 React repositories",
    );
    assert_profile_adr(&vue_path, vue_manifest, "vue3", "50 Vue 3 repositories");

    let hybrid = ts_plugin
        .plugin
        .resolve_framework_profiles(
            "",
            &[tsx_path, vue_path],
            hybrid_manifest,
            FrameworkProfileSelection::Auto,
        )
        .expect("hybrid profile");
    assert_eq!(hybrid.active_profiles, vec!["react", "vue3"]);
    assert!(hybrid.reference_ranges.is_empty());
    assert!(hybrid.reference_range_note.is_some());

    let rust = rust_plugin
        .plugin
        .resolve_framework_profiles("", &[], react_manifest, FrameworkProfileSelection::Auto)
        .expect("rust profile");
    assert!(rust.active_profiles.is_empty());
    assert!(rust.reference_ranges.is_empty());
}
