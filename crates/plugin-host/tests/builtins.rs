use std::collections::BTreeMap;

use codeint_config::SqlDialect;
use codeint_model::{CodeModel, Manifest, RepoPath, SourceFile};
use codeint_plugin_host::{
    SQL_PLUGIN_NAME, SqlSchemaScopeSummary, TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME,
    apply_sql_schema_pass, built_in_language_descriptors, built_in_language_hazard_rules,
    built_in_language_names, built_in_language_plugin, built_in_language_plugins, lower_sql_source,
    manifests_declare_framework_reference_ranges, select_language_reference_ranges,
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
fn select_language_reference_ranges_applies_react_and_vue_framework_precedence() {
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

    assert!(manifests_declare_framework_reference_ranges(
        TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME,
        &react_manifest
    ));
    assert!(manifests_declare_framework_reference_ranges(
        TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME,
        &vue_manifest
    ));
    assert!(!manifests_declare_framework_reference_ranges(
        "rust",
        &react_manifest
    ));

    let tsx_path = RepoPath::new("src/App.tsx").expect("valid path");
    let tsx_file = ts_plugin
        .plugin
        .lower(
            &tsx_path,
            "export function App() { return <div>hi</div>; }\n",
        )
        .expect("tsx lowers");
    let vue_path = RepoPath::new("src/App.vue").expect("valid path");
    let vue_file = ts_plugin
        .plugin
        .lower(&vue_path, "<script setup>\nconst x = 1;\n</script>\n")
        .expect("vue lowers");

    let react_model = CodeModel {
        files: vec![SourceFile {
            path: tsx_path.clone(),
            model: tsx_file.clone(),
        }],
        ..CodeModel::default()
    };
    let react_ranges = select_language_reference_ranges(&ts_plugin, &react_model, &react_manifest)
        .expect("react ranges");
    assert!(
        react_ranges
            .for_metric("erosion")
            .expect("erosion range")
            .source
            .contains("ADR 0020")
    );

    let vue_model = CodeModel {
        files: vec![SourceFile {
            path: vue_path.clone(),
            model: vue_file.clone(),
        }],
        ..CodeModel::default()
    };
    let vue_ranges = select_language_reference_ranges(&ts_plugin, &vue_model, &vue_manifest)
        .expect("vue ranges");
    assert!(
        vue_ranges
            .for_metric("erosion")
            .expect("erosion range")
            .source
            .contains("ADR 0015")
    );

    let hybrid_model = CodeModel {
        files: vec![
            SourceFile {
                path: tsx_path,
                model: tsx_file,
            },
            SourceFile {
                path: vue_path,
                model: vue_file,
            },
        ],
        ..CodeModel::default()
    };
    let hybrid_ranges =
        select_language_reference_ranges(&ts_plugin, &hybrid_model, &hybrid_manifest)
            .expect("hybrid ranges");
    assert!(
        hybrid_ranges
            .for_metric("erosion")
            .expect("erosion range")
            .source
            .contains("ADR 0008")
    );

    let rust_ranges =
        select_language_reference_ranges(&rust_plugin, &CodeModel::default(), &react_manifest)
            .expect("rust ranges");
    assert!(
        rust_ranges
            .for_metric("erosion")
            .expect("erosion range")
            .source
            .contains("ADR 0010")
    );
}
