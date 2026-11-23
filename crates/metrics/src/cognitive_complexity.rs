use std::collections::BTreeMap;

use codeint_model::{CodeModel, Package, RepoPath, SourceFile};

use crate::finding_id::{FindingId, FindingIds, function_finding_id, group_by_package};

#[cfg(test)]
mod tests;

const COGNITIVE_COMPLEXITY_METRIC_KEY: &str = "cognitive_complexity";
const HIGH_COGNITIVE_COMPLEXITY_ABOVE: u32 = 15;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComplexFunction {
    pub id: FindingId,
    pub path: RepoPath,
    pub line: u32,
    pub end_line: u32,
    pub name: Option<String>,
    pub cognitive_complexity: u32,
    pub cyclomatic_complexity: u32,
    pub source_lines: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CognitiveComplexityTotals {
    pub complex: u64,
    pub total: u64,
}

impl CognitiveComplexityTotals {
    pub fn ratio(&self) -> f64 {
        match self.total {
            0 => 0.0,
            total => self.complex as f64 / total as f64,
        }
    }
}

fn merge_totals(target: &mut CognitiveComplexityTotals, delta: CognitiveComplexityTotals) {
    target.complex += delta.complex;
    target.total += delta.total;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CognitiveComplexity {
    totals: CognitiveComplexityTotals,
    totals_by_file: BTreeMap<RepoPath, CognitiveComplexityTotals>,
    complex_functions: Vec<ComplexFunction>,
}

impl CognitiveComplexity {
    pub fn measure(model: &CodeModel) -> CognitiveComplexity {
        let mut totals = CognitiveComplexityTotals::default();
        let mut totals_by_file = BTreeMap::new();
        let mut complex_functions = Vec::new();
        let mut ids = FindingIds::for_key(COGNITIVE_COMPLEXITY_METRIC_KEY);
        for file in &model.files {
            let file_totals = measure_file(file, &mut ids, &mut complex_functions);
            merge_totals(&mut totals, file_totals);
            totals_by_file.insert(file.path.clone(), file_totals);
        }
        complex_functions.sort_by_key(|item| {
            (
                std::cmp::Reverse(item.cognitive_complexity),
                std::cmp::Reverse(item.source_lines),
                item.path.clone(),
                item.line,
            )
        });
        CognitiveComplexity {
            totals,
            totals_by_file,
            complex_functions,
        }
    }

    pub fn complex_functions(&self) -> &[ComplexFunction] {
        &self.complex_functions
    }

    pub fn by_package<'model>(
        &self,
        model: &'model CodeModel,
    ) -> BTreeMap<Package<'model>, CognitiveComplexityTotals> {
        group_by_package(model, &self.totals_by_file, merge_totals)
    }

    pub fn totals(&self) -> CognitiveComplexityTotals {
        self.totals
    }

    pub fn ratio(&self) -> f64 {
        self.totals.ratio()
    }

    pub fn complex_cognitive_complexity(&self) -> u64 {
        self.totals.complex
    }

    pub fn total_cognitive_complexity(&self) -> u64 {
        self.totals.total
    }
}

fn measure_file(
    file: &SourceFile,
    ids: &mut FindingIds,
    findings: &mut Vec<ComplexFunction>,
) -> CognitiveComplexityTotals {
    let mut totals = CognitiveComplexityTotals::default();
    for function in &file.model.functions {
        let cognitive = u64::from(function.cognitive_complexity);
        totals.total += cognitive;
        if function.cognitive_complexity <= HIGH_COGNITIVE_COMPLEXITY_ABOVE {
            continue;
        }
        totals.complex += cognitive;
        findings.push(ComplexFunction {
            id: function_finding_id(ids, &file.path, function, &file.model.tokens),
            path: file.path.clone(),
            line: function.line,
            end_line: function.end_line,
            name: function.name.clone(),
            cognitive_complexity: function.cognitive_complexity,
            cyclomatic_complexity: function.decision_points.saturating_add(1),
            source_lines: function.source_lines,
        });
    }
    totals
}
