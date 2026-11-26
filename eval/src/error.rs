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
        "the scratch folder {scratch} is inside the codeint repository or a repository the run reads, {repository}; point --scratch-root or the system temporary folder outside every repository"
    )]
    ScratchInsideRepository {
        scratch: PathBuf,
        repository: PathBuf,
    },
    #[error("no repository in {path} matches the selected languages and repositories")]
    EmptySelection { path: PathBuf },
    #[error("repository {repository} has no measured files for declared language {language}")]
    MissingLanguage {
        repository: String,
        language: String,
    },
    #[error("cannot write the evaluation summary as JSON: {0}")]
    Serialize(#[from] serde_json::Error),
}

pub(crate) fn io_error(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> EvaluationError {
    let path = path.into();
    move |source| EvaluationError::Io { path, source }
}
