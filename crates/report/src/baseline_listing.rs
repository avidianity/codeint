use std::collections::BTreeSet;

use codeint_config::{BaselineFormat, BaselineUpdateSummary, MeasuredBaselineMetric};
use codeint_model::{Metric, SourceSet};

use crate::baseline::{Identified, counted_findings};
use crate::render::{Findings, ReportError, render_text_report};
use crate::sections::{DependenciesSection, MeasuredSection, SourceSetSection};

pub(crate) trait BaselineListing {
    fn listed_ids(&self) -> BTreeSet<String>;
    fn hidden(&self) -> usize;
}

impl<T: Identified> BaselineListing for Findings<T> {
    fn listed_ids(&self) -> BTreeSet<String> {
        self.listed.iter().map(|row| row.id().to_owned()).collect()
    }

    fn hidden(&self) -> usize {
        self.baselined.hidden
    }
}

type ListingVisitor<'v> = dyn FnMut(&str, u32, &dyn BaselineListing) + 'v;
type ScopedListingVisitor<'v> = dyn FnMut(Option<SourceSet>, &str, u32, &dyn BaselineListing) + 'v;

macro_rules! visit_findings {
    ($visit:ident, $metric:expr, $section:expr) => {
        $visit(
            $metric.key(),
            $section.header.metric_version,
            &$section.findings,
        )
    };
}

macro_rules! visit_optional_findings {
    ($visit:ident, $metric:expr, $section:expr) => {
        if let Some(section) = &$section {
            visit_findings!($visit, $metric, section);
        }
    };
}

impl SourceSetSection {
    fn visit_listings(&self, visit: &mut ListingVisitor<'_>) {
        visit_findings!(visit, Metric::Erosion, self.erosion);
        visit_findings!(
            visit,
            Metric::CognitiveComplexity,
            self.cognitive_complexity
        );
        visit(
            Metric::Duplication.key(),
            self.duplication.header.metric_version,
            &self.duplication.clone_groups,
        );
        visit_findings!(visit, Metric::Verbosity, self.verbosity);
        visit_optional_findings!(visit, Metric::ImportCycles, self.import_cycles);
        visit_optional_findings!(visit, Metric::Boundaries, self.boundaries);
        visit_optional_findings!(visit, Metric::OrphanFiles, self.orphan_files);
        visit_optional_findings!(visit, Metric::TestGap, self.test_gap);
        visit_optional_findings!(visit, Metric::Hazards, self.hazards);
        for (key, section) in &self.external_metrics {
            visit(key, section.header.metric_version, &section.findings);
        }
    }
}

impl DependenciesSection {
    fn visit_listings(&self, visit: &mut ListingVisitor<'_>) {
        visit_findings!(visit, Metric::UnusedDependencies, self.unused_dependencies);
        visit_findings!(
            visit,
            Metric::UnlistedDependencies,
            self.unlisted_dependencies
        );
        visit_findings!(
            visit,
            Metric::DevInProductionDependencies,
            self.dev_in_production_dependencies
        );
    }
}

impl MeasuredSection {
    fn visit_listings(&self, visit: &mut ScopedListingVisitor<'_>) {
        for (source_set, section) in self.source_sets.iter() {
            section.visit_listings(&mut |metric, version, listing| {
                visit(Some(source_set), metric, version, listing);
            });
        }
        if let Some(dependencies) = &self.dependencies {
            dependencies.visit_listings(&mut |metric, version, listing| {
                visit(None, metric, version, listing);
            });
        }
    }

    pub(crate) fn hidden_by_baseline(&self) -> usize {
        let mut hidden = 0;
        self.visit_listings(&mut |_, _, _, listing| hidden += listing.hidden());
        hidden
    }

    pub(crate) fn baseline_metrics(&self) -> Vec<MeasuredBaselineMetric> {
        let mut metrics = Vec::new();
        self.visit_listings(&mut |source_set, metric, metric_version, listing| {
            metrics.push(MeasuredBaselineMetric {
                source_set,
                metric: metric.to_owned(),
                metric_version,
                findings: listing.listed_ids(),
            });
        });
        metrics
    }
}

pub fn render_baseline_update(
    summary: &BaselineUpdateSummary,
    format: BaselineFormat,
) -> Result<String, ReportError> {
    render_text_report(summary, format.into(), format_update_text)
}

fn format_update_text(summary: &BaselineUpdateSummary) -> String {
    let action = if summary.created {
        "Created"
    } else {
        "Updated"
    };
    let details: Vec<String> = [
        (summary.added, "added"),
        (summary.removed, "removed"),
        (summary.refreshed, "refreshed"),
    ]
    .into_iter()
    .filter(|(count, _)| *count > 0)
    .map(|(count, verb)| format!("{count} {verb}"))
    .collect();
    let counted = if details.is_empty() {
        String::new()
    } else {
        format!(": {}", details.join(", "))
    };
    let mut text = format!(
        "{action} {} ({}{counted})",
        summary.path,
        counted_findings(summary.total_findings)
    );
    if summary.skipped_new > 0 {
        text.push_str(&format!(
            "\n{} not added; codeint check still reports them. Fix them, or run codeint baseline update --reset to add them.",
            counted_findings(summary.skipped_new).replacen(' ', " new ", 1)
        ));
    }
    text
}
