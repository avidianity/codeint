use codeint_model::StableHasher;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::lowering::{LoweredFile, LoweredOutcome};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditCategory {
    SourceSets,
    FlaggedLines,
    OrphanFiles,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditSample {
    pub category: AuditCategory,
    pub seed: String,
    pub population: u64,
    pub items: Vec<String>,
}

#[derive(Default)]
pub(crate) struct AuditPopulation {
    source_sets: Vec<String>,
    flagged_lines: Vec<String>,
    orphan_files: Vec<String>,
    commits: Vec<String>,
}

fn findings(section: &Value) -> impl Iterator<Item = &Value> {
    section["findings"].as_array().into_iter().flatten()
}

fn text<'value>(finding: &'value Value, field: &str) -> &'value str {
    finding[field].as_str().unwrap_or_default()
}

impl AuditPopulation {
    pub(crate) fn add_repository(
        &mut self,
        repository: &str,
        commit: &str,
        report: &Value,
        lowered: &[LoweredFile],
    ) {
        self.commits.push(commit.to_owned());
        let source_sets = report["source_sets"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        for section in source_sets.values() {
            self.flagged_lines
                .extend(findings(&section["verbosity"]).map(|finding| {
                    format!(
                        "{repository} {}:{} {}",
                        text(finding, "path"),
                        finding["line"],
                        text(finding, "rule")
                    )
                }));
        }
        self.orphan_files.extend(
            findings(&report["source_sets"]["production"]["orphan_files"]).map(|finding| {
                format!(
                    "{repository} {} {}",
                    text(finding, "path"),
                    text(finding, "kind")
                )
            }),
        );
        self.source_sets
            .extend(lowered.iter().filter_map(|file| match &file.outcome {
                LoweredOutcome::Measured { source_set, .. } => {
                    Some(format!("{repository} {} {}", file.path, source_set.key()))
                }
                LoweredOutcome::Unparsable | LoweredOutcome::NotMeasured => None,
            }));
    }

    pub(crate) fn absorb(&mut self, mut other: AuditPopulation) {
        self.source_sets.append(&mut other.source_sets);
        self.flagged_lines.append(&mut other.flagged_lines);
        self.orphan_files.append(&mut other.orphan_files);
        self.commits.append(&mut other.commits);
    }

    pub(crate) fn draw(self, size: usize) -> Vec<AuditSample> {
        let seed = seed_for(&self.commits);
        [
            (AuditCategory::SourceSets, self.source_sets),
            (AuditCategory::FlaggedLines, self.flagged_lines),
            (AuditCategory::OrphanFiles, self.orphan_files),
        ]
        .into_iter()
        .map(|(category, items)| AuditSample {
            category,
            seed: format!("{seed:016x}"),
            population: items.len() as u64,
            items: seeded_sample(items, seed, size),
        })
        .collect()
    }
}

fn seed_for(commits: &[String]) -> u64 {
    let mut sorted = commits.to_vec();
    sorted.sort();
    sorted
        .iter()
        .fold(StableHasher::new(), |hasher, commit| {
            hasher.write_bytes(commit.as_bytes())
        })
        .finish()
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^ (mixed >> 31)
    }
}

pub(crate) fn seeded_sample(mut items: Vec<String>, seed: u64, size: usize) -> Vec<String> {
    items.sort();
    let take = size.min(items.len());
    let mut random = SplitMix64(seed);
    for index in 0..take {
        let remaining = (items.len() - index) as u64;
        let chosen = index + (random.next() % remaining) as usize;
        items.swap(index, chosen);
    }
    items.truncate(take);
    items
}

#[cfg(test)]
mod tests;
