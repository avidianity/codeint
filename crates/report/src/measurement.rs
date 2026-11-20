use codeint_model::{CodeModel, SourceSet};
use serde::{Deserialize, Serialize};

use crate::{
    CloneGroupFinding, ErodedFunctionFinding, FlaggedLineRow, ImportCycleFinding, Measured,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CommitMeasurement {
    production: SourceSetMeasurement,
    test: SourceSetMeasurement,
    generated: SourceSetMeasurement,
    vendored: SourceSetMeasurement,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SourceSetMeasurement {
    pub(crate) files: usize,
    pub(crate) erosion: MetricMeasurement<ErodedFunctionFinding>,
    pub(crate) duplication: MetricMeasurement<CloneGroupFinding>,
    pub(crate) verbosity: MetricMeasurement<FlaggedLineRow>,
    pub(crate) import_cycles: MetricMeasurement<ImportCycleFinding>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct MetricMeasurement<T> {
    pub(crate) ratio: f64,
    pub(crate) findings_worst_first: Vec<T>,
}

impl CommitMeasurement {
    pub fn of(model: CodeModel) -> CommitMeasurement {
        let source_sets = model.into_source_sets();
        let measure = |source_set| SourceSetMeasurement::of(source_sets.get(source_set));
        CommitMeasurement {
            production: measure(SourceSet::Production),
            test: measure(SourceSet::Test),
            generated: measure(SourceSet::Generated),
            vendored: measure(SourceSet::Vendored),
        }
    }

    pub(crate) fn into_source_sets(self) -> [(SourceSet, SourceSetMeasurement); 4] {
        [
            (SourceSet::Production, self.production),
            (SourceSet::Test, self.test),
            (SourceSet::Generated, self.generated),
            (SourceSet::Vendored, self.vendored),
        ]
    }
}

impl SourceSetMeasurement {
    fn of(model: &CodeModel) -> SourceSetMeasurement {
        let measured = Measured::new(model);
        SourceSetMeasurement {
            files: model.files.len(),
            erosion: erosion(&measured),
            duplication: duplication(&measured),
            verbosity: verbosity(&measured),
            import_cycles: import_cycles(&measured),
        }
    }
}

fn erosion(measured: &Measured) -> MetricMeasurement<ErodedFunctionFinding> {
    MetricMeasurement {
        ratio: measured.erosion.ratio(),
        findings_worst_first: measured
            .erosion
            .eroded_functions()
            .iter()
            .map(ErodedFunctionFinding::new)
            .collect(),
    }
}

fn duplication(measured: &Measured) -> MetricMeasurement<CloneGroupFinding> {
    MetricMeasurement {
        ratio: measured.duplication.ratio(),
        findings_worst_first: measured
            .duplication
            .clone_groups
            .iter()
            .map(CloneGroupFinding::new)
            .collect(),
    }
}

fn verbosity(measured: &Measured) -> MetricMeasurement<FlaggedLineRow> {
    MetricMeasurement {
        ratio: measured.verbosity.ratio(),
        findings_worst_first: measured
            .verbosity
            .flagged_lines()
            .iter()
            .map(FlaggedLineRow::from)
            .collect(),
    }
}

fn import_cycles(measured: &Measured) -> MetricMeasurement<ImportCycleFinding> {
    MetricMeasurement {
        ratio: measured.import_cycles.ratio(),
        findings_worst_first: measured
            .import_cycles
            .cycles()
            .iter()
            .map(ImportCycleFinding::new)
            .collect(),
    }
}
