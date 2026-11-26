use std::collections::HashSet;

use codeint_config::{BASELINE_PATH, Baseline, FindingAnchor};
use codeint_metrics::{FindingContext, FindingId};
use codeint_model::SourceSet;
use serde::{Deserialize, Serialize};

pub(crate) fn id_and_anchor(
    id: FindingId,
    context: FindingContext,
) -> (String, Option<FindingAnchor>) {
    let rendered = id.to_string();
    let anchor = FindingAnchor::from_hashes(
        rendered.clone(),
        context.content_hash,
        context.before,
        context.after,
        context.line,
    );
    (rendered, Some(anchor))
}

pub(crate) trait Identified {
    fn id(&self) -> &str;
    fn anchor(&self) -> FindingAnchor;
    fn set_anchor(&mut self, anchor: FindingAnchor);
}

pub(crate) trait BaselineWeight: Sized {
    fn weight(rows: &[&Self]) -> f64 {
        rows.len() as f64
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Baselined {
    pub(crate) hidden: usize,
    pub(crate) retained_share: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BudgetedRatio {
    pub(crate) ratio: f64,
    pub(crate) non_baselined: bool,
}

impl From<f64> for BudgetedRatio {
    fn from(ratio: f64) -> BudgetedRatio {
        BudgetedRatio {
            ratio,
            non_baselined: false,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct BaselineScope<'a> {
    pub(crate) baseline: &'a Baseline,
    pub(crate) source_set: Option<SourceSet>,
}

impl BaselineScope<'_> {
    pub(crate) fn split<R: Identified + BaselineWeight>(
        &self,
        metric: &str,
        rows: Vec<R>,
    ) -> (Vec<R>, Baselined, HashSet<String>) {
        let anchors: Vec<FindingAnchor> = rows.iter().map(Identified::anchor).collect();
        let matched = self
            .baseline
            .match_findings(self.source_set, metric, &anchors);
        if matched.matched_pairs.is_empty() {
            return (rows, Baselined::default(), HashSet::new());
        }
        let is_hidden = matched.matched_candidates(rows.len());
        let mut hidden = Vec::with_capacity(matched.matched_pairs.len());
        let mut visible = Vec::with_capacity(rows.len() - matched.matched_pairs.len());
        let mut hidden_ids = HashSet::with_capacity(matched.matched_pairs.len());
        for (row, hide) in rows.into_iter().zip(is_hidden) {
            if hide {
                hidden_ids.insert(row.id().to_owned());
                hidden.push(row);
            } else {
                visible.push(row);
            }
        }
        let visible_weight = R::weight(&visible.iter().collect::<Vec<_>>());
        let total_weight = visible_weight + R::weight(&hidden.iter().collect::<Vec<_>>());
        let retained_share = if total_weight > 0.0 {
            visible_weight / total_weight
        } else {
            1.0
        };
        let baselined = Baselined {
            hidden: hidden.len(),
            retained_share: Some(retained_share),
        };
        (visible, baselined, hidden_ids)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ReportBaseline {
    pub(crate) path: String,
    pub(crate) hidden_findings: usize,
}

impl ReportBaseline {
    pub(crate) fn new(hidden_findings: usize) -> ReportBaseline {
        ReportBaseline {
            path: BASELINE_PATH.to_owned(),
            hidden_findings,
        }
    }

    pub(crate) fn sentence(&self) -> String {
        format!(
            "{} hidden by the Baseline in {}; run codeint check --ignore-baseline to show every Finding.",
            counted_findings(self.hidden_findings),
            self.path
        )
    }
}

pub(crate) fn counted_findings(count: usize) -> String {
    let noun = if count == 1 { "Finding" } else { "Findings" };
    format!("{count} {noun}")
}
