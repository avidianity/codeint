use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::lowering::{LoweredFile, LoweredOutcome};

pub const RECALL_BAR: f64 = 0.98;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecallCount {
    pub matched: u64,
    pub total: u64,
}

impl RecallCount {
    pub fn ratio(&self) -> f64 {
        if self.total == 0 {
            return 1.0;
        }
        self.matched as f64 / self.total as f64
    }

    fn add(&mut self, other: &RecallCount) {
        self.matched += other.matched;
        self.total += other.total;
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageRecall {
    pub files_compared: u64,
    pub functions: RecallCount,
    pub decision_points: RecallCount,
    pub imports: RecallCount,
    pub oracle_unparsable_files: Vec<String>,
    pub unexpected_unparsable_files: Vec<String>,
    pub unmeasured_files: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OracleFile {
    pub(crate) path: String,
    #[serde(default)]
    functions: Option<Vec<[u32; 3]>>,
    #[serde(default)]
    imports: Option<Vec<String>>,
    #[serde(default)]
    error: Option<String>,
}

impl LanguageRecall {
    pub fn meets_bar(&self) -> bool {
        self.unexpected_unparsable_files.is_empty()
            && [&self.functions, &self.decision_points, &self.imports]
                .iter()
                .all(|count| count.ratio() >= RECALL_BAR)
    }

    pub(crate) fn compare(oracle: &[OracleFile], lowered: &[LoweredFile]) -> LanguageRecall {
        let by_path: BTreeMap<&str, &LoweredOutcome> = lowered
            .iter()
            .map(|file| (file.path.as_str(), &file.outcome))
            .collect();
        let mut recall = LanguageRecall::default();
        for file in oracle {
            recall.compare_file(file, by_path.get(file.path.as_str()).copied());
        }
        recall
    }

    fn compare_file(&mut self, oracle: &OracleFile, codeint: Option<&LoweredOutcome>) {
        if oracle.error.is_some() {
            self.oracle_unparsable_files.push(oracle.path.clone());
            return;
        }
        match codeint {
            Some(LoweredOutcome::Measured {
                functions, imports, ..
            }) => {
                self.files_compared += 1;
                self.compare_functions(oracle.functions.as_deref().unwrap_or_default(), functions);
                self.compare_imports(oracle.imports.as_deref().unwrap_or_default(), imports);
            }
            Some(LoweredOutcome::Unparsable) => {
                self.unexpected_unparsable_files.push(oracle.path.clone());
            }
            Some(LoweredOutcome::NotMeasured) | None => {
                self.unmeasured_files.push(oracle.path.clone());
                self.compare_functions(oracle.functions.as_deref().unwrap_or_default(), &[]);
                self.compare_imports(
                    oracle.imports.as_deref().unwrap_or_default(),
                    &BTreeSet::new(),
                );
            }
        }
    }

    fn compare_functions(&mut self, oracle: &[[u32; 3]], codeint: &[[u32; 3]]) {
        let mut available: BTreeMap<(u32, u32), Vec<u32>> = BTreeMap::new();
        for [line, end_line, decisions] in codeint {
            available
                .entry((*line, *end_line))
                .or_default()
                .push(*decisions);
        }
        for [line, end_line, decisions] in oracle {
            self.functions.total += 1;
            self.decision_points.total += u64::from(*decisions);
            let Some(codeint_decisions) = available.get_mut(&(*line, *end_line)).and_then(Vec::pop)
            else {
                continue;
            };
            self.functions.matched += 1;
            self.decision_points.matched += u64::from(codeint_decisions.min(*decisions));
        }
    }

    fn compare_imports(&mut self, oracle: &[String], codeint: &BTreeSet<String>) {
        let expected: BTreeSet<&String> = oracle.iter().collect();
        self.imports.total += expected.len() as u64;
        self.imports.matched += expected
            .iter()
            .filter(|import| codeint.contains(**import))
            .count() as u64;
    }

    pub(crate) fn absorb(&mut self, repository: &str, other: &LanguageRecall) {
        self.files_compared += other.files_compared;
        self.functions.add(&other.functions);
        self.decision_points.add(&other.decision_points);
        self.imports.add(&other.imports);
        let prefixed = |paths: &[String]| -> Vec<String> {
            paths
                .iter()
                .map(|path| format!("{repository} {path}"))
                .collect()
        };
        self.oracle_unparsable_files
            .extend(prefixed(&other.oracle_unparsable_files));
        self.unexpected_unparsable_files
            .extend(prefixed(&other.unexpected_unparsable_files));
        self.unmeasured_files
            .extend(prefixed(&other.unmeasured_files));
    }
}

#[cfg(test)]
mod tests;
