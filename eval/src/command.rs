use std::process::{Command, Output};

use crate::error::EvaluationError;

pub(crate) fn describe(command: &Command) -> String {
    let mut parts = vec![command.get_program().to_string_lossy().into_owned()];
    parts.extend(
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned()),
    );
    parts.join(" ")
}

pub(crate) fn run_checked(command: &mut Command) -> Result<Output, EvaluationError> {
    let output = command.output().map_err(|error| EvaluationError::Command {
        command: describe(command),
        detail: error.to_string(),
    })?;
    if output.status.success() {
        return Ok(output);
    }
    Err(EvaluationError::Command {
        command: describe(command),
        detail: format!(
            "{}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ),
    })
}

pub(crate) fn stdout_text(command: &mut Command) -> Result<String, EvaluationError> {
    let output = run_checked(command)?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
