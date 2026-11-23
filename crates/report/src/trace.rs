use codeint_config::TraceFormat;
use codeint_metrics::{TraceError, TraceResult, trace_imports};
use codeint_model::{CodeModel, RepoPath};
use serde::Serialize;

use crate::ReportError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TraceReport {
    pub from: String,
    pub to: String,
    pub connected: bool,
    pub shortest_distance: Option<usize>,
    pub paths: Vec<Vec<String>>,
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
        trace_imports(model, from, to, all_shortest, max_paths).map(TraceReport::of)
    }

    pub fn of(result: TraceResult) -> TraceReport {
        TraceReport {
            from: result.from.to_string(),
            to: result.to.to_string(),
            connected: result.connected,
            shortest_distance: result.shortest_distance,
            paths: result
                .paths
                .into_iter()
                .map(|chain| chain.into_iter().map(|path| path.to_string()).collect())
                .collect(),
            truncated: result.truncated,
        }
    }

    pub fn render(&self, format: TraceFormat) -> Result<String, ReportError> {
        match format {
            TraceFormat::Toon => toon_format::encode_default(self).map_err(ReportError::Toon),
            TraceFormat::Json => serde_json::to_string_pretty(self).map_err(ReportError::Json),
            TraceFormat::Text => Ok(render_text(self)),
        }
    }
}

fn render_text(report: &TraceReport) -> String {
    if !report.connected {
        return format!(
            "No runtime import path from {} to {}",
            report.from, report.to
        );
    }
    let mut lines: Vec<String> = report
        .paths
        .iter()
        .map(|chain| chain.join(" -> "))
        .collect();
    if report.truncated {
        lines.push(format!("(truncated at {} paths)", report.paths.len()));
    }
    lines.join("\n")
}
