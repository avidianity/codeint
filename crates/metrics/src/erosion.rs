use std::collections::BTreeMap;

use codeint_model::{CodeModel, Metric, Package, RepoPath};

use crate::FindingId;
use crate::finding_id::{FindingIds, function_finding_id, group_by_package};

#[cfg(test)]
mod tests;

const ERODED_ABOVE_CYCLOMATIC_COMPLEXITY: u32 = 10;

#[derive(Clone, Debug, PartialEq)]
pub struct ErodedFunction {
    pub id: FindingId,
    pub path: RepoPath,
    pub line: u32,
    pub end_line: u32,
    pub name: Option<String>,
    pub cyclomatic_complexity: u32,
    pub cognitive_complexity: u32,
    pub source_lines: u32,
    pub function_mass: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FunctionMasses {
    pub eroded: f64,
    pub total: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Erosion {
    masses: FunctionMasses,
    masses_by_file: BTreeMap<RepoPath, FunctionMasses>,
    eroded_functions: Vec<ErodedFunction>,
}

impl FunctionMasses {
    fn add(&mut self, other: FunctionMasses) {
        self.eroded += other.eroded;
        self.total += other.total;
    }

    pub fn ratio(&self) -> f64 {
        if self.total > 0.0 {
            self.eroded / self.total
        } else {
            0.0
        }
    }
}

impl Erosion {
    pub fn measure(model: &CodeModel) -> Erosion {
        let mut masses = FunctionMasses::default();
        let mut masses_by_file = BTreeMap::new();
        let mut eroded_functions = Vec::new();
        let mut ids = FindingIds::new(Metric::Erosion);
        for file in &model.files {
            let mut file_masses = FunctionMasses::default();
            for function in &file.model.functions {
                let cyclomatic_complexity = function.decision_points.saturating_add(1);
                let function_mass =
                    f64::from(cyclomatic_complexity) * f64::from(function.source_lines).sqrt();
                file_masses.total += function_mass;
                if cyclomatic_complexity > ERODED_ABOVE_CYCLOMATIC_COMPLEXITY {
                    file_masses.eroded += function_mass;
                    eroded_functions.push(ErodedFunction {
                        id: function_finding_id(&mut ids, &file.path, function, &file.model.tokens),
                        path: file.path.clone(),
                        line: function.line,
                        end_line: function.end_line,
                        name: function.name.clone(),
                        cyclomatic_complexity,
                        cognitive_complexity: function.cognitive_complexity,
                        source_lines: function.source_lines,
                        function_mass,
                    });
                }
            }
            masses.add(file_masses);
            masses_by_file.insert(file.path.clone(), file_masses);
        }
        eroded_functions.sort_by(|left, right| {
            right
                .function_mass
                .total_cmp(&left.function_mass)
                .then_with(|| left.path.cmp(&right.path))
                .then_with(|| left.line.cmp(&right.line))
        });
        Erosion {
            masses,
            masses_by_file,
            eroded_functions,
        }
    }

    pub fn ratio(&self) -> f64 {
        self.masses.ratio()
    }

    pub fn eroded_function_mass(&self) -> f64 {
        self.masses.eroded
    }

    pub fn total_function_mass(&self) -> f64 {
        self.masses.total
    }

    pub fn eroded_functions(&self) -> &[ErodedFunction] {
        &self.eroded_functions
    }

    pub fn by_package<'model>(
        &self,
        model: &'model CodeModel,
    ) -> BTreeMap<Package<'model>, FunctionMasses> {
        group_by_package(model, &self.masses_by_file, FunctionMasses::add)
    }
}
