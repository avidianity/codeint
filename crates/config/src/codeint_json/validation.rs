use std::path::PathBuf;

use codeint_model::{Metric, MetricScope, SourceSet};
use serde_json::Value;

use super::schema::{DuplicationConfigFile, PluginEntry};
use super::types::{
    Budgets, Configuration, DuplicationConfig, MinSimilarity, PluginDeclaration, PluginSource,
    Sha256Digest,
};
use crate::RegressionLimits;
use crate::config_error::{
    ConfigFileKind, ConfigurationFileError, KeyPath, KeySegment, contains_parent_traversal,
    is_absolute_repo_path,
};
use crate::resolved::BudgetOverride;

impl Configuration {
    pub fn validate_cli_budget_metrics(
        &self,
        overrides: &[BudgetOverride],
        external_metric_keys: &[&str],
    ) -> Result<(), ConfigurationFileError> {
        for entry in overrides {
            if let Some(metric) = Metric::from_key(&entry.metric) {
                if is_production_only_outside_production(metric, entry.source_set) {
                    return Err(cli_production_only_budget_error(entry.source_set, metric));
                }
                if metric.scope().applies_to_source_set(entry.source_set) {
                    continue;
                }
            } else if external_metric_keys.contains(&entry.metric.as_str()) {
                continue;
            }
            return Err(ConfigurationFileError::UnknownCliBudgetMetric {
                source_set: entry.source_set,
                metric: entry.metric.clone(),
                allowed: allowed_budget_metric_keys(entry.source_set, external_metric_keys),
            });
        }
        Ok(())
    }

    pub fn validate_metrics(
        &self,
        external_metric_keys: &[&str],
    ) -> Result<(), ConfigurationFileError> {
        validate_source_set_budgets(
            self.config_file,
            "budgets",
            &self.budgets,
            external_metric_keys,
        )?;
        validate_source_set_budgets(
            self.config_file,
            "new_file_budgets",
            &self.new_file_budgets,
            external_metric_keys,
        )?;
        validate_regression_limit_keys(
            self.config_file,
            &self.regression_limits,
            external_metric_keys,
        )
    }
}

fn validate_source_set_budgets(
    config_file: ConfigFileKind,
    section: &str,
    budgets: &Budgets,
    external_metric_keys: &[&str],
) -> Result<(), ConfigurationFileError> {
    for (source_set, metric_key, _) in &budgets.set {
        if let Some(metric) = Metric::from_key(metric_key) {
            if is_production_only_outside_production(metric, *source_set) {
                return Err(production_only_budget_error(
                    config_file,
                    section,
                    *source_set,
                    metric,
                ));
            }
            if metric.scope().applies_to_source_set(*source_set) {
                continue;
            }
        } else if external_metric_keys.contains(&metric_key.as_str()) {
            continue;
        }
        return Err(ConfigurationFileError::UnknownKey {
            file: config_file,
            key: budget_key_path(section, *source_set, metric_key),
            allowed: allowed_budget_metric_keys(*source_set, external_metric_keys),
        });
    }
    Ok(())
}

fn is_production_only_outside_production(metric: Metric, source_set: SourceSet) -> bool {
    metric.scope() == MetricScope::ProductionOnly
        && !metric.scope().applies_to_source_set(source_set)
}

fn budget_key_path(section: &str, source_set: SourceSet, metric_key: &str) -> KeyPath {
    KeyPath::default()
        .child(KeySegment::Key(section.to_owned()))
        .child(KeySegment::Key(source_set.key().to_owned()))
        .child(KeySegment::Key(metric_key.to_owned()))
}

fn regression_limit_key_path(source_set: Option<SourceSet>, metric_key: &str) -> KeyPath {
    let mut path = KeyPath::default().child(KeySegment::Key("regression_limits".to_owned()));
    if let Some(source_set) = source_set {
        path = path.child(KeySegment::Key(source_set.key().to_owned()));
    }
    path.child(KeySegment::Key(metric_key.to_owned()))
}

fn production_only_budget_error(
    file: ConfigFileKind,
    section: &str,
    source_set: SourceSet,
    metric: Metric,
) -> ConfigurationFileError {
    let key = budget_key_path(section, source_set, metric.key());
    match metric {
        Metric::TestGap => ConfigurationFileError::TestGapBudgetOutsideProduction { file, key },
        other => ConfigurationFileError::MetricBudgetOutsideProduction {
            file,
            key,
            metric: other,
        },
    }
}

fn cli_production_only_budget_error(
    source_set: SourceSet,
    metric: Metric,
) -> ConfigurationFileError {
    match metric {
        Metric::TestGap => ConfigurationFileError::CliTestGapBudgetOutsideProduction { source_set },
        other => ConfigurationFileError::CliMetricBudgetOutsideProduction {
            source_set,
            metric: other,
        },
    }
}

fn validate_regression_limit_keys(
    config_file: ConfigFileKind,
    limits: &RegressionLimits,
    external_metric_keys: &[&str],
) -> Result<(), ConfigurationFileError> {
    for (source_set, metric_key) in limits.metric_keys() {
        validate_regression_limit_key(config_file, source_set, metric_key, external_metric_keys)?;
    }
    Ok(())
}

fn validate_regression_limit_key(
    config_file: ConfigFileKind,
    source_set: Option<SourceSet>,
    metric_key: &str,
    external_metric_keys: &[&str],
) -> Result<(), ConfigurationFileError> {
    if let Some(metric) = Metric::from_key(metric_key) {
        if !metric.supports_diff_regression() {
            return Err(ConfigurationFileError::TestGapRegressionLimit {
                file: config_file,
                key: regression_limit_key_path(source_set, metric_key),
            });
        }
        let Some(set) = source_set else {
            return Ok(());
        };
        if is_production_only_outside_production(metric, set) {
            return Err(
                ConfigurationFileError::MetricRegressionLimitOutsideProduction {
                    file: config_file,
                    key: regression_limit_key_path(Some(set), metric_key),
                    metric,
                },
            );
        }
        if metric.scope().applies_to_source_set(set) {
            return Ok(());
        }
    } else if external_metric_keys.contains(&metric_key) {
        return Ok(());
    }
    Err(ConfigurationFileError::UnknownKey {
        file: config_file,
        key: regression_limit_key_path(source_set, metric_key),
        allowed: allowed_regression_limit_metric_keys(source_set, external_metric_keys),
    })
}

pub(super) fn is_repository_wide_diff_metric(metric: Metric) -> bool {
    metric.scope() == MetricScope::RepositoryWide && metric.supports_diff_regression()
}

fn ordered_built_in_metrics(predicate: impl Fn(Metric) -> bool) -> Vec<Metric> {
    let mut built_in: Vec<Metric> = Metric::ALL
        .into_iter()
        .filter(|metric| predicate(*metric))
        .collect();
    move_metric_after(
        &mut built_in,
        Metric::CognitiveComplexity,
        Metric::ImportCycles,
    );
    move_metric_after(&mut built_in, Metric::TestGap, Metric::Hazards);
    built_in
}

fn move_metric_after(metrics: &mut Vec<Metric>, target: Metric, anchor: Metric) {
    if let (Some(target_pos), Some(anchor_pos)) = (
        metrics.iter().position(|metric| *metric == target),
        metrics.iter().position(|metric| *metric == anchor),
    ) && target_pos < anchor_pos
    {
        let metric = metrics.remove(target_pos);
        metrics.insert(anchor_pos, metric);
    }
}

fn allowed_budget_metric_keys(source_set: SourceSet, external_metric_keys: &[&str]) -> Vec<String> {
    ordered_built_in_metrics(|metric| metric.scope().applies_to_source_set(source_set))
        .into_iter()
        .map(|metric| metric.key().to_owned())
        .chain(external_metric_keys.iter().map(|key| (*key).to_owned()))
        .collect()
}

pub(super) fn allowed_regression_limit_metric_keys(
    source_set: Option<SourceSet>,
    external_metric_keys: &[&str],
) -> Vec<String> {
    let built_in = match source_set {
        Some(source_set) => ordered_built_in_metrics(|metric| {
            metric.scope().applies_to_source_set(source_set) && metric.supports_diff_regression()
        }),
        None => ordered_built_in_metrics(|metric| {
            metric.scope() != MetricScope::RepositoryWide && metric.supports_diff_regression()
        }),
    };
    let mut allowed: Vec<String> = built_in
        .into_iter()
        .map(|metric| metric.key().to_owned())
        .chain(external_metric_keys.iter().map(|key| (*key).to_owned()))
        .collect();
    if source_set.is_none() {
        allowed.extend(
            Metric::ALL
                .into_iter()
                .filter(|metric| is_repository_wide_diff_metric(*metric))
                .map(|metric| metric.key().to_owned()),
        );
        allowed.extend(SourceSet::ALL.map(|set| set.key().to_owned()));
    }
    allowed
}

pub(super) fn reject_unsupported_metric_keys(root: &Value) -> Result<(), ConfigurationFileError> {
    let Some(root) = root.as_object() else {
        return Ok(());
    };
    if let Some(budgets) = root.get("budgets").and_then(Value::as_object) {
        reject_unsupported_budget_keys("budgets", budgets)?;
    }
    if let Some(new_file_budgets) = root.get("new_file_budgets").and_then(Value::as_object) {
        reject_unsupported_budget_keys("new_file_budgets", new_file_budgets)?;
    }
    if let Some(limits) = root.get("regression_limits").and_then(Value::as_object) {
        reject_unsupported_regression_limit_keys(limits)?;
    }
    Ok(())
}

fn reject_unsupported_budget_keys(
    section_name: &str,
    budgets: &serde_json::Map<String, Value>,
) -> Result<(), ConfigurationFileError> {
    for source_set in SourceSet::ALL {
        let Some(section) = budgets.get(source_set.key()).and_then(Value::as_object) else {
            continue;
        };
        for metric in Metric::ALL {
            if !section.contains_key(metric.key()) {
                continue;
            }
            if is_production_only_outside_production(metric, source_set) {
                return Err(production_only_budget_error(
                    ConfigFileKind::Json,
                    section_name,
                    source_set,
                    metric,
                ));
            }
            if metric.scope() == MetricScope::RepositoryWide {
                return Err(ConfigurationFileError::UnknownKey {
                    file: ConfigFileKind::Json,
                    key: budget_key_path(section_name, source_set, metric.key()),
                    allowed: allowed_budget_metric_keys(source_set, &[]),
                });
            }
        }
    }
    Ok(())
}

fn reject_unsupported_regression_limit_keys(
    limits: &serde_json::Map<String, Value>,
) -> Result<(), ConfigurationFileError> {
    for metric in Metric::ALL {
        if !metric.supports_diff_regression() && limits.contains_key(metric.key()) {
            return Err(ConfigurationFileError::TestGapRegressionLimit {
                file: ConfigFileKind::Json,
                key: regression_limit_key_path(None, metric.key()),
            });
        }
    }
    for source_set in SourceSet::ALL {
        let Some(section) = limits.get(source_set.key()).and_then(Value::as_object) else {
            continue;
        };
        reject_unsupported_source_set_regression_limit_keys(source_set, section)?;
    }
    Ok(())
}

fn reject_unsupported_source_set_regression_limit_keys(
    source_set: SourceSet,
    section: &serde_json::Map<String, Value>,
) -> Result<(), ConfigurationFileError> {
    for metric in Metric::ALL {
        if !section.contains_key(metric.key()) {
            continue;
        }
        if !metric.supports_diff_regression() {
            return Err(ConfigurationFileError::TestGapRegressionLimit {
                file: ConfigFileKind::Json,
                key: regression_limit_key_path(Some(source_set), metric.key()),
            });
        }
        if is_production_only_outside_production(metric, source_set) {
            return Err(
                ConfigurationFileError::MetricRegressionLimitOutsideProduction {
                    file: ConfigFileKind::Json,
                    key: regression_limit_key_path(Some(source_set), metric.key()),
                    metric,
                },
            );
        }
    }
    Ok(())
}

pub(super) fn coverage_reports(reports: &[String]) -> Result<Vec<String>, ConfigurationFileError> {
    let reports_key = KeyPath::default().child(KeySegment::Key("coverage_reports".to_owned()));
    let mut validated = Vec::with_capacity(reports.len());
    for (index, entry) in reports.iter().enumerate() {
        let entry_key = reports_key.child(KeySegment::Index(index));
        if entry.trim().is_empty() {
            return Err(ConfigurationFileError::EmptyCoverageReportPath {
                file: ConfigFileKind::Json,
                key: entry_key,
            });
        }
        if is_absolute_repo_path(entry) {
            return Err(ConfigurationFileError::AbsoluteCoverageReportPath {
                file: ConfigFileKind::Json,
                key: entry_key,
                path: entry.clone(),
            });
        }
        if contains_parent_traversal(entry) {
            return Err(ConfigurationFileError::ParentTraversalCoverageReportPath {
                file: ConfigFileKind::Json,
                key: entry_key,
                path: entry.clone(),
            });
        }
        validated.push(entry.clone());
    }
    Ok(validated)
}

pub(super) fn duplication_config(
    file: DuplicationConfigFile,
) -> Result<DuplicationConfig, ConfigurationFileError> {
    let (near_clones, min_similarity) = file.into_parts();
    let min_similarity = match min_similarity {
        Some(value) => MinSimilarity::new(value).ok_or_else(|| {
            ConfigurationFileError::OutOfExclusiveMinRange {
                file: ConfigFileKind::Json,
                key: KeyPath::default()
                    .child(KeySegment::Key("duplication".to_owned()))
                    .child(KeySegment::Key("min_similarity".to_owned())),
                exclusive_minimum: 0.0,
                maximum: 1.0,
                found: value,
            }
        })?,
        None => MinSimilarity::DEFAULT,
    };
    Ok(DuplicationConfig {
        near_clones: near_clones.unwrap_or(false),
        min_similarity,
    })
}

pub(super) fn plugins(
    entries: Vec<PluginEntry>,
) -> Result<Vec<PluginDeclaration>, ConfigurationFileError> {
    let plugins_key = KeyPath::default().child(KeySegment::Key("plugins".to_owned()));
    entries
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            let entry_key = plugins_key.child(KeySegment::Index(index));
            let (name, kind, path, url, sha256) = entry.into_parts();
            if name.trim().is_empty() {
                return Err(ConfigurationFileError::EmptyPluginField {
                    file: ConfigFileKind::Json,
                    key: entry_key.child(KeySegment::Key("name".to_owned())),
                });
            }
            let source = match (path, url) {
                (Some(path), None) => {
                    if path.trim().is_empty() {
                        return Err(ConfigurationFileError::EmptyPluginField {
                            file: ConfigFileKind::Json,
                            key: entry_key.child(KeySegment::Key("path".to_owned())),
                        });
                    }
                    PluginSource::Path(PathBuf::from(path))
                }
                (None, Some(url)) => {
                    let url_key = entry_key.child(KeySegment::Key("url".to_owned()));
                    if url.trim().is_empty() {
                        return Err(ConfigurationFileError::EmptyPluginField {
                            file: ConfigFileKind::Json,
                            key: url_key,
                        });
                    }
                    let normalized = normalize_https_url(&url).ok_or_else(|| {
                        ConfigurationFileError::NonHttpsPluginUrl {
                            file: ConfigFileKind::Json,
                            key: url_key,
                            name: name.clone(),
                            url,
                        }
                    })?;
                    PluginSource::HttpsUrl(normalized)
                }
                (None, None) => {
                    return Err(ConfigurationFileError::MissingPluginSource {
                        file: ConfigFileKind::Json,
                        key: entry_key,
                        name,
                    });
                }
                (Some(_), Some(_)) => {
                    return Err(ConfigurationFileError::AmbiguousPluginSource {
                        file: ConfigFileKind::Json,
                        key: entry_key,
                        name,
                    });
                }
            };
            let sha256 = Sha256Digest::parse_hex(&sha256).ok_or_else(|| {
                ConfigurationFileError::InvalidSha256 {
                    file: ConfigFileKind::Json,
                    key: entry_key.child(KeySegment::Key("sha256".to_owned())),
                    found: sha256,
                }
            })?;
            Ok(PluginDeclaration {
                name,
                kind,
                source,
                sha256,
            })
        })
        .collect()
}

fn normalize_https_url(url: &str) -> Option<String> {
    let prefix = url.get(..8)?;
    if !prefix.eq_ignore_ascii_case("https://") {
        return None;
    }
    let rest = &url[8..];
    if rest.is_empty() || rest.starts_with('/') || url.chars().any(char::is_whitespace) {
        return None;
    }
    Some(format!("https://{rest}"))
}
