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
            let mut report: Report = serde_json::from_value(value).map_err(|source| {
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
