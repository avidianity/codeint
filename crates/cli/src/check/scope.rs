use std::collections::{BTreeMap, HashSet};

use codeint_config::{ConfigFileKind, Configuration, DialectRules, validate_dialects};
use codeint_model::{
    CodeModel, FileScopeArtifact, Package, RepoPath, Workspace, validate_scope_analysis,
};
use codeint_plugin_host::NamedLanguagePlugin;
use codeint_report::{FileDiagnosticEntry, ScopeSummaryEntry};

use super::errors::CheckError;
use super::package_filter::SelectedPackage;

pub(crate) fn validate_configured_dialects(
    plugins: &[NamedLanguagePlugin<'_>],
    configuration: &Configuration,
) -> Result<(), CheckError> {
    if configuration.dialects.is_empty() {
        return Ok(());
    }
    let known_by_language: BTreeMap<String, Vec<String>> = plugins
        .iter()
        .map(|named| (named.name.to_owned(), named.plugin.known_dialects()))
        .collect();
    validate_dialects(
        configuration.config_file(),
        &configuration.dialects,
        &known_by_language,
    )?;
    Ok(())
}

pub(crate) fn resolve_file_dialects(
    plugins: &[NamedLanguagePlugin<'_>],
    covered_files: &[(&RepoPath, usize)],
    dialects: &BTreeMap<String, DialectRules>,
    config_file: ConfigFileKind,
) -> Result<Vec<Option<String>>, CheckError> {
    covered_files
        .iter()
        .map(|&(path, index)| {
            let language = plugins[index].name;
            let Some(rules) = dialects.get(language) else {
                return Ok(None);
            };
            rules
                .dialect_for(language, path)
                .map(|matched| matched.map(str::to_owned))
                .map_err(|conflict| CheckError::DialectConflict {
                    config_file,
                    conflict,
                })
        })
        .collect()
}

pub(crate) fn apply_scope_analysis(
    plugins: &[NamedLanguagePlugin<'_>],
    models: &mut [CodeModel],
    file_dialects: &BTreeMap<RepoPath, Option<String>>,
    file_line_counts: &BTreeMap<RepoPath, usize>,
    config_file: ConfigFileKind,
) -> Result<Vec<ScopeSummaryEntry>, CheckError> {
    let mut all_summaries = Vec::new();
    for (named, model) in plugins.iter().zip(models.iter_mut()) {
        if !named.plugin.has_scope_analysis() || model.files.is_empty() {
            continue;
        }
        let summaries =
            analyze_language_scopes(named, model, file_dialects, file_line_counts, config_file)?;
        all_summaries.extend(summaries);
    }
    all_summaries.sort_by(|left, right| {
        (
            left.language.as_str(),
            left.package.as_deref(),
            left.dialect.as_deref(),
            left.kind.as_str(),
        )
            .cmp(&(
                right.language.as_str(),
                right.package.as_deref(),
                right.dialect.as_deref(),
                right.kind.as_str(),
            ))
    });
    Ok(all_summaries)
}

fn analyze_language_scopes(
    named: &NamedLanguagePlugin<'_>,
    model: &mut CodeModel,
    file_dialects: &BTreeMap<RepoPath, Option<String>>,
    file_line_counts: &BTreeMap<RepoPath, usize>,
    config_file: ConfigFileKind,
) -> Result<Vec<ScopeSummaryEntry>, CheckError> {
    let mut scopes: BTreeMap<(Option<RepoPath>, Option<String>), Vec<usize>> = BTreeMap::new();
    for (index, file) in model.files.iter().enumerate() {
        if file.model.scope_artifact.is_none() {
            continue;
        }
        let package = match model.workspace.package_of(&file.path) {
            Package::Root => None,
            Package::Member(root) => Some(root.clone()),
        };
        let dialect = file_dialects.get(&file.path).cloned().flatten();
        scopes.entry((package, dialect)).or_default().push(index);
    }

    let known_hazard_rules: HashSet<String> = if named.plugin.has_hazard_rules() {
        named.plugin.hazard_rules().into_iter().collect()
    } else {
        HashSet::new()
    };

    let mut summaries = Vec::new();
    for ((package, dialect), mut indices) in scopes {
        indices.sort_by(|&a, &b| model.files[a].path.cmp(&model.files[b].path));
        let package_root = package
            .as_ref()
            .map_or_else(String::new, |root| root.as_str().to_owned());
        let (artifacts, line_counts, path_to_index) =
            collect_scope_group(&model.files, &indices, file_line_counts);
        let artifact_paths: Vec<&str> = path_to_index.keys().map(String::as_str).collect();
        let raw_analysis = named
            .plugin
            .analyze_scope(&package_root, dialect.as_deref(), artifacts)
            .map_err(|error| CheckError::language_plugin(config_file, error))?;
        let analysis = validate_scope_analysis(
            named.name,
            &artifact_paths,
            &known_hazard_rules,
            Some(&line_counts),
            raw_analysis,
        )
        .map_err(|error| CheckError::language_plugin(config_file, error))?;

        merge_extra_hazards(&mut model.files, &path_to_index, analysis.extra_hazards);
        for summary in analysis.summaries {
            summaries.push(ScopeSummaryEntry {
                language: named.name.to_owned(),
                package: package.as_ref().map(RepoPath::to_string),
                dialect: dialect.clone(),
                kind: summary.kind,
                fields: summary
                    .fields
                    .into_iter()
                    .map(|field| (field.key, field.value))
                    .collect(),
            });
        }
    }
    Ok(summaries)
}

fn collect_scope_group(
    files: &[codeint_model::SourceFile],
    indices: &[usize],
    file_line_counts: &BTreeMap<RepoPath, usize>,
) -> (
    Vec<FileScopeArtifact>,
    BTreeMap<String, usize>,
    BTreeMap<String, usize>,
) {
    let mut artifacts = Vec::with_capacity(indices.len());
    let mut line_counts = BTreeMap::new();
    let mut path_to_index = BTreeMap::new();
    for &index in indices {
        let file = &files[index];
        let Some(bytes) = &file.model.scope_artifact else {
            continue;
        };
        let path_str = file.path.to_string();
        if let Some(&count) = file_line_counts.get(&file.path) {
            line_counts.insert(path_str.clone(), count);
        }
        path_to_index.insert(path_str.clone(), index);
        artifacts.push(FileScopeArtifact {
            path: path_str,
            source_set: file.model.source_set,
            artifact: bytes.clone(),
        });
    }
    (artifacts, line_counts, path_to_index)
}

fn merge_extra_hazards(
    files: &mut [codeint_model::SourceFile],
    path_to_index: &BTreeMap<String, usize>,
    extra_hazards: Vec<codeint_model::FileHazards>,
) {
    for entry in extra_hazards {
        if entry.hazard_lines.is_empty() {
            continue;
        }
        let Some(&index) = path_to_index.get(&entry.path) else {
            continue;
        };
        let target = &mut files[index].model.hazard_lines;
        target.extend(entry.hazard_lines);
        target.sort_by(|left, right| (left.line, &left.rule).cmp(&(right.line, &right.rule)));
        target.dedup();
    }
}

pub(crate) fn filter_extras_to_package(
    diagnostics: &mut Vec<FileDiagnosticEntry>,
    summaries: &mut Vec<ScopeSummaryEntry>,
    workspace: &Workspace,
    selected: &SelectedPackage,
) {
    diagnostics.retain(|entry| {
        RepoPath::new(&entry.path)
            .ok()
            .is_some_and(|path| selected.matches(workspace.package_of(&path)))
    });
    summaries.retain(|summary| match (selected, summary.package.as_deref()) {
        (SelectedPackage::Root, None) => true,
        (SelectedPackage::Member(root), Some(pkg)) => pkg == root.as_str(),
        _ => false,
    });
}

pub(crate) fn validate_disabled_hazard_rules(
    plugins: &[NamedLanguagePlugin<'_>],
    configuration: &Configuration,
) -> Result<(), CheckError> {
    if configuration.hazards.disabled_rules.is_empty() {
        return Ok(());
    }
    let known: HashSet<String> = plugins
        .iter()
        .filter(|named| named.plugin.has_hazard_rules())
        .flat_map(|named| named.plugin.hazard_rules())
        .collect();
    for rule in &configuration.hazards.disabled_rules {
        if !known.contains(rule) {
            return Err(CheckError::UnknownHazardRule {
                config_file: configuration.config_file(),
                rule: rule.clone(),
            });
        }
    }
    Ok(())
}
