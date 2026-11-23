use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use codeint_config::{
    Configuration, ConfigurationInspection, LanguageCoverage, PluginDeclaration, PluginHostConfig,
    PluginKind, PluginSource, SchemaVersionStatus, inspect_configuration,
};
use codeint_git::{GitError, Repository, Snapshot};
use codeint_plugin_host::{sha256, sha256_hex};
use codeint_report::{DoctorCheck, DoctorReport};

use crate::check::{BUILT_IN_LANGUAGE_PLUGINS, find_workspace, read_manifests};
use crate::describe;
use crate::remedy::{configuration_file_remedy, git_remedy};

const NEWER_SCHEMA_REMEDY: &str = "Run codeint update (or upgrade the codeint npm package) to match or exceed the $schema version, or lower the $schema tag in codeint.json.";

pub(crate) fn doctor(folder: &Path, plugin_host: &PluginHostConfig) -> DoctorReport {
    let mut checks = Vec::new();
    let (git_check, repository, root) = check_git(folder);
    checks.push(git_check);

    let (config_checks, configuration) = check_configuration(&root);
    checks.extend(config_checks);

    if let Some(repository) = &repository {
        checks.extend(check_repository_contents(
            repository,
            configuration.as_ref(),
        ));
    }

    if let Some(configuration) = &configuration {
        checks.extend(check_plugins(&root, configuration, plugin_host));
    } else if repository.is_some() && !root.join("codeint.json").exists() {
        checks.push(DoctorCheck::pass(
            "plugins",
            "No external Wasm Plugins configured",
        ));
    }

    DoctorReport::new(
        env!("CARGO_PKG_VERSION"),
        root.display().to_string(),
        checks,
    )
}

fn check_git(folder: &Path) -> (DoctorCheck, Option<Repository>, PathBuf) {
    match Repository::discover(folder) {
        Ok(repository) => {
            let root = repository.root().to_path_buf();
            let check = DoctorCheck::pass(
                "git",
                format!("Git repository detected at {}", root.display()),
            );
            (check, Some(repository), root)
        }
        Err(GitError::NotInRepository { .. }) => {
            let check = DoctorCheck::warn("git", "Not inside a Git repository")
                .with_detail(
                    "codeint check, diff, trend, and init require a Git repository; diagnostics below inspect the target directory directly.",
                )
                .with_remedy("Run git init or run codeint doctor inside a Git repository.");
            (check, None, folder.to_path_buf())
        }
        Err(error) => {
            let check = DoctorCheck::error("git", describe(&error)).with_remedy(git_remedy(&error));
            (check, None, folder.to_path_buf())
        }
    }
}

fn check_configuration(root: &Path) -> (Vec<DoctorCheck>, Option<Configuration>) {
    match inspect_configuration(root) {
        ConfigurationInspection::Missing { .. } => {
            let check =
                DoctorCheck::warn("config", "No codeint.json found; using built-in defaults")
                    .with_remedy("Run codeint init to write a starter codeint.json.");
            (vec![check], None)
        }
        ConfigurationInspection::Valid {
            schema,
            configuration,
            ..
        } => {
            let checks = valid_configuration_checks(&schema, &configuration);
            (checks, Some(*configuration))
        }
        ConfigurationInspection::Invalid { schema, error, .. } => {
            let config_check = DoctorCheck::error("config", describe(&error))
                .with_remedy(configuration_file_remedy(&error));
            let mut checks = vec![config_check];
            if let Some(schema_status) = schema {
                checks.push(schema_check(&schema_status));
            }
            (checks, None)
        }
    }
}

fn valid_configuration_checks(
    schema: &SchemaVersionStatus,
    configuration: &Configuration,
) -> Vec<DoctorCheck> {
    let has_external_metrics = configuration
        .plugins
        .iter()
        .any(|plugin| plugin.kind == PluginKind::Metric);
    let config_check =
        if !has_external_metrics && let Err(error) = configuration.validate_metrics(&[]) {
            DoctorCheck::error("config", describe(&error))
                .with_remedy(configuration_file_remedy(&error))
        } else {
            DoctorCheck::pass("config", "codeint.json is valid")
        };
    vec![config_check, schema_check(schema)]
}

fn schema_check(status: &SchemaVersionStatus) -> DoctorCheck {
    let binary_version = env!("CARGO_PKG_VERSION");
    match status {
        SchemaVersionStatus::Missing => {
            DoctorCheck::pass("schema", "No $schema set in codeint.json (optional)")
        }
        SchemaVersionStatus::Compatible {
            url,
            version: Some(version),
        } => DoctorCheck::pass(
            "schema",
            format!("$schema targets codeint v{version} (compatible with v{binary_version})"),
        )
        .with_detail(url.clone()),
        SchemaVersionStatus::Compatible { url, version: None } => {
            DoctorCheck::pass("schema", "Custom $schema configured").with_detail(url.clone())
        }
        SchemaVersionStatus::NewerThanBinary {
            url,
            config_version,
            binary_version,
        } => DoctorCheck::error(
            "schema",
            format!(
                "codeint.json targets codeint v{config_version} via $schema, which is newer than this binary (v{binary_version})"
            ),
        )
        .with_detail(url.clone())
        .with_remedy(NEWER_SCHEMA_REMEDY),
    }
}

fn check_repository_contents(
    repository: &Repository,
    configuration: Option<&Configuration>,
) -> Vec<DoctorCheck> {
    let tree = match repository.working_tree() {
        Ok(tree) => tree,
        Err(error) => {
            return vec![
                DoctorCheck::error("languages", describe(&error)).with_remedy(git_remedy(&error)),
            ];
        }
    };
    vec![
        check_languages(&tree, configuration),
        check_workspace(&tree),
    ]
}

fn check_languages(tree: &Snapshot, configuration: Option<&Configuration>) -> DoctorCheck {
    let mut plugin_names: Vec<&str> = BUILT_IN_LANGUAGE_PLUGINS
        .iter()
        .map(|plugin| plugin.name)
        .collect();
    if let Some(config) = configuration {
        for plugin in &config.plugins {
            if plugin.kind == PluginKind::Language && !plugin_names.contains(&plugin.name.as_str())
            {
                plugin_names.push(plugin.name.as_str());
            }
        }
    }
    let default_config = Configuration::default();
    let languages = &configuration.unwrap_or(&default_config).languages;
    let coverage = match languages.coverage_of(&plugin_names) {
        Ok(coverage) => coverage,
        Err(error) => {
            return DoctorCheck::error("languages", describe(&error))
                .with_remedy(configuration_file_remedy(&error));
        }
    };
    let counts = count_language_files(tree, &coverage);
    if counts.is_empty() {
        return DoctorCheck::warn(
            "languages",
            "No measurable source files matched by any built-in Language plugin",
        );
    }
    let total_files: usize = counts.iter().map(|(_, count)| *count).sum();
    let breakdown: Vec<String> = counts
        .iter()
        .map(|(name, count)| format!("{name}: {count}"))
        .collect();
    DoctorCheck::pass(
        "languages",
        format!(
            "Detected {} Language {} across {} {} ({})",
            counts.len(),
            if counts.len() == 1 {
                "plugin"
            } else {
                "plugins"
            },
            total_files,
            if total_files == 1 { "file" } else { "files" },
            breakdown.join(", ")
        ),
    )
}

fn count_language_files(
    tree: &Snapshot,
    coverage: &[LanguageCoverage<'_>],
) -> Vec<(String, usize)> {
    let mut counts = Vec::new();
    for (index, named) in BUILT_IN_LANGUAGE_PLUGINS.iter().enumerate() {
        let extensions = named.plugin.extensions();
        let matched = tree
            .files()
            .iter()
            .filter(|path| {
                path.extension()
                    .is_some_and(|ext| extensions.iter().any(|c| c == ext))
                    && coverage[index].covers(path)
            })
            .count();
        if matched > 0 {
            counts.push((named.name.to_owned(), matched));
        }
    }
    counts
}

fn check_workspace(tree: &Snapshot) -> DoctorCheck {
    let manifests = match read_manifests(tree, &BUILT_IN_LANGUAGE_PLUGINS) {
        Ok(manifests) => manifests,
        Err(error) => {
            return DoctorCheck::error("workspace", describe(&error));
        }
    };
    let workspace = match find_workspace(&BUILT_IN_LANGUAGE_PLUGINS, &manifests) {
        Ok(workspace) => workspace,
        Err(error) => {
            return DoctorCheck::error("workspace", describe(&error)).with_remedy(
                "Fix the manifest that the message names so that your package manager accepts it.",
            );
        }
    };
    if !workspace.has_members() {
        return DoctorCheck::pass(
            "workspace",
            "Single-package repository (no Workspace packages)",
        );
    }
    let members: Vec<String> = workspace
        .member_roots()
        .map(|root| root.as_str().to_owned())
        .collect();
    DoctorCheck::pass(
        "workspace",
        format!(
            "Discovered {} Workspace {}",
            members.len(),
            if members.len() == 1 {
                "package"
            } else {
                "packages"
            }
        ),
    )
    .with_detail(members.join(", "))
}

fn check_plugins(
    root: &Path,
    configuration: &Configuration,
    plugin_host: &PluginHostConfig,
) -> Vec<DoctorCheck> {
    if configuration.plugins.is_empty() {
        return vec![DoctorCheck::pass(
            "plugins",
            "No external Wasm Plugins configured",
        )];
    }
    let mut seen_names: BTreeSet<String> = BUILT_IN_LANGUAGE_PLUGINS
        .iter()
        .map(|plugin| plugin.name.to_owned())
        .collect();
    configuration
        .plugins
        .iter()
        .map(|declaration| check_single_plugin(root, declaration, &mut seen_names, plugin_host))
        .collect()
}

fn check_single_plugin(
    root: &Path,
    declaration: &PluginDeclaration,
    seen_names: &mut BTreeSet<String>,
    plugin_host: &PluginHostConfig,
) -> DoctorCheck {
    let id = format!("plugin:{}", declaration.name);
    if !seen_names.insert(declaration.name.clone()) {
        return DoctorCheck::error(
            id,
            format!(
                "External Wasm Plugin {:?} duplicates an existing Plugin name",
                declaration.name
            ),
        )
        .with_remedy(
            "Give each external plugin in codeint.json a unique name that does not collide with built-in plugins.",
        );
    }
    match &declaration.source {
        PluginSource::Path(declared_path) => {
            let resolved = if declared_path.is_absolute() {
                declared_path.clone()
            } else {
                root.join(declared_path)
            };
            verify_plugin_file(
                id,
                &declaration.name,
                &resolved,
                &declaration.sha256.to_hex(),
                &declared_path.display().to_string(),
            )
        }
        PluginSource::HttpsUrl(url) => check_remote_plugin(id, declaration, url, plugin_host),
    }
}

fn check_remote_plugin(
    id: String,
    declaration: &PluginDeclaration,
    url: &str,
    plugin_host: &PluginHostConfig,
) -> DoctorCheck {
    let expected = declaration.sha256.to_hex();
    let cached_path = plugin_host
        .cache_dir
        .as_ref()
        .map(|dir| dir.join(format!("{expected}.wasm")));
    if let Some(path) = cached_path
        && path.exists()
    {
        return verify_plugin_file(
            id,
            &declaration.name,
            &path,
            &expected,
            &format!("cached from {url}"),
        );
    }
    DoctorCheck::warn(
        id,
        format!(
            "External Wasm Plugin {:?} is not cached locally yet",
            declaration.name
        ),
    )
    .with_detail(format!(
        "url: {url} (doctor runs offline and does not download remote plugins)"
    ))
    .with_remedy("Run codeint check to download and verify this plugin into the local cache.")
}

fn verify_plugin_file(
    id: String,
    name: &str,
    resolved_path: &Path,
    expected_hex: &str,
    label: &str,
) -> DoctorCheck {
    let bytes = match std::fs::read(resolved_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return DoctorCheck::error(
                id,
                format!("Cannot read external Wasm Plugin {name:?} at {label}: {error}"),
            )
            .with_remedy(
                "Check that the plugin path in codeint.json points to an existing readable WebAssembly component file.",
            );
        }
    };
    let actual_hex = sha256_hex(&sha256(&bytes));
    if actual_hex != expected_hex {
        return DoctorCheck::error(
            id,
            format!("SHA-256 hash mismatch for external Wasm Plugin {name:?} at {label}"),
        )
        .with_detail(format!("expected {expected_hex}, found {actual_hex}"))
        .with_remedy(
            "Verify the plugin file has not been modified, then update sha256 in codeint.json to match the trusted file.",
        );
    }
    DoctorCheck::pass(
        id,
        format!("External Wasm Plugin {name:?} verified ({label})"),
    )
    .with_detail(format!("sha256: {expected_hex}"))
}
