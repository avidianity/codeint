use codeint_config::ReportRenderFormat;
use serde_json::{Map, Value};

use crate::{DiffReport, Report, ReportError};

#[cfg(test)]
mod tests;

#[derive(Debug, thiserror::Error)]
pub enum SavedReportError {
    #[error("the input is not valid JSON")]
    InvalidJson(#[source] serde_json::Error),
    #[error("the saved report must be a JSON object")]
    NotAnObject,
    #[error("unsupported report schema version {found}; this codeint supports schema version 1")]
    UnsupportedSchemaVersion { found: String },
    #[error("the input is not a recognized codeint check or diff JSON report")]
    UnrecognizedReport,
    #[error("cannot parse the saved {kind} report")]
    InvalidPayload {
        kind: &'static str,
        #[source]
        source: serde_json::Error,
    },
}

#[derive(Debug)]
pub enum SavedReport {
    Check(Box<Report>),
    Diff(Box<DiffReport>),
}

impl SavedReport {
    pub fn from_json(text: &str) -> Result<SavedReport, SavedReportError> {
        let value: Value = serde_json::from_str(text).map_err(SavedReportError::InvalidJson)?;
        let object = value.as_object().ok_or(SavedReportError::NotAnObject)?;
        validate_schema_version(object)?;
        if is_diff_shape(object) {
            let mut report: DiffReport = serde_json::from_value(value).map_err(|source| {
                SavedReportError::InvalidPayload {
                    kind: "diff",
                    source,
                }
            })?;
            report.populate_external_metric_names();
            return Ok(SavedReport::Diff(Box::new(report)));
        }
        if is_check_shape(object) {
            let mut normalized = value;
            if let Some(map) = normalized.as_object_mut() {
                normalize_legacy_check_keys(map);
            }
            let mut report: Report = serde_json::from_value(normalized).map_err(|source| {
                SavedReportError::InvalidPayload {
                    kind: "check",
                    source,
                }
            })?;
            report.populate_external_metric_names();
            return Ok(SavedReport::Check(Box::new(report)));
        }
        Err(SavedReportError::UnrecognizedReport)
    }

    pub fn with_show_all(mut self, show_all: bool) -> SavedReport {
        match &mut self {
            SavedReport::Check(report) => report.set_show_all(show_all),
            SavedReport::Diff(report) => report.set_show_all(show_all),
        }
        self
    }

    pub fn render(&self, format: ReportRenderFormat) -> Result<String, ReportError> {
        let report_format = format.as_report_format();
        match self {
            SavedReport::Check(report) => report.render(report_format),
            SavedReport::Diff(report) => report.render(report_format),
        }
    }
}

fn validate_schema_version(object: &Map<String, Value>) -> Result<(), SavedReportError> {
    if let Some(version) = object.get("schema_version")
        && !is_supported_schema_version(version)
    {
        return Err(SavedReportError::UnsupportedSchemaVersion {
            found: format_schema_value(version),
        });
    }
    if let Some(schema) = object.get("$schema") {
        return Err(SavedReportError::UnsupportedSchemaVersion {
            found: format_schema_value(schema),
        });
    }
    if let Some(version) = object.get("version")
        && !is_supported_schema_version(version)
    {
        return Err(SavedReportError::UnsupportedSchemaVersion {
            found: format_schema_value(version),
        });
    }
    Ok(())
}

fn is_supported_schema_version(value: &Value) -> bool {
    value.as_u64() == Some(1) || value.as_str() == Some("1")
}

fn format_schema_value(value: &Value) -> String {
    match value.as_str() {
        Some(text) => format!("\"{text}\""),
        None => value.to_string(),
    }
}

fn is_diff_shape(object: &Map<String, Value>) -> bool {
    object.contains_key("base") && object.contains_key("head") && object.contains_key("source_sets")
}

fn is_check_shape(object: &Map<String, Value>) -> bool {
    object.contains_key("files")
        && object.contains_key("source_lines")
        && object.contains_key("source_sets")
}

fn normalize_legacy_check_keys(object: &mut Map<String, Value>) {
    migrate_legacy_array(
        object,
        "file_diagnostics",
        "rejected_statements",
        convert_legacy_rejected_statement,
    );
    migrate_legacy_array(
        object,
        "scope_summaries",
        "sql_schemas",
        convert_legacy_sql_schema,
    );
}

fn migrate_legacy_array(
    object: &mut Map<String, Value>,
    new_key: &str,
    old_key: &str,
    convert: fn(&Value) -> Option<Value>,
) {
    if !object.contains_key(new_key)
        && let Some(Value::Array(items)) = object.remove(old_key)
    {
        let converted: Vec<Value> = items.iter().filter_map(convert).collect();
        object.insert(new_key.to_owned(), Value::Array(converted));
    }
}

fn convert_legacy_rejected_statement(item: &Value) -> Option<Value> {
    let path = item.get("path")?.as_str()?;
    let line = item.get("line")?.as_u64()?;
    let dialect = item.get("dialect")?.as_str()?;
    Some(serde_json::json!({
        "path": path,
        "line": line,
        "kind": "rejected-statement",
        "message": format!("could not parse statement as {dialect}"),
    }))
}

fn convert_legacy_sql_schema(item: &Value) -> Option<Value> {
    let dialect = item.get("dialect")?.as_str()?;
    let tables = item.get("tables")?.as_u64()?;
    let mut entry = Map::new();
    entry.insert("language".to_owned(), Value::String("sql".to_owned()));
    if let Some(package) = item.get("package").and_then(Value::as_str) {
        entry.insert("package".to_owned(), Value::String(package.to_owned()));
    }
    entry.insert("dialect".to_owned(), Value::String(dialect.to_owned()));
    entry.insert("kind".to_owned(), Value::String("sql-schema".to_owned()));
    entry.insert(
        "fields".to_owned(),
        serde_json::json!({
            "dialect": dialect,
            "tables": tables.to_string(),
        }),
    );
    Some(Value::Object(entry))
}
