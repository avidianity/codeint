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
mod run;
mod scratch;
mod summary;

pub use audit::{AuditCategory, AuditSample};
pub use ceilings::Ceilings;
pub use corpus::{Corpus, CorpusEntry, SizeBucket, load_corpus};
pub use error::EvaluationError;
pub use recall::{LanguageRecall, RECALL_BAR, RecallCount};
pub use run::{EvaluationConfig, run_evaluation};
pub use scratch::{StaleSweep, sweep_stale_scratch_folders};
pub use summary::{
    EvaluationSummary, LanguageSummary, MachineProfile, OracleReport, OracleStatus,
    PerformanceSummary, RepositoryResult,
};
