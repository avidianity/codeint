use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Write;
use std::path::{Path, PathBuf};

use codeint_model::{Metric, MetricScope, RepoPath, SourceSet};

use super::schema::{ConfigurationFile, LanguageGlobs};
use super::starter_formats::{StarterRenderInput, StructuredSyntax, render_structured_starter};
use crate::baseline::MeasuredBaselineMetric;
use crate::boundaries::BoundaryPreset;
use crate::command::InitFormat;
use crate::config_error::ConfigFileKind;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedLanguage {
    pub plugin: String,
    pub include: Vec<RepoPath>,
    pub known_dialects: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StarterBoundaryZone {
    pub name: String,
    pub pattern: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StarterBoundaries {
    WorkspaceZones(Vec<StarterBoundaryZone>),
    Preset(BoundaryPreset),
}

impl StarterBoundaries {
    pub fn from_workspace_packages(roots: &[RepoPath]) -> Option<StarterBoundaries> {
        let unique_roots: BTreeSet<&RepoPath> = roots.iter().collect();
        if unique_roots.len() < 2 {
            return None;
        }
        let mut basename_counts: BTreeMap<&str, usize> = BTreeMap::new();
        for root in &unique_roots {
            *basename_counts.entry(workspace_basename(root)).or_insert(0) += 1;
        }
        let mut zones: Vec<StarterBoundaryZone> = unique_roots
            .into_iter()
            .map(|root| starter_zone_for_root(root, &basename_counts))
            .collect();
        zones.sort_by(|left, right| left.name.cmp(&right.name));
        Some(StarterBoundaries::WorkspaceZones(zones))
    }
}

fn workspace_basename(root: &RepoPath) -> &str {
    root.as_str()
        .rsplit_once('/')
        .map_or(root.as_str(), |(_, last)| last)
}

fn starter_zone_for_root(
    root: &RepoPath,
    basename_counts: &BTreeMap<&str, usize>,
) -> StarterBoundaryZone {
    let basename = workspace_basename(root);
    let candidate = if basename_counts.get(basename).copied().unwrap_or(0) == 1
        && is_valid_zone_name(basename)
    {
        basename
    } else {
        root.as_str()
    };
    let name = if is_valid_zone_name(candidate) {
        candidate.to_owned()
    } else {
        format!("package-{candidate}")
    };
    let pattern = format!("{}/**", globset::escape(root.as_str()));
    StarterBoundaryZone { name, pattern }
}

fn is_valid_zone_name(name: &str) -> bool {
    !name.trim().is_empty() && name != "*"
}

#[derive(Debug, thiserror::Error)]
pub enum StarterConfigurationError {
    #[error("{} already exists, and codeint init never replaces it", path.display())]
    AlreadyExists { path: PathBuf },
    #[error("cannot write {}", path.display())]
    Unwritable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub fn published_schema_address() -> String {
    format!(
        "https://raw.githubusercontent.com/avidianity/codeint/v{}/schema/codeint.schema.json",
        env!("CARGO_PKG_VERSION")
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StarterKind {
    Standard,
    Adoption,
}

pub fn write_starter_configuration(
    repository_root: &Path,
    languages: &[DetectedLanguage],
    boundaries: Option<&StarterBoundaries>,
) -> Result<PathBuf, StarterConfigurationError> {
    write_starter_configuration_in_format(repository_root, languages, boundaries, InitFormat::Jsonc)
}

pub fn write_starter_configuration_in_format(
    repository_root: &Path,
    languages: &[DetectedLanguage],
    boundaries: Option<&StarterBoundaries>,
    format: InitFormat,
) -> Result<PathBuf, StarterConfigurationError> {
    write_starter(
        repository_root,
        languages,
        boundaries,
        StarterKind::Standard,
        format,
    )
}

pub fn write_adoption_starter_configuration(
    repository_root: &Path,
    languages: &[DetectedLanguage],
    boundaries: Option<&StarterBoundaries>,
    format: InitFormat,
) -> Result<PathBuf, StarterConfigurationError> {
    write_starter(
        repository_root,
        languages,
        boundaries,
        StarterKind::Adoption,
        format,
    )
}

pub fn write_adoption_zero_budgets(
    repository_root: &Path,
    languages: &[DetectedLanguage],
    boundaries: Option<&StarterBoundaries>,
    recorded: &[MeasuredBaselineMetric],
    format: InitFormat,
) -> Result<PathBuf, StarterConfigurationError> {
    let path = repository_root.join(format.file_name());
    let text = starter_configuration_text_for_format(
        languages,
        boundaries,
        StarterKind::Adoption,
        &zero_budget_keys(recorded),
        format,
    );
    std::fs::write(&path, text).map_err(|source| StarterConfigurationError::Unwritable {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

pub(super) struct ZeroBudgetKeys {
    pub(super) production: Vec<&'static str>,
    pub(super) test: Vec<&'static str>,
    pub(super) dependencies: Vec<&'static str>,
}

impl ZeroBudgetKeys {
    fn none() -> ZeroBudgetKeys {
        ZeroBudgetKeys {
            production: Vec::new(),
            test: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    fn insert_into(&self, json: &mut serde_json::Value) {
        let root = json
            .as_object_mut()
            .expect("the starter configuration serializes to a JSON object");
        let budgets = object_entry(root, "budgets");
        for key in &self.dependencies {
            budgets.insert((*key).to_owned(), serde_json::json!(0));
        }
        for (source_set, keys) in [("production", &self.production), ("test", &self.test)] {
            let section = object_entry(budgets, source_set);
            for key in keys {
                section.insert((*key).to_owned(), serde_json::json!(0));
            }
        }
    }
}

fn object_entry<'a>(
    parent: &'a mut serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> &'a mut serde_json::Map<String, serde_json::Value> {
    let entry = parent
        .entry(key.to_owned())
        .or_insert_with(|| serde_json::json!({}));
    if !entry.is_object() {
        *entry = serde_json::json!({});
    }
    entry
        .as_object_mut()
        .expect("the entry was just made a JSON object")
}

fn zero_budget_keys(recorded: &[MeasuredBaselineMetric]) -> ZeroBudgetKeys {
    let keys_in = |source_set: Option<SourceSet>| -> Vec<&'static str> {
        Metric::ALL
            .into_iter()
            .filter(|metric| *metric != Metric::TestGap)
            .filter(|metric| match source_set {
                Some(source_set) => metric.scope().applies_to_source_set(source_set),
                None => metric.scope() == MetricScope::RepositoryWide,
            })
            .map(Metric::key)
            .filter(|key| {
                recorded
                    .iter()
                    .any(|item| item.source_set == source_set && item.metric == *key)
            })
            .collect()
    };
    ZeroBudgetKeys {
        production: keys_in(Some(SourceSet::Production)),
        test: keys_in(Some(SourceSet::Test)),
        dependencies: keys_in(None),
    }
}

fn write_starter(
    repository_root: &Path,
    languages: &[DetectedLanguage],
    boundaries: Option<&StarterBoundaries>,
    kind: StarterKind,
    format: InitFormat,
) -> Result<PathBuf, StarterConfigurationError> {
    for existing_kind in ConfigFileKind::ALL {
        let existing_path = repository_root.join(existing_kind.file_name());
        match std::fs::symlink_metadata(&existing_path) {
            Ok(_) => {
                return Err(StarterConfigurationError::AlreadyExists {
                    path: existing_path,
                });
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) => {}
            Err(source) => {
                return Err(StarterConfigurationError::Unwritable {
                    path: existing_path,
                    source,
                });
            }
        }
    }
    let path = repository_root.join(format.file_name());
    let text = starter_configuration_text_for_format(
        languages,
        boundaries,
        kind,
        &ZeroBudgetKeys::none(),
        format,
    );
    let unwritable = |source| StarterConfigurationError::Unwritable {
        path: path.clone(),
        source,
    };
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|source| {
            if source.kind() == std::io::ErrorKind::AlreadyExists {
                StarterConfigurationError::AlreadyExists { path: path.clone() }
            } else {
                unwritable(source)
            }
        })?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.flush())
        .map_err(unwritable)?;
    Ok(path)
}

#[cfg(test)]
fn starter_configuration_text(languages: &[DetectedLanguage]) -> String {
    starter_configuration_text_with_boundaries(
        languages,
        None,
        StarterKind::Standard,
        &ZeroBudgetKeys::none(),
    )
}

fn starter_configuration_text_for_format(
    languages: &[DetectedLanguage],
    boundaries: Option<&StarterBoundaries>,
    kind: StarterKind,
    zero_budgets: &ZeroBudgetKeys,
    format: InitFormat,
) -> String {
    let schema_address = published_schema_address();
    match format {
        InitFormat::Jsonc => starter_json_text(
            &schema_address,
            languages,
            boundaries,
            kind,
            zero_budgets,
            true,
        ),
        InitFormat::Json => starter_json_text(
            &schema_address,
            languages,
            boundaries,
            kind,
            zero_budgets,
            false,
        ),
        InitFormat::Toml => render_structured(
            StructuredSyntax::Toml,
            &schema_address,
            languages,
            boundaries,
            kind,
            zero_budgets,
        ),
        InitFormat::Yaml => render_structured(
            StructuredSyntax::Yaml,
            &schema_address,
            languages,
            boundaries,
            kind,
            zero_budgets,
        ),
    }
}

fn render_structured(
    syntax: StructuredSyntax,
    schema_address: &str,
    languages: &[DetectedLanguage],
    boundaries: Option<&StarterBoundaries>,
    kind: StarterKind,
    zero_budgets: &ZeroBudgetKeys,
) -> String {
    let language_pairs: Vec<(String, Vec<RepoPath>, Vec<String>)> = languages
        .iter()
        .map(|lang| {
            (
                lang.plugin.clone(),
                lang.include.clone(),
                lang.known_dialects.clone(),
            )
        })
        .collect();
    let zones: Option<Vec<(String, String)>> = match boundaries {
        Some(StarterBoundaries::WorkspaceZones(zones)) => Some(
            zones
                .iter()
                .map(|z| (z.name.clone(), z.pattern.clone()))
                .collect(),
        ),
        _ => None,
    };
    let preset = match boundaries {
        Some(StarterBoundaries::Preset(preset)) => Some(*preset),
        _ => None,
    };
    let input = StarterRenderInput {
        schema_address,
        languages: &language_pairs,
        boundary_zones: zones.as_deref(),
        boundary_preset: preset,
        adoption: kind == StarterKind::Adoption,
        production_zeros: &zero_budgets.production,
        test_zeros: &zero_budgets.test,
        dependency_zeros: &zero_budgets.dependencies,
    };
    render_structured_starter(syntax, &input)
}

#[cfg(test)]
fn starter_configuration_text_with_boundaries(
    languages: &[DetectedLanguage],
    boundaries: Option<&StarterBoundaries>,
    kind: StarterKind,
    zero_budgets: &ZeroBudgetKeys,
) -> String {
    starter_configuration_text_for_format(
        languages,
        boundaries,
        kind,
        zero_budgets,
        InitFormat::Jsonc,
    )
}

fn starter_json_text(
    schema_address: &str,
    languages: &[DetectedLanguage],
    boundaries: Option<&StarterBoundaries>,
    kind: StarterKind,
    zero_budgets: &ZeroBudgetKeys,
    with_comments: bool,
) -> String {
    let detected_languages = (!languages.is_empty()).then(|| {
        languages
            .iter()
            .map(|language| {
                LanguageGlobs::new(
                    language.plugin.clone(),
                    language
                        .include
                        .iter()
                        .map(|path| globset::escape(path.as_str()))
                        .collect(),
                )
            })
            .collect()
    });
    let starter = match kind {
        StarterKind::Standard => {
            ConfigurationFile::starter(schema_address.to_owned(), detected_languages)
        }
        StarterKind::Adoption => {
            ConfigurationFile::adoption_starter(schema_address.to_owned(), detected_languages)
        }
    };
    let mut json = serde_json::to_value(&starter)
        .expect("the configuration types serialize to JSON without fallible keys");
    if let Some(root) = json.as_object_mut() {
        root.insert("coverage_reports".to_owned(), serde_json::json!([]));
    }
    zero_budgets.insert_into(&mut json);
    let rendered = format!("{json:#}\n");
    if !with_comments {
        return rendered;
    }
    add_jsonc_hints(rendered, languages, boundaries, kind, zero_budgets)
}

fn add_jsonc_hints(
    rendered: String,
    languages: &[DetectedLanguage],
    boundaries: Option<&StarterBoundaries>,
    kind: StarterKind,
    zero_budgets: &ZeroBudgetKeys,
) -> String {
    let rendered = if zero_budgets.production.is_empty() {
        rendered.replacen(
            "  \"budgets\": {\n    \"production\": {},",
            "  \"budgets\": {\n    \"production\": {\n      // \"test_gap\": 0.2\n    },",
            1,
        )
    } else {
        rendered.replacen(
            "  \"budgets\": {\n    \"production\": {\n",
            "  \"budgets\": {\n    \"production\": {\n      // \"test_gap\": 0.2,\n",
            1,
        )
    };
    let mut trailing_hints = String::new();
    if kind == StarterKind::Adoption {
        trailing_hints.push_str(NEW_FILE_BUDGETS_HINT);
    }
    for language in languages
        .iter()
        .filter(|language| !language.known_dialects.is_empty())
    {
        let plugin_json = serde_json::to_string(&language.plugin).expect("strings serialize");
        let _ = writeln!(
            trailing_hints,
            "  // Optional: map {} folders to a Dialect with dialects.{} ({}).",
            language.plugin,
            language.plugin,
            language.known_dialects.join(", ")
        );
        let _ = writeln!(trailing_hints, "  // \"dialects\": {{ {plugin_json}: [] }}");
    }
    if let Some(starter_boundaries) = boundaries {
        render_commented_boundaries(&mut trailing_hints, starter_boundaries);
    }
    if !trailing_hints.is_empty()
        && let Some(prefix) = rendered.strip_suffix("}\n")
    {
        return format!("{prefix}{trailing_hints}}}\n");
    }
    rendered
}

const NEW_FILE_BUDGETS_HINT: &str = "  // Optional: uncomment to hold every new file to a strict bar in codeint diff (add a comma above).\n  // \"new_file_budgets\": {\n  //   \"production\": {\n  //     \"erosion\": 0,\n  //     \"cognitive_complexity\": 0,\n  //     \"hazards\": 0,\n  //     \"boundaries\": 0\n  //   }\n  // }\n";

fn render_commented_boundaries(output: &mut String, boundaries: &StarterBoundaries) {
    match boundaries {
        StarterBoundaries::WorkspaceZones(zones) if !zones.is_empty() => {
            output.push_str(
                "  // Optional: uncomment to enforce Import boundaries between discovered Workspace packages (add a comma above).\n",
            );
            output.push_str("  // \"boundaries\": {\n");
            output.push_str("  //   \"zones\": {\n");
            for (index, zone) in zones.iter().enumerate() {
                let name = serde_json::to_string(&zone.name).expect("strings serialize to JSON");
                let pattern =
                    serde_json::to_string(&zone.pattern).expect("strings serialize to JSON");
                let comma = if index + 1 < zones.len() { "," } else { "" };
                let _ = writeln!(
                    output,
                    "  //     {name}: {{ \"patterns\": [{pattern}] }}{comma}"
                );
            }
            output.push_str("  //   },\n");
            output.push_str("  //   \"rules\": []\n");
            output.push_str("  // }\n");
        }
        StarterBoundaries::WorkspaceZones(_) => {}
        StarterBoundaries::Preset(preset) => {
            let preset_name = preset.as_str();
            let _ = writeln!(
                output,
                "  // Optional: uncomment to enable the {preset_name} Import boundaries preset (add a comma above)."
            );
            let _ = writeln!(
                output,
                "  // \"boundaries\": {{ \"preset\": \"{preset_name}\" }}"
            );
        }
    }
}
