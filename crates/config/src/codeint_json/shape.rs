use serde_json::{Map, Value};

use super::{ConfigurationFileError, JsonType, KeyPath, KeySegment};

#[cfg(test)]
mod tests;

const DEFINITION_PREFIX: &str = "#/$defs/";

pub(super) fn check_shape(
    value: &Value,
    root_schema: &Value,
) -> Result<(), ConfigurationFileError> {
    Shape { root_schema }.check(value, root_schema, &KeyPath::default())
}

struct Shape<'schema> {
    root_schema: &'schema Value,
}

impl<'schema> Shape<'schema> {
    fn check(
        &self,
        value: &Value,
        schema: &'schema Value,
        key: &KeyPath,
    ) -> Result<(), ConfigurationFileError> {
        let schema = self.resolve(schema);
        self.check_type(value, schema, key)?;
        check_range(value, schema, key)?;
        match value {
            Value::Object(members) => self.check_members(members, schema, key),
            Value::Array(items) => self.check_items(items, schema, key),
            _ => Ok(()),
        }
    }

    fn resolve(&self, schema: &'schema Value) -> &'schema Value {
        let definition = schema
            .get("$ref")
            .and_then(Value::as_str)
            .and_then(|reference| reference.strip_prefix(DEFINITION_PREFIX))
            .and_then(|name| self.root_schema.get("$defs")?.get(name));
        definition.map_or(schema, |definition| self.resolve(definition))
    }

    fn check_type(
        &self,
        value: &Value,
        schema: &Value,
        key: &KeyPath,
    ) -> Result<(), ConfigurationFileError> {
        let expected = expected_types(schema);
        if expected.is_empty() || expected.iter().any(|json_type| json_type.accepts(value)) {
            Ok(())
        } else {
            Err(ConfigurationFileError::WrongType {
                key: key.clone(),
                expected,
                found: JsonType::of(value),
            })
        }
    }

    fn check_members(
        &self,
        members: &Map<String, Value>,
        schema: &'schema Value,
        key: &KeyPath,
    ) -> Result<(), ConfigurationFileError> {
        let properties = schema.get("properties").and_then(Value::as_object);
        let closed = schema.get("additionalProperties") == Some(&Value::Bool(false));
        for (name, member) in members {
            let member_key = key.child(KeySegment::Key(name.clone()));
            match properties.and_then(|properties| properties.get(name)) {
                Some(member_schema) => self.check(member, member_schema, &member_key)?,
                None if closed => {
                    return Err(ConfigurationFileError::UnknownKey {
                        key: member_key,
                        allowed: properties
                            .map(|properties| properties.keys().cloned().collect())
                            .unwrap_or_default(),
                    });
                }
                None => {}
            }
        }
        let missing = schema
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .find(|name| !members.contains_key(*name));
        match missing {
            Some(name) => Err(ConfigurationFileError::MissingKey {
                key: key.clone(),
                missing: name.to_owned(),
            }),
            None => Ok(()),
        }
    }

    fn check_items(
        &self,
        items: &[Value],
        schema: &'schema Value,
        key: &KeyPath,
    ) -> Result<(), ConfigurationFileError> {
        let Some(item_schema) = schema.get("items") else {
            return Ok(());
        };
        items.iter().enumerate().try_for_each(|(index, item)| {
            self.check(item, item_schema, &key.child(KeySegment::Index(index)))
        })
    }
}

fn check_range(value: &Value, schema: &Value, key: &KeyPath) -> Result<(), ConfigurationFileError> {
    let (Some(found), Some(minimum), Some(maximum)) = (
        value.as_f64(),
        schema.get("minimum").and_then(Value::as_f64),
        schema.get("maximum").and_then(Value::as_f64),
    ) else {
        return Ok(());
    };
    if (minimum..=maximum).contains(&found) {
        Ok(())
    } else {
        Err(ConfigurationFileError::OutOfRange {
            key: key.clone(),
            minimum,
            maximum,
            found,
        })
    }
}

fn expected_types(schema: &Value) -> Vec<JsonType> {
    match schema.get("type") {
        Some(Value::String(name)) => JsonType::from_schema_name(name).into_iter().collect(),
        Some(Value::Array(names)) => names
            .iter()
            .filter_map(Value::as_str)
            .filter_map(JsonType::from_schema_name)
            .collect(),
        _ => Vec::new(),
    }
}
