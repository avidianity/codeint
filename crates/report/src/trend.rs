use std::collections::BTreeMap;

use codeint_config::{CalendarDay, TrendFormat};
use codeint_model::{Metric, SourceSet};
use serde::Serialize;

use crate::measurement::{CommitMeasurement, SourceSetMeasurement};
use crate::{Decimal, ReportError, TEST_GAP_TREND_NOTE};

mod markdown;

pub struct TrendPoint {
    pub commit: String,
    pub committed_on: CalendarDay,
    pub measurement: CommitMeasurement,
}

#[derive(Debug, Serialize)]
pub struct TrendReport {
    commits: usize,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    metric_versions: BTreeMap<String, u32>,
    source_sets: SourceSetTrends,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    test_gap_note: Option<String>,
}

#[derive(Debug, Default, Serialize)]
struct SourceSetTrends {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    production: Vec<TrendRow>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    test: Vec<TrendRow>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    generated: Vec<TrendRow>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    vendored: Vec<TrendRow>,
}

#[derive(Debug, Serialize)]
struct TrendRow {
    commit: String,
    committed_on: String,
    files: usize,
    erosion: Decimal,
    cognitive_complexity: Decimal,
    duplication: Decimal,
    verbosity: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    import_cycles: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hazards: Option<Decimal>,
    #[serde(flatten)]
    external_metrics: BTreeMap<String, Decimal>,
}

impl TrendReport {
    pub fn of(points_oldest_first: Vec<TrendPoint>) -> TrendReport {
        TrendReport::of_with_test_gap_note(points_oldest_first, false)
    }

    pub fn of_with_test_gap_note(
        points_oldest_first: Vec<TrendPoint>,
        include_test_gap_note: bool,
    ) -> TrendReport {
        let commits = points_oldest_first.len();
        let mut source_sets = SourceSetTrends::default();
        let mut external_versions = BTreeMap::new();
        for point in points_oldest_first {
            let committed_on = point.committed_on.to_string();
            for (source_set, measured) in point.measurement.into_source_sets() {
                for (key, entry) in &measured.external_metrics {
                    external_versions.insert(key.clone(), entry.metric_version);
                }
                source_sets.rows_of(source_set).push(TrendRow::of(
                    &point.commit,
                    &committed_on,
                    &measured,
                ));
            }
        }
        source_sets.leave_out_source_sets_without_files();
        let metric_versions = trend_metric_versions(&source_sets, &external_versions);
        TrendReport {
            commits,
            metric_versions,
            source_sets,
            test_gap_note: include_test_gap_note.then(|| TEST_GAP_TREND_NOTE.to_owned()),
        }
    }

    pub fn render(&self, format: TrendFormat) -> Result<String, ReportError> {
        match format {
            TrendFormat::Toon => crate::render_toon(self),
            TrendFormat::Json => crate::render_json(self),
            TrendFormat::Markdown => Ok(markdown::render(self)),
        }
    }
}

impl SourceSetTrends {
    fn rows_of(&mut self, source_set: SourceSet) -> &mut Vec<TrendRow> {
        match source_set {
            SourceSet::Production => &mut self.production,
            SourceSet::Test => &mut self.test,
            SourceSet::Generated => &mut self.generated,
            SourceSet::Vendored => &mut self.vendored,
        }
    }

    fn leave_out_source_sets_without_files(&mut self) {
        for rows in [
            &mut self.production,
            &mut self.test,
            &mut self.generated,
            &mut self.vendored,
        ] {
            if rows.iter().all(|row| row.files == 0) {
                rows.clear();
            }
        }
    }

    fn named(&self) -> Vec<(&'static str, &[TrendRow])> {
        [
            ("Production", self.production.as_slice()),
            ("Test", self.test.as_slice()),
            ("Generated", self.generated.as_slice()),
            ("Vendored", self.vendored.as_slice()),
        ]
        .into_iter()
        .filter(|(_, rows)| !rows.is_empty())
        .collect()
    }
}

fn trend_metric_versions(
    source_sets: &SourceSetTrends,
    external_versions: &BTreeMap<String, u32>,
) -> BTreeMap<String, u32> {
    let mut versions = BTreeMap::new();
    let all_rows = source_sets.named();
    if all_rows.is_empty() {
        return versions;
    }
    for metric in [
        Metric::Erosion,
        Metric::CognitiveComplexity,
        Metric::Duplication,
        Metric::Verbosity,
    ] {
        versions.insert(metric.key().to_owned(), metric.version());
    }
    let has_import_cycles = all_rows
        .iter()
        .flat_map(|(_, rows)| rows.iter())
        .any(|row| row.import_cycles.is_some());
    if has_import_cycles {
        versions.insert(
            Metric::ImportCycles.key().to_owned(),
            Metric::ImportCycles.version(),
        );
    }
    let has_hazards = all_rows
        .iter()
        .flat_map(|(_, rows)| rows.iter())
        .any(|row| row.hazards.is_some());
    if has_hazards {
        versions.insert(Metric::Hazards.key().to_owned(), Metric::Hazards.version());
    }
    for (key, version) in external_versions {
        versions.insert(key.clone(), *version);
    }
    versions
}

impl TrendRow {
    fn of(commit: &str, committed_on: &str, measured: &SourceSetMeasurement) -> TrendRow {
        TrendRow {
            commit: commit.to_owned(),
            committed_on: committed_on.to_owned(),
            files: measured.files,
            erosion: Decimal::rounded(measured.erosion.ratio),
            cognitive_complexity: Decimal::rounded(measured.cognitive_complexity.ratio),
            duplication: Decimal::rounded(measured.duplication.ratio),
            verbosity: Decimal::rounded(measured.verbosity.ratio),
            import_cycles: measured
                .import_cycles
                .as_ref()
                .map(|cycles| Decimal::rounded(cycles.ratio)),
            hazards: measured
                .hazards
                .as_ref()
                .map(|hazards| Decimal::rounded(hazards.ratio)),
            external_metrics: measured
                .external_metrics
                .iter()
                .map(|(key, entry)| (key.clone(), Decimal::rounded(entry.measurement.ratio)))
                .collect(),
        }
    }
}
