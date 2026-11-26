use std::collections::BTreeSet;

use codeint_config::FindingAnchor;

use crate::baseline::{BaselineWeight, Identified};
use crate::dependencies::{ImportedDependencyRow, UnusedDependencyRow};
use crate::findings::{
    BoundaryFindingRow, CloneFamilyFinding, CloneGroupFinding, CloneLocation,
    CognitiveComplexityFinding, ErodedFunctionFinding, ExternalMetricFinding, FlaggedLineRow,
    HazardLineRow, ImportCycleFinding, OrphanFileRow, TestGapFindingRow,
};

fn unique_lines<'a>(lines: impl Iterator<Item = (&'a str, u32)>) -> f64 {
    lines.collect::<BTreeSet<_>>().len() as f64
}

fn location_lines(location: &CloneLocation) -> impl Iterator<Item = (&str, u32)> {
    (location.start_line..=location.end_line).map(move |line| (location.path.as_str(), line))
}

impl BaselineWeight for ErodedFunctionFinding {
    fn weight(rows: &[&Self]) -> f64 {
        rows.iter().map(|row| row.function_mass.0).sum()
    }
}

impl BaselineWeight for CognitiveComplexityFinding {
    fn weight(rows: &[&Self]) -> f64 {
        rows.iter()
            .map(|row| f64::from(row.cognitive_complexity))
            .sum()
    }
}

impl BaselineWeight for TestGapFindingRow {
    fn weight(rows: &[&Self]) -> f64 {
        rows.iter()
            .map(|row| row.function_mass.0 * row.untested_share.0)
            .sum()
    }
}

impl BaselineWeight for CloneGroupFinding {
    fn weight(rows: &[&Self]) -> f64 {
        unique_lines(
            rows.iter()
                .flat_map(|row| row.locations.iter().flat_map(location_lines)),
        )
    }
}

impl BaselineWeight for FlaggedLineRow {
    fn weight(rows: &[&Self]) -> f64 {
        unique_lines(rows.iter().map(|row| (row.path.as_str(), row.line)))
    }
}

impl BaselineWeight for HazardLineRow {
    fn weight(rows: &[&Self]) -> f64 {
        unique_lines(rows.iter().map(|row| (row.path.as_str(), row.line)))
    }
}

impl BaselineWeight for ImportCycleFinding {
    fn weight(rows: &[&Self]) -> f64 {
        rows.iter()
            .flat_map(|row| row.modules.iter())
            .collect::<BTreeSet<_>>()
            .len() as f64
    }
}

impl BaselineWeight for BoundaryFindingRow {}
impl BaselineWeight for OrphanFileRow {}
impl BaselineWeight for ExternalMetricFinding {}
impl BaselineWeight for UnusedDependencyRow {}
impl BaselineWeight for ImportedDependencyRow {}

macro_rules! impl_identified {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl Identified for $ty {
                fn id(&self) -> &str {
                    &self.id
                }

                fn anchor(&self) -> FindingAnchor {
                    self.anchor
                        .clone()
                        .unwrap_or_else(|| FindingAnchor::id_only(&self.id))
                }

                fn set_anchor(&mut self, anchor: FindingAnchor) {
                    self.anchor = Some(anchor);
                }
            }
        )+
    };
}

impl_identified!(
    ErodedFunctionFinding,
    CognitiveComplexityFinding,
    CloneGroupFinding,
    CloneFamilyFinding,
    FlaggedLineRow,
    ImportCycleFinding,
    ExternalMetricFinding,
    BoundaryFindingRow,
    UnusedDependencyRow,
    ImportedDependencyRow,
    OrphanFileRow,
    HazardLineRow,
    TestGapFindingRow,
);
