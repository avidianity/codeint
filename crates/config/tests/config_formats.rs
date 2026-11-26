use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use codeint_config::{
    BoundaryPreset, ConfigFileKind, ConfigOverrides, ConfigurationFileError,
    ConfigurationInspection, DetectedLanguage, InitFormat, MeasuredBaselineMetric, OutputFormat,
    SchemaVersionStatus, StarterBoundaries, StarterConfigurationError, discover_config_file,
    inspect_configuration, published_schema_address, read_configuration,
    read_resolved_configuration, rewrite_schema_tag, write_adoption_starter_configuration,
    write_adoption_zero_budgets, write_starter_configuration_in_format,
};
use codeint_model::{Metric, RepoPath, SourceSet};

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

fn with_temp_repo(label: &str, body: impl FnOnce(&Path)) {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("config-formats-tests")
        .join(format!("{label}-{id}"));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp workspace");
    body(&root);
    let _ = fs::remove_dir_all(&root);
}

fn write_repo_file(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap_or(root)).expect("create parent directory");
    fs::write(path, contents).expect("write config file");
}

fn read_error(label: &str, file_name: &str, body: &str) -> ConfigurationFileError {
    let mut result = None;
    with_temp_repo(label, |root| {
        write_repo_file(root, file_name, body);
        result = Some(read_configuration(root).unwrap_err());
    });
    result.expect("error captured")
}

#[test]
fn discovers_and_reads_all_ten_config_filenames_and_rejects_cross_format_ambiguity() {
    for kind in ConfigFileKind::ALL {
        with_temp_repo("discover", |root| {
            let content = match kind.file_name() {
                name if name.ends_with(".toml") => "format = \"text\"\n",
                name if name.ends_with(".yaml") || name.ends_with(".yml") => "format: text\n",
                _ => "{ \"format\": \"text\" }\n",
            };
            write_repo_file(root, kind.file_name(), content);

            let discovered = discover_config_file(root)
                .expect("discover config")
                .expect("config file present");
            assert_eq!(discovered.kind, kind);

            let config = read_configuration(root).expect("read config");
            assert_eq!(config.config_file(), kind);
            assert_eq!(config.format, OutputFormat::Text);
        });
    }

    with_temp_repo("ambiguous", |root| {
        write_repo_file(root, "codeint.toml", "format = \"text\"\n");
        write_repo_file(root, ".codeint/config.yaml", "format: text\n");
        let err = read_configuration(root).unwrap_err();
        assert!(matches!(
            err,
            ConfigurationFileError::AmbiguousConfigFiles { ref files }
                if files == &[ConfigFileKind::Toml, ConfigFileKind::DotCodeintYaml]
        ));
    });
}

#[test]
fn equivalent_json_toml_and_yaml_configs_produce_identical_effective_configuration() {
    let json_text = r#"{
  "format": "text",
  "source_sets": { "generated": ["gen/**"] },
  "budgets": { "production": { "erosion": 0.1, "hazards": 0 }, "unused_dependencies": 0 },
  "new_file_budgets": { "production": { "erosion": 0 } },
  "regression_limits": { "per_file": true, "production": { "erosion": { "ratio_rise": 0.02 } } },
  "languages": [{ "plugin": "rust", "include": ["crates"], "exclude": ["crates/gen"] }],
  "plugin_limits": { "memory_mb": 128, "timeout_ms": 8000 },
  "boundaries": {
    "strict_zones": true,
    "zones": { "core": { "patterns": ["crates/core/**"] } },
    "rules": [{ "from": "core", "deny_external": ["left-pad"] }]
  },
  "dependencies": { "ignore": ["serde"] },
  "duplication": { "near_clones": true, "min_similarity": 0.9 },
  "entry_points": ["src/main.rs"],
  "coverage_reports": ["coverage/lcov.info"],
  "sql": { "dialects": [{ "dialect": "postgresql", "include": ["db/**"] }] },
  "hazards": { "disabled_rules": ["select-star"] },
  "framework_profiles": { "exclude": ["nextjs"] },
  "packages": { "crates/core": { "framework_profiles": false } },
  "ratchet": { "path": ".codeint/ratchet.json" }
}"#;

    let toml_text = r#"
format = "text"
entry_points = ["src/main.rs"]
coverage_reports = ["coverage/lcov.info"]

[source_sets]
generated = ["gen/**"]

[budgets]
unused_dependencies = 0

[budgets.production]
erosion = 0.1
hazards = 0

[new_file_budgets.production]
erosion = 0

[regression_limits]
per_file = true

[regression_limits.production.erosion]
ratio_rise = 0.02

[[languages]]
plugin = "rust"
include = ["crates"]
exclude = ["crates/gen"]

[plugin_limits]
memory_mb = 128
timeout_ms = 8000

[boundaries]
strict_zones = true

[boundaries.zones.core]
patterns = ["crates/core/**"]

[[boundaries.rules]]
from = "core"
deny_external = ["left-pad"]

[dependencies]
ignore = ["serde"]

[duplication]
near_clones = true
min_similarity = 0.9

[[sql.dialects]]
dialect = "postgresql"
include = ["db/**"]

[hazards]
disabled_rules = ["select-star"]

[framework_profiles]
exclude = ["nextjs"]

[packages."crates/core"]
framework_profiles = false

[ratchet]
path = ".codeint/ratchet.json"
"#;

    let yaml_text = r#"
format: text
source_sets:
  generated:
    - gen/**
budgets:
  production:
    erosion: 0.1
    hazards: 0
  unused_dependencies: 0
new_file_budgets:
  production:
    erosion: 0
regression_limits:
  per_file: true
  production:
    erosion:
      ratio_rise: 0.02
languages:
  - plugin: rust
    include:
      - crates
    exclude:
      - crates/gen
plugin_limits:
  memory_mb: 128
  timeout_ms: 8000
boundaries:
  strict_zones: true
  zones:
    core:
      patterns:
        - crates/core/**
  rules:
    - from: core
      deny_external:
        - left-pad
dependencies:
  ignore:
    - serde
duplication:
  near_clones: true
  min_similarity: 0.9
entry_points:
  - src/main.rs
coverage_reports:
  - coverage/lcov.info
sql:
  dialects:
    - dialect: postgresql
      include:
        - db/**
hazards:
  disabled_rules:
    - select-star
framework_profiles:
  exclude:
    - nextjs
packages:
  crates/core:
    framework_profiles: false
ratchet:
  path: .codeint/ratchet.json
"#;

    let mut canonical = Vec::new();
    for (file_name, text) in [
        ("codeint.json", json_text),
        ("codeint.toml", toml_text),
        ("codeint.yaml", yaml_text),
    ] {
        with_temp_repo("parity", |root| {
            write_repo_file(root, file_name, text);
            let resolved = read_resolved_configuration(root, &ConfigOverrides::default())
                .unwrap_or_else(|err| panic!("failed to read {file_name}: {err}"));
            assert_eq!(resolved.configuration.format, OutputFormat::Text);
            assert!(resolved.configuration.duplication.near_clones);
            assert_eq!(resolved.configuration.plugin_limits.memory_mb, 128);
            canonical.push(
                serde_json::to_string(&resolved.configuration).expect("serialize canonical config"),
            );
        });
    }
    assert_eq!(canonical[0], canonical[1]);
    assert_eq!(canonical[0], canonical[2]);
}

#[test]
fn empty_and_comment_only_toml_and_yaml_resolve_to_defaults() {
    for (file_name, text) in [
        ("codeint.toml", ""),
        ("codeint.toml", "# only comments\n\n"),
        (
            "codeint.toml",
            "\u{FEFF}#:schema https://example.invalid/s.json\n",
        ),
        ("codeint.yaml", ""),
        ("codeint.yaml", "# only comments\n   \n"),
        (
            "codeint.yml",
            "\u{FEFF}# yaml-language-server: $schema=https://example.invalid/s.json\n",
        ),
    ] {
        with_temp_repo("defaults", |root| {
            write_repo_file(root, file_name, text);
            let config = read_configuration(root).unwrap_or_else(|err| {
                panic!("expected defaults for {file_name} ({text:?}): {err}")
            });
            assert_eq!(config.format, OutputFormat::Toon);
            assert!(config.coverage_reports.is_empty());
        });
    }
}

#[test]
fn rejects_unsafe_and_invalid_toml_and_yaml_inputs() {
    for (label, yaml) in [
        ("null root", "null\n"),
        ("tilde null root", "~\n"),
        ("array root", "- a\n- b\n"),
        ("scalar string root", "\"hello\"\n"),
        ("scalar int root", "42\n"),
        ("scalar bool root", "true\n"),
        ("duplicate key", "format: text\nformat: json\n"),
        ("multi-document", "format: text\n---\nformat: json\n"),
        ("integer mapping key", "123: text\n"),
        ("boolean mapping key", "true: text\n"),
        ("null mapping key", "null: text\n"),
        ("sequence mapping key", "[a, b]: text\n"),
        ("anchor definition", "format: &fmt text\n"),
        (
            "anchor and alias",
            "a: &x 0.1\nbudgets:\n  production:\n    erosion: *x\n",
        ),
        ("merge key", "budgets:\n  <<: {}\n"),
        (
            "non-finite float",
            "budgets:\n  production:\n    erosion: .nan\n",
        ),
    ] {
        let _ = read_error(label, "codeint.yaml", yaml);
    }

    for (label, toml) in [
        ("toml-dup", "format = \"text\"\nformat = \"json\"\n"),
        ("toml-nan", "[budgets.production]\nerosion = nan\n"),
    ] {
        assert!(matches!(
            read_error(label, "codeint.toml", toml),
            ConfigurationFileError::InvalidToml {
                file: ConfigFileKind::Toml,
                ..
            }
        ));
    }

    let dt_err = read_error(
        "toml-datetime",
        "codeint.toml",
        "[budgets.production]\nerosion = 2026-11-26T00:00:00Z\n",
    );
    assert!(matches!(
        dt_err,
        ConfigurationFileError::UnsupportedTomlDatetime {
            file: ConfigFileKind::Toml,
            ref key,
        } if key.to_string() == "budgets.production.erosion"
    ));
}

fn schema_doc(kind: ConfigFileKind, url: &str) -> String {
    match kind {
        ConfigFileKind::Toml => format!("#:schema {url}\nformat = \"text\"\n"),
        _ => format!("# yaml-language-server: $schema={url}\nformat: text\n"),
    }
}

#[test]
fn schema_directives_and_dollar_schema_keys_validate_and_detect_conflicts() {
    let newer_url =
        "https://raw.githubusercontent.com/avidianity/codeint/v99.0.0/schema/codeint.schema.json";
    let current_url = published_schema_address();

    for kind in [ConfigFileKind::Toml, ConfigFileKind::Yaml] {
        let err = read_error("newer", kind.file_name(), &schema_doc(kind, newer_url));
        assert!(matches!(
            err,
            ConfigurationFileError::NewerSchemaVersion {
                file,
                ref config_version,
                ..
            } if file == kind && config_version == "99.0.0"
        ));
    }

    with_temp_repo("toml-matching", |root| {
        write_repo_file(
            root,
            "codeint.toml",
            &format!("#:schema {current_url}\n\"$schema\" = \"{current_url}\"\n"),
        );
        let inspection = inspect_configuration(root);
        assert!(matches!(
            inspection,
            ConfigurationInspection::Valid {
                kind: ConfigFileKind::Toml,
                schema: SchemaVersionStatus::Compatible { .. },
                ..
            }
        ));
    });

    let conflicts = [
        (
            ConfigFileKind::Yaml,
            format!("# yaml-language-server: $schema={current_url}\n$schema: {newer_url}\n"),
        ),
        (
            ConfigFileKind::Toml,
            format!("#:schema {current_url}\n\"$schema\" = \"{newer_url}\"\n"),
        ),
    ];
    for (kind, body) in conflicts {
        assert!(matches!(
            read_error("conflict", kind.file_name(), &body),
            ConfigurationFileError::ConflictingSchemaDirectives { file, .. } if file == kind
        ));
    }
}

#[test]
fn rewrite_schema_tag_updates_toml_and_yaml_directives_keys_and_skips_conflicts() {
    let old =
        "https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json";
    let new =
        "https://raw.githubusercontent.com/avidianity/codeint/v9.9.9/schema/codeint.schema.json";
    let cases = [
        (
            ConfigFileKind::Toml,
            format!("#:schema {old}\n\nformat = \"text\"\n"),
            Some(format!("#:schema {new}\n\nformat = \"text\"\n")),
        ),
        (
            ConfigFileKind::Toml,
            format!("#:schema {old}\n\"$schema\" = \"{old}\"\n"),
            Some(format!("#:schema {new}\n\"$schema\" = \"{new}\"\n")),
        ),
        (
            ConfigFileKind::Yaml,
            format!("# yaml-language-server: $schema={old}\nformat: text\n"),
            Some(format!(
                "# yaml-language-server: $schema={new}\nformat: text\n"
            )),
        ),
        (
            ConfigFileKind::Yaml,
            format!("$schema: {old}\nformat: text\n"),
            Some(format!("$schema: {new}\nformat: text\n")),
        ),
        (
            ConfigFileKind::Toml,
            format!(
                "#:schema {old}\n\"$schema\" = \"https://raw.githubusercontent.com/avidianity/codeint/v0.2.0/schema/codeint.schema.json\"\n"
            ),
            None,
        ),
    ];
    for (kind, input, expected) in cases {
        let actual = rewrite_schema_tag(kind, &input, "v9.9.9");
        assert_eq!(actual, expected.map(|body| (body, new.to_owned())));
    }
}

fn verify_uncommented_starter(file_name: &str, uncommented: &str) {
    with_temp_repo("hints", |hint_dir| {
        write_repo_file(hint_dir, file_name, uncommented);
        let active = read_configuration(hint_dir).unwrap_or_else(|err| {
            panic!("uncommented {file_name} hints failed: {err}\n{uncommented}")
        });
        assert!(active.boundaries.is_some());
        assert_eq!(active.budgets.production_test_gap(), Some(0.2));
    });
}

#[test]
fn starter_formats_generate_valid_round_trippable_configs_and_refuse_existing_files() {
    let languages = [
        DetectedLanguage {
            plugin: "rust".to_owned(),
            include: vec![RepoPath::new("crates").expect("repo path")],
        },
        DetectedLanguage {
            plugin: "sql".to_owned(),
            include: vec![RepoPath::new("db").expect("repo path")],
        },
    ];
    let boundaries = StarterBoundaries::Preset(BoundaryPreset::Layered);
    let recorded = [
        MeasuredBaselineMetric {
            source_set: Some(SourceSet::Production),
            metric: Metric::Erosion.key().to_owned(),
            metric_version: Metric::Erosion.version(),
            findings: Vec::new(),
        },
        MeasuredBaselineMetric {
            source_set: None,
            metric: Metric::UnusedDependencies.key().to_owned(),
            metric_version: Metric::UnusedDependencies.version(),
            findings: Vec::new(),
        },
    ];

    for format in [
        InitFormat::Jsonc,
        InitFormat::Json,
        InitFormat::Toml,
        InitFormat::Yaml,
    ] {
        with_temp_repo("starter", |standard_dir| {
            let path = write_starter_configuration_in_format(
                standard_dir,
                &languages,
                Some(&boundaries),
                format,
            )
            .expect("write starter");
            assert!(path.ends_with(format.file_name()));
            let text = fs::read_to_string(&path).expect("read starter");
            match format {
                InitFormat::Json => {
                    assert!(!text.contains("test_gap") && !text.contains("Optional:"));
                    serde_json::from_str::<serde_json::Value>(&text).expect("valid strict JSON");
                }
                InitFormat::Toml => {
                    assert!(text.starts_with("#:schema https://"));
                    verify_uncommented_starter("codeint.toml", &uncomment_hash_hints(&text));
                }
                InitFormat::Yaml => {
                    assert!(text.starts_with("# yaml-language-server: $schema=https://"));
                    verify_uncommented_starter("codeint.yaml", &uncomment_yaml_hints(&text));
                }
                InitFormat::Jsonc => {}
            }
            let parsed = read_configuration(standard_dir).expect("read starter");
            assert!(parsed.coverage_reports.is_empty());

            assert!(matches!(
                write_starter_configuration_in_format(standard_dir, &[], None, InitFormat::Toml),
                Err(StarterConfigurationError::AlreadyExists { .. })
            ));
        });

        with_temp_repo("adopt", |adopt_dir| {
            write_adoption_starter_configuration(adopt_dir, &languages, Some(&boundaries), format)
                .expect("write adoption starter");
            let adopted_path = write_adoption_zero_budgets(
                adopt_dir,
                &languages,
                Some(&boundaries),
                &recorded,
                format,
            )
            .expect("write adoption zero budgets");
            let adopted_text = fs::read_to_string(&adopted_path).expect("read adopted");
            if format == InitFormat::Json {
                serde_json::from_str::<serde_json::Value>(&adopted_text)
                    .expect("adopted strict JSON is valid JSON");
            }
            let adopted = read_configuration(adopt_dir).expect("read adopted");
            assert_eq!(
                adopted
                    .budgets
                    .budget(SourceSet::Production, Metric::Erosion)
                    .map(|budget| budget.ratio()),
                Some(0.0)
            );
            assert_eq!(
                adopted
                    .budgets
                    .dependency_budget(Metric::UnusedDependencies.key())
                    .map(|budget| budget.ratio()),
                Some(0.0)
            );
        });
    }
}

fn uncomment_hash_hints(text: &str) -> String {
    text.lines()
        .map(|line| {
            if line.starts_with("#:schema ") || line.starts_with("# Optional:") {
                line
            } else {
                line.strip_prefix("# ").unwrap_or(line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn uncomment_yaml_hints(text: &str) -> String {
    text.lines()
        .filter(|line| *line != "  production: {}")
        .map(|line| {
            if line.starts_with("# yaml-language-server:") || line.starts_with("# Optional:") {
                line.to_owned()
            } else if let Some(rest) = line.strip_prefix("  # ") {
                format!("  {rest}")
            } else {
                line.strip_prefix("# ").unwrap_or(line).to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}
