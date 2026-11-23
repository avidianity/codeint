use super::*;

#[test]
fn defaults_disable_near_clones_with_0_85_min_similarity() {
    let config = parse("{}").expect("empty config parses");

    assert!(!config.duplication.near_clones);
    assert_eq!(config.duplication.min_similarity.ratio(), 0.85);
}

#[test]
fn parses_valid_near_clones_and_min_similarity_bounds() {
    let enabled = parse(r#"{ "duplication": { "near_clones": true, "min_similarity": 1.0 } }"#)
        .expect("min_similarity 1.0 parses");
    let low = parse(r#"{ "duplication": { "near_clones": true, "min_similarity": 0.01 } }"#)
        .expect("min_similarity 0.01 parses");

    assert!(enabled.duplication.near_clones);
    assert_eq!(enabled.duplication.min_similarity.ratio(), 1.0);
    assert_eq!(low.duplication.min_similarity.ratio(), 0.01);
}

#[test]
fn rejects_min_similarity_outside_open_closed_unit_interval() {
    for invalid in ["0", "0.0", "-0.1", "1.01", "1.5"] {
        let text = format!(r#"{{ "duplication": {{ "min_similarity": {invalid} }} }}"#);
        let error = parse(&text).expect_err("invalid min_similarity must fail");
        assert!(
            matches!(
                error,
                ConfigurationFileError::OutOfExclusiveMinRange {
                    exclusive_minimum: 0.0,
                    maximum: 1.0,
                    ..
                }
            ),
            "unexpected error for {invalid}: {error}"
        );
    }
}

#[test]
fn effective_serialization_matches_default_when_set_to_defaults_and_differs_when_enabled() {
    let default_json = serde_json::to_string(&parse("{}").expect("default")).expect("json");
    let explicit_default_json = serde_json::to_string(
        &parse(r#"{ "duplication": { "near_clones": false, "min_similarity": 0.85 } }"#)
            .expect("explicit default"),
    )
    .expect("json");
    let enabled_json = serde_json::to_string(
        &parse(r#"{ "duplication": { "near_clones": true } }"#).expect("enabled"),
    )
    .expect("json");

    assert_eq!(default_json, explicit_default_json);
    assert_ne!(default_json, enabled_json);
}
