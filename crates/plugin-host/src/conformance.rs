use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use codeint_config::{PluginKind, PluginLimits};
use codeint_model::language::{
    FlaggedLine, Function, FunctionCoverage, HazardLine, Token, TokenKind,
};
use codeint_model::{
    CodeModel, FileModel, FileScopeArtifact, FrameworkProfileSelection, ImportResolution,
    LanguageFacts, LanguagePlugin, LanguagePluginError, LowerError, Manifest, Metric, MetricPlugin,
    MetricPluginError, Package, PackageFrameworkProfiles, RepoPath, ResolveError, RuntimeImport,
    SourceFile, SourceSet, Workspace, choose_manifests, empty_file_model, validate_flagged_lines,
    validate_hazard_lines, validate_manifest_analysis, validate_scope_analysis,
};

use crate::{PluginHostError, WasmLanguagePlugin, WasmMetricPlugin, verify_plugin_component};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConformanceStatus {
    Pass,
    Fail,
    Skipped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConformanceCheck {
    pub id: &'static str,
    pub status: ConformanceStatus,
    pub message: String,
    pub hint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndToEndFile {
    pub path: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndToEndProbe {
    pub files: Vec<EndToEndFile>,
    pub framework_profiles: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConformanceReport {
    pub kind: PluginKind,
    pub checks: Vec<ConformanceCheck>,
    pub end_to_end: Option<EndToEndProbe>,
}

impl ConformanceReport {
    pub fn passed(&self) -> bool {
        self.checks
            .iter()
            .all(|check| check.status != ConformanceStatus::Fail)
    }

    fn push(&mut self, check: ConformanceCheck) {
        self.checks.push(check);
    }

    fn pass(&mut self, id: &'static str, message: impl Into<String>) {
        self.push(ConformanceCheck {
            id,
            status: ConformanceStatus::Pass,
            message: message.into(),
            hint: None,
        });
    }

    fn fail(&mut self, id: &'static str, message: impl Into<String>, hint: impl Into<String>) {
        self.push(ConformanceCheck {
            id,
            status: ConformanceStatus::Fail,
            message: message.into(),
            hint: Some(hint.into()),
        });
    }

    fn skip(&mut self, id: &'static str, message: impl Into<String>) {
        self.push(ConformanceCheck {
            id,
            status: ConformanceStatus::Skipped,
            message: message.into(),
            hint: None,
        });
    }

    fn skip_remaining(&mut self, ids: &[&'static str], reason: &str) {
        for id in ids {
            if !self.checks.iter().any(|check| check.id == *id) {
                self.skip(id, reason);
            }
        }
    }

    fn finalize(&mut self, order: &[&'static str]) {
        self.skip_remaining(order, "the check did not run");
        self.checks.sort_by_key(|check| {
            order
                .iter()
                .position(|id| *id == check.id)
                .unwrap_or(order.len())
        });
    }
}

#[derive(Default)]
struct LimitWatch {
    first: Option<String>,
}

impl LimitWatch {
    fn note(&mut self, message: impl Into<String>) {
        self.first.get_or_insert_with(|| message.into());
    }

    fn note_host(&mut self, error: &PluginHostError) {
        if matches!(
            error,
            PluginHostError::MemoryLimitExceeded { .. } | PluginHostError::TimeLimitExceeded { .. }
        ) {
            self.note(error.to_string());
        }
    }

    fn note_language(&mut self, error: &LanguagePluginError) {
        if matches!(
            error,
            LanguagePluginError::MemoryLimitExceeded { .. }
                | LanguagePluginError::TimeLimitExceeded { .. }
        ) {
            self.note(error.to_string());
        }
    }

    fn note_metric(&mut self, error: &MetricPluginError) {
        if matches!(
            error,
            MetricPluginError::MemoryLimitExceeded { .. }
                | MetricPluginError::TimeLimitExceeded { .. }
        ) {
            self.note(error.to_string());
        }
    }

    fn settle(
        self,
        report: &mut ConformanceReport,
        peak_memory_bytes: usize,
        limits: PluginLimits,
    ) {
        if report.checks.iter().any(|check| check.id == "limits") {
            return;
        }
        match self.first {
            Some(message) => report.fail(
                "limits",
                message,
                "keep every call within the memory and time limits, or raise them with --plugin-memory-mb and --plugin-timeout-ms",
            ),
            None => report.pass(
                "limits",
                format!(
                    "peak memory {peak_memory_bytes} bytes within {} MiB and {} ms per call",
                    limits.memory_mb, limits.timeout_ms
                ),
            ),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ConformanceSource {
    pub file_name: String,
    pub text: String,
}

pub const END_TO_END_CHECK: &str = "end-to-end";

pub const LANGUAGE_CHECKS: [&str; 17] = [
    "component",
    "wit-contract",
    "zero-imports",
    "extensions",
    "dialects",
    "flagged-line-rules",
    "hazard-rules",
    "framework-profiles",
    "lower",
    "manifest-paths",
    "analyze-manifests",
    "resolve-imports",
    "entry-points",
    "scope-analysis",
    "reference-ranges",
    END_TO_END_CHECK,
    "limits",
];

pub const METRIC_CHECKS: [&str; 12] = [
    "component",
    "wit-contract",
    "zero-imports",
    "metric-key",
    "metric-version",
    "metric-description",
    "measurement-bounds",
    "finding-ids",
    "finding-lines",
    "finding-paths",
    END_TO_END_CHECK,
    "limits",
];

pub fn run_conformance(
    kind: PluginKind,
    path: &Path,
    limits: PluginLimits,
    sources: &[ConformanceSource],
) -> Result<ConformanceReport, PluginHostError> {
    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("plugin")
        .to_owned();
    let bytes = std::fs::read(path).map_err(|source| PluginHostError::Read {
        name: name.clone(),
        path: path.to_path_buf(),
        source,
    })?;
    let mut report = ConformanceReport {
        kind,
        checks: Vec::new(),
        end_to_end: None,
    };
    match kind {
        PluginKind::Language => {
            run_language(&mut report, name, path, &bytes, limits, sources);
            report.finalize(&LANGUAGE_CHECKS);
        }
        PluginKind::Metric => {
            run_metric(&mut report, name, path, &bytes, limits);
            report.finalize(&METRIC_CHECKS);
        }
    }
    Ok(report)
}

fn finish_load<Plugin>(
    report: &mut ConformanceReport,
    kind: PluginKind,
    loaded: Result<Plugin, PluginHostError>,
    checks: &[&'static str],
) -> Option<Plugin> {
    match loaded {
        Ok(plugin) => {
            pass_structural_checks(report);
            Some(plugin)
        }
        Err(error) => {
            record_load_failure(report, &error, kind);
            let failing = report.checks.last().map_or("component", |check| check.id);
            settle_load_failure(report, failing, kind, checks);
            None
        }
    }
}

const STRUCTURAL_CHECKS: [&str; 3] = ["component", "wit-contract", "zero-imports"];

fn pass_structural_checks(report: &mut ConformanceReport) {
    for id in STRUCTURAL_CHECKS {
        pass_load_check(report, id);
    }
}

fn pass_load_check(report: &mut ConformanceReport, id: &'static str) {
    let message = match id {
        "component" => "valid WebAssembly component",
        "wit-contract" => "WIT contract matches codeint:plugin@0.5.0",
        "zero-imports" => "no WASI or host imports",
        "metric-key" => "the key is valid and unique",
        _ => "the version is valid",
    };
    report.pass(id, message);
}

fn settle_load_failure(
    report: &mut ConformanceReport,
    failing: &'static str,
    kind: PluginKind,
    checks: &[&'static str],
) {
    let load_order: &[&'static str] = match kind {
        PluginKind::Language => &STRUCTURAL_CHECKS,
        PluginKind::Metric => &[
            "component",
            "wit-contract",
            "zero-imports",
            "metric-key",
            "metric-version",
        ],
    };
    let verified_before = if failing == "limits" {
        STRUCTURAL_CHECKS.len()
    } else {
        load_order.iter().position(|id| *id == failing).unwrap_or(0)
    };
    for id in &load_order[..verified_before] {
        if !report.checks.iter().any(|check| check.id == *id) {
            pass_load_check(report, id);
        }
    }
    report.skip_remaining(checks, &format!("skipped because {failing} failed"));
}

fn record_load_failure(report: &mut ConformanceReport, error: &PluginHostError, kind: PluginKind) {
    let expected_kind = match kind {
        PluginKind::Language => "language",
        PluginKind::Metric => "metric",
    };
    match error {
        PluginHostError::IncompatibleContractVersion {
            expected, found, ..
        } => {
            report.fail(
                "wit-contract",
                error.to_string(),
                format!(
                    "rebuild the Plugin against {expected} (it targets {found}) with the current codeint-plugin-sdk"
                ),
            );
        }
        PluginHostError::InvalidMetricKey { .. } => {
            report.fail(
                "metric-key",
                error.to_string(),
                "return a key that starts with a lowercase ASCII letter, contains only lowercase ASCII letters, digits, underscores, or hyphens, and is not a built-in Metric key or a reserved report key",
            );
        }
        PluginHostError::InvalidMetricVersion { .. } => {
            report.fail(
                "metric-version",
                error.to_string(),
                "return a Metric version of 1 or higher and bump it whenever the measurement changes",
            );
        }
        PluginHostError::Compile { reason, .. } | PluginHostError::Instantiate { reason, .. } => {
            let text = reason.to_ascii_lowercase();
            if text.contains("component imports") || text.contains("wasi:") {
                report.fail(
                    "zero-imports",
                    error.to_string(),
                    "build the Plugin with codeint-plugin-sdk for a target that imports no WASI interfaces; a Plugin may not read files, clocks, sockets, or the environment",
                );
            } else {
                report.fail(
                    "component",
                    error.to_string(),
                    format!(
                        "build a WebAssembly component that exports codeint:plugin/{expected_kind}@0.5.0; check that --kind matches the kind of the Plugin"
                    ),
                );
            }
        }
        PluginHostError::MemoryLimitExceeded { .. } | PluginHostError::TimeLimitExceeded { .. } => {
            report.fail(
                "limits",
                error.to_string(),
                "keep initialization within the memory and time limits, or raise them with --plugin-memory-mb and --plugin-timeout-ms",
            );
        }
        other => {
            report.fail(
                "component",
                other.to_string(),
                "rebuild the Plugin as a valid WebAssembly component",
            );
        }
    }
}

fn run_language(
    report: &mut ConformanceReport,
    name: String,
    path: &Path,
    bytes: &[u8],
    limits: PluginLimits,
    sources: &[ConformanceSource],
) {
    let loaded = verify_plugin_component(&name, path, PluginKind::Language, bytes)
        .and_then(|()| WasmLanguagePlugin::from_verified_bytes(name.clone(), path, bytes, limits));
    let Some(plugin) = finish_load(report, PluginKind::Language, loaded, &LANGUAGE_CHECKS) else {
        return;
    };
    let mut watch = LimitWatch::default();
    let extensions = plugin.extensions();
    check_extensions(report, &extensions);
    let dialects = plugin.known_dialects();
    check_dialects(report, &dialects);
    let flagged_line_rules = plugin.flagged_line_rules();
    check_flagged_line_rules(report, &flagged_line_rules);
    let hazard_rules = plugin.hazard_rules();
    check_hazard_rules(report, &plugin, &hazard_rules);
    let probe_extensions = probe_extensions(&extensions);
    let repository = probe_repository(&probe_extensions);
    let chosen = if plugin.has_imports() {
        check_manifest_paths(report, &plugin, &repository, &mut watch)
    } else {
        Vec::new()
    };
    report.end_to_end = Some(EndToEndProbe {
        files: end_to_end_files(&repository, &chosen, &probe_extensions, sources),
        framework_profiles: plugin
            .known_framework_profiles()
            .into_iter()
            .filter(|profile| !profile.is_empty())
            .collect(),
    });
    check_framework_profiles(
        report,
        &plugin,
        FrameworkProbeInputs {
            repository: &repository,
            chosen: &chosen,
        },
        &mut watch,
    );
    let scope_artifacts = check_lower(
        report,
        &plugin,
        LowerProbeSetup {
            plugin_name: &name,
            extensions: &probe_extensions,
            dialects: &dialects,
            flagged_line_rules: &flagged_line_rules,
            hazard_rules: &hazard_rules,
            sources,
        },
        &mut watch,
    );
    check_repository_queries(
        report,
        &plugin,
        RepositoryProbe {
            repository: &repository,
            chosen,
            importers: probe_importers(&probe_extensions),
        },
        &mut watch,
    );
    check_scope_analysis(
        report,
        &name,
        &plugin,
        &dialects,
        &hazard_rules,
        scope_artifacts,
        &mut watch,
    );
    watch.settle(report, plugin.peak_memory_bytes(), limits);
}

fn probe_extensions(extensions: &[String]) -> Vec<String> {
    let mut usable: Vec<String> = Vec::new();
    for extension in extensions {
        if usable_extension(extension) && !usable.contains(extension) {
            usable.push(extension.clone());
        }
    }
    if usable.is_empty() {
        usable.push("txt".to_owned());
    }
    usable
}

fn usable_extension(extension: &str) -> bool {
    !extension.is_empty() && RepoPath::new(format!("src/probe.{extension}")).is_ok()
}

fn check_extensions(report: &mut ConformanceReport, extensions: &[String]) {
    let invalid = extensions.iter().find(|extension| {
        extension.is_empty()
            || extension.starts_with('.')
            || extension.contains('/')
            || extension.chars().any(char::is_whitespace)
    });
    if extensions.is_empty() {
        report.fail(
            "extensions",
            "the Plugin claims no file extensions",
            "return at least one extension from extensions(), such as \"ts\"",
        );
    } else if let Some(extension) = invalid {
        report.fail(
            "extensions",
            format!("invalid extension {extension:?}"),
            "return extensions that are not empty and have no leading dot, slash, or whitespace",
        );
    } else {
        report.pass("extensions", format!("claims {}", extensions.join(", ")));
    }
}

fn check_non_empty_list(
    report: &mut ConformanceReport,
    id: &'static str,
    items: &[String],
    empty_message: &str,
    hint: &str,
    label: &str,
) {
    if items.iter().any(|item| item.trim().is_empty()) {
        report.fail(id, empty_message, hint);
        return;
    }
    let mut seen = HashSet::with_capacity(items.len());
    if let Some(duplicate) = items.iter().find(|item| !seen.insert(item.trim())) {
        report.fail(
            id,
            format!("{id} lists duplicate entry {duplicate:?}"),
            hint,
        );
        return;
    }
    report.pass(id, format!("{} {label} listed", items.len()));
}

fn check_dialects(report: &mut ConformanceReport, dialects: &[String]) {
    check_non_empty_list(
        report,
        "dialects",
        dialects,
        "known-dialects holds an empty or blank Dialect name",
        "list each Dialect once with a non-empty stable identifier, or return an empty list",
        "Dialects",
    );
}

fn check_flagged_line_rules(report: &mut ConformanceReport, rules: &[String]) {
    check_non_empty_list(
        report,
        "flagged-line-rules",
        rules,
        "flagged-line-rules holds an empty or blank rule name",
        "list each Flagged line rule once with a non-empty stable identifier",
        "Flagged line rules",
    );
}

fn check_hazard_rules(
    report: &mut ConformanceReport,
    plugin: &WasmLanguagePlugin,
    hazard_rules: &[String],
) {
    if plugin.has_hazard_rules() != !hazard_rules.is_empty() {
        report.fail(
            "hazard-rules",
            format!(
                "has-hazard-rules is {} but hazard-rules lists {} rules",
                plugin.has_hazard_rules(),
                hazard_rules.len()
            ),
            "return true from has-hazard-rules exactly when hazard-rules is not empty",
        );
    } else if hazard_rules.iter().any(String::is_empty) {
        report.fail(
            "hazard-rules",
            "hazard-rules holds an empty rule name",
            "list each Hazard rule with a non-empty stable identifier",
        );
    } else {
        report.pass(
            "hazard-rules",
            format!("{} Hazard rules listed", hazard_rules.len()),
        );
    }
}

struct FrameworkProbeInputs<'a> {
    repository: &'a [RepoPath],
    chosen: &'a [RepoPath],
}

fn probe_workspace() -> Workspace {
    Workspace::new([repo_path(PROBE_PACKAGE)])
}

fn reported_workspace(plugin: &WasmLanguagePlugin, chosen: &[RepoPath]) -> Workspace {
    if !plugin.has_imports() {
        return Workspace::default();
    }
    match plugin.analyze_manifests(chosen_manifests(chosen, true)) {
        Ok(analysis) => Workspace::new(
            analysis
                .workspace_packages
                .into_iter()
                .filter_map(|root| RepoPath::new(root).ok()),
        ),
        Err(_) => Workspace::default(),
    }
}

fn package_keys(workspace: &Workspace) -> Vec<String> {
    workspace
        .packages()
        .map(|package| match package {
            Package::Root => String::new(),
            Package::Member(root) => root.as_str().to_owned(),
        })
        .collect()
}

fn package_key(workspace: &Workspace, path: &RepoPath) -> String {
    match workspace.package_of(path) {
        Package::Root => String::new(),
        Package::Member(root) => root.as_str().to_owned(),
    }
}

fn package_inputs(
    inputs: &FrameworkProbeInputs<'_>,
    workspace: &Workspace,
    package_root: &str,
) -> Vec<(&'static str, Vec<RepoPath>, Vec<Manifest>)> {
    let files: Vec<RepoPath> = inputs
        .repository
        .iter()
        .filter(|path| package_key(workspace, path) == package_root)
        .cloned()
        .collect();
    let in_package = |path: &&RepoPath| package_key(workspace, path) == package_root;
    let chosen: Vec<RepoPath> = inputs.chosen.iter().filter(in_package).cloned().collect();
    let manifests = chosen_manifests(&chosen, true);
    vec![("the manifests it chose", files, manifests)]
}

struct ProfileCall<'a> {
    plugin: &'a WasmLanguagePlugin,
    package_root: &'a str,
    files: &'a [RepoPath],
    manifests: &'a [Manifest],
    manifest_label: &'a str,
}

impl ProfileCall<'_> {
    fn failure(
        &self,
        label: &str,
        selection: &FrameworkProfileSelection,
        watch: &mut LimitWatch,
    ) -> Option<(String, &'static str)> {
        match self.plugin.resolve_framework_profiles(
            self.package_root,
            self.files,
            self.manifests.to_vec(),
            selection.clone(),
        ) {
            Ok(result) if label == "disabled" && !result.active_profiles.is_empty() => Some((
                format!(
                    "disabled selection still activated {:?}",
                    result.active_profiles
                ),
                "return no active profiles when the selection is disabled",
            )),
            Ok(_) => None,
            Err(error) => {
                watch.note_language(&error);
                let place = if self.package_root.is_empty() {
                    "the root package".to_owned()
                } else {
                    format!("package {}", self.package_root)
                };
                Some((
                    format!(
                        "{label} selection failed for {place} with {}: {error}",
                        self.manifest_label
                    ),
                    "resolve-framework-profiles must answer the disabled, auto, exclude, and only selections, with every known profile, with each profile alone, and with the first two of three or more profiles, without an error, and may return only entry points and generated paths that are files it was passed; it is sent the real package files and only the manifests manifest-paths chose, as codeint check sends them",
                ))
            }
        }
    }
}

fn profile_selections(profiles: &[String]) -> Vec<(String, FrameworkProfileSelection)> {
    let mut selections = vec![
        ("disabled".to_owned(), FrameworkProfileSelection::Disabled),
        ("auto".to_owned(), FrameworkProfileSelection::Auto),
        (
            "exclude".to_owned(),
            FrameworkProfileSelection::Exclude(profiles.to_vec()),
        ),
        (
            "only".to_owned(),
            FrameworkProfileSelection::Only(profiles.to_vec()),
        ),
    ];
    for profile in profiles {
        selections.push((
            format!("exclude {profile} alone"),
            FrameworkProfileSelection::Exclude(vec![profile.clone()]),
        ));
        selections.push((
            format!("only {profile} alone"),
            FrameworkProfileSelection::Only(vec![profile.clone()]),
        ));
    }
    if profiles.len() >= 3 {
        let subset: Vec<String> = profiles.iter().take(2).cloned().collect();
        selections.push((
            "exclude the first two".to_owned(),
            FrameworkProfileSelection::Exclude(subset.clone()),
        ));
        selections.push((
            "only the first two".to_owned(),
            FrameworkProfileSelection::Only(subset),
        ));
    }
    selections
}

fn profile_scopes(
    plugin: &WasmLanguagePlugin,
    chosen: &[RepoPath],
) -> Vec<(Workspace, Vec<String>)> {
    let reported = reported_workspace(plugin, chosen);
    let reported_roots = package_keys(&reported);
    let mut scopes = vec![(reported.clone(), reported_roots)];
    let probe = probe_workspace();
    if probe != reported {
        scopes.push((probe, vec![String::new(), PROBE_PACKAGE.to_owned()]));
    }
    scopes
}

fn first_profile_failure(
    plugin: &WasmLanguagePlugin,
    inputs: &FrameworkProbeInputs<'_>,
    scopes: &[(Workspace, Vec<String>)],
    selections: &[(String, FrameworkProfileSelection)],
    watch: &mut LimitWatch,
) -> Option<(String, &'static str)> {
    for (workspace, package_roots) in scopes {
        for package_root in package_roots {
            for (manifest_label, files, manifests) in
                package_inputs(inputs, workspace, package_root)
            {
                let call = ProfileCall {
                    plugin,
                    package_root,
                    files: &files,
                    manifests: &manifests,
                    manifest_label,
                };
                for (label, selection) in selections {
                    if let Some(failure) = call.failure(label, selection, watch) {
                        return Some(failure);
                    }
                }
            }
        }
    }
    None
}

fn check_framework_profiles(
    report: &mut ConformanceReport,
    plugin: &WasmLanguagePlugin,
    inputs: FrameworkProbeInputs<'_>,
    watch: &mut LimitWatch,
) {
    let profiles = plugin.known_framework_profiles();
    if profiles.is_empty() {
        report.skip(
            "framework-profiles",
            "known-framework-profiles is empty, so codeint never resolves Framework profiles",
        );
        return;
    }
    if profiles.iter().any(String::is_empty) {
        report.fail(
            "framework-profiles",
            "known-framework-profiles holds an empty name",
            "list each Framework profile with a non-empty stable name",
        );
        return;
    }
    let selections = profile_selections(&profiles);
    let scopes = profile_scopes(plugin, inputs.chosen);
    if let Some((message, hint)) =
        first_profile_failure(plugin, &inputs, &scopes, &selections, watch)
    {
        report.fail("framework-profiles", message, hint);
        return;
    }
    report.pass(
        "framework-profiles",
        format!(
            "{} Framework profiles resolve under disabled, auto, exclude, and only selections, with every profile and with each profile alone",
            profiles.len()
        ),
    );
}

fn repo_path(text: &str) -> RepoPath {
    RepoPath::new(text).expect("conformance probe path is valid")
}

struct LowerProbe {
    label: String,
    extension: String,
    file_name: String,
    text: String,
}

struct LowerProbeSetup<'a> {
    plugin_name: &'a str,
    extensions: &'a [String],
    dialects: &'a [String],
    flagged_line_rules: &'a [String],
    hazard_rules: &'a [String],
    sources: &'a [ConformanceSource],
}

#[derive(Default)]
struct CollectedScopeArtifacts {
    by_dialect: BTreeMap<Option<String>, Vec<FileScopeArtifact>>,
    line_counts: BTreeMap<Option<String>, BTreeMap<String, usize>>,
}

impl CollectedScopeArtifacts {
    fn record(
        &mut self,
        dialect: Option<&str>,
        path: &RepoPath,
        source_set: SourceSet,
        line_count: usize,
        artifact: Vec<u8>,
    ) {
        let key = dialect.map(str::to_owned);
        self.by_dialect
            .entry(key.clone())
            .or_default()
            .push(FileScopeArtifact {
                path: path.to_string(),
                source_set,
                artifact,
            });
        self.line_counts
            .entry(key)
            .or_default()
            .entry(path.to_string())
            .and_modify(|max_lines| *max_lines = (*max_lines).max(line_count))
            .or_insert(line_count);
    }
}

fn probe_repo_path(file_name: &str, extension: &str) -> RepoPath {
    let sanitized = file_name.replace('\\', "_");
    RepoPath::new(format!("src/{sanitized}"))
        .or_else(|_| RepoPath::new(format!("src/probe.{extension}")))
        .unwrap_or_else(|_| repo_path("src/probe.txt"))
}

fn built_in_probes(extensions: &[String], sources: &[ConformanceSource]) -> Vec<LowerProbe> {
    let samples: [(&str, String); 8] = [
        ("empty file", String::new()),
        ("blank lines", "\n\n\n".to_owned()),
        (
            "C-like code",
            "function add(a, b) {\n  if (a > b) {\n    return a;\n  }\n  return b;\n}\n".to_owned(),
        ),
        (
            "C-like code with CRLF line endings (CRLF)",
            "function add(a, b) {\r\n  if (a > b) {\r\n    return a;\r\n  }\r\n  return b;\r\n}\r\n".to_owned(),
        ),
        (
            "Rust-like code",
            "fn add(a: i32, b: i32) -> i32 {\n    if a > b { a } else { b }\n}\n".to_owned(),
        ),
        (
            "Python-like code",
            "def add(a, b):\n    if a > b:\n        return a\n    return b\n".to_owned(),
        ),
        (
            "unbalanced input",
            "}}} ((( \"unterminated\n/* open comment\n\u{1F600}\n".to_owned(),
        ),
        ("2000 lines", "x = 1\n".repeat(2000)),
    ];
    let mut probes: Vec<LowerProbe> = Vec::new();
    for extension in extensions {
        let label_suffix = if extensions.len() > 1 {
            format!(" (.{extension})")
        } else {
            String::new()
        };
        probes.extend(
            samples
                .iter()
                .enumerate()
                .map(|(index, (label, text))| LowerProbe {
                    label: format!("{label}{label_suffix}"),
                    extension: extension.clone(),
                    file_name: format!("probe-{}.{extension}", index + 1),
                    text: text.clone(),
                }),
        );
    }
    probes.extend(sources.iter().map(|source| LowerProbe {
        label: format!("source {}", source.file_name),
        extension: extensions[0].clone(),
        file_name: source.file_name.clone(),
        text: source.text.clone(),
    }));
    probes
}

fn check_lower(
    report: &mut ConformanceReport,
    plugin: &WasmLanguagePlugin,
    setup: LowerProbeSetup<'_>,
    watch: &mut LimitWatch,
) -> CollectedScopeArtifacts {
    let listed_flagged: HashSet<String> = setup.flagged_line_rules.iter().cloned().collect();
    let listed_hazards: HashSet<String> = setup.hazard_rules.iter().cloned().collect();
    let mut lowered_count = 0;
    let mut scope_artifacts = CollectedScopeArtifacts::default();
    let mut dialect_options: Vec<Option<&str>> = vec![None];
    dialect_options.extend(
        setup
            .dialects
            .iter()
            .filter(|dialect| !dialect.trim().is_empty())
            .map(|dialect| Some(dialect.as_str())),
    );
    for probe in built_in_probes(setup.extensions, setup.sources) {
        let path = probe_repo_path(&probe.file_name, &probe.extension);
        let line_count = probe.text.lines().count();
        for &dialect in &dialect_options {
            let outcome = run_lower_probe(
                report,
                &LowerProbeRun {
                    plugin,
                    setup: &setup,
                    listed_flagged: &listed_flagged,
                    listed_hazards: &listed_hazards,
                    probe: &probe,
                    path: &path,
                    dialect,
                },
                watch,
            );
            let Some(model) = outcome else {
                return CollectedScopeArtifacts::default();
            };
            if let Some(model) = model {
                lowered_count += 1;
                if let Some(artifact) = model.scope_artifact {
                    scope_artifacts.record(dialect, &path, model.source_set, line_count, artifact);
                }
            }
        }
    }
    let unclaimed = repo_path("src/probe.codeint-unclaimed-extension");
    match plugin.lower(&unclaimed, "x\n", None) {
        Err(LowerError::UnsupportedFile { .. } | LowerError::Unparsable { .. }) => {}
        other => {
            report.fail(
                "lower",
                format!("a file with an unclaimed extension was not rejected: {other:?}"),
                "return unsupported-file from lower for any path whose extension the Plugin does not claim",
            );
            return CollectedScopeArtifacts::default();
        }
    }
    report.pass(
        "lower",
        format!("{lowered_count} probe sources lower deterministically"),
    );
    scope_artifacts
}

struct LowerProbeRun<'a> {
    plugin: &'a WasmLanguagePlugin,
    setup: &'a LowerProbeSetup<'a>,
    listed_flagged: &'a HashSet<String>,
    listed_hazards: &'a HashSet<String>,
    probe: &'a LowerProbe,
    path: &'a RepoPath,
    dialect: Option<&'a str>,
}

fn run_lower_probe(
    report: &mut ConformanceReport,
    run: &LowerProbeRun<'_>,
    watch: &mut LimitWatch,
) -> Option<Option<FileModel>> {
    let first = run.plugin.lower(run.path, &run.probe.text, run.dialect);
    let second = run.plugin.lower(run.path, &run.probe.text, run.dialect);
    for outcome in [&first, &second] {
        if let Err(error) = outcome
            && is_limit_error(error)
        {
            watch.note(error.to_string());
            report.fail(
                "lower",
                format!("{}: {error}", run.probe.label),
                "keep lower within the memory and time limits, or raise them with --plugin-memory-mb and --plugin-timeout-ms",
            );
            return None;
        }
    }
    if format!("{first:?}") != format!("{second:?}") {
        report.fail(
            "lower",
            format!("{}: two lower calls on the same text differ", run.probe.label),
            "make lower deterministic and stateless: no clocks, randomness, or state kept between calls",
        );
        return None;
    }
    if let Err(LowerError::Contract(error)) = first {
        report.fail(
            "lower",
            format!("{}: {error}", run.probe.label),
            "fix the FileModel so Flagged line rules are listed in flagged-line-rules and lines are 1-based and within the file",
        );
        return None;
    }
    if let Ok(model) = first {
        let line_count = run.probe.text.lines().count();
        if let Some(problem) = model_problem(run, &model, line_count) {
            report.fail(
                "lower",
                format!("{}: {problem}", run.probe.label),
                "fix the FileModel so lines are 1-based and within the file, functions are sorted, diagnostics are valid, scope-artifact matches has-scope-analysis, Flagged line rules are listed in flagged-line-rules, and Hazard rules are listed in hazard-rules",
            );
            return None;
        }
        return Some(Some(model));
    }
    Some(None)
}

fn is_limit_error(error: &LowerError) -> bool {
    matches!(
        error,
        LowerError::MemoryLimitExceeded { .. } | LowerError::TimeLimitExceeded { .. }
    )
}

fn model_problem(run: &LowerProbeRun<'_>, model: &FileModel, line_count: usize) -> Option<String> {
    let in_bounds = |line: u32| line >= 1 && usize::try_from(line).is_ok_and(|n| n <= line_count);
    if usize::try_from(model.source_lines).is_ok_and(|n| n > line_count) {
        return Some(format!(
            "source-lines {} exceeds the {line_count} lines of the file",
            model.source_lines
        ));
    }
    function_problem(model, in_bounds)
        .or_else(|| line_problem(model, in_bounds))
        .or_else(|| diagnostic_problem(model, in_bounds))
        .or_else(|| {
            (!run.plugin.has_imports() && !model.runtime_imports.is_empty())
                .then(|| "runtime-imports is not empty although has-imports is false".to_owned())
        })
        .or_else(|| {
            (!run.plugin.has_scope_analysis() && model.scope_artifact.is_some()).then(|| {
                "scope-artifact is present although has-scope-analysis is false".to_owned()
            })
        })
        .or_else(|| {
            validate_flagged_lines(
                run.setup.plugin_name,
                run.path,
                line_count,
                &model.flagged_lines,
                run.listed_flagged,
            )
            .err()
            .map(|error| error.to_string())
        })
        .or_else(|| {
            validate_hazard_lines(
                run.setup.plugin_name,
                run.path,
                line_count,
                &model.hazard_lines,
                run.listed_hazards,
            )
            .err()
            .map(|error| error.to_string())
        })
}

fn diagnostic_problem(model: &FileModel, in_bounds: impl Fn(u32) -> bool) -> Option<String> {
    model
        .diagnostics
        .iter()
        .any(|diagnostic| {
            !in_bounds(diagnostic.line)
                || diagnostic.kind.trim().is_empty()
                || diagnostic.message.trim().is_empty()
        })
        .then(|| "a file diagnostic is out of bounds or has an empty kind or message".to_owned())
}

fn function_problem(model: &FileModel, in_bounds: impl Fn(u32) -> bool) -> Option<String> {
    for function in &model.functions {
        if function.line < 1 || function.end_line < function.line || !in_bounds(function.end_line) {
            return Some(format!(
                "function spans lines {}-{} outside the file",
                function.line, function.end_line
            ));
        }
        if function.coverage.is_some() {
            return Some("a function carries coverage; a Language plugin returns none".to_owned());
        }
    }
    model
        .functions
        .windows(2)
        .any(|pair| pair[0].line > pair[1].line)
        .then(|| "functions are not in the order their first lines appear".to_owned())
}

fn line_problem(model: &FileModel, in_bounds: impl Fn(u32) -> bool) -> Option<String> {
    let tokens_misplaced = model.tokens.iter().any(|token| !in_bounds(token.line))
        || model
            .tokens
            .windows(2)
            .any(|pair| pair[0].line > pair[1].line);
    if tokens_misplaced {
        return Some("tokens are out of bounds or not in source order".to_owned());
    }
    model
        .flagged_lines
        .iter()
        .any(|flagged| !in_bounds(flagged.line) || flagged.rule.is_empty())
        .then(|| "a flagged line is out of bounds or has an empty rule".to_owned())
}

const IMPORT_CHECKS: [&str; 4] = [
    "manifest-paths",
    "analyze-manifests",
    "resolve-imports",
    "entry-points",
];

const PROBE_PACKAGE: &str = "packages/probe";

fn probe_repository(extensions: &[String]) -> Vec<RepoPath> {
    let mut repository = Vec::new();
    for extension in extensions {
        for prefix in ["src", "packages/probe/src"] {
            repository.push(repo_path(&format!("{prefix}/main.{extension}")));
            repository.push(repo_path(&format!("{prefix}/lib.{extension}")));
        }
    }
    for (name, _) in PROBE_MANIFESTS {
        repository.push(repo_path(name));
        repository.push(repo_path(&format!("{PROBE_PACKAGE}/{name}")));
    }
    repository
}

const METRIC_PROBE_FILES: [(&str, &str); 3] = [
    (
        "src/app.ts",
        "import { add } from './util';\nexport function main(a: number, b: number): number {\n  if (a > b) {\n    return add(a, b);\n  }\n  return add(b, a);\n}\n",
    ),
    (
        "src/util.ts",
        "export function add(a: number, b: number): number {\n  return a + b;\n}\n",
    ),
    (
        "tests/app.test.ts",
        "import { main } from '../src/app';\nexport function testMain(): boolean {\n  return main(2, 1) === 3;\n}\n",
    ),
];

fn end_to_end_files(
    repository: &[RepoPath],
    chosen: &[RepoPath],
    extensions: &[String],
    sources: &[ConformanceSource],
) -> Vec<EndToEndFile> {
    let mut files: Vec<EndToEndFile> = repository
        .iter()
        .filter(|path| !is_probe_manifest(path) || chosen.contains(path))
        .map(|path| EndToEndFile {
            path: path.to_string(),
            text: probe_file_text(path).to_owned(),
        })
        .collect();
    for source in sources {
        let path = probe_repo_path(&source.file_name, &extensions[0]).to_string();
        if !files.iter().any(|file| file.path == path) {
            files.push(EndToEndFile {
                path,
                text: source.text.clone(),
            });
        }
    }
    files
}

fn probe_importers(extensions: &[String]) -> Vec<SourceFile> {
    let mut importers = Vec::new();
    for extension in extensions {
        for prefix in ["src", "packages/probe/src"] {
            importers.push(SourceFile {
                path: repo_path(&format!("{prefix}/main.{extension}")),
                model: FileModel {
                    runtime_imports: vec![
                        "./lib".to_owned(),
                        "./missing".to_owned(),
                        "left-pad".to_owned(),
                        "fs".to_owned(),
                    ],
                    ..empty_file_model(SourceSet::Production)
                },
            });
        }
    }
    importers
}

struct RepositoryProbe<'a> {
    repository: &'a [RepoPath],
    chosen: Vec<RepoPath>,
    importers: Vec<SourceFile>,
}

fn check_repository_queries(
    report: &mut ConformanceReport,
    plugin: &WasmLanguagePlugin,
    probe: RepositoryProbe<'_>,
    watch: &mut LimitWatch,
) {
    if plugin.has_imports() {
        check_import_calls(report, plugin, probe, watch);
    } else {
        for id in IMPORT_CHECKS {
            report.skip(id, "has-imports is false, so codeint never calls it");
        }
    }
    check_reference_ranges(report, plugin, watch);
}

const PROBE_MANIFESTS: [(&str, &str); 6] = [
    ("package.json", "{\"name\":\"probe\"}"),
    (
        "Cargo.toml",
        "[package]\nname = \"probe\"\nversion = \"0.1.0\"\n",
    ),
    ("go.mod", "module probe\n"),
    ("pyproject.toml", "[project]\nname = \"probe\"\n"),
    ("composer.json", "{\"name\":\"probe/probe\"}"),
    ("pubspec.yaml", "name: probe\n"),
];

const INVALID_MANIFEST_TEXT: &str = "not a valid manifest \u{0}\n[[[";

const PROBE_SOURCE_TEXT: &str = "function add(a, b) {\n  return a + b;\n}\n";

fn is_probe_manifest(path: &RepoPath) -> bool {
    let file_name = path.as_str().rsplit('/').next().unwrap_or_default();
    PROBE_MANIFESTS.iter().any(|(name, _)| *name == file_name)
}

fn probe_file_text(path: &RepoPath) -> &'static str {
    let file_name = path.as_str().rsplit('/').next().unwrap_or_default();
    PROBE_MANIFESTS
        .iter()
        .find(|(name, _)| *name == file_name)
        .map_or(PROBE_SOURCE_TEXT, |(_, text)| text)
}

fn chosen_manifests(chosen: &[RepoPath], valid: bool) -> Vec<Manifest> {
    chosen
        .iter()
        .map(|path| Manifest {
            path: path.to_string(),
            text: if valid {
                probe_file_text(path).to_owned()
            } else {
                INVALID_MANIFEST_TEXT.to_owned()
            },
        })
        .collect()
}

fn check_import_calls(
    report: &mut ConformanceReport,
    plugin: &WasmLanguagePlugin,
    probe: RepositoryProbe<'_>,
    watch: &mut LimitWatch,
) {
    let RepositoryProbe {
        repository,
        chosen,
        importers,
    } = probe;
    check_analyze_manifests(report, plugin, &chosen, watch);
    check_imports(
        report,
        plugin,
        ImportProbe {
            repository,
            importers: &importers,
            manifests: chosen_manifests(&chosen, true),
        },
        watch,
    );

    match plugin.entry_points(repository, chosen_manifests(&chosen, true)) {
        Ok(_) => report.pass("entry-points", "entry points are repository files"),
        Err(error) => {
            watch.note_language(&error);
            report.fail(
                "entry-points",
                error.to_string(),
                "return only entry points that are files from the repository paths the host passes in",
            );
        }
    }
}

fn check_manifest_paths(
    report: &mut ConformanceReport,
    plugin: &WasmLanguagePlugin,
    repository: &[RepoPath],
    watch: &mut LimitWatch,
) -> Vec<RepoPath> {
    if let Err(error) = plugin.manifest_paths_checked(repository) {
        watch.note_host(&error);
        report.fail(
            "manifest-paths",
            error.to_string(),
            "manifest-paths must never trap; return an empty list when no manifest is needed",
        );
        return Vec::new();
    }
    match choose_manifests(plugin, repository) {
        Ok(chosen) => {
            report.pass("manifest-paths", "asks only for repository files");
            chosen
        }
        Err(error) => {
            report.fail(
                "manifest-paths",
                error.to_string(),
                "return only paths taken from the repository paths the host passes in",
            );
            Vec::new()
        }
    }
}

fn check_analyze_manifests(
    report: &mut ConformanceReport,
    plugin: &WasmLanguagePlugin,
    chosen: &[RepoPath],
    watch: &mut LimitWatch,
) {
    if !check_manifest_batch(report, plugin, "no manifests", Vec::new(), false, watch) {
        return;
    }
    if chosen.is_empty() {
        report.pass(
            "analyze-manifests",
            "answers valid and invalid manifests in one pass without traps or invalid paths",
        );
        return;
    }
    if !check_manifest_batch(
        report,
        plugin,
        "the manifests it chose, with valid content",
        chosen_manifests(chosen, true),
        false,
        watch,
    ) {
        return;
    }
    if !check_manifest_batch(
        report,
        plugin,
        "the manifests it chose, with invalid content",
        chosen_manifests(chosen, false),
        true,
        watch,
    ) {
        return;
    }
    report.pass(
        "analyze-manifests",
        "answers valid and invalid manifests in one pass without traps or invalid paths",
    );
}

fn check_manifest_batch(
    report: &mut ConformanceReport,
    plugin: &WasmLanguagePlugin,
    label: &str,
    manifests: Vec<Manifest>,
    allow_invalid: bool,
    watch: &mut LimitWatch,
) -> bool {
    let analysis = match plugin.analyze_manifests_checked(&manifests) {
        Ok(analysis) => analysis,
        Err(error) => {
            watch.note_host(&error);
            report.fail(
                "analyze-manifests",
                format!("{label}: {error}"),
                "analyze-manifests must never trap; record unreadable manifests in invalid-manifests",
            );
            return false;
        }
    };
    let validated = match validate_manifest_analysis(&manifests, analysis) {
        Ok(validated) => validated,
        Err(error) => {
            report.fail(
                "analyze-manifests",
                format!("{label}: {error}"),
                "return valid relative paths, non-empty dependency names, and only passed manifest paths in invalid-manifests",
            );
            return false;
        }
    };
    if !allow_invalid && !validated.invalid_manifests.is_empty() {
        let first = &validated.invalid_manifests[0];
        report.fail(
            "analyze-manifests",
            format!(
                "{label}: reported invalid manifest {}: {}",
                first.path, first.message
            ),
            "record a manifest in invalid-manifests only when its content cannot be parsed",
        );
        return false;
    }
    if validated
        .workspace_packages
        .iter()
        .any(|package| package.is_empty())
    {
        report.fail(
            "analyze-manifests",
            format!(
                "{label}: analyze-manifests returned the repository root as a workspace package"
            ),
            "return only member package folders; the root package is implicit",
        );
        return false;
    }
    true
}

fn check_reference_ranges(
    report: &mut ConformanceReport,
    plugin: &WasmLanguagePlugin,
    watch: &mut LimitWatch,
) {
    let ranges = match plugin.reference_ranges() {
        Ok(ranges) => ranges,
        Err(error) => {
            watch.note_language(&error);
            report.fail(
                "reference-ranges",
                error.to_string(),
                "return at most one Reference range per Metric with 0 <= low <= high <= 1",
            );
            return;
        }
    };
    let stale: Vec<String> = ranges
        .iter()
        .filter_map(|range| {
            let current = Metric::from_key(&range.metric)?.version();
            range.is_stale_for(current).then(|| {
                format!(
                    "{} has version {}, current version is {current}",
                    range.metric, range.metric_version
                )
            })
        })
        .collect();
    if stale.is_empty() {
        report.pass(
            "reference-ranges",
            format!(
                "{} Reference ranges within 0..=1, each at its Metric's current version",
                ranges.iter().count()
            ),
        );
        return;
    }
    let summary = stale.join("; ");
    report.fail(
        "reference-ranges",
        format!("stale Reference ranges: {summary}"),
        format!(
            "codeint check hides a Reference range whose metric_version differs from the current Metric version; recalibrate the range or update its metric_version ({summary})"
        ),
    );
}

struct ImportProbe<'a> {
    repository: &'a [RepoPath],
    importers: &'a [SourceFile],
    manifests: Vec<Manifest>,
}

fn check_imports(
    report: &mut ConformanceReport,
    plugin: &WasmLanguagePlugin,
    probe: ImportProbe<'_>,
    watch: &mut LimitWatch,
) {
    let known = plugin.known_framework_profiles();
    let mut profile_sets: Vec<Vec<PackageFrameworkProfiles>> = vec![Vec::new()];
    for profile in &known {
        profile_sets.push(vec![
            PackageFrameworkProfiles {
                package_root: String::new(),
                active_profiles: vec![profile.clone()],
            },
            PackageFrameworkProfiles {
                package_root: PROBE_PACKAGE.to_owned(),
                active_profiles: vec![profile.clone()],
            },
        ]);
    }
    if known.len() > 1 {
        profile_sets.push(vec![
            PackageFrameworkProfiles {
                package_root: String::new(),
                active_profiles: known.clone(),
            },
            PackageFrameworkProfiles {
                package_root: PROBE_PACKAGE.to_owned(),
                active_profiles: known,
            },
        ]);
    }
    let importers: Vec<&SourceFile> = probe.importers.iter().collect();
    for package_profiles in &profile_sets {
        let outcome = codeint_model::resolve_runtime_imports(
            plugin,
            probe.repository,
            probe.manifests.clone(),
            &importers,
            package_profiles,
        );
        if let Err(error) = outcome {
            if let ResolveError::Plugin(inner) = &error {
                watch.note_language(inner);
            }
            let context = if package_profiles.is_empty() {
                "with no Framework profile active".to_owned()
            } else {
                format!(
                    "with Framework profiles {:?} active",
                    package_profiles[0].active_profiles
                )
            };
            report.fail(
                "resolve-imports",
                format!("{context}: {error}"),
                "return exactly one resolution per request; a resolved import lists one or more repository files, under every set of active Framework profiles",
            );
            return;
        }
    }
    report.pass(
        "resolve-imports",
        "answers every import request with repository files, a package, a built-in, or unresolved, with and without each Framework profile active",
    );
}

fn check_scope_analysis(
    report: &mut ConformanceReport,
    plugin_name: &str,
    plugin: &WasmLanguagePlugin,
    dialects: &[String],
    hazard_rules: &[String],
    artifacts: CollectedScopeArtifacts,
    watch: &mut LimitWatch,
) {
    if !plugin.has_scope_analysis() {
        report.skip(
            "scope-analysis",
            "has-scope-analysis is false, so codeint never calls analyze-scope",
        );
        return;
    }
    let listed: HashSet<String> = hazard_rules.iter().cloned().collect();
    let empty_counts = BTreeMap::new();
    let mut dialect_options: Vec<Option<&str>> = vec![None];
    dialect_options.extend(
        dialects
            .iter()
            .filter(|dialect| !dialect.trim().is_empty())
            .map(|dialect| Some(dialect.as_str())),
    );
    for dialect in dialect_options {
        let key = dialect.map(str::to_owned);
        let dialect_artifacts = artifacts
            .by_dialect
            .get(&key)
            .map_or(&[][..], Vec::as_slice);
        let line_counts = artifacts.line_counts.get(&key).unwrap_or(&empty_counts);
        let analysis = match plugin.analyze_scope_checked("", dialect, dialect_artifacts) {
            Ok(analysis) => analysis,
            Err(error) => {
                watch.note_host(&error);
                report.fail(
                    "scope-analysis",
                    error.to_string(),
                    "analyze-scope must never trap or exceed limits",
                );
                return;
            }
        };
        if let Some(problem) = scope_analysis_problem(
            plugin_name,
            analysis,
            dialect_artifacts,
            line_counts,
            &listed,
        ) {
            report.fail(
                "scope-analysis",
                problem,
                "return valid repository paths from the analyzed artifacts, 1-based line numbers within each file, listed Hazard rules, and non-empty summary kinds and field keys",
            );
            return;
        }
    }
    report.pass(
        "scope-analysis",
        "analyze-scope returns valid extra Hazard lines and scope summaries",
    );
}

fn scope_analysis_problem(
    plugin_name: &str,
    analysis: codeint_model::ScopeAnalysis,
    artifacts: &[FileScopeArtifact],
    line_counts: &BTreeMap<String, usize>,
    listed: &HashSet<String>,
) -> Option<String> {
    validate_scope_analysis(plugin_name, artifacts, listed, Some(line_counts), analysis)
        .err()
        .map(|error| error.to_string())
}

fn run_metric(
    report: &mut ConformanceReport,
    name: String,
    path: &Path,
    bytes: &[u8],
    limits: PluginLimits,
) {
    let loaded = verify_plugin_component(&name, path, PluginKind::Metric, bytes)
        .and_then(|()| WasmMetricPlugin::from_verified_bytes(name.clone(), path, bytes, limits));
    let Some(plugin) = finish_load(report, PluginKind::Metric, loaded, &METRIC_CHECKS) else {
        return;
    };
    report.pass(
        "metric-key",
        format!("key {:?} is valid and unique", plugin.key()),
    );
    report.pass("metric-version", format!("version {}", plugin.version()));
    report.end_to_end = Some(EndToEndProbe {
        files: METRIC_PROBE_FILES
            .iter()
            .map(|(path, text)| EndToEndFile {
                path: (*path).to_owned(),
                text: (*text).to_owned(),
            })
            .collect(),
        framework_profiles: Vec::new(),
    });
    if plugin.declared_name_is_blank() || plugin.description().trim().is_empty() {
        report.fail(
            "metric-description",
            "the Metric name or description is empty",
            "return a non-empty name() and description() so reports can explain the Metric",
        );
    } else {
        report.pass("metric-description", "name and description are set");
    }
    check_measure(report, &plugin, limits);
}

const MEASURE_CHECKS: [&str; 4] = [
    "measurement-bounds",
    "finding-ids",
    "finding-lines",
    "finding-paths",
];

fn check_measure(report: &mut ConformanceReport, plugin: &WasmMetricPlugin, limits: PluginLimits) {
    let mut watch = LimitWatch::default();
    let mut stopped = false;
    for model in &probe_code_models() {
        let first = plugin.measure(model);
        let second = plugin.measure(model);
        match (&first, &second) {
            (Ok(a), Ok(b)) if a != b => {
                report.fail(
                    "measurement-bounds",
                    "two measure calls on the same Code model differ",
                    "make measure deterministic and stateless",
                );
                stopped = true;
            }
            (Err(error), _) | (_, Err(error)) => {
                watch.note_metric(error);
                record_measure_error(report, error);
                stopped = true;
            }
            _ => {}
        }
        if stopped {
            break;
        }
    }
    if stopped {
        report.skip_remaining(
            &MEASURE_CHECKS,
            "an earlier measurement error stopped the run",
        );
    } else {
        report.pass(
            "measurement-bounds",
            "ratio within 0..=1, numbers finite and non-negative",
        );
        report.pass("finding-ids", "Finding ids are non-empty");
        report.pass("finding-lines", "Finding lines start at 1");
        report.pass("finding-paths", "Finding paths are repository-relative");
    }
    watch.settle(report, plugin.peak_memory_bytes(), limits);
}

fn record_measure_error(report: &mut ConformanceReport, error: &MetricPluginError) {
    let message = error.to_string();
    match error {
        MetricPluginError::InvalidRatio { .. } | MetricPluginError::InvalidNumber { .. } => report
            .fail(
                "measurement-bounds",
                message,
                "return a ratio between 0 and 1 and a finite, non-negative numerator and denominator",
            ),
        MetricPluginError::EmptyFindingId { .. } => report.fail(
            "finding-ids",
            message,
            "give every Finding a stable, non-empty id",
        ),
        MetricPluginError::InvalidFindingLine { .. } => report.fail(
            "finding-lines",
            message,
            "number Finding lines from 1",
        ),
        MetricPluginError::InvalidFindingPath { .. } => report.fail(
            "finding-paths",
            message,
            "return Finding paths relative to the repository root with forward slashes, never absolute or containing ..",
        ),
        MetricPluginError::MemoryLimitExceeded { .. }
        | MetricPluginError::TimeLimitExceeded { .. } => {}
        _ => report.fail(
            "measurement-bounds",
            message,
            "measure must return a Measurement for every Code model without trapping",
        ),
    }
}

fn probe_code_models() -> Vec<CodeModel> {
    let production = production_files();
    let test = test_files();
    let combined: Vec<SourceFile> = production.iter().chain(&test).cloned().collect();
    vec![
        CodeModel::default(),
        synthetic_model(production),
        synthetic_model(test),
        synthetic_model(combined),
        covered_model(),
    ]
}

fn covered_model() -> CodeModel {
    let mut files = vec![
        probe_file("src/app.ts", SourceSet::Production),
        probe_file("src/generated.ts", SourceSet::Generated),
        probe_file("vendor/lib.ts", SourceSet::Vendored),
    ];
    for (index, file) in files.iter_mut().enumerate() {
        for function in &mut file.model.functions {
            function.coverage = Some(FunctionCoverage {
                listed_lines: function.source_lines,
                executed_lines: if index == 0 { 0 } else { function.source_lines },
            });
        }
        file.model.functions.push(Function {
            name: None,
            coverage: Some(FunctionCoverage {
                listed_lines: 0,
                executed_lines: 0,
            }),
            ..function("anonymous", 13, 14)
        });
        file.model.tokens.push(Token {
            kind: TokenKind::Other,
            text_hash: 7,
            line: 13,
        });
        file.model.tokens.push(Token {
            kind: TokenKind::ImportOrExport,
            text_hash: 8,
            line: 14,
        });
        file.model.source_lines = 14;
        file.model.program_start = true;
    }
    let mut model = synthetic_model(files);
    for facts in &mut model.languages {
        facts.has_coverage = true;
    }
    model
}

fn function(name: &str, line: u32, end_line: u32) -> Function {
    Function {
        line,
        end_line,
        decision_points: 3,
        source_lines: end_line - line + 1,
        name: Some(name.to_owned()),
        cognitive_complexity: 5,
        coverage: None,
    }
}

fn probe_file(path: &str, source_set: SourceSet) -> SourceFile {
    SourceFile {
        path: repo_path(path),
        model: FileModel {
            source_set,
            source_lines: 12,
            functions: vec![function("first", 1, 5), function("second", 7, 12)],
            tokens: (1..=12)
                .map(|line| Token {
                    kind: if line % 2 == 0 {
                        TokenKind::Identifier
                    } else {
                        TokenKind::Literal
                    },
                    text_hash: u64::from(line % 3),
                    line,
                })
                .collect(),
            flagged_lines: vec![FlaggedLine {
                line: 2,
                rule: "probe-flag".to_owned(),
            }],
            hazard_lines: vec![HazardLine {
                line: 3,
                rule: "probe-hazard".to_owned(),
            }],
            runtime_imports: vec!["./util".to_owned(), "left-pad".to_owned(), "fs".to_owned()],
            ..empty_file_model(source_set)
        },
    }
}

fn production_files() -> Vec<SourceFile> {
    vec![
        probe_file("src/app.ts", SourceSet::Production),
        probe_file("src/util.ts", SourceSet::Production),
    ]
}

fn test_files() -> Vec<SourceFile> {
    vec![
        probe_file("tests/app.test.ts", SourceSet::Test),
        probe_file("tests/util.test.ts", SourceSet::Test),
    ]
}

fn synthetic_model(files: Vec<SourceFile>) -> CodeModel {
    let importer = repo_path("src/app.ts");
    CodeModel {
        files,
        workspace: Workspace::new([repo_path("packages/probe")]),
        runtime_imports: vec![
            RuntimeImport {
                importer: importer.clone(),
                specifier: "./util".to_owned(),
                resolution: ImportResolution::Resolved(vec![repo_path("src/util.ts")]),
            },
            RuntimeImport {
                importer: importer.clone(),
                specifier: "left-pad".to_owned(),
                resolution: ImportResolution::External("left-pad".to_owned()),
            },
            RuntimeImport {
                importer: importer.clone(),
                specifier: "fs".to_owned(),
                resolution: ImportResolution::Builtin,
            },
            RuntimeImport {
                importer,
                specifier: "./missing".to_owned(),
                resolution: ImportResolution::Unresolved {
                    reason: "no such file".to_owned(),
                },
            },
        ],
        languages: vec![LanguageFacts {
            language: "typescript-javascript".to_owned(),
            has_coverage: false,
            has_hazard_rules: true,
            has_imports: true,
        }],
        ..CodeModel::default()
    }
}
