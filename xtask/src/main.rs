#![forbid(unsafe_code)]

mod checks;
mod config;
mod git;
mod lexer;
mod runner;

use std::process::ExitCode;

fn main() -> ExitCode {
    let invocation = config::invocation();
    match runner::run(&invocation.command, &invocation.args) {
        Ok(violations) if violations.is_empty() => ExitCode::SUCCESS,
        Ok(violations) => {
            for violation in violations {
                eprintln!("{violation}");
            }
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("xtask {}: {error}", invocation.command);
            ExitCode::from(2)
        }
    }
}
