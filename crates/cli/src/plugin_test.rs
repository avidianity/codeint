use std::path::{Path, PathBuf};

use codeint_config::{FindingsShown, PluginHostConfig, PluginKind, PluginLimits};
use codeint_git::Repository;
use codeint_plugin_host::{
    ConformanceCheck, ConformanceSource, ConformanceStatus, END_TO_END_CHECK, EndToEndProbe,
    PluginHostError, run_conformance, sha256, sha256_hex,
};
use codeint_report::{PluginTestCheck, PluginTestReport, PluginTestStatus};

use crate::check::{CheckScope, check};

const PLUGIN_UNDER_TEST: &str = "plugin-under-test";

#[derive(Debug, thiserror::Error)]
pub(crate) enum PluginTestError {
    #[error(transparent)]
    PluginHost(#[from] PluginHostError),
    #[error("cannot prepare the end-to-end probe repository")]
    Scratch(#[source] std::io::Error),
    #[error("cannot prepare the end-to-end probe repository")]
    ScratchGit(#[from] codeint_git::GitError),
    #[error("cannot read source file {}", path.display())]
    Source {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub(crate) fn plugin_test(
    path: &Path,
    kind: PluginKind,
    source_paths: &[PathBuf],
    limits: PluginLimits,
) -> Result<PluginTestReport, PluginTestError> {
    let sources = source_paths
        .iter()
        .map(PathBuf::as_path)
        .map(read_source)
        .collect::<Result<Vec<_>, _>>()?;
    let mut conformance = run_conformance(kind, path, limits, &sources)?;
    if let Some(probe) = conformance.end_to_end.take() {
        let outcome = end_to_end(path, kind, limits, &probe)?;
        match conformance
            .checks
            .iter_mut()
            .find(|check| check.id == END_TO_END_CHECK)
        {
            Some(slot) => *slot = outcome,
            None => conformance.checks.push(outcome),
        }
    }
    let checks = conformance
        .checks
        .into_iter()
        .map(|check| PluginTestCheck {
            id: check.id.to_owned(),
            status: match check.status {
                ConformanceStatus::Pass => PluginTestStatus::Pass,
                ConformanceStatus::Fail => PluginTestStatus::Fail,
                ConformanceStatus::Skipped => PluginTestStatus::Skipped,
            },
            message: check.message,
            hint: check.hint,
        })
        .collect();
    let kind_name = match kind {
        PluginKind::Language => "language",
        PluginKind::Metric => "metric",
    };
    Ok(PluginTestReport::new(
        path.display().to_string(),
        kind_name,
        checks,
    ))
}

fn read_source(path: &Path) -> Result<ConformanceSource, PluginTestError> {
    let text = std::fs::read_to_string(path).map_err(|source| PluginTestError::Source {
        path: path.to_path_buf(),
        source,
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("source")
        .to_owned();
    Ok(ConformanceSource { file_name, text })
}

fn end_to_end(
    path: &Path,
    kind: PluginKind,
    limits: PluginLimits,
    probe: &EndToEndProbe,
) -> Result<ConformanceCheck, PluginTestError> {
    let bytes = std::fs::read(path).map_err(PluginTestError::Scratch)?;
    let plugin_path = std::path::absolute(path).map_err(PluginTestError::Scratch)?;
    let mut runs: Vec<(String, Option<&str>)> =
        vec![("with no Framework profile configuration".to_owned(), None)];
    runs.extend(probe.framework_profiles.iter().map(|profile| {
        (
            format!("with framework_profiles [{profile:?}]"),
            Some(profile.as_str()),
        )
    }));
    let folder = tempfile::tempdir().map_err(PluginTestError::Scratch)?;
    for file in &probe.files {
        let full = folder.path().join(&file.path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).map_err(PluginTestError::Scratch)?;
        }
        std::fs::write(&full, &file.text).map_err(PluginTestError::Scratch)?;
    }
    Repository::init(folder.path())?;
    let mut checked = 0;
    for (label, profile) in &runs {
        let configuration = probe_configuration(
            &plugin_path,
            &sha256_hex(&sha256(&bytes)),
            kind,
            limits,
            *profile,
        );
        std::fs::write(
            folder.path().join("codeint.json"),
            configuration.to_string(),
        )
        .map_err(PluginTestError::Scratch)?;
        if let Err(error) = measure_probe(folder.path()) {
            return Ok(ConformanceCheck {
                id: END_TO_END_CHECK,
                status: ConformanceStatus::Fail,
                message: format!(
                    "codeint check stops with exit code 2 on the probe repository {label}: {}",
                    error_chain(&error)
                ),
                hint: Some(
                    "codeint check runs the whole measurement pipeline on a repository, so fix the Plugin until it accepts the probe repository that plugin test writes: the probe files and manifests for every claimed extension, in the root package and in packages/probe, with and without each Framework profile configured"
                        .to_owned(),
                ),
            });
        }
        checked += 1;
    }
    Ok(ConformanceCheck {
        id: END_TO_END_CHECK,
        status: ConformanceStatus::Pass,
        message: format!(
            "codeint check measures the probe repository in {checked} configuration{}",
            if checked == 1 { "" } else { "s" }
        ),
        hint: None,
    })
}

fn probe_configuration(
    plugin_path: &Path,
    digest: &str,
    kind: PluginKind,
    limits: PluginLimits,
    profile: Option<&str>,
) -> serde_json::Value {
    let kind_name = match kind {
        PluginKind::Language => "language",
        PluginKind::Metric => "metric",
    };
    let mut configuration = serde_json::json!({
        "plugins": [{
            "name": PLUGIN_UNDER_TEST,
            "kind": kind_name,
            "path": plugin_path,
            "sha256": digest,
        }],
        "plugin_limits": {
            "memory_mb": limits.memory_mb,
            "timeout_ms": limits.timeout_ms,
        },
    });
    if kind == PluginKind::Language {
        configuration["languages"] = serde_json::json!([
            { "plugin": PLUGIN_UNDER_TEST, "include": ["src", "packages"] }
        ]);
    }
    if let Some(profile) = profile {
        configuration["framework_profiles"] = serde_json::json!([profile]);
    }
    configuration
}

fn measure_probe(folder: &Path) -> Result<(), crate::check::CheckError> {
    check(
        folder,
        CheckScope {
            findings: FindingsShown::All,
            package: None,
            owner: None,
            coverage_reports: &[],
            ignore_baseline: true,
        },
        &PluginHostConfig::default(),
    )
    .map(|_| ())
}

fn error_chain(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(inner) = source {
        message.push_str(": ");
        message.push_str(&inner.to_string());
        source = inner.source();
    }
    message
}
