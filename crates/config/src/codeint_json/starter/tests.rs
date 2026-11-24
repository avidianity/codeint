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
        let starter: Value = parse_jsonc(&text).expect("the starter configuration is valid JSONC");

        shape::check_shape(&starter, &schema).expect("the starter configuration fits the schema");
        let parsed = parse(&text).expect("the starter configuration parses cleanly");
        assert!(parsed.budgets.production_test_gap().is_none());
        assert!(parsed.coverage_reports.is_empty());
    }
}
