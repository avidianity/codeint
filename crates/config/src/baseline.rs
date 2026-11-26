use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use codeint_model::SourceSet;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::finding_anchor::{AnchorMatch, FindingAnchor, match_anchors};
use crate::repo_state::{
    DEPENDENCY_METRIC_KEYS, LoadedForUpdate, MetricVersionMismatch, RepoStateFile,
    SourceSetEntries, StateFileError, StateFileFault, metric_label, read_state_file,
    recover_invalid_for_update, write_state_file,
};

#[cfg(test)]
mod tests;

pub const BASELINE_PATH: &str = ".codeint/baseline.json";
pub const BASELINE_SCHEMA_VERSION: u32 = 2;
const LEGACY_BASELINE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineEntry {
    pub metric_version: u32,
    #[serde(default)]
    pub findings: Vec<FindingAnchor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Baseline {
    pub schema_version: u32,
    #[serde(default)]
    pub source_sets: SourceSetEntries<BaselineEntry>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, BaselineEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyBaselineEntry {
    metric_version: u32,
    #[serde(default)]
    findings: BTreeSet<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyBaseline {
    schema_version: u32,
    #[serde(default)]
    source_sets: SourceSetEntries<LegacyBaselineEntry>,
    #[serde(default)]
    dependencies: BTreeMap<String, LegacyBaselineEntry>,
}

impl From<LegacyBaselineEntry> for BaselineEntry {
    fn from(entry: LegacyBaselineEntry) -> BaselineEntry {
        BaselineEntry {
            metric_version: entry.metric_version,
            findings: entry
                .findings
                .into_iter()
                .map(FindingAnchor::id_only)
                .collect(),
        }
    }
}

fn convert_legacy_entries(
    entries: BTreeMap<String, LegacyBaselineEntry>,
) -> BTreeMap<String, BaselineEntry> {
    entries
        .into_iter()
        .map(|(metric, entry)| (metric, BaselineEntry::from(entry)))
        .collect()
}

impl From<LegacyBaseline> for Baseline {
    fn from(legacy: LegacyBaseline) -> Baseline {
        Baseline {
            schema_version: legacy.schema_version,
            source_sets: SourceSetEntries {
                production: convert_legacy_entries(legacy.source_sets.production),
                test: convert_legacy_entries(legacy.source_sets.test),
                generated: convert_legacy_entries(legacy.source_sets.generated),
                vendored: convert_legacy_entries(legacy.source_sets.vendored),
            },
            dependencies: convert_legacy_entries(legacy.dependencies),
        }
    }
}

impl Default for Baseline {
    fn default() -> Baseline {
        Baseline {
            schema_version: BASELINE_SCHEMA_VERSION,
            source_sets: SourceSetEntries::default(),
            dependencies: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredBaselineMetric {
    pub source_set: Option<SourceSet>,
    pub metric: String,
    pub metric_version: u32,
    pub findings: Vec<FindingAnchor>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BaselineUpdateSummary {
    pub path: String,
    pub created: bool,
    pub reset: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub replaced_invalid: bool,
    pub added: usize,
    pub removed: usize,
    pub refreshed: usize,
    pub skipped_new: usize,
    pub total_findings: usize,
}

pub type BaselineFileError = StateFileError;

fn baseline_fault(fault: StateFileFault) -> StateFileError {
    StateFileError::from_fault(RepoStateFile::Baseline, BASELINE_PATH, fault)
}

fn invalid_json(source: serde_json::Error) -> BaselineFileError {
    StateFileError::InvalidJson {
        file: RepoStateFile::Baseline,
        path: BASELINE_PATH.to_owned(),
        source,
    }
}

fn parse_versioned_value(value: Value) -> Result<Baseline, BaselineFileError> {
    let Some(version) = value.get("schema_version").and_then(Value::as_u64) else {
        return serde_json::from_value(value).map_err(invalid_json);
    };
    if version == u64::from(LEGACY_BASELINE_SCHEMA_VERSION) {
        let legacy: LegacyBaseline = serde_json::from_value(value).map_err(invalid_json)?;
        return Ok(Baseline::from(legacy));
    }
    if version == u64::from(BASELINE_SCHEMA_VERSION) {
        return serde_json::from_value(value).map_err(invalid_json);
    }
    Err(StateFileError::UnsupportedSchemaVersion {
        file: RepoStateFile::Baseline,
        path: BASELINE_PATH.to_owned(),
        found: u32::try_from(version).unwrap_or(u32::MAX),
        expected: BASELINE_SCHEMA_VERSION,
    })
}

impl Baseline {
    pub fn load_optional(repository_root: &Path) -> Result<Option<Baseline>, BaselineFileError> {
        read_state_file(repository_root, BASELINE_PATH)
            .map_err(baseline_fault)?
            .map(|text| Baseline::parse(&text))
            .transpose()
    }

    pub fn load_for_update(
        repository_root: &Path,
        reset: bool,
    ) -> Result<LoadedForUpdate<Baseline>, BaselineFileError> {
        recover_invalid_for_update(Baseline::load_optional(repository_root), reset)
    }

    pub fn parse(text: &str) -> Result<Baseline, BaselineFileError> {
        let value: Value = serde_json::from_str(text).map_err(invalid_json)?;
        let baseline = parse_versioned_value(value)?;
        baseline.validate()?;
        Ok(baseline)
    }

    pub fn write_atomic(&self, repository_root: &Path) -> Result<PathBuf, BaselineFileError> {
        write_state_file(repository_root, BASELINE_PATH, self).map_err(baseline_fault)
    }

    pub fn entry(&self, source_set: Option<SourceSet>, metric: &str) -> Option<&BaselineEntry> {
        match source_set {
            Some(set) => self.source_sets.get(set).get(metric),
            None => self.dependencies.get(metric),
        }
    }

    pub fn match_findings(
        &self,
        source_set: Option<SourceSet>,
        metric: &str,
        candidates: &[FindingAnchor],
    ) -> AnchorMatch {
        let Some(entry) = self.entry(source_set, metric) else {
            return AnchorMatch::default();
        };
        match_anchors(&entry.findings, candidates)
    }

    pub fn total_findings(&self) -> usize {
        self.source_sets
            .iter()
            .into_iter()
            .flat_map(|(_, entries)| entries.values())
            .chain(self.dependencies.values())
            .map(|entry| entry.findings.len())
            .sum()
    }

    pub fn version_mismatches(
        &self,
        measured: &[MeasuredBaselineMetric],
    ) -> Vec<MetricVersionMismatch> {
        measured
            .iter()
            .filter_map(|item| {
                let stored = self.entry(item.source_set, &item.metric)?;
                (stored.metric_version != item.metric_version).then(|| MetricVersionMismatch {
                    state_file: RepoStateFile::Baseline,
                    source_set: item.source_set,
                    metric: item.metric.clone(),
                    recorded_version: stored.metric_version,
                    current_version: item.metric_version,
                })
            })
            .collect()
    }

    pub fn update(
        existing: Option<&Baseline>,
        measured: &[MeasuredBaselineMetric],
        reset: bool,
    ) -> (Baseline, BaselineUpdateSummary) {
        let mut next = existing.cloned().unwrap_or_default();
        next.schema_version = BASELINE_SCHEMA_VERSION;
        let mut summary = BaselineUpdateSummary {
            path: BASELINE_PATH.to_owned(),
            created: existing.is_none(),
            reset,
            ..BaselineUpdateSummary::default()
        };
        for item in measured {
            let previous = existing.and_then(|prev| prev.entry(item.source_set, &item.metric));
            let entry = next_entry(previous, item, previous.is_none() || reset, &mut summary);
            next.insert(item.source_set, item.metric.clone(), entry);
        }
        summary.total_findings = next.total_findings();
        (next, summary)
    }

    fn insert(&mut self, source_set: Option<SourceSet>, metric: String, entry: BaselineEntry) {
        match source_set {
            Some(set) => self.source_sets.get_mut(set).insert(metric, entry),
            None => self.dependencies.insert(metric, entry),
        };
    }

    fn validate(&self) -> Result<(), BaselineFileError> {
        if self.schema_version != LEGACY_BASELINE_SCHEMA_VERSION
            && self.schema_version != BASELINE_SCHEMA_VERSION
        {
            return Err(StateFileError::UnsupportedSchemaVersion {
                file: RepoStateFile::Baseline,
                path: BASELINE_PATH.to_owned(),
                found: self.schema_version,
                expected: BASELINE_SCHEMA_VERSION,
            });
        }
        let labelled = self
            .source_sets
            .iter()
            .into_iter()
            .flat_map(|(source_set, entries)| {
                entries
                    .iter()
                    .map(move |(metric, entry)| (metric_label(Some(source_set), metric), entry))
            })
            .chain(
                self.dependencies
                    .iter()
                    .map(|(metric, entry)| (metric.clone(), entry)),
            );
        for dependency in self.dependencies.keys() {
            if !DEPENDENCY_METRIC_KEYS.contains(&dependency.as_str()) {
                return Err(StateFileError::UnknownDependencyMetric {
                    file: RepoStateFile::Baseline,
                    path: BASELINE_PATH.to_owned(),
                    metric: dependency.clone(),
                    allowed: DEPENDENCY_METRIC_KEYS.join(", "),
                });
            }
        }
        for (metric, entry) in labelled {
            if entry.metric_version == 0 {
                return Err(StateFileError::InvalidMetricVersion {
                    file: RepoStateFile::Baseline,
                    path: BASELINE_PATH.to_owned(),
                    metric,
                });
            }
        }
        Ok(())
    }
}

fn next_entry(
    previous: Option<&BaselineEntry>,
    item: &MeasuredBaselineMetric,
    accept_current: bool,
    summary: &mut BaselineUpdateSummary,
) -> BaselineEntry {
    let stored: &[FindingAnchor] = previous.map_or(&[], |entry| entry.findings.as_slice());
    let version_changed = previous.is_some_and(|entry| entry.metric_version != item.metric_version);
    let matched = match_anchors(stored, &item.findings);
    let matched_count = matched.matched_pairs.len();
    let unmatched_current = item.findings.len() - matched_count;
    let unmatched_stored = stored.len() - matched_count;
    let mut findings = if accept_current || version_changed {
        summary.refreshed += usize::from(version_changed);
        summary.added += unmatched_current;
        item.findings.clone()
    } else {
        summary.skipped_new += unmatched_current;
        matched
            .matched_pairs
            .iter()
            .map(|&(_, current_index)| item.findings[current_index].clone())
            .collect()
    };
    findings.sort();
    summary.removed += unmatched_stored;
    BaselineEntry {
        metric_version: item.metric_version,
        findings,
    }
}
