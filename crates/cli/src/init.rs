use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use codeint_config::{
    BASELINE_PATH, BoundaryPreset, DEFAULT_RATCHET_PATH, DetectedLanguage, PluginHostConfig,
    StarterBoundaries, StarterConfigurationError, write_adoption_starter_configuration,
    write_starter_configuration,
};
use codeint_git::{GitError, Repository, Snapshot};
use codeint_model::{InvalidRepoPath, RepoPath, SourceSet, settle_source_set};
use codeint_plugin_host::{NamedLanguagePlugin, built_in_language_plugins};

use crate::baseline;
use crate::check::{CheckError, find_workspace, load_and_measure_whole_repo, read_manifests};
use crate::ratchet::{self, RatchetError};

#[derive(Debug, thiserror::Error)]
pub(crate) enum InitError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    StarterConfiguration(#[from] StarterConfigurationError),
    #[error(transparent)]
    Check(#[from] CheckError),
    #[error(transparent)]
    Ratchet(#[from] RatchetError),
    #[error(
        "{} already exists, and codeint init --adopt never replaces it",
        path.display()
    )]
    StateFileExists {
        path: PathBuf,
        update_command: &'static str,
    },
    #[error("cannot inspect {}", path.display())]
    Inspect {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub(crate) struct AdoptedFiles {
    pub(crate) configuration: PathBuf,
    pub(crate) ratchet: PathBuf,
    pub(crate) ratchet_entries: usize,
    pub(crate) baseline: PathBuf,
    pub(crate) baselined_findings: usize,
}

pub(crate) fn init(folder: &Path) -> Result<PathBuf, InitError> {
    let repository = Repository::discover(folder)?;
    let starter = discover_starter(&repository)?;
    Ok(write_starter_configuration(
        repository.root(),
        &starter.languages,
        starter.boundaries.as_ref(),
    )?)
}

pub(crate) fn adopt(
    folder: &Path,
    plugin_host: &PluginHostConfig,
) -> Result<AdoptedFiles, InitError> {
    let repository = Repository::discover(folder)?;
    let root = repository.root().to_path_buf();
    refuse_existing_state_file(&root, DEFAULT_RATCHET_PATH, "codeint ratchet update")?;
    refuse_existing_state_file(&root, BASELINE_PATH, "codeint baseline update")?;
    let state_folder = root.join(".codeint");
    let state_folder_existed = state_folder.exists();
    let starter = discover_starter(&repository)?;
    let configuration = write_adoption_starter_configuration(
        &root,
        &starter.languages,
        starter.boundaries.as_ref(),
    )?;
    write_adopted_state(&root, plugin_host, configuration.clone()).inspect_err(|_| {
        let _ = std::fs::remove_file(&configuration);
        let _ = std::fs::remove_file(root.join(DEFAULT_RATCHET_PATH));
        let _ = std::fs::remove_file(root.join(BASELINE_PATH));
        if !state_folder_existed {
            let _ = std::fs::remove_dir(&state_folder);
        }
    })
}

fn write_adopted_state(
    root: &Path,
    plugin_host: &PluginHostConfig,
    configuration: PathBuf,
) -> Result<AdoptedFiles, InitError> {
    let (context, report) = load_and_measure_whole_repo(root, &[], plugin_host)?;
    let ratchet = ratchet::update_from_report(&context, &report, false)?;
    let baseline = baseline::update_from_report(&context, &report, false)?;
    Ok(AdoptedFiles {
        configuration,
        ratchet: root.join(DEFAULT_RATCHET_PATH),
        ratchet_entries: ratchet.summary().total_entries,
        baseline: root.join(BASELINE_PATH),
        baselined_findings: baseline.total_findings,
    })
}

fn refuse_existing_state_file(
    root: &Path,
    relative: &str,
    update_command: &'static str,
) -> Result<(), InitError> {
    let path = root.join(relative);
    match std::fs::symlink_metadata(&path) {
        Ok(_) => Err(InitError::StateFileExists {
            path,
            update_command,
        }),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(())
        }
        Err(source) => Err(InitError::Inspect { path, source }),
    }
}

const PULL_REQUEST_WORKFLOW: &str = include_str!("init_adopt_workflow.yml");

pub(crate) fn render_adoption(adopted: &AdoptedFiles) -> String {
    format!(
        "Wrote {configuration}\n\
         Wrote {ratchet} ({entries} Metric ceilings)\n\
         Wrote {baseline} ({findings} existing Findings)\n\
         \n\
         Next steps:\n\
         1. Commit codeint.jsonc and the .codeint folder.\n\
         2. Run codeint check locally and in your coding agent loop; it hides the existing Findings and fails on any new one.\n\
         3. Save this as .github/workflows/codeint.yml to gate every pull request:\n\
         \n\
         {workflow}\n\
         4. After you fix legacy code, run codeint ratchet update and codeint baseline update, and commit the result.\n\
         5. To hold every new file to a strict bar, uncomment new_file_budgets in codeint.jsonc.\n\
         \n\
         Guide: https://github.com/avidianity/codeint/blob/main/docs/guide/adopting-on-an-existing-codebase.md\n",
        configuration = adopted.configuration.display(),
        ratchet = adopted.ratchet.display(),
        entries = adopted.ratchet_entries,
        baseline = adopted.baseline.display(),
        findings = adopted.baselined_findings,
        workflow = PULL_REQUEST_WORKFLOW,
    )
}

struct Starter {
    languages: Vec<DetectedLanguage>,
    boundaries: Option<StarterBoundaries>,
}

fn discover_starter(repository: &Repository) -> Result<Starter, GitError> {
    let tree = repository.working_tree()?;
    let plugins = built_in_language_plugins();
    let languages = detect_languages(tree.files(), plugins)?;
    let boundaries = discover_starter_boundaries(&tree, plugins)?;
    Ok(Starter {
        languages,
        boundaries,
    })
}

fn discover_starter_boundaries(
    tree: &Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
) -> Result<Option<StarterBoundaries>, GitError> {
    let workspace_roots = discover_workspace_roots(tree, plugins)?;
    if let Some(workspace_boundaries) = StarterBoundaries::from_workspace_packages(&workspace_roots)
    {
        return Ok(Some(workspace_boundaries));
    }
    let source_files = import_capable_production_files(tree.files(), plugins);
    Ok(BoundaryPreset::detect(&source_files).map(StarterBoundaries::Preset))
}

fn discover_workspace_roots(
    tree: &Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
) -> Result<Vec<RepoPath>, GitError> {
    let manifests = match read_manifests(tree, plugins) {
        Ok(manifests) => manifests,
        Err(CheckError::Git(error)) => return Err(error),
        Err(_) => return Ok(Vec::new()),
    };
    let workspace = match find_workspace(plugins, &manifests) {
        Ok(workspace) => workspace,
        Err(CheckError::Git(error)) => return Err(error),
        Err(_) => return Ok(Vec::new()),
    };
    Ok(workspace.member_roots().cloned().collect())
}

fn import_capable_production_files(
    files: &[RepoPath],
    plugins: &[NamedLanguagePlugin<'_>],
) -> Vec<RepoPath> {
    files
        .iter()
        .filter(|path| is_import_capable_production_file(path, plugins))
        .cloned()
        .collect()
}

fn is_import_capable_production_file(path: &RepoPath, plugins: &[NamedLanguagePlugin<'_>]) -> bool {
    if settle_source_set(path, "", SourceSet::Production) != SourceSet::Production {
        return false;
    }
    path.extension().is_some_and(|extension| {
        plugins.iter().any(|named| {
            named.plugin.has_imports()
                && named
                    .plugin
                    .extensions()
                    .iter()
                    .any(|claimed| claimed == extension)
        })
    })
}

fn detect_languages(
    files: &[RepoPath],
    plugins: &[NamedLanguagePlugin<'_>],
) -> Result<Vec<DetectedLanguage>, GitError> {
    let mut detected = Vec::new();
    for named in plugins {
        let extensions = named.plugin.extensions();
        let top_level_paths: BTreeSet<RepoPath> = files
            .iter()
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extensions.iter().any(|claimed| claimed == extension))
            })
            .map(top_level_path)
            .collect::<Result<_, InvalidRepoPath>>()?;
        if !top_level_paths.is_empty() {
            detected.push(DetectedLanguage {
                plugin: named.name.to_owned(),
                include: top_level_paths.into_iter().collect(),
            });
        }
    }
    Ok(detected)
}

fn top_level_path(path: &RepoPath) -> Result<RepoPath, InvalidRepoPath> {
    let first_segment = path.as_str().split('/').next().unwrap_or(path.as_str());
    RepoPath::new(first_segment)
}
