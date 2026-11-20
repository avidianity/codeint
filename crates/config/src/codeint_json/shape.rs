use serde_json::Value;

use super::{ConfigurationFileError, KeyPath};

pub(super) fn check_shape(
    value: &Value,
    schema: &Value,
    key: &KeyPath,
) -> Result<(), ConfigurationFileError> {
    todo!("{value} {schema} {key}")
}
