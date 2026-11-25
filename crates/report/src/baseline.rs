use codeint_config::{BASELINE_PATH, Baseline};
use codeint_model::SourceSet;
use serde::{Deserialize, Serialize};

pub(crate) trait Identified {
    fn id(&self) -> &str;
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
    pub(crate) fn hides(&self, metric: &str, id: &str) -> bool {
        self.baseline.is_baselined(self.source_set, metric, id)
    }

    pub(crate) fn split<R: Identified + BaselineWeight>(
        &self,
        metric: &str,
        rows: Vec<R>,
    ) -> (Vec<R>, Baselined) {
        let (hidden, visible): (Vec<R>, Vec<R>) = rows
            .into_iter()
            .partition(|row| self.hides(metric, row.id()));
        if hidden.is_empty() {
            return (visible, Baselined::default());
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
        (visible, baselined)
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
