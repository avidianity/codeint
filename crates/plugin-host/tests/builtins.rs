use std::collections::BTreeMap;

use codeint_config::SqlDialect;
use codeint_model::{CodeModel, FrameworkProfileSelection, Manifest, RepoPath, SourceFile};
use codeint_plugin_host::{
    SQL_PLUGIN_NAME, SqlSchemaScopeSummary, TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME,
    apply_sql_schema_pass, built_in_language_descriptors, built_in_language_flagged_line_rules,
    built_in_language_framework_profiles, built_in_language_hazard_rules, built_in_language_names,
    built_in_language_plugin, built_in_language_plugins, lower_sql_source,
};

#[test]
fn built_in_language_plugins_register_all_seven_languages_with_descriptors_and_hazard_rules() {
    let expected_names = [
        "typescript-javascript",
        "rust",
        "php",
        "dart",
        "python",
        "go",
        "sql",
    ];
    let names = built_in_language_names();
    assert_eq!(names, expected_names);

    let plugins = built_in_language_plugins();
    assert_eq!(plugins.len(), 7);
    assert!(built_in_language_plugin(TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME).is_some());
    assert!(built_in_language_plugin(SQL_PLUGIN_NAME).is_some());
    assert!(built_in_language_plugin("unknown-language").is_none());

    let descriptors = built_in_language_descriptors();
    assert_eq!(descriptors.len(), 7);
    for (plugin, descriptor) in plugins.iter().zip(&descriptors) {
        assert_eq!(plugin.name, descriptor.name);
        assert_eq!(plugin.plugin.extensions(), descriptor.extensions);
        assert!(!descriptor.extensions.is_empty());
    }

    let hazard_rules = built_in_language_hazard_rules();
    let sql_rules = hazard_rules
        .get(SQL_PLUGIN_NAME)
        .expect("sql built-in plugin exposes hazard rules");
    assert!(
        sql_rules
            .iter()
            .any(|rule| rule == "filter-on-unindexed-column")
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
fn sql_host_helpers_lower_with_dialect_and_apply_cross_file_schema_hazards() {
    let migration_path = RepoPath::new("migrations/001_init.sql").expect("valid path");
    let down_path = RepoPath::new("migrations/001_init.down.sql").expect("valid path");
    let query_path = RepoPath::new("queries/get_users.sql").expect("valid path");

    let migration = lower_sql_source(
        &migration_path,
        "CREATE TABLE users (id INT PRIMARY KEY, email TEXT);\n",
        SqlDialect::Postgresql,
    )
    .expect("migration lowers");
    let down = lower_sql_source(&down_path, "DROP TABLE users;\n", SqlDialect::Postgresql)
        .expect("down migration lowers");
    let query = lower_sql_source(
        &query_path,
        "SELECT id FROM users WHERE email = 'a@b.com';\n",
        SqlDialect::Postgresql,
    )
    .expect("query lowers");

    let mut parsed_by_path = BTreeMap::new();
    parsed_by_path.insert(migration_path.clone(), migration.parsed);
    parsed_by_path.insert(down_path.clone(), down.parsed);
    parsed_by_path.insert(query_path.clone(), query.parsed);

    let mut model = CodeModel {
        files: vec![
            SourceFile {
                path: migration_path,
                model: migration.file_model,
            },
            SourceFile {
                path: down_path,
                model: down.file_model,
            },
            SourceFile {
                path: query_path.clone(),
                model: query.file_model,
            },
        ],
        ..CodeModel::default()
    };

    let summaries = apply_sql_schema_pass(&mut model, &parsed_by_path);
    assert_eq!(
        summaries,
        vec![SqlSchemaScopeSummary {
            package: None,
            dialect: "postgresql".to_owned(),
            tables: 1,
        }]
    );

    let query_file = model
        .files
        .iter()
        .find(|file| file.path == query_path)
        .expect("query file present");
    assert!(
        query_file
            .model
            .hazard_lines
            .iter()
            .any(|hazard| hazard.rule == "filter-on-unindexed-column"),
        "expected cross-file schema hazard, got {:?}",
        query_file.model.hazard_lines
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

    assert_profile_adr(&tsx_path, react_manifest.clone(), "react", "ADR 0020");
    assert_profile_adr(&vue_path, vue_manifest, "vue3", "ADR 0015");

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
