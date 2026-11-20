use codeint_model::{CodeModel, RepoPath};

use crate::CloneLines;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FlaggedLineFinding {
    pub path: RepoPath,
    pub line: u32,
    pub rule: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verbosity {
    verbose_lines: u64,
    flagged_line_count: u64,
    clone_line_count: u64,
    source_lines: u64,
    flagged_lines: Vec<FlaggedLineFinding>,
}

impl Verbosity {
    pub fn measure(model: &CodeModel, clone_lines: &CloneLines) -> Verbosity {
        _ = (model, clone_lines);
        todo!()
    }

    pub fn verbose_lines(&self) -> u64 {
        self.verbose_lines
    }

    pub fn flagged_line_count(&self) -> u64 {
        todo!()
    }

    pub fn clone_line_count(&self) -> u64 {
        todo!()
    }

    pub fn source_lines(&self) -> u64 {
        self.source_lines
    }

    pub fn flagged_lines(&self) -> &[FlaggedLineFinding] {
        &self.flagged_lines
    }

    pub fn ratio(&self) -> f64 {
        if self.source_lines == 0 {
            0.0
        } else {
            self.verbose_lines as f64 / self.source_lines as f64
        }
    }
}
