use std::collections::{BTreeMap, BTreeSet};

use codeint_model::{CodeModel, Metric, Package, RepoPath, SourceFile, Token, token_text_hash};

use crate::FindingId;
use crate::finding_id::FindingIds;

#[cfg(test)]
mod tests;

pub const HAZARDS_METRIC_KEY: &str = Metric::Hazards.key();

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HazardLines {
    pub hazard_lines: u64,
    pub source_lines: u64,
}

impl HazardLines {
    fn add(&mut self, other: HazardLines) {
        self.hazard_lines += other.hazard_lines;
        self.source_lines += other.source_lines;
    }

    pub fn ratio(&self) -> f64 {
        if self.source_lines == 0 {
            0.0
        } else {
            self.hazard_lines as f64 / self.source_lines as f64
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HazardLineFinding {
    pub id: FindingId,
    pub path: RepoPath,
    pub line: u32,
    pub rule: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hazards {
    totals: HazardLines,
    lines_by_file: BTreeMap<RepoPath, HazardLines>,
    findings: Vec<HazardLineFinding>,
}

impl Hazards {
    pub fn measure(model: &CodeModel, disabled_rules: &BTreeSet<String>) -> Option<Hazards> {
        if !model.languages.iter().any(|facts| facts.has_hazard_rules) {
            return None;
        }
        let per_file = group_files_in_path_order(model);
        let mut lines_by_file = BTreeMap::new();
        let mut findings = Vec::new();
        let mut totals = HazardLines::default();
        let mut ids = FindingIds::new(Metric::Hazards);
        for (path, group) in per_file {
            let file_lines =
                measure_grouped_file(path, &group, disabled_rules, &mut ids, &mut findings);
            totals.add(file_lines);
            lines_by_file.insert(path.clone(), file_lines);
        }
        Some(Hazards {
            totals,
            lines_by_file,
            findings,
        })
    }

    pub fn by_package<'model>(
        &self,
        model: &'model CodeModel,
    ) -> BTreeMap<Package<'model>, HazardLines> {
        let mut grouped: BTreeMap<Package<'model>, HazardLines> = BTreeMap::new();
        for (path, &lines) in &self.lines_by_file {
            grouped
                .entry(model.workspace.package_of(path))
                .or_default()
                .add(lines);
        }
        grouped
    }

    pub fn totals(&self) -> HazardLines {
        self.totals
    }

    pub fn hazard_lines(&self) -> u64 {
        self.totals.hazard_lines
    }

    pub fn source_lines(&self) -> u64 {
        self.totals.source_lines
    }

    pub fn ratio(&self) -> f64 {
        self.totals.ratio()
    }

    pub fn findings(&self) -> &[HazardLineFinding] {
        &self.findings
    }
}

fn group_files_in_path_order(model: &CodeModel) -> BTreeMap<&RepoPath, Vec<&SourceFile>> {
    let mut grouped: BTreeMap<&RepoPath, Vec<&SourceFile>> = BTreeMap::new();
    for file in &model.files {
        grouped.entry(&file.path).or_default().push(file);
    }
    grouped
}

fn measure_grouped_file(
    path: &RepoPath,
    files: &[&SourceFile],
    disabled_rules: &BTreeSet<String>,
    ids: &mut FindingIds,
    findings: &mut Vec<HazardLineFinding>,
) -> HazardLines {
    let source_lines: u64 = files
        .iter()
        .map(|file| u64::from(file.model.source_lines))
        .sum();
    let active_rules: BTreeSet<(u32, &str)> = files
        .iter()
        .flat_map(|file| file.model.hazard_lines.iter())
        .filter(|hazard| !disabled_rules.contains(hazard.rule.as_str()))
        .map(|hazard| (hazard.line, hazard.rule.as_str()))
        .collect();
    let distinct_lines: BTreeSet<u32> = active_rules.iter().map(|&(line, _)| line).collect();
    let tokens = files
        .first()
        .map(|file| file.model.tokens.as_slice())
        .unwrap_or_default();
    for (line, rule) in active_rules {
        let content = std::iter::once(token_text_hash(rule)).chain(line_token_hashes(tokens, line));
        findings.push(HazardLineFinding {
            id: ids.next_in_source_order([path], content),
            path: path.clone(),
            line,
            rule: rule.to_owned(),
        });
    }
    HazardLines {
        hazard_lines: distinct_lines.len() as u64,
        source_lines,
    }
}

fn line_token_hashes(tokens: &[Token], line: u32) -> impl Iterator<Item = u64> + '_ {
    let start = tokens.partition_point(|token| token.line < line);
    let end = tokens.partition_point(|token| token.line <= line);
    tokens[start..end].iter().map(|token| token.text_hash)
}
