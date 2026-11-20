use codeint_config::{CalendarDay, TrendFormat};
use codeint_model::SourceSet;
use serde::Serialize;

use crate::measurement::{CommitMeasurement, SourceSetMeasurement};
use crate::{Decimal, ReportError};

mod markdown;

pub struct TrendPoint {
    pub commit: String,
    pub committed_on: CalendarDay,
    pub measurement: CommitMeasurement,
}

#[derive(Debug, Serialize)]
pub struct TrendReport {
    commits: usize,
    source_sets: SourceSetTrends,
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
    duplication: Decimal,
    verbosity: Decimal,
    import_cycles: Decimal,
}

impl TrendReport {
    pub fn of(points_oldest_first: Vec<TrendPoint>) -> TrendReport {
        let commits = points_oldest_first.len();
        let mut source_sets = SourceSetTrends::default();
        for point in points_oldest_first {
            let committed_on = point.committed_on.to_string();
            for (source_set, measured) in point.measurement.into_source_sets() {
                source_sets.rows_of(source_set).push(TrendRow::of(
                    &point.commit,
                    &committed_on,
                    &measured,
                ));
            }
        }
        source_sets.leave_out_source_sets_without_files();
        TrendReport {
            commits,
            source_sets,
        }
    }

    pub fn render(&self, format: TrendFormat) -> Result<String, ReportError> {
        match format {
            TrendFormat::Toon => toon_format::encode_default(self).map_err(ReportError::Toon),
            TrendFormat::Json => serde_json::to_string_pretty(self).map_err(ReportError::Json),
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

impl TrendRow {
    fn of(commit: &str, committed_on: &str, measured: &SourceSetMeasurement) -> TrendRow {
        TrendRow {
            commit: commit.to_owned(),
            committed_on: committed_on.to_owned(),
            files: measured.files,
            erosion: Decimal::rounded(measured.erosion.ratio),
            duplication: Decimal::rounded(measured.duplication.ratio),
            verbosity: Decimal::rounded(measured.verbosity.ratio),
            import_cycles: Decimal::rounded(measured.import_cycles.ratio),
        }
    }
}
