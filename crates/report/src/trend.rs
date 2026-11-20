use codeint_config::{CalendarDay, TrendFormat};
use serde::Serialize;

use crate::measurement::CommitMeasurement;
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
        let _ = points_oldest_first;
        todo!()
    }

    pub fn render(&self, format: TrendFormat) -> Result<String, ReportError> {
        match format {
            TrendFormat::Toon => toon_format::encode_default(self).map_err(ReportError::Toon),
            TrendFormat::Json => serde_json::to_string_pretty(self).map_err(ReportError::Json),
            TrendFormat::Markdown => Ok(markdown::render(self)),
        }
    }
}
