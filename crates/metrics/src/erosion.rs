use codeint_model::language::Function;
use codeint_model::{CodeModel, RepoPath, Token};

use crate::FindingId;
use crate::finding_id::{FindingIds, MetricKey};

const ERODED_ABOVE_CYCLOMATIC_COMPLEXITY: u32 = 10;

#[derive(Clone, Debug, PartialEq)]
pub struct ErodedFunction {
    pub id: FindingId,
    pub path: RepoPath,
    pub line: u32,
    pub cyclomatic_complexity: u32,
    pub source_lines: u32,
    pub function_mass: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Erosion {
    eroded_function_mass: f64,
    total_function_mass: f64,
    eroded_functions: Vec<ErodedFunction>,
}

impl Erosion {
    pub fn measure(model: &CodeModel) -> Erosion {
        let mut total_function_mass = 0.0;
        let mut eroded_functions = Vec::new();
        let mut ids = FindingIds::new(MetricKey::Erosion);
        for file in &model.files {
            for function in &file.model.functions {
                let cyclomatic_complexity = function.decision_points.saturating_add(1);
                let function_mass =
                    f64::from(cyclomatic_complexity) * f64::from(function.source_lines).sqrt();
                total_function_mass += function_mass;
                if cyclomatic_complexity > ERODED_ABOVE_CYCLOMATIC_COMPLEXITY {
                    eroded_functions.push(ErodedFunction {
                        id: ids.next_in_source_order(
                            [&file.path],
                            tokens_of(function, &file.model.tokens)
                                .iter()
                                .map(|token| token.text_hash),
                        ),
                        path: file.path.clone(),
                        line: function.line,
                        cyclomatic_complexity,
                        source_lines: function.source_lines,
                        function_mass,
                    });
                }
            }
        }
        eroded_functions.sort_by(|left, right| {
            right
                .function_mass
                .total_cmp(&left.function_mass)
                .then_with(|| left.path.cmp(&right.path))
                .then_with(|| left.line.cmp(&right.line))
        });
        Erosion {
            eroded_function_mass: eroded_functions
                .iter()
                .map(|function| function.function_mass)
                .sum(),
            total_function_mass,
            eroded_functions,
        }
    }

    pub fn ratio(&self) -> f64 {
        if self.total_function_mass > 0.0 {
            self.eroded_function_mass / self.total_function_mass
        } else {
            0.0
        }
    }

    pub fn eroded_function_mass(&self) -> f64 {
        self.eroded_function_mass
    }

    pub fn total_function_mass(&self) -> f64 {
        self.total_function_mass
    }

    pub fn eroded_functions(&self) -> &[ErodedFunction] {
        &self.eroded_functions
    }
}

fn tokens_of<'tokens>(function: &Function, tokens: &'tokens [Token]) -> &'tokens [Token] {
    let first = tokens.partition_point(|token| token.line < function.line);
    let after_last = tokens.partition_point(|token| token.line <= function.end_line);
    &tokens[first..after_last.max(first)]
}
