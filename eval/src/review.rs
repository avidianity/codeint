use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::audit::{AuditCategory, AuditSample};
use crate::error::{EvaluationError, io_error};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Judgement {
    Correct,
    FalsePositive,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    pub category: AuditCategory,
    pub item: String,
    pub verdict: Judgement,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerdictFile {
    pub reviewer: String,
    pub verdicts: Vec<Verdict>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CategoryReview {
    pub category: AuditCategory,
    pub sampled: u64,
    pub reviewed: u64,
    pub false_positives: u64,
    pub false_positive_percent: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AuditReview {
    pub reviewer: String,
    pub categories: Vec<CategoryReview>,
}

pub fn load_verdicts(path: &Path) -> Result<VerdictFile, EvaluationError> {
    let text = std::fs::read_to_string(path).map_err(io_error(path))?;
    serde_json::from_str(&text).map_err(|error| EvaluationError::Manifest {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

pub(crate) fn review_samples(samples: &[AuditSample], file: &VerdictFile) -> AuditReview {
    let judged: BTreeMap<(AuditCategory, &str), Judgement> = file
        .verdicts
        .iter()
        .map(|verdict| ((verdict.category, verdict.item.as_str()), verdict.verdict))
        .collect();
    let categories = samples
        .iter()
        .map(|sample| {
            let judgements: Vec<Judgement> = sample
                .items
                .iter()
                .filter_map(|item| judged.get(&(sample.category, item.as_str())).copied())
                .collect();
            let false_positives = judgements
                .iter()
                .filter(|judgement| **judgement == Judgement::FalsePositive)
                .count() as u64;
            let reviewed = judgements.len() as u64;
            CategoryReview {
                category: sample.category,
                sampled: sample.items.len() as u64,
                reviewed,
                false_positives,
                false_positive_percent: if reviewed == 0 {
                    0.0
                } else {
                    false_positives as f64 * 100.0 / reviewed as f64
                },
            }
        })
        .collect();
    AuditReview {
        reviewer: file.reviewer.clone(),
        categories,
    }
}

#[cfg(test)]
mod tests;
