use std::io::Read;
use std::path::PathBuf;

use codeint_config::{ReportInput, ReportRenderFormat};
use codeint_report::{ReportError, SavedReport, SavedReportError};

#[derive(Debug, thiserror::Error)]
pub(crate) enum RerenderError {
    #[error("cannot read standard input")]
    ReadStdin(#[source] std::io::Error),
    #[error("cannot read {}", path.display())]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Parse(#[from] SavedReportError),
    #[error(transparent)]
    Render(#[from] ReportError),
}

pub(crate) fn rerender(
    input: &ReportInput,
    format: ReportRenderFormat,
) -> Result<String, RerenderError> {
    let text = read_input(input)?;
    let report = SavedReport::from_json(&text)?;
    Ok(report.render(format)?)
}

fn read_input(input: &ReportInput) -> Result<String, RerenderError> {
    match input {
        ReportInput::StandardInput => {
            let mut buffer = String::new();
            std::io::stdin()
                .read_to_string(&mut buffer)
                .map_err(RerenderError::ReadStdin)?;
            Ok(buffer)
        }
        ReportInput::File(path) => {
            std::fs::read_to_string(path).map_err(|source| RerenderError::ReadFile {
                path: path.clone(),
                source,
            })
        }
    }
}
