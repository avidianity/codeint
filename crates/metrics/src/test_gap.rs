use std::collections::BTreeMap;

use codeint_model::language::Function;
use codeint_model::{CodeModel, Metric, Package, RepoPath, SourceFile, SourceSet};

use crate::FindingId;
use crate::erosion::function_mass;
use crate::finding_id::{FindingContext, FindingIds, function_finding_identity, group_by_package};

#[cfg(test)]
mod tests;

pub const TEST_GAP_METRIC_KEY: &str = Metric::TestGap.key();

#[derive(Clone, Debug, PartialEq)]
pub struct TestGapFinding {
    pub id: FindingId,
    pub context: FindingContext,
    pub path: RepoPath,
    pub name: Option<String>,
    pub line: u32,
    pub end_line: u32,
    pub function_mass: f64,
    pub untested_share: f64,
    pub untested_mass: f64,
    pub listed_lines: Option<u32>,
    pub executed_lines: Option<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TestGapMasses {
    pub untested: f64,
    pub total: f64,
    pub measured_functions: usize,
    pub files: usize,
}

impl TestGapMasses {
    fn add(&mut self, other: TestGapMasses) {
        self.untested += other.untested;
        self.total += other.total;
        self.measured_functions += other.measured_functions;
        self.files += other.files;
    }

    pub fn ratio(&self) -> f64 {
        if self.total > 0.0 {
            self.untested / self.total
        } else {
            0.0
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TestGap {
    masses: TestGapMasses,
    masses_by_file: BTreeMap<RepoPath, TestGapMasses>,
    findings: Vec<TestGapFinding>,
}

impl TestGap {
    pub fn measure(model: &CodeModel) -> Option<TestGap> {
        if !model_has_coverage(model) {
            return None;
        }
        let has_production_files = model
            .files
            .iter()
            .any(|file| file.model.source_set == SourceSet::Production);
        if !model.files.is_empty() && !has_production_files {
            return None;
        }
        let mut masses = TestGapMasses::default();
        let mut masses_by_file = BTreeMap::new();
        let mut findings = Vec::new();
        let mut ids = FindingIds::new(Metric::TestGap);
        for file in &model.files {
            if file.model.source_set != SourceSet::Production {
                continue;
            }
            let file_masses = measure_file(file, &mut ids, &mut findings);
            masses.add(file_masses);
            masses_by_file.insert(file.path.clone(), file_masses);
        }
        findings.sort_by(|left, right| {
            right
                .untested_mass
                .total_cmp(&left.untested_mass)
                .then_with(|| left.path.cmp(&right.path))
                .then_with(|| left.line.cmp(&right.line))
                .then_with(|| left.name.cmp(&right.name))
        });
        Some(TestGap {
            masses,
            masses_by_file,
            findings,
        })
    }

    pub fn measured_functions(&self) -> usize {
        self.masses.measured_functions
    }

    pub fn untested_mass(&self) -> f64 {
        self.masses.untested
    }

    pub fn total_mass(&self) -> f64 {
        self.masses.total
    }

    pub fn ratio(&self) -> f64 {
        self.masses.ratio()
    }

    pub fn totals(&self) -> TestGapMasses {
        self.masses
    }

    pub fn findings(&self) -> &[TestGapFinding] {
        &self.findings
    }

    pub fn by_package<'model>(
        &self,
        model: &'model CodeModel,
    ) -> BTreeMap<Package<'model>, TestGapMasses> {
        group_by_package(model, &self.masses_by_file, TestGapMasses::add)
    }
}

fn model_has_coverage(model: &CodeModel) -> bool {
    if !model.languages.is_empty() {
        return model.languages.iter().any(|facts| facts.has_coverage);
    }
    model
        .files
        .iter()
        .filter(|file| file.model.source_set == SourceSet::Production)
        .flat_map(|file| &file.model.functions)
        .any(|function| function.coverage.is_some())
}

fn measure_file(
    file: &SourceFile,
    ids: &mut FindingIds,
    findings: &mut Vec<TestGapFinding>,
) -> TestGapMasses {
    let mut totals = TestGapMasses {
        files: 1,
        ..TestGapMasses::default()
    };
    for function in &file.model.functions {
        let Some((untested_share, listed_lines, executed_lines)) =
            function_untested_share(function)
        else {
            continue;
        };
        let mass = function_mass(function);
        let untested_mass = mass * untested_share;
        totals.total += mass;
        totals.untested += untested_mass;
        totals.measured_functions += 1;
        if untested_share > 0.0 {
            let (id, context) =
                function_finding_identity(ids, &file.path, function, &file.model.tokens);
            findings.push(TestGapFinding {
                id,
                context,
                path: file.path.clone(),
                name: function.name.clone(),
                line: function.line,
                end_line: function.end_line,
                function_mass: mass,
                untested_share,
                untested_mass,
                listed_lines,
                executed_lines,
            });
        }
    }
    totals
}

fn function_untested_share(function: &Function) -> Option<(f64, Option<u32>, Option<u32>)> {
    match function.coverage {
        Some(coverage) if coverage.listed_lines == 0 => None,
        Some(coverage) => {
            let untested_lines = coverage
                .listed_lines
                .saturating_sub(coverage.executed_lines);
            let share = f64::from(untested_lines) / f64::from(coverage.listed_lines);
            Some((
                share,
                Some(coverage.listed_lines),
                Some(coverage.executed_lines),
            ))
        }
        None => Some((1.0, None, None)),
    }
}
