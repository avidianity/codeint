use std::collections::BTreeMap;
use std::path::PathBuf;

use codeint_model::{Metric, SourceSet};
use serde_json::{Value, json};

use super::schema::{
    Budget, ConfigurationFile, DuplicationConfigFile, LanguageGlobs, OutputFormat, PluginEntry,
    SourceSetBudgets, SourceSetGlobs,
};
use super::shape;
use super::types::{
    Budgets, Configuration, DEFAULT_PLUGIN_MEMORY_MB, DEFAULT_PLUGIN_TIMEOUT_MS, DuplicationConfig,
    EntryPoints, PluginLimits, SourceSetOverrides, budgets, entry_points, languages,
    new_file_budgets, source_set_overrides,
};
use super::validation::{
    allowed_regression_limit_metric_keys, coverage_reports, duplication_config,
    is_repository_wide_diff_metric, plugins, reject_unsupported_metric_keys,
};
use crate::RegressionLimits;
use crate::boundaries::{Boundaries, BoundariesFile};
use crate::config_error::{ConfigFileKind, ConfigurationFileError, compile_glob};
use crate::dependencies::{DependenciesConfig, DependenciesFile};
use crate::dialects::{DialectEntryFile, DialectRules, parse_dialects_map};
use crate::framework_profiles::{
    FrameworkProfileRuleFile, FrameworkProfilesConfig, PackageConfigFile,
};
use crate::hazards::{HazardsConfig, HazardsConfigFile};
use crate::ratchet::{RatchetConfig, RatchetConfigFile};
use crate::regression_limits::ResolvedRegressionLimit;
use crate::resolved::{
    BudgetOverride, ConfigOverrides, ConfigProvenance, ResolvedConfiguration, ResolvedSetting,
    SourceSetOverride, resolve_with_provenance,
};

fn parse_configuration_file(
    config_file_kind: ConfigFileKind,
    raw: Option<&Value>,
) -> Result<ConfigurationFile, ConfigurationFileError> {
    let Some(value) = raw else {
        return Ok(ConfigurationFile::default());
    };
    reject_unsupported_metric_keys(value)?;
    let schema = schemars::schema_for!(ConfigurationFile);
    shape::check_shape(value, schema.as_value())?;
    serde_json::from_value(value.clone()).map_err(|source| ConfigurationFileError::NotJson {
        file: config_file_kind,
        source,
    })
}

struct ParsedFeatureSections {
    boundaries: Option<Boundaries>,
    dependencies: DependenciesConfig,
    duplication: DuplicationConfig,
    entry_points: EntryPoints,
    coverage_reports: Vec<String>,
}

fn parse_feature_sections(
    raw: Option<&Value>,
    boundaries_file: Option<BoundariesFile>,
    dependencies_file: DependenciesFile,
    duplication_file: DuplicationConfigFile,
    entry_points_globs: &[String],
    coverage_report_paths: &[String],
    settings: &mut Vec<ResolvedSetting>,
) -> Result<ParsedFeatureSections, ConfigurationFileError> {
    let boundaries = boundaries_file.map(Boundaries::from_file).transpose()?;
    append_boundaries_settings(raw, boundaries.as_ref(), settings);
    let dependencies = DependenciesConfig::from_file(dependencies_file)?;
    append_dependencies_settings(raw, &dependencies, settings);
    let duplication = duplication_config(duplication_file)?;
    append_duplication_settings(raw, duplication, settings);
    let entry_points = entry_points(entry_points_globs)?;
    append_string_list_setting(raw, "entry_points", entry_points_globs, settings);
    let coverage_reports = coverage_reports(coverage_report_paths)?;
    append_string_list_setting(raw, "coverage_reports", &coverage_reports, settings);
    Ok(ParsedFeatureSections {
        boundaries,
        dependencies,
        duplication,
        entry_points,
        coverage_reports,
    })
}

pub(super) fn parse_value_with_overrides(
    config_file_kind: ConfigFileKind,
    config_file: PathBuf,
    config_file_present: bool,
    raw: Option<Value>,
    overrides: &ConfigOverrides,
) -> Result<ResolvedConfiguration, ConfigurationFileError> {
    let file = parse_configuration_file(config_file_kind, raw.as_ref())?;
    let mut effective = file.normalized_for_cache();
    let configured_format = file.format();
    let sections = file.into_sections();
    let mut settings = Vec::new();
    let (format, format_source) =
        resolve_with_provenance(OutputFormat::Toon, configured_format, None);
    push_setting(&mut settings, "format", json!(format), format_source);
    let source_set_overrides = merge_source_sets(
        raw.as_ref(),
        effective.source_sets_mut(),
        &overrides.source_sets,
        &mut settings,
    )?;
    let budgets = merge_budgets(effective.budgets_mut(), &overrides.budgets, &mut settings);
    let new_file_budgets = new_file_budgets(sections.new_file_budgets);
    append_new_file_budget_settings(&new_file_budgets, &mut settings);
    let regression_limits = sections.regression_limits.into_regression_limits();
    append_regression_limit_settings(raw.as_ref(), &regression_limits, &mut settings);
    append_languages_setting(effective.languages(), &mut settings);
    let languages = languages(config_file_kind, sections.languages)?;
    append_plugins_setting(raw.as_ref(), effective.plugins(), &mut settings);
    let plugins = plugins(sections.plugins)?;
    let plugin_limits = merge_plugin_limits(&mut effective, overrides, &mut settings);
    let features = parse_feature_sections(
        raw.as_ref(),
        sections.boundaries,
        sections.dependencies,
        sections.duplication,
        effective.entry_points(),
        effective.coverage_reports(),
        &mut settings,
    )?;
    let (dialects, hazards, ratchet) = parse_dialects_hazards_and_ratchet(
        raw.as_ref(),
        sections.dialects,
        sections.hazards,
        sections.ratchet,
        &mut settings,
    )?;
    let framework_profiles = parse_framework_profiles(
        raw.as_ref(),
        sections.framework_profiles,
        sections.packages,
        &mut settings,
    )?;
    let configuration = Configuration {
        format,
        config_file: config_file_kind,
        effective,
        source_set_overrides,
        budgets,
        new_file_budgets,
        languages,
        regression_limits,
        plugins,
        plugin_limits,
        boundaries: features.boundaries,
        dependencies: features.dependencies,
        duplication: features.duplication,
        entry_points: features.entry_points,
        coverage_reports: features.coverage_reports,
        dialects,
        hazards,
        framework_profiles,
        ratchet,
    };
    Ok(ResolvedConfiguration {
        config_file,
        config_file_present,
        configuration,
        settings,
    })
}

fn parse_dialects_hazards_and_ratchet(
    raw: Option<&Value>,
    dialects_file: BTreeMap<String, Vec<DialectEntryFile>>,
    hazards_file: HazardsConfigFile,
    ratchet_file: RatchetConfigFile,
    settings: &mut Vec<ResolvedSetting>,
) -> Result<(BTreeMap<String, DialectRules>, HazardsConfig, RatchetConfig), ConfigurationFileError>
{
    let dialects_snapshot = dialects_file.clone();
    let dialects = parse_dialects_map(dialects_file)?;
    if dialects_snapshot.is_empty() {
        push_setting(settings, "dialects", json!({}), ConfigProvenance::Default);
    } else {
        for (language, entries) in dialects_snapshot {
            let source = if has_json_path(raw, &["dialects", language.as_str()]) {
                ConfigProvenance::File
            } else {
                ConfigProvenance::Default
            };
            push_setting(
                settings,
                format!("dialects.{language}"),
                serde_json::to_value(&entries).unwrap_or_else(|_| json!([])),
                source,
            );
        }
    }

    let hazards = HazardsConfig::from_file(hazards_file)?;
    let hazards_source = if has_json_path(raw, &["hazards", "disabled_rules"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "hazards.disabled_rules",
        json!(hazards.disabled_rules),
        hazards_source,
    );

    let ratchet = RatchetConfig::from_file(ratchet_file)?;
    let ratchet_source = if has_json_path(raw, &["ratchet", "path"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "ratchet.path",
        json!(ratchet.path),
        ratchet_source,
    );
    Ok((dialects, hazards, ratchet))
}

fn parse_framework_profiles(
    raw: Option<&Value>,
    global: Option<FrameworkProfileRuleFile>,
    packages: BTreeMap<String, PackageConfigFile>,
    settings: &mut Vec<ResolvedSetting>,
) -> Result<FrameworkProfilesConfig, ConfigurationFileError> {
    let config = FrameworkProfilesConfig::from_files(global, packages)?;
    let global_source = if has_json_path(raw, &["framework_profiles"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "framework_profiles",
        config.resolved_global_value(),
        global_source,
    );
    let package_settings = config.resolved_package_settings();
    if package_settings.is_empty() {
        push_setting(settings, "packages", json!({}), ConfigProvenance::Default);
    } else {
        for (key, value) in package_settings {
            push_setting(settings, key, value, ConfigProvenance::File);
        }
    }
    Ok(config)
}

fn append_dependencies_settings(
    raw: Option<&Value>,
    dependencies: &DependenciesConfig,
    settings: &mut Vec<ResolvedSetting>,
) {
    let source = if has_json_path(raw, &["dependencies", "ignore"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    let ignored: Vec<&str> = dependencies.ignored().iter().map(String::as_str).collect();
    push_setting(settings, "dependencies.ignore", json!(ignored), source);
}

fn append_string_list_setting(
    raw: Option<&Value>,
    key: &str,
    entries: &[String],
    settings: &mut Vec<ResolvedSetting>,
) {
    let source = if has_json_path(raw, &[key]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(settings, key, json!(entries), source);
}

fn append_duplication_settings(
    raw: Option<&Value>,
    duplication: DuplicationConfig,
    settings: &mut Vec<ResolvedSetting>,
) {
    let near_clones_source = if has_json_path(raw, &["duplication", "near_clones"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "duplication.near_clones",
        json!(duplication.near_clones),
        near_clones_source,
    );
    let min_similarity_source = if has_json_path(raw, &["duplication", "min_similarity"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "duplication.min_similarity",
        json!(duplication.min_similarity.ratio()),
        min_similarity_source,
    );
}

fn has_json_path(raw: Option<&Value>, path: &[&str]) -> bool {
    let mut current = match raw {
        Some(value) => value,
        None => return false,
    };
    for segment in path {
        match current.get(*segment) {
            Some(next) => current = next,
            None => return false,
        }
    }
    true
}

fn push_setting(
    settings: &mut Vec<ResolvedSetting>,
    key: impl Into<String>,
    value: Value,
    source: ConfigProvenance,
) {
    settings.push(ResolvedSetting {
        key: key.into(),
        value,
        source,
    });
}

fn merge_source_sets(
    raw: Option<&Value>,
    globs: &mut SourceSetGlobs,
    overrides: &[SourceSetOverride],
    settings: &mut Vec<ResolvedSetting>,
) -> Result<SourceSetOverrides, ConfigurationFileError> {
    for entry in overrides {
        compile_glob(&entry.glob).map_err(|source| ConfigurationFileError::InvalidCliGlob {
            source_set: entry.source_set,
            glob: entry.glob.clone(),
            source,
        })?;
    }
    let cli_by_set = group_source_set_overrides(overrides);
    for (source_set, slot) in globs.slots_mut() {
        let source = if let Some(cli_globs) = cli_by_set.get(&source_set) {
            *slot = cli_globs.clone();
            ConfigProvenance::Cli
        } else if has_json_path(raw, &["source_sets", source_set.key()]) {
            ConfigProvenance::File
        } else {
            ConfigProvenance::Default
        };
        push_setting(
            settings,
            format!("source_sets.{}", source_set.key()),
            json!(slot),
            source,
        );
    }
    source_set_overrides(globs.clone())
}

fn group_source_set_overrides(overrides: &[SourceSetOverride]) -> BTreeMap<SourceSet, Vec<String>> {
    let mut grouped: BTreeMap<SourceSet, Vec<String>> = BTreeMap::new();
    for entry in overrides {
        grouped
            .entry(entry.source_set)
            .or_default()
            .push(entry.glob.clone());
    }
    grouped
}

fn merge_budgets(
    file_budgets: &mut SourceSetBudgets,
    overrides: &[BudgetOverride],
    settings: &mut Vec<ResolvedSetting>,
) -> Budgets {
    let parsed_file = budgets(file_budgets.clone());
    let mut resolved: BTreeMap<(SourceSet, String), (Budget, ConfigProvenance)> = BTreeMap::new();
    for (source_set, metric, budget) in parsed_file.set {
        resolved.insert((source_set, metric), (budget, ConfigProvenance::File));
    }
    for entry in overrides {
        file_budgets.set(entry.source_set, &entry.metric, entry.budget);
        resolved.insert(
            (entry.source_set, entry.metric.clone()),
            (entry.budget, ConfigProvenance::Cli),
        );
    }
    append_budget_settings(&resolved, &parsed_file.dependency_set, settings);
    Budgets {
        set: resolved
            .into_iter()
            .map(|((source_set, metric), (budget, _))| (source_set, metric, budget))
            .collect(),
        dependency_set: parsed_file.dependency_set,
    }
}

fn append_budget_settings(
    resolved: &BTreeMap<(SourceSet, String), (Budget, ConfigProvenance)>,
    dependency_set: &[(String, Budget)],
    settings: &mut Vec<ResolvedSetting>,
) {
    if resolved.is_empty() && dependency_set.is_empty() {
        push_setting(settings, "budgets", json!({}), ConfigProvenance::Default);
        return;
    }
    for source_set in SourceSet::ALL {
        for ((_, metric), (budget, source)) in
            resolved.iter().filter(|((set, _), _)| *set == source_set)
        {
            push_setting(
                settings,
                format!("budgets.{}.{metric}", source_set.key()),
                json!(budget.ratio()),
                *source,
            );
        }
    }
    for (metric, budget) in dependency_set {
        push_setting(
            settings,
            format!("budgets.{metric}"),
            json!(budget.ratio()),
            ConfigProvenance::File,
        );
    }
}

fn append_new_file_budget_settings(budgets: &Budgets, settings: &mut Vec<ResolvedSetting>) {
    if budgets.is_empty() {
        push_setting(
            settings,
            "new_file_budgets",
            json!({}),
            ConfigProvenance::Default,
        );
        return;
    }
    let resolved: BTreeMap<(SourceSet, &str), Budget> = budgets
        .set
        .iter()
        .map(|(source_set, metric, budget)| ((*source_set, metric.as_str()), *budget))
        .collect();
    for source_set in SourceSet::ALL {
        for ((_, metric), budget) in resolved.iter().filter(|((set, _), _)| *set == source_set) {
            push_setting(
                settings,
                format!("new_file_budgets.{}.{metric}", source_set.key()),
                json!(budget.ratio()),
                ConfigProvenance::File,
            );
        }
    }
}

fn push_resolved_regression_limit(
    settings: &mut Vec<ResolvedSetting>,
    prefix: &str,
    resolved: ResolvedRegressionLimit,
) {
    let ratio_rise_source = if resolved.ratio_rise_from_file {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    let new_findings_source = if resolved.new_findings_from_file {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        format!("{prefix}.ratio_rise"),
        json!(resolved.limit.ratio_rise.ratio()),
        ratio_rise_source,
    );
    push_setting(
        settings,
        format!("{prefix}.new_findings"),
        json!(resolved.limit.new_findings),
        new_findings_source,
    );
}

fn append_regression_limit_settings(
    raw: Option<&Value>,
    limits: &RegressionLimits,
    settings: &mut Vec<ResolvedSetting>,
) {
    let per_file_source = if has_json_path(raw, &["regression_limits", "per_file"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "regression_limits.per_file",
        json!(limits.per_file()),
        per_file_source,
    );
    let extra_metric_keys = limits.extra_metric_keys();
    for source_set in SourceSet::ALL {
        let metric_keys = allowed_regression_limit_metric_keys(Some(source_set), &[]);
        for metric_key in metric_keys.iter().chain(&extra_metric_keys) {
            let resolved = limits.resolved_limit_for_key(source_set, metric_key);
            let prefix = format!("regression_limits.{}.{metric_key}", source_set.key());
            push_resolved_regression_limit(settings, &prefix, resolved);
        }
    }
    for metric in Metric::ALL
        .into_iter()
        .filter(|metric| is_repository_wide_diff_metric(*metric))
    {
        let metric_key = metric.key();
        let resolved = limits.resolved_limit_for_dependency(metric_key);
        let prefix = format!("regression_limits.{metric_key}");
        push_resolved_regression_limit(settings, &prefix, resolved);
    }
}

fn append_languages_setting(
    entries: &Option<Vec<LanguageGlobs>>,
    settings: &mut Vec<ResolvedSetting>,
) {
    match entries {
        Some(entries) => push_setting(
            settings,
            "languages",
            serde_json::to_value(entries).unwrap_or(Value::Null),
            ConfigProvenance::File,
        ),
        None => push_setting(
            settings,
            "languages",
            Value::Null,
            ConfigProvenance::Default,
        ),
    }
}

fn append_plugins_setting(
    raw: Option<&Value>,
    entries: &[PluginEntry],
    settings: &mut Vec<ResolvedSetting>,
) {
    let source = if has_json_path(raw, &["plugins"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "plugins",
        serde_json::to_value(entries).unwrap_or_else(|_| json!([])),
        source,
    );
}

fn merge_plugin_limits(
    file: &mut ConfigurationFile,
    overrides: &ConfigOverrides,
    settings: &mut Vec<ResolvedSetting>,
) -> PluginLimits {
    let (memory_mb, memory_source) = resolve_with_provenance(
        DEFAULT_PLUGIN_MEMORY_MB,
        file.plugin_memory_mb(),
        overrides.plugin_memory_mb,
    );
    let (timeout_ms, timeout_source) = resolve_with_provenance(
        DEFAULT_PLUGIN_TIMEOUT_MS,
        file.plugin_timeout_ms(),
        overrides.plugin_timeout_ms,
    );
    if let Some(value) = overrides.plugin_memory_mb {
        file.set_plugin_memory_mb(value);
    }
    if let Some(value) = overrides.plugin_timeout_ms {
        file.set_plugin_timeout_ms(value);
    }
    push_setting(
        settings,
        "plugin_limits.memory_mb",
        json!(memory_mb),
        memory_source,
    );
    push_setting(
        settings,
        "plugin_limits.timeout_ms",
        json!(timeout_ms),
        timeout_source,
    );
    PluginLimits {
        memory_mb,
        timeout_ms,
    }
}

fn append_boundaries_settings(
    raw: Option<&Value>,
    boundaries: Option<&Boundaries>,
    settings: &mut Vec<ResolvedSetting>,
) {
    let Some(boundaries) = boundaries else {
        push_setting(
            settings,
            "boundaries",
            Value::Null,
            ConfigProvenance::Default,
        );
        return;
    };
    let preset_source = if boundaries.preset().is_some() {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "boundaries.preset",
        serde_json::to_value(boundaries.preset()).unwrap_or(Value::Null),
        preset_source,
    );
    let strict_source = if has_json_path(raw, &["boundaries", "strict_zones"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "boundaries.strict_zones",
        json!(boundaries.strict_zones()),
        strict_source,
    );
    let has_non_empty_source_sets = raw
        .and_then(|value| value.get("boundaries"))
        .and_then(|value| value.get("source_sets"))
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty());
    let source_sets_source = if has_non_empty_source_sets {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "boundaries.source_sets",
        json!(boundaries.source_set_keys()),
        source_sets_source,
    );
    let zones_source = if has_json_path(raw, &["boundaries", "zones"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "boundaries.zones",
        boundaries.resolved_zones_json(),
        zones_source,
    );
    let rules_source = if has_json_path(raw, &["boundaries", "rules"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "boundaries.rules",
        boundaries.resolved_rules_json(),
        rules_source,
    );
}
