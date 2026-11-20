use std::collections::BTreeSet;

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
        let flagged_lines: BTreeSet<FlaggedLineFinding> = model
            .files
            .iter()
            .flat_map(|file| {
                file.model
                    .flagged_lines
                    .iter()
                    .map(|flagged| FlaggedLineFinding {
                        path: file.path.clone(),
                        line: flagged.line,
                        rule: flagged.rule.clone(),
                    })
            })
            .collect();
        let distinct_flagged_lines: BTreeSet<(&RepoPath, u32)> = flagged_lines
            .iter()
            .map(|finding| (&finding.path, finding.line))
            .collect();
        let flagged_lines_outside_clones = distinct_flagged_lines
            .iter()
            .filter(|(path, line)| !clone_lines.contains(path, *line))
            .count() as u64;
        Verbosity {
            verbose_lines: clone_lines.count() + flagged_lines_outside_clones,
            flagged_line_count: distinct_flagged_lines.len() as u64,
            clone_line_count: clone_lines.count(),
            source_lines: model
                .files
                .iter()
                .map(|file| u64::from(file.model.source_lines))
                .sum(),
            flagged_lines: flagged_lines.into_iter().collect(),
        }
    }

    pub fn verbose_lines(&self) -> u64 {
        self.verbose_lines
    }

    pub fn flagged_line_count(&self) -> u64 {
        self.flagged_line_count
    }

    pub fn clone_line_count(&self) -> u64 {
        self.clone_line_count
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
