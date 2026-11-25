use codeint_model::RepoPath;
use serde_json::Value;

use super::{DetectedLanguage, starter_configuration_text};
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
    }];

    for languages in [&detected[..], &[]] {
        let text = starter_configuration_text(languages);
        assert!(text.contains("\"coverage_reports\": []"), "{text}");
        assert!(text.contains("// \"test_gap\": 0.2"), "{text}");
        assert!(!text.contains("sql.dialects"), "{text}");
        let starter: Value = parse_jsonc(&text).expect("the starter configuration is valid JSONC");

        shape::check_shape(&starter, &schema).expect("the starter configuration fits the schema");
        let parsed = parse(&text).expect("the starter configuration parses cleanly");
        assert!(parsed.budgets.production_test_gap().is_none());
        assert!(parsed.coverage_reports.is_empty());
        assert!(parsed.sql.is_empty());
    }
}

#[test]
fn the_starter_configuration_mentions_optional_sql_dialects_only_when_sql_is_detected() {
    let schema = published_schema();
    let detected = [DetectedLanguage {
        plugin: "sql".to_owned(),
        include: vec![path("db")],
    }];

    let text = starter_configuration_text(&detected);
    assert!(text.contains("sql.dialects"), "{text}");
    assert!(text.contains("// \"sql\": { \"dialects\": [] }"), "{text}");
    let starter: Value = parse_jsonc(&text).expect("the starter configuration is valid JSONC");
    shape::check_shape(&starter, &schema).expect("the starter configuration fits the schema");
    let parsed = parse(&text).expect("the starter configuration parses cleanly");
    assert!(parsed.sql.is_empty());
}

fn uncomment_boundaries_hint(text: &str) -> String {
    let mut output = String::new();
    for line in text.lines() {
        if line == "  \"coverage_reports\": []" {
            output.push_str("  \"coverage_reports\": [],\n");
        } else if let Some(rest) = line.strip_prefix("  // ")
            && !rest.starts_with("Optional:")
            && !rest.starts_with("\"sql\":")
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
        },
        DetectedLanguage {
            plugin: "sql".to_owned(),
            include: vec![path("db")],
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
        let text = starter_configuration_text_with_boundaries(&detected, Some(&boundaries));
        assert!(text.contains("sql.dialects"), "{text}");
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
