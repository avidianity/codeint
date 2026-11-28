use std::fmt;

use serde::Deserialize;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

use crate::config_error::{ConfigFileKind, ConfigurationFileError, KeyPath, KeySegment};

pub(super) fn parse_toml_value(
    text: &str,
    file: ConfigFileKind,
) -> Result<Value, ConfigurationFileError> {
    let without_bom = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let table: toml::Table =
        toml::from_str(without_bom).map_err(|error| ConfigurationFileError::InvalidToml {
            file,
            message: error.to_string().trim().to_owned(),
        })?;
    toml_table_to_json(table, file, &KeyPath::default())
}

fn toml_table_to_json(
    table: toml::Table,
    file: ConfigFileKind,
    key: &KeyPath,
) -> Result<Value, ConfigurationFileError> {
    let mut object = Map::with_capacity(table.len());
    for (name, value) in table {
        let child_key = key.child(KeySegment::Key(name.clone()));
        let converted = toml_value_to_json(value, file, &child_key)?;
        object.insert(name, converted);
    }
    Ok(Value::Object(object))
}

fn toml_value_to_json(
    value: toml::Value,
    file: ConfigFileKind,
    key: &KeyPath,
) -> Result<Value, ConfigurationFileError> {
    match value {
        toml::Value::String(text) => Ok(Value::String(text)),
        toml::Value::Integer(number) => Ok(Value::Number(number.into())),
        toml::Value::Float(number) => {
            Number::from_f64(number).map(Value::Number).ok_or_else(|| {
                ConfigurationFileError::InvalidToml {
                    file,
                    message: format!("{key}: non-finite float is not supported"),
                }
            })
        }
        toml::Value::Boolean(flag) => Ok(Value::Bool(flag)),
        toml::Value::Datetime(_) => Err(ConfigurationFileError::UnsupportedTomlDatetime {
            file,
            key: key.clone(),
        }),
        toml::Value::Array(items) => toml_array_to_json(items, file, key),
        toml::Value::Table(table) => toml_table_to_json(table, file, key),
    }
}

fn toml_array_to_json(
    items: Vec<toml::Value>,
    file: ConfigFileKind,
    key: &KeyPath,
) -> Result<Value, ConfigurationFileError> {
    let mut values = Vec::with_capacity(items.len());
    for (index, item) in items.into_iter().enumerate() {
        let child_key = key.child(KeySegment::Index(index));
        values.push(toml_value_to_json(item, file, &child_key)?);
    }
    Ok(Value::Array(values))
}

pub(super) fn parse_yaml_value(
    text: &str,
    file: ConfigFileKind,
) -> Result<Value, ConfigurationFileError> {
    let without_bom = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    if is_blank_or_comment_only_yaml(without_bom) {
        return Ok(Value::Object(Map::new()));
    }
    let parsed: StrictYamlValue =
        serde_saphyr::from_str_with_options(without_bom, strict_yaml_options()).map_err(
            |error| ConfigurationFileError::InvalidYaml {
                file,
                message: error.to_string().trim().to_owned(),
            },
        )?;
    Ok(parsed.0)
}

fn is_blank_or_comment_only_yaml(text: &str) -> bool {
    text.lines().all(|line| {
        let trimmed = line.trim();
        trimmed.is_empty() || trimmed.starts_with('#')
    })
}

fn strict_yaml_options() -> serde_saphyr::Options {
    let mut budget = serde_saphyr::Budget::default();
    budget.max_documents = 1;
    budget.max_anchors = 0;
    budget.max_aliases = 0;
    budget.max_merge_keys = 0;
    let mut options = serde_saphyr::Options::default();
    options.budget = Some(budget);
    options.duplicate_keys = serde_saphyr::DuplicateKeyPolicy::Error;
    options.merge_keys = serde_saphyr::MergeKeyPolicy::Error;
    options.non_finite_float_policy = serde_saphyr::NonFiniteFloatPolicy::Reject;
    options.strict_booleans = true;
    options.reject_unsupported_tags = true;
    options.with_snippet = false;
    options
}

struct StrictYamlValue(Value);

impl<'de> Deserialize<'de> for StrictYamlValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer
            .deserialize_any(StrictYamlValueVisitor)
            .map(StrictYamlValue)
    }
}

struct StrictYamlValueVisitor;

impl<'de> Visitor<'de> for StrictYamlValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a YAML value")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite float is not supported"))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
        Ok(Value::String(value))
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element::<StrictYamlValue>()? {
            items.push(item.0);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut object = Map::new();
        while let Some((StrictYamlValue(key), StrictYamlValue(value))) = map.next_entry()? {
            let Value::String(key) = key else {
                return Err(de::Error::custom("YAML mapping keys must be strings"));
            };
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }
}
