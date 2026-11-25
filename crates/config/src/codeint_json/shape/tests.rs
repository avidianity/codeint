use serde_json::Value;

use crate::configuration_schema;

const CHECKED_KEYWORDS: [&str; 12] = [
    "$ref",
    "$defs",
    "anyOf",
    "type",
    "properties",
    "additionalProperties",
    "items",
    "minimum",
    "exclusiveMinimum",
    "maximum",
    "required",
    "enum",
];

const ANNOTATION_KEYWORDS: [&str; 5] = ["$schema", "title", "description", "default", "format"];

fn check_nested_keyword(keyword: &str, value: &Value, found: &mut Vec<String>) -> bool {
    match keyword {
        "$defs" | "properties" => {
            for nested in value.as_object().into_iter().flat_map(|map| map.values()) {
                keywords_outside_the_checked_subset(nested, found);
            }
            true
        }
        "anyOf" => {
            for nested in value.as_array().into_iter().flatten() {
                keywords_outside_the_checked_subset(nested, found);
            }
            true
        }
        "items" => {
            keywords_outside_the_checked_subset(value, found);
            true
        }
        "additionalProperties" => {
            if value.is_object() {
                keywords_outside_the_checked_subset(value, found);
            } else if value != &Value::Bool(false) {
                found.push(keyword.to_owned());
            }
            true
        }
        _ => false,
    }
}

fn keywords_outside_the_checked_subset(schema: &Value, found: &mut Vec<String>) {
    let Some(members) = schema.as_object() else {
        return;
    };
    for (keyword, value) in members {
        if check_nested_keyword(keyword, value, found)
            || CHECKED_KEYWORDS.contains(&keyword.as_str())
            || ANNOTATION_KEYWORDS.contains(&keyword.as_str())
        {
            continue;
        }
        found.push(keyword.clone());
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
