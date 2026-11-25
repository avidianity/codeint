use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Write;
use std::path::{Path, PathBuf};

use codeint_model::RepoPath;

use super::schema::{ConfigurationFile, LanguageGlobs};
use crate::boundaries::BoundaryPreset;
use crate::config_error::{CONFIGURATION_FILE_NAME, CONFIGURATION_JSONC_FILE_NAME};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedLanguage {
    pub plugin: String,
    pub include: Vec<RepoPath>,
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

pub fn write_starter_configuration(
    repository_root: &Path,
    languages: &[DetectedLanguage],
    boundaries: Option<&StarterBoundaries>,
) -> Result<PathBuf, StarterConfigurationError> {
    for existing_name in [CONFIGURATION_FILE_NAME, CONFIGURATION_JSONC_FILE_NAME] {
        let existing_path = repository_root.join(existing_name);
        match std::fs::symlink_metadata(&existing_path) {
            Ok(_) => {
                return Err(StarterConfigurationError::AlreadyExists {
                    path: existing_path,
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(StarterConfigurationError::Unwritable {
                    path: existing_path,
                    source,
                });
            }
        }
    }
    let path = repository_root.join(CONFIGURATION_JSONC_FILE_NAME);
    let text = starter_configuration_text_with_boundaries(languages, boundaries);
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
    starter_configuration_text_with_boundaries(languages, None)
}

fn starter_configuration_text_with_boundaries(
    languages: &[DetectedLanguage],
    boundaries: Option<&StarterBoundaries>,
) -> String {
    let starter = ConfigurationFile::starter(
        published_schema_address(),
        (!languages.is_empty()).then(|| {
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
        }),
    );
    let mut json = serde_json::to_value(&starter)
        .expect("the configuration types serialize to JSON without fallible keys");
    if let Some(root) = json.as_object_mut() {
        root.insert("coverage_reports".to_owned(), serde_json::json!([]));
    }
    let rendered = format!("{json:#}\n").replacen(
        "  \"budgets\": {\n    \"production\": {},",
        "  \"budgets\": {\n    \"production\": {\n      // \"test_gap\": 0.2\n    },",
        1,
    );
    let mut trailing_hints = String::new();
    if languages.iter().any(|language| language.plugin == "sql") {
        trailing_hints.push_str(
            "  // Optional: map SQL folders to a Dialect with sql.dialects (postgresql, mysql, sqlite, or duckdb).\n  // \"sql\": { \"dialects\": [] }\n",
        );
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
