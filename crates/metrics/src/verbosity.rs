use std::collections::{BTreeMap, BTreeSet};

use codeint_model::{CodeModel, Package, RepoPath};

use crate::line_share::{LineShare, line_shares_by_package};

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
    source_lines: u64,
    flagged_lines: Vec<FlaggedLineFinding>,
}

impl Verbosity {
    pub fn measure(model: &CodeModel) -> Verbosity {
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
        let verbose_lines: BTreeSet<(&RepoPath, u32)> = flagged_lines
            .iter()
            .map(|finding| (&finding.path, finding.line))
            .collect();
        Verbosity {
            verbose_lines: verbose_lines.len() as u64,
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

    pub fn source_lines(&self) -> u64 {
        self.source_lines
    }

    pub fn flagged_lines(&self) -> &[FlaggedLineFinding] {
        &self.flagged_lines
    }

    pub fn by_package<'model>(
        &self,
        model: &'model CodeModel,
    ) -> BTreeMap<Package<'model>, LineShare> {
        let verbose_lines: BTreeSet<(&RepoPath, u32)> = self
            .flagged_lines
            .iter()
            .map(|finding| (&finding.path, finding.line))
            .collect();
        line_shares_by_package(model, verbose_lines.into_iter().map(|(path, _)| (path, 1)))
    }

    pub fn ratio(&self) -> f64 {
        if self.source_lines == 0 {
            0.0
        } else {
            self.verbose_lines as f64 / self.source_lines as f64
        }
    }
}
