use crate::summary::{OracleStatus, RepositoryResult};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ceilings {
    pub max_wall_seconds: Option<f64>,
    pub max_peak_rss_bytes: Option<u64>,
}

pub(crate) fn ceiling_violations(
    result: &RepositoryResult,
    ceilings: &Ceilings,
    require_oracles: bool,
) -> Vec<String> {
    [
        skipped_oracle(result, require_oracles),
        wall_time(result.median_wall_seconds, ceilings.max_wall_seconds),
        peak_memory(result.peak_rss_bytes, ceilings.max_peak_rss_bytes),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn skipped_oracle(result: &RepositoryResult, require_oracles: bool) -> Option<String> {
    (require_oracles && result.oracle.status == OracleStatus::Skipped).then(|| {
        format!(
            "oracle skipped: {}",
            result.oracle.reason.as_deref().unwrap_or("no reason given")
        )
    })
}

fn wall_time(measured: Option<f64>, ceiling: Option<f64>) -> Option<String> {
    let ceiling = ceiling?;
    match measured {
        Some(seconds) if seconds > ceiling => Some(format!(
            "median wall time {seconds:.2}s exceeds the ceiling of {ceiling}s"
        )),
        Some(_) => None,
        None => Some("no wall time measurement to hold to the ceiling".to_owned()),
    }
}

fn peak_memory(measured: Option<u64>, ceiling: Option<u64>) -> Option<String> {
    let ceiling = ceiling?;
    match measured {
        Some(bytes) if bytes > ceiling => Some(format!(
            "peak memory {} MiB exceeds the ceiling of {} MiB",
            bytes / (1024 * 1024),
            ceiling / (1024 * 1024)
        )),
        Some(_) => None,
        None => Some("peak memory was not measured, so the ceiling cannot be checked".to_owned()),
    }
}

#[cfg(test)]
mod tests;
