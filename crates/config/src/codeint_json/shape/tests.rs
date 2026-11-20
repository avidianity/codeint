use serde_json::Value;

use crate::configuration_schema;

const CHECKED_KEYWORDS: [&str; 8] = [
    "$ref",
    "$defs",
    "type",
    "properties",
    "additionalProperties",
    "items",
    "minimum",
    "maximum",
];

const ANNOTATION_KEYWORDS: [&str; 5] = ["$schema", "title", "description", "default", "format"];

fn keywords_outside_the_checked_subset(schema: &Value, found: &mut Vec<String>) {
    let Some(members) = schema.as_object() else {
        return;
    };
    for (keyword, value) in members {
        match keyword.as_str() {
            "$defs" | "properties" => {
                for nested in value.as_object().into_iter().flat_map(|map| map.values()) {
                    keywords_outside_the_checked_subset(nested, found);
                }
            }
            "items" => keywords_outside_the_checked_subset(value, found),
            "additionalProperties" if value != &Value::Bool(false) => found.push(keyword.clone()),
            keyword if CHECKED_KEYWORDS.contains(&keyword) => {}
            keyword if ANNOTATION_KEYWORDS.contains(&keyword) => {}
            keyword => found.push(keyword.to_owned()),
        }
    }
}

#[test]
fn the_shape_check_understands_every_keyword_of_the_configuration_schema() {
    let schema: Value =
        serde_json::from_str(&configuration_schema()).expect("the schema is valid JSON");
    let mut unchecked = Vec::new();

    keywords_outside_the_checked_subset(&schema, &mut unchecked);

    assert_eq!(unchecked, Vec::<String>::new());
}
