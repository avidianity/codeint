use codeint_model::{Metric, RepoPath, SourceSet};
use serde_json::Value;

use super::{
    DetectedLanguage, StarterKind, ZeroBudgetKeys, starter_configuration_text,
    starter_configuration_text_with_boundaries, zero_budget_keys,
};
use crate::baseline::MeasuredBaselineMetric;
use crate::codeint_json::{configuration_schema, parse, parse_jsonc, shape};

fn path(text: &str) -> RepoPath {
    RepoPath::new(text).expect("a repository-relative path")
}

fn published_schema() -> Value {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schema/codeint.schema.json"
    ))
    .expect("read the published schema");
    serde_json::from_str(&text).expect("the published schema is JSON")
}

#[test]
fn the_starter_configuration_passes_the_shape_check_of_the_published_schema_with_and_without_languages()
 {
    let schema = published_schema();
    assert_eq!(
        schema,
        serde_json::from_str::<Value>(&configuration_schema()).expect("the schema is JSON"),
        "the published schema is current"
    );
    let detected = [DetectedLanguage {
        plugin: "typescript-javascript".to_owned(),
        include: vec![path("[draft]"), path("src")],
        known_dialects: Vec::new(),
    }];

    for languages in [&detected[..], &[]] {
        let text = starter_configuration_text(languages);
        assert!(text.contains("\"coverage_reports\": []"), "{text}");
        assert!(text.contains("// \"test_gap\": 0.2"), "{text}");
        assert!(!text.contains("dialects."), "{text}");
        let starter: Value = parse_jsonc(&text).expect("the starter configuration is valid JSONC");

        shape::check_shape(&starter, &schema).expect("the starter configuration fits the schema");
        let parsed = parse(&text).expect("the starter configuration parses cleanly");
        assert!(parsed.budgets.production_test_gap().is_none());
        assert!(parsed.coverage_reports.is_empty());
        assert!(parsed.dialects.is_empty());
    }
}

#[test]
fn the_starter_configuration_mentions_optional_dialects_only_when_detected_language_declares_them()
{
    let schema = published_schema();
    let detected = [DetectedLanguage {
        plugin: "sql".to_owned(),
        include: vec![path("db")],
        known_dialects: vec![
            "postgresql".to_owned(),
            "mysql".to_owned(),
            "sqlite".to_owned(),
            "duckdb".to_owned(),
        ],
    }];

    let text = starter_configuration_text(&detected);
    assert!(text.contains("dialects.sql"), "{text}");
    assert!(text.contains("// \"dialects\": { \"sql\": [] }"), "{text}");
    let starter: Value = parse_jsonc(&text).expect("the starter configuration is valid JSONC");
    shape::check_shape(&starter, &schema).expect("the starter configuration fits the schema");
    let parsed = parse(&text).expect("the starter configuration parses cleanly");
    assert!(parsed.dialects.is_empty());
}

fn uncomment_boundaries_hint(text: &str) -> String {
    let mut output = String::new();
    for line in text.lines() {
        if line == "  \"coverage_reports\": []" {
            output.push_str("  \"coverage_reports\": [],\n");
        } else if let Some(rest) = line.strip_prefix("  // ")
            && !rest.starts_with("Optional:")
            && !rest.starts_with("\"dialects\":")
        {
            output.push_str("  ");
            output.push_str(rest);
            output.push('\n');
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

#[test]
fn starter_boundaries_workspace_packages_requires_two_packages_disambiguates_and_escapes_globs() {
    use super::{StarterBoundaries, StarterBoundaryZone};

    assert_eq!(StarterBoundaries::from_workspace_packages(&[]), None);
    assert_eq!(
        StarterBoundaries::from_workspace_packages(&[path("packages/only")]),
        None
    );

    let simple =
        StarterBoundaries::from_workspace_packages(&[path("packages/web"), path("packages/core")])
            .expect("two packages produce starter zones");
    assert_eq!(
        simple,
        StarterBoundaries::WorkspaceZones(vec![
            StarterBoundaryZone {
                name: "core".to_owned(),
                pattern: "packages/core/**".to_owned(),
            },
            StarterBoundaryZone {
                name: "web".to_owned(),
                pattern: "packages/web/**".to_owned(),
            },
        ])
    );

    let colliding = StarterBoundaries::from_workspace_packages(&[
        path("apps/web"),
        path("packages/web"),
        path("packages/[draft]"),
    ])
    .expect("colliding basenames fall back to full package roots");
    assert_eq!(
        colliding,
        StarterBoundaries::WorkspaceZones(vec![
            StarterBoundaryZone {
                name: "[draft]".to_owned(),
                pattern: "packages/[[]draft[]]/**".to_owned(),
            },
            StarterBoundaryZone {
                name: "apps/web".to_owned(),
                pattern: "apps/web/**".to_owned(),
            },
            StarterBoundaryZone {
                name: "packages/web".to_owned(),
                pattern: "packages/web/**".to_owned(),
            },
        ])
    );
}

#[test]
fn boundary_preset_detect_picks_higher_zone_count_and_breaks_ties_deterministically() {
    use crate::boundaries::BoundaryPreset;

    assert_eq!(
        BoundaryPreset::detect(&[path("src/main.ts"), path("src/utils/a.ts")]),
        None
    );
    assert_eq!(
        BoundaryPreset::detect(&[path("src/domain/order.ts"), path("src/utils/a.ts")]),
        None
    );
    assert_eq!(
        BoundaryPreset::detect(&[
            path("src/presentation/http.ts"),
            path("src/domain/order.ts"),
            path("src/infrastructure/db.ts"),
        ]),
        Some(BoundaryPreset::Layered)
    );
    assert_eq!(
        BoundaryPreset::detect(&[
            path("src/domain/order.ts"),
            path("src/ports/store.ts"),
            path("src/adapters/pg.ts"),
        ]),
        Some(BoundaryPreset::Hexagonal)
    );
    assert_eq!(
        BoundaryPreset::detect(&[path("src/domain/order.ts"), path("src/adapters/pg.ts")]),
        Some(BoundaryPreset::Hexagonal)
    );
    assert_eq!(
        BoundaryPreset::detect(&[
            path("src/domain/order.ts"),
            path("src/infrastructure/pg.ts"),
        ]),
        Some(BoundaryPreset::Layered)
    );
    assert_eq!(
        BoundaryPreset::detect(&[
            path("src/domain/order.ts"),
            path("src/application/checkout.ts"),
        ]),
        Some(BoundaryPreset::Layered)
    );
}

#[test]
fn starter_configuration_with_commented_boundaries_stays_valid_jsonc_and_activates_cleanly_when_uncommented()
 {
    use super::{StarterBoundaries, starter_configuration_text_with_boundaries};
    use crate::boundaries::BoundaryPreset;

    let schema = published_schema();
    let detected = [
        DetectedLanguage {
            plugin: "typescript-javascript".to_owned(),
            include: vec![path("packages")],
            known_dialects: Vec::new(),
        },
        DetectedLanguage {
            plugin: "sql".to_owned(),
            include: vec![path("db")],
            known_dialects: vec!["postgresql".to_owned()],
        },
    ];
    let workspace_boundaries =
        StarterBoundaries::from_workspace_packages(&[path("packages/core"), path("packages/web")])
            .expect("two packages");

    for boundaries in [
        workspace_boundaries,
        StarterBoundaries::Preset(BoundaryPreset::Layered),
        StarterBoundaries::Preset(BoundaryPreset::Hexagonal),
    ] {
        let text = starter_configuration_text_with_boundaries(
            &detected,
            Some(&boundaries),
            StarterKind::Standard,
            &ZeroBudgetKeys::none(),
        );
        assert!(text.contains("dialects.sql"), "{text}");
        let starter: Value = parse_jsonc(&text).expect("starter with boundaries is valid JSONC");
        shape::check_shape(&starter, &schema).expect("starter fits schema");
        let parsed = parse(&text).expect("starter parses cleanly");
        assert!(parsed.boundaries.is_none());

        let uncommented = uncomment_boundaries_hint(&text);
        let active_value: Value =
            parse_jsonc(&uncommented).expect("uncommented boundaries is valid JSONC");
        shape::check_shape(&active_value, &schema).expect("uncommented boundaries fits schema");
        let active = parse(&uncommented).expect("uncommented boundaries parses cleanly");
        assert!(active.boundaries.is_some());
    }
}

#[test]
fn the_adoption_starter_enables_per_file_and_keeps_new_file_budgets_commented_and_valid() {
    let schema = published_schema();

    let text = starter_configuration_text_with_boundaries(
        &[],
        None,
        StarterKind::Adoption,
        &ZeroBudgetKeys::none(),
    );

    assert!(text.contains("// \"new_file_budgets\": {"), "{text}");
    let starter: Value = parse_jsonc(&text).expect("the adoption starter is valid JSONC");
    assert_eq!(
        starter["regression_limits"],
        serde_json::json!({ "per_file": true })
    );
    assert!(starter.get("new_file_budgets").is_none());
    shape::check_shape(&starter, &schema).expect("the adoption starter fits the schema");
}

#[test]
fn the_adoption_starter_writes_zero_budgets_for_recorded_metrics_and_keeps_test_gap_commented() {
    let schema = published_schema();
    let zero_budgets = ZeroBudgetKeys {
        production: vec!["erosion", "hazards"],
        test: vec!["erosion"],
        dependencies: vec!["unused_dependencies"],
    };

    let text =
        starter_configuration_text_with_boundaries(&[], None, StarterKind::Adoption, &zero_budgets);

    let starter: Value = parse_jsonc(&text).expect("the adoption starter is valid JSONC");
    assert_eq!(
        starter["budgets"]["production"],
        serde_json::json!({ "erosion": 0, "hazards": 0 })
    );
    assert_eq!(
        starter["budgets"]["test"],
        serde_json::json!({ "erosion": 0 })
    );
    assert_eq!(
        starter["budgets"]["unused_dependencies"],
        serde_json::json!(0)
    );
    assert!(text.contains("// \"test_gap\": 0.2,"), "{text}");
    shape::check_shape(&starter, &schema).expect("the adoption starter fits the schema");
    let activated = text.replace("// \"test_gap\": 0.2,", "\"test_gap\": 0.2,");
    parse(&activated).expect("uncommenting test_gap keeps the file valid");
}

#[test]
fn zero_budget_keys_cover_every_recorded_metric_of_the_registry_except_test_gap() {
    let recorded: Vec<MeasuredBaselineMetric> = Metric::ALL
        .into_iter()
        .flat_map(|metric| {
            [
                Some(SourceSet::Production),
                Some(SourceSet::Test),
                Some(SourceSet::Generated),
                None,
            ]
            .map(|source_set| MeasuredBaselineMetric {
                source_set,
                metric: metric.key().to_owned(),
                metric_version: metric.version(),
                findings: Vec::new(),
            })
        })
        .collect();

    let keys = zero_budget_keys(&recorded);

    assert_eq!(
        keys.production,
        vec![
            "erosion",
            "cognitive_complexity",
            "duplication",
            "verbosity",
            "import_cycles",
            "boundaries",
            "orphan_files",
            "hazards"
        ]
    );
    assert_eq!(
        keys.test,
        vec![
            "erosion",
            "cognitive_complexity",
            "duplication",
            "verbosity",
            "import_cycles",
            "boundaries",
            "hazards"
        ]
    );
    assert_eq!(
        keys.dependencies,
        vec![
            "unused_dependencies",
            "unlisted_dependencies",
            "dev_in_production_dependencies"
        ]
    );
}
