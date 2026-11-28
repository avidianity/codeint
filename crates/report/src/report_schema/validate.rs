use serde_json::Value;

use crate::ReportSchemaKind;

pub fn validate_report_json(kind: ReportSchemaKind, value: &Value) -> Result<(), String> {
    let schema = super::schema_value_for(kind);
    validate_json_against_schema(value, &schema)
}

pub fn validate_json_against_schema(value: &Value, schema: &Value) -> Result<(), String> {
    validate_value(value, schema, schema, "$")
}

fn validate_value(value: &Value, schema: &Value, root: &Value, path: &str) -> Result<(), String> {
    if let Some(allowed) = schema.as_bool() {
        return check_bool_schema(allowed, value, path);
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return check_ref(value, reference, root, path);
    }
    if let Some(branches) = schema.get("oneOf").and_then(Value::as_array) {
        return check_one_of(value, branches, root, path);
    }
    check_json_type(value, schema, path)?;
    check_enum(value, schema, path)?;
    check_number_bounds(value, schema, path)?;
    check_object_schema(value, schema, root, path)?;
    check_array_schema(value, schema, root, path)
}

fn check_bool_schema(allowed: bool, value: &Value, path: &str) -> Result<(), String> {
    if allowed {
        Ok(())
    } else {
        Err(format!("value disallowed by schema at {path}: {value}"))
    }
}

fn check_ref(value: &Value, reference: &str, root: &Value, path: &str) -> Result<(), String> {
    let def_name = reference
        .strip_prefix("#/$defs/")
        .ok_or_else(|| format!("non-local $ref {reference} at {path}"))?;
    let target = root
        .get("$defs")
        .and_then(|defs| defs.get(def_name))
        .ok_or_else(|| format!("missing $defs entry {def_name:?} at {path}"))?;
    validate_value(value, target, root, path)
}

fn check_one_of(value: &Value, branches: &[Value], root: &Value, path: &str) -> Result<(), String> {
    let matches = branches
        .iter()
        .filter(|branch| validate_value(value, branch, root, path).is_ok())
        .count();
    if matches == 1 {
        Ok(())
    } else {
        Err(format!(
            "expected exactly 1 oneOf match at {path}, got {matches} for {value}"
        ))
    }
}

fn check_json_type(value: &Value, schema: &Value, path: &str) -> Result<(), String> {
    let Some(expected_type) = schema.get("type").and_then(Value::as_str) else {
        return Ok(());
    };
    let ok = match expected_type {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.is_u64() || value.is_i64(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        other => return Err(format!("unsupported schema type {other} at {path}")),
    };
    if ok {
        Ok(())
    } else {
        Err(format!("expected {expected_type} at {path}, got {value}"))
    }
}

fn check_enum(value: &Value, schema: &Value, path: &str) -> Result<(), String> {
    if let Some(allowed) = schema.get("enum").and_then(Value::as_array)
        && !allowed.contains(value)
    {
        return Err(format!("value {value} not in enum {allowed:?} at {path}"));
    }
    Ok(())
}

fn check_number_bounds(value: &Value, schema: &Value, path: &str) -> Result<(), String> {
    let Some(number) = value.as_f64() else {
        return Ok(());
    };
    if let Some(min) = schema.get("minimum").and_then(Value::as_f64)
        && number < min
    {
        return Err(format!("value {number} below minimum {min} at {path}"));
    }
    if let Some(max) = schema.get("maximum").and_then(Value::as_f64)
        && number > max
    {
        return Err(format!("value {number} above maximum {max} at {path}"));
    }
    Ok(())
}

fn check_object_schema(
    value: &Value,
    schema: &Value,
    root: &Value,
    path: &str,
) -> Result<(), String> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    for key in schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let key_str = key
            .as_str()
            .ok_or_else(|| "required key string".to_owned())?;
        if !object.contains_key(key_str) {
            return Err(format!("missing required key {key_str:?} at {path}"));
        }
    }
    let properties = schema.get("properties").and_then(Value::as_object);
    for (key, child) in object {
        let child_path = format!("{path}.{key}");
        let child_schema = properties
            .and_then(|props| props.get(key))
            .or_else(|| schema.get("additionalProperties"));
        if let Some(target) = child_schema {
            validate_value(child, target, root, &child_path)?;
        }
    }
    Ok(())
}

fn check_array_schema(
    value: &Value,
    schema: &Value,
    root: &Value,
    path: &str,
) -> Result<(), String> {
    let (Some(items), Some(item_schema)) = (value.as_array(), schema.get("items")) else {
        return Ok(());
    };
    for (index, item) in items.iter().enumerate() {
        validate_value(item, item_schema, root, &format!("{path}[{index}]"))?;
    }
    Ok(())
}
