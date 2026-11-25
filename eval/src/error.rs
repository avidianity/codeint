use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum EvaluationError {
    #[error("{path}: {message}")]
    Manifest { path: PathBuf, message: String },
    #[error("cannot read or write {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{command} failed: {detail}")]
    Command { command: String, detail: String },
    #[error(
        "the scratch folder {scratch} is inside the codeint repository {repository}; point the system temporary folder outside the repository"
    )]
    ScratchInsideRepository {
        scratch: PathBuf,
        repository: PathBuf,
    },
    #[error("no repository in {path} matches the selected languages and repositories")]
    EmptySelection { path: PathBuf },
    #[error("cannot write the evaluation summary as JSON: {0}")]
    Serialize(#[from] serde_json::Error),
}

pub(crate) fn io_error(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> EvaluationError {
    let path = path.into();
    move |source| EvaluationError::Io { path, source }
}
