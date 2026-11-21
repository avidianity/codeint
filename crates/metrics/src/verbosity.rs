use std::collections::{BTreeMap, BTreeSet, HashMap};

use codeint_model::{CodeModel, Metric, Package, RepoPath, Token, token_text_hash};

use crate::finding_id::FindingIds;
use crate::line_share::{LineShare, line_shares_by_package};
use crate::{CloneLines, FindingId};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlaggedLineFinding {
    pub id: FindingId,
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
    verbose_lines_by_file: BTreeMap<RepoPath, u64>,
}

impl Verbosity {
    pub fn measure(model: &CodeModel, clone_lines: &CloneLines) -> Verbosity {
        let flagged_lines: BTreeSet<(&RepoPath, u32, &str)> = model
            .files
            .iter()
            .flat_map(|file| {
                file.model
                    .flagged_lines
                    .iter()
                    .map(|flagged| (&file.path, flagged.line, flagged.rule.as_str()))
            })
            .collect();
        let distinct_flagged_lines: BTreeSet<(&RepoPath, u32)> = flagged_lines
            .iter()
            .map(|&(path, line, _)| (path, line))
            .collect();
        let mut verbose_lines_by_file: BTreeMap<RepoPath, u64> = clone_lines
            .count_by_file()
            .map(|(path, lines)| (path.clone(), lines))
            .collect();
        for (path, _) in distinct_flagged_lines
            .iter()
            .filter(|(path, line)| !clone_lines.contains(path, *line))
        {
            *verbose_lines_by_file.entry((*path).clone()).or_default() += 1;
        }
        Verbosity {
            verbose_lines: verbose_lines_by_file.values().sum(),
            flagged_line_count: distinct_flagged_lines.len() as u64,
            clone_line_count: clone_lines.count(),
            source_lines: model
                .files
                .iter()
                .map(|file| u64::from(file.model.source_lines))
                .sum(),
            flagged_lines: flagged_line_findings(model, &flagged_lines),
            verbose_lines_by_file,
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

    pub fn by_package<'model>(
        &self,
        model: &'model CodeModel,
    ) -> BTreeMap<Package<'model>, LineShare> {
        line_shares_by_package(
            model,
            self.verbose_lines_by_file
                .iter()
                .map(|(path, lines)| (path, *lines)),
        )
    }
}

fn flagged_line_findings(
    model: &CodeModel,
    flagged_lines: &BTreeSet<(&RepoPath, u32, &str)>,
) -> Vec<FlaggedLineFinding> {
    let tokens_by_path: HashMap<&RepoPath, &[Token]> = model
        .files
        .iter()
        .map(|file| (&file.path, file.model.tokens.as_slice()))
        .collect();
    let mut ids = FindingIds::new(Metric::Verbosity);
    flagged_lines
        .iter()
        .map(|&(path, line, rule)| {
            let tokens = tokens_by_path.get(path).copied().unwrap_or_default();
            let first = tokens.partition_point(|token| token.line < line);
            let after_last = tokens.partition_point(|token| token.line <= line);
            let line_tokens = tokens[first..after_last]
                .iter()
                .map(|token| token.text_hash);
            FlaggedLineFinding {
                id: ids.next_in_source_order(
                    [path],
                    std::iter::once(token_text_hash(rule)).chain(line_tokens),
                ),
                path: path.clone(),
                line,
                rule: rule.to_owned(),
            }
        })
        .collect()
}
