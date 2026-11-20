use codeint_model::{CodeModel, RepoPath};

#[derive(Clone, Debug, PartialEq)]
pub struct ErodedFunction {
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
        todo!("{model:?}")
    }

    pub fn ratio(&self) -> f64 {
        todo!()
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
