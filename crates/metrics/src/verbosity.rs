use codeint_model::{CodeModel, RepoPath};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FlaggedLineFinding {
    pub path: RepoPath,
    pub line: u32,
    pub rule: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verbosity {
    pub verbose_lines: u64,
    pub source_lines: u64,
    pub flagged_lines: Vec<FlaggedLineFinding>,
}

impl Verbosity {
    pub fn measure(model: &CodeModel) -> Verbosity {
        todo!("{model:?}")
    }

    pub fn ratio(&self) -> f64 {
        todo!()
    }
}
