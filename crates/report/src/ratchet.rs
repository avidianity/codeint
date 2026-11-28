use std::fmt;

use codeint_config::{
    MeasuredRatchetMetric, RatchetFormat, RatchetUpdateSummary, RatchetViolation,
};
use serde::{Deserialize, Serialize};

use crate::{Decimal, ReportError, render_text_report};

pub const RATCHET_PACKAGE_SKIPPED_NOTE: &str = "Ratchet enforcement skipped because --package scopes the run to one Workspace package; Ratchets apply to whole-repository runs only.";

pub(crate) fn push_ratchet_entries<'a>(
    out: &mut Vec<MeasuredRatchetMetric>,
    source_set: Option<codeint_model::SourceSet>,
    entries: impl IntoIterator<Item = (&'a str, f64, u32)>,
) {
    out.extend(
        entries
            .into_iter()
            .map(|(key, ratio, metric_version)| MeasuredRatchetMetric {
                source_set,
                metric: key.to_owned(),
                ratio,
                metric_version,
            }),
    );
}

pub(crate) trait RatchetMetricsSource {
    fn push_ratchet_metrics(
        &self,
        source_set: Option<codeint_model::SourceSet>,
        out: &mut Vec<MeasuredRatchetMetric>,
    );
}

pub(crate) fn collect_ratchet_metrics<S: RatchetMetricsSource, D: RatchetMetricsSource>(
    source_sets: &crate::PerSourceSet<S>,
    dependencies: Option<&D>,
) -> Vec<MeasuredRatchetMetric> {
    let mut metrics = Vec::new();
    for (source_set, section) in source_sets.iter() {
        section.push_ratchet_metrics(Some(source_set), &mut metrics);
    }
    if let Some(deps) = dependencies {
        deps.push_ratchet_metrics(None, &mut metrics);
    }
    metrics
}

pub(crate) fn evaluate_ratchet(
    ratchet: &codeint_config::Ratchet,
    measured: &[MeasuredRatchetMetric],
) -> Result<Vec<RatchetFailure>, Vec<codeint_config::RatchetVersionMismatch>> {
    Ok(ratchet
        .check(measured)?
        .into_iter()
        .map(RatchetFailure::from)
        .collect())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RatchetFailure {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) source_set: Option<String>,
    pub(crate) metric: String,
    pub(crate) ratio: Decimal,
    pub(crate) ceiling: Decimal,
}

impl From<RatchetViolation> for RatchetFailure {
    fn from(violation: RatchetViolation) -> RatchetFailure {
        RatchetFailure {
            source_set: violation
                .source_set
                .map(|source_set| source_set.key().to_owned()),
            metric: violation.metric,
            ratio: Decimal::rounded(violation.ratio),
            ceiling: Decimal::rounded(violation.ceiling),
        }
    }
}

impl fmt::Display for RatchetFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.source_set.as_deref() {
            Some(source_set) => write!(
                formatter,
                "the {source_set} {} ratio {} exceeds its Ratchet ceiling {}",
                self.metric, self.ratio.0, self.ceiling.0
            ),
            None => write!(
                formatter,
                "the {} ratio {} exceeds its Ratchet ceiling {}",
                self.metric, self.ratio.0, self.ceiling.0
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RatchetUpdateReport {
    #[serde(flatten)]
    summary: RatchetUpdateSummary,
}

impl RatchetUpdateReport {
    pub fn from_summary(summary: RatchetUpdateSummary) -> RatchetUpdateReport {
        RatchetUpdateReport { summary }
    }

    pub fn summary(&self) -> &RatchetUpdateSummary {
        &self.summary
    }

    pub fn render(&self, format: RatchetFormat) -> Result<String, ReportError> {
        render_text_report(self, format.into(), RatchetUpdateReport::format_text)
    }

    fn format_text(&self) -> String {
        let s = &self.summary;
        let action = if s.created { "Created" } else { "Updated" };
        let mut details = Vec::new();
        if s.added > 0 {
            details.push(format!("{} added", s.added));
        }
        if s.tightened > 0 {
            details.push(format!("{} tightened", s.tightened));
        }
        if s.loosened > 0 {
            details.push(format!("{} loosened", s.loosened));
        }
        if s.refreshed > 0 {
            details.push(format!("{} refreshed", s.refreshed));
        }
        if s.unchanged > 0 {
            details.push(format!("{} unchanged", s.unchanged));
        }
        if s.removed > 0 {
            details.push(format!("{} removed", s.removed));
        }
        let mut text = if details.is_empty() {
            format!("{action} {} ({} entries)", s.path, s.total_entries)
        } else {
            format!(
                "{action} {} ({} entries: {})",
                s.path,
                s.total_entries,
                details.join(", ")
            )
        };
        if s.replaced_invalid {
            text.push_str(&format!(
                "\nReplaced invalid {}; the previous content is recoverable from Git.",
                s.path
            ));
        }
        text
    }
}
