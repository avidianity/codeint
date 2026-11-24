use codeint_config::TraceFormat;
use codeint_metrics::{TraceError, TraceResult, trace_imports};
use codeint_model::{CodeModel, RepoPath, SourceSet};
use serde::Serialize;

use crate::ReportError;

const PROGRAM_START_ENTRY_POINT_REASON: &str = "program-start mark";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TraceMultiFileImport {
    pub importer: String,
    pub specifier: String,
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TraceReport {
    pub from: String,
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_entry_point: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_entry_point: Option<String>,
    pub connected: bool,
    pub shortest_distance: Option<usize>,
    pub paths: Vec<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub multi_file_imports: Vec<TraceMultiFileImport>,
    pub truncated: bool,
}

impl TraceReport {
    pub fn trace(
        model: &CodeModel,
        from: &RepoPath,
        to: &RepoPath,
        all_shortest: bool,
        max_paths: usize,
    ) -> Result<TraceReport, TraceError> {
        let mut report =
            trace_imports(model, from, to, all_shortest, max_paths).map(TraceReport::of)?;
        report.from_entry_point = program_start_reason(model, from);
        report.to_entry_point = program_start_reason(model, to);
        Ok(report)
    }

    pub fn of(result: TraceResult) -> TraceReport {
        TraceReport {
            from: result.from.to_string(),
            to: result.to.to_string(),
            from_entry_point: None,
            to_entry_point: None,
            connected: result.connected,
            shortest_distance: result.shortest_distance,
            paths: result
                .paths
                .into_iter()
                .map(|chain| chain.into_iter().map(|path| path.to_string()).collect())
                .collect(),
            multi_file_imports: result
                .multi_file_imports
                .into_iter()
                .map(|import| TraceMultiFileImport {
                    importer: import.importer.to_string(),
                    specifier: import.specifier,
                    targets: import
                        .targets
                        .into_iter()
                        .map(|target| target.to_string())
                        .collect(),
                })
                .collect(),
            truncated: result.truncated,
        }
    }

    pub fn render(&self, format: TraceFormat) -> Result<String, ReportError> {
        crate::render_text_report(self, format.into(), render_text)
    }
}

fn program_start_reason(model: &CodeModel, path: &RepoPath) -> Option<String> {
    let has_imports =
        model.languages.is_empty() || model.languages.iter().any(|facts| facts.has_imports);
    if !has_imports || codeint_metrics::is_non_runtime_tooling_path(model, path) {
        return None;
    }
    let marked_production = model.files.iter().any(|file| {
        &file.path == path
            && file.model.program_start
            && file.model.source_set == SourceSet::Production
    });
    marked_production.then(|| PROGRAM_START_ENTRY_POINT_REASON.to_owned())
}

fn render_text(report: &TraceReport) -> String {
    let mut lines = Vec::new();
    if let Some(reason) = &report.from_entry_point {
        lines.push(format!("Entry point: {} ({reason})", report.from));
    }
    if let Some(reason) = &report.to_entry_point
        && report.to != report.from
    {
        lines.push(format!("Entry point: {} ({reason})", report.to));
    }
    if !report.connected {
        lines.push(format!(
            "No runtime import path from {} to {}",
            report.from, report.to
        ));
        return lines.join("\n");
    }
    lines.extend(report.paths.iter().map(|chain| chain.join(" -> ")));
    for import in &report.multi_file_imports {
        lines.push(format!(
            "Import {} in {} leads to: {}",
            import.specifier,
            import.importer,
            import.targets.join(", ")
        ));
    }
    if report.truncated {
        lines.push(format!("(truncated at {} paths)", report.paths.len()));
    }
    lines.join("\n")
}
