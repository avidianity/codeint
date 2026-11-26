#![forbid(unsafe_code)]

mod audit;
mod ceilings;
mod command;
mod corpus;
mod error;
mod lowering;
mod machine;
mod measure;
mod oracle;
mod recall;
mod report;
mod review;
mod run;
mod scratch;
mod stats;
mod summary;

pub use audit::{AuditCategory, AuditSample};
pub use ceilings::Ceilings;
pub use corpus::{CaseStudy, Corpus, CorpusEntry, SizeBucket, load_corpus};
pub use error::EvaluationError;
pub use recall::{LanguageRecall, RECALL_BAR, RecallCount};
pub use review::{AuditReview, CategoryReview, Judgement, Verdict, VerdictFile, load_verdicts};
pub use run::{EvaluationConfig, run_evaluation};
pub use scratch::{StaleSweep, sweep_stale_scratch_folders};
pub use stats::Distribution;
pub use summary::{
    CaseSnapshot, CaseStudyResult, EvaluationSummary, LanguageSummary, MachineProfile,
    MetricReading, OracleReport, OracleStatus, PanelMetric, PanelSummary, PerformanceSummary,
    RepositoryResult,
};
