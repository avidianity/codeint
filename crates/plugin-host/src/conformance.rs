use std::collections::HashSet;
use std::path::Path;

use codeint_config::{PluginKind, PluginLimits};
use codeint_model::language::{FlaggedLine, Function, HazardLine, Token, TokenKind};
use codeint_model::{
    CodeModel, FileModel, FrameworkProfileSelection, ImportResolution, LanguageFacts,
    LanguagePlugin, LanguagePluginError, LowerError, Manifest, Metric, MetricPlugin,
    MetricPluginError, RepoPath, ResolveError, RuntimeImport, SourceFile, SourceSet, Workspace,
    WorkspaceError, choose_manifests, validate_hazard_lines,
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
pub struct ConformanceReport {
    pub kind: PluginKind,
    pub checks: Vec<ConformanceCheck>,
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

pub const LANGUAGE_CHECKS: [&str; 14] = [
    "component",
    "wit-contract",
    "zero-imports",
    "extensions",
    "hazard-rules",
    "framework-profiles",
    "lower",
    "manifest-paths",
    "workspace-packages",
    "resolve-imports",
    "declared-dependencies",
    "entry-points",
    "reference-ranges",
    "limits",
];

pub const METRIC_CHECKS: [&str; 11] = [
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
        "wit-contract" => "WIT contract matches codeint:plugin@0.4.0",
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
                "return a lowercase snake_case key that is not a built-in Metric key or a reserved report key",
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
                        "build a WebAssembly component that exports codeint:plugin/{expected_kind}@0.4.0; check that --kind matches the kind of the Plugin"
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
    let hazard_rules = plugin.hazard_rules();
    check_hazard_rules(report, &plugin, &hazard_rules);
    check_framework_profiles(report, &plugin, &mut watch);
    let probe_extension = probe_extension(&extensions);
    check_lower(
        report,
        &plugin,
        LowerProbeSetup {
            plugin_name: &name,
            extension: &probe_extension,
            hazard_rules: &hazard_rules,
            sources,
        },
        &mut watch,
    );
    check_repository_queries(report, &plugin, &probe_extension, &mut watch);
    watch.settle(report, plugin.peak_memory_bytes(), limits);
}

fn probe_extension(extensions: &[String]) -> String {
    for extension in extensions {
        if usable_extension(extension) {
            return extension.clone();
        }
    }
    "txt".to_owned()
}

fn usable_extension(extension: &str) -> bool {
    !extension.is_empty() && RepoPath::new(format!("src/probe.{extension}")).is_ok()
}

fn check_extensions(report: &mut ConformanceReport, extensions: &[String]) {
    let mut seen = HashSet::new();
    let invalid = extensions.iter().find(|extension| {
        extension.is_empty()
            || extension.starts_with('.')
            || extension.contains('/')
            || extension.chars().any(char::is_whitespace)
            || !seen.insert(extension.as_str())
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
            format!("invalid or duplicate extension {extension:?}"),
            "return unique extensions without a leading dot, a slash, or whitespace",
        );
    } else {
        report.pass("extensions", format!("claims {}", extensions.join(", ")));
    }
}

fn check_hazard_rules(
    report: &mut ConformanceReport,
    plugin: &WasmLanguagePlugin,
    hazard_rules: &[String],
) {
    let unique: HashSet<&String> = hazard_rules.iter().collect();
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
    } else if hazard_rules.iter().any(String::is_empty) || unique.len() != hazard_rules.len() {
        report.fail(
            "hazard-rules",
            "hazard-rules holds an empty or duplicate rule name",
            "list each Hazard rule once with a non-empty stable identifier",
        );
    } else {
        report.pass(
            "hazard-rules",
            format!("{} Hazard rules listed", hazard_rules.len()),
        );
    }
}

fn check_framework_profiles(
    report: &mut ConformanceReport,
    plugin: &WasmLanguagePlugin,
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
    let unique: HashSet<&String> = profiles.iter().collect();
    if profiles.iter().any(String::is_empty) || unique.len() != profiles.len() {
        report.fail(
            "framework-profiles",
            "known-framework-profiles holds an empty or duplicate name",
            "list each Framework profile once with a non-empty stable name",
        );
        return;
    }
    let files = vec![repo_path("src/probe.txt")];
    let selections = [
        ("disabled", FrameworkProfileSelection::Disabled),
        ("auto", FrameworkProfileSelection::Auto),
        (
            "exclude",
            FrameworkProfileSelection::Exclude(profiles.clone()),
        ),
        ("only", FrameworkProfileSelection::Only(profiles.clone())),
    ];
    for (label, selection) in selections {
        match plugin.resolve_framework_profiles("", &files, Vec::new(), selection) {
            Ok(result) if label == "disabled" && !result.active_profiles.is_empty() => {
                report.fail(
                    "framework-profiles",
                    format!(
                        "disabled selection still activated {:?}",
                        result.active_profiles
                    ),
                    "return no active profiles when the selection is disabled",
                );
                return;
            }
            Ok(_) => {}
            Err(error) => {
                watch.note_language(&error);
                report.fail(
                    "framework-profiles",
                    format!("{label} selection failed: {error}"),
                    "resolve-framework-profiles must answer the disabled, auto, exclude, and only selections without an error",
                );
                return;
            }
        }
    }
    report.pass(
        "framework-profiles",
        format!(
            "{} Framework profiles resolve under disabled, auto, exclude, and only selections",
            profiles.len()
        ),
    );
}

fn repo_path(text: &str) -> RepoPath {
    RepoPath::new(text).expect("conformance probe path is valid")
}

struct LowerProbe {
    label: String,
    file_name: String,
    text: String,
}

struct LowerProbeSetup<'a> {
    plugin_name: &'a str,
    extension: &'a str,
    hazard_rules: &'a [String],
    sources: &'a [ConformanceSource],
}

fn probe_repo_path(file_name: &str, extension: &str) -> RepoPath {
    let sanitized = file_name.replace('\\', "_");
    RepoPath::new(format!("src/{sanitized}"))
        .or_else(|_| RepoPath::new(format!("src/probe.{extension}")))
        .unwrap_or_else(|_| repo_path("src/probe.txt"))
}

fn built_in_probes(extension: &str, sources: &[ConformanceSource]) -> Vec<LowerProbe> {
    let samples: [(&str, String); 7] = [
        ("empty file", String::new()),
        ("blank lines", "\n\n\n".to_owned()),
        (
            "C-like code",
            "function add(a, b) {\n  if (a > b) {\n    return a;\n  }\n  return b;\n}\n".to_owned(),
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
    let mut probes: Vec<LowerProbe> = samples
        .into_iter()
        .map(|(label, text)| LowerProbe {
            label: label.to_owned(),
            file_name: format!("probe.{extension}"),
            text,
        })
        .collect();
    probes.extend(sources.iter().map(|source| LowerProbe {
        label: format!("source {}", source.file_name),
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
) {
    let listed: HashSet<String> = setup.hazard_rules.iter().cloned().collect();
    let mut lowered_count = 0;
    for probe in built_in_probes(setup.extension, setup.sources) {
        let path = probe_repo_path(&probe.file_name, setup.extension);
        let first = plugin.lower(&path, &probe.text);
        let second = plugin.lower(&path, &probe.text);
        if let Err(error) = &first
            && is_limit_error(error)
        {
            watch.note(error.to_string());
            report.fail(
                "lower",
                format!("{}: {error}", probe.label),
                "keep lower within the memory and time limits, or raise them with --plugin-memory-mb and --plugin-timeout-ms",
            );
            return;
        }
        if format!("{first:?}") != format!("{second:?}") {
            report.fail(
                "lower",
                format!("{}: two lower calls on the same text differ", probe.label),
                "make lower deterministic and stateless: no clocks, randomness, or state kept between calls",
            );
            return;
        }
        if let Ok(model) = first {
            lowered_count += 1;
            let line_count = probe.text.lines().count();
            if let Some(problem) = model_problem(
                setup.plugin_name,
                &path,
                &model,
                line_count,
                &listed,
                plugin.has_imports(),
            ) {
                report.fail(
                    "lower",
                    format!("{}: {problem}", probe.label),
                    "fix the FileModel so lines are 1-based and within the file, functions are sorted, and Hazard rules are listed in hazard-rules",
                );
                return;
            }
        }
    }
    let unclaimed = repo_path("src/probe.codeint-unclaimed-extension");
    match plugin.lower(&unclaimed, "x\n") {
        Err(LowerError::UnsupportedFile { .. } | LowerError::Unparsable { .. }) => {}
        other => {
            report.fail(
                "lower",
                format!("a file with an unclaimed extension was not rejected: {other:?}"),
                "return unsupported-file from lower for any path whose extension the Plugin does not claim",
            );
            return;
        }
    }
    report.pass(
        "lower",
        format!("{lowered_count} probe sources lower deterministically"),
    );
}

fn is_limit_error(error: &LowerError) -> bool {
    matches!(
        error,
        LowerError::MemoryLimitExceeded { .. } | LowerError::TimeLimitExceeded { .. }
    )
}

fn model_problem(
    plugin_name: &str,
    path: &RepoPath,
    model: &FileModel,
    line_count: usize,
    listed: &HashSet<String>,
    has_imports: bool,
) -> Option<String> {
    let in_bounds = |line: u32| line >= 1 && usize::try_from(line).is_ok_and(|n| n <= line_count);
    if usize::try_from(model.source_lines).is_ok_and(|n| n > line_count) {
        return Some(format!(
            "source-lines {} exceeds the {line_count} lines of the file",
            model.source_lines
        ));
    }
    function_problem(model, in_bounds)
        .or_else(|| line_problem(model, in_bounds))
        .or_else(|| {
            (!has_imports && !model.runtime_imports.is_empty())
                .then(|| "runtime-imports is not empty although has-imports is false".to_owned())
        })
        .or_else(|| {
            validate_hazard_lines(plugin_name, path, line_count, &model.hazard_lines, listed)
                .err()
                .map(|error| error.to_string())
        })
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

const IMPORT_CHECKS: [&str; 5] = [
    "manifest-paths",
    "workspace-packages",
    "resolve-imports",
    "declared-dependencies",
    "entry-points",
];

fn check_repository_queries(
    report: &mut ConformanceReport,
    plugin: &WasmLanguagePlugin,
    extension: &str,
    watch: &mut LimitWatch,
) {
    let source = repo_path(&format!("src/main.{extension}"));
    let other = repo_path(&format!("src/lib.{extension}"));
    let repository = vec![
        source.clone(),
        other,
        repo_path("package.json"),
        repo_path("Cargo.toml"),
    ];
    if plugin.has_imports() {
        check_import_calls(report, plugin, &repository, &source, watch);
    } else {
        for id in IMPORT_CHECKS {
            report.skip(id, "has-imports is false, so codeint never calls it");
        }
    }
    check_reference_ranges(report, plugin);
}

fn probe_manifests() -> Vec<Manifest> {
    vec![package_manifest(), cargo_manifest()]
}

fn package_manifest() -> Manifest {
    Manifest {
        path: "package.json".to_owned(),
        text: "{\"name\":\"probe\"}".to_owned(),
    }
}

fn cargo_manifest() -> Manifest {
    Manifest {
        path: "Cargo.toml".to_owned(),
        text: "not a valid manifest \u{0}\n[[[".to_owned(),
    }
}

fn check_import_calls(
    report: &mut ConformanceReport,
    plugin: &WasmLanguagePlugin,
    repository: &[RepoPath],
    source: &RepoPath,
    watch: &mut LimitWatch,
) {
    check_manifest_paths(report, plugin, repository, watch);
    check_workspace_packages(report, plugin, watch);
    check_imports(report, plugin, repository, source, probe_manifests(), watch);

    match plugin.declared_dependencies(probe_manifests()) {
        Ok(_) => report.pass("declared-dependencies", "dependency declarations are valid"),
        Err(error) if is_call_error(&error) => {
            watch.note_language(&error);
            report.fail(
                "declared-dependencies",
                error.to_string(),
                "declared-dependencies must never trap; return an empty list for manifests it does not understand",
            );
        }
        Err(error) => {
            watch.note_language(&error);
            report.fail(
                "declared-dependencies",
                error.to_string(),
                "return dependencies with a repository-relative package root and a non-empty name",
            );
        }
    }

    match plugin.entry_points(repository, probe_manifests()) {
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
) {
    if let Err(error) = plugin.manifest_paths_checked(repository) {
        watch.note_host(&error);
        report.fail(
            "manifest-paths",
            error.to_string(),
            "manifest-paths must never trap; return an empty list when no manifest is needed",
        );
        return;
    }
    match choose_manifests(plugin, repository) {
        Ok(_) => report.pass("manifest-paths", "asks only for repository files"),
        Err(error) => report.fail(
            "manifest-paths",
            error.to_string(),
            "return only paths taken from the repository paths the host passes in",
        ),
    }
}

fn check_workspace_packages(
    report: &mut ConformanceReport,
    plugin: &WasmLanguagePlugin,
    watch: &mut LimitWatch,
) {
    let probes = [
        ("no manifests", Vec::new(), true),
        ("a valid package.json", vec![package_manifest()], true),
        ("an invalid Cargo.toml", vec![cargo_manifest()], false),
        ("both manifests", probe_manifests(), false),
    ];
    for (label, manifests, must_accept) in probes {
        match plugin.workspace_packages_checked(manifests) {
            Err(error) => {
                watch.note_host(&error);
                report.fail(
                    "workspace-packages",
                    format!("{label}: {error}"),
                    "workspace-packages must never trap; return a workspace-error for a manifest it cannot read",
                );
                return;
            }
            Ok(Err(error @ WorkspaceError::Manifest { .. })) if must_accept => {
                report.fail(
                    "workspace-packages",
                    format!("{label}: {error}"),
                    "return a workspace-error only for a manifest it cannot read; codeint check stops on any workspace-error, so never return one for a valid manifest or no manifests",
                );
                return;
            }
            Ok(Err(error @ WorkspaceError::InvalidPackageRoot(_))) => {
                report.fail(
                    "workspace-packages",
                    format!("{label}: {error}"),
                    "return only package roots relative to the repository root with forward slashes, never absolute or containing ..",
                );
                return;
            }
            Ok(Ok(packages)) if packages.iter().any(|package| package.as_str().is_empty()) => {
                report.fail(
                    "workspace-packages",
                    format!("{label}: workspace-packages returned the repository root"),
                    "return only member package folders; the root package is implicit",
                );
                return;
            }
            Ok(_) => {}
        }
    }
    report.pass(
        "workspace-packages",
        "answers valid and invalid manifests without a trap or an invalid path",
    );
}

fn check_reference_ranges(report: &mut ConformanceReport, plugin: &WasmLanguagePlugin) {
    let ranges = match plugin.reference_ranges() {
        Ok(ranges) => ranges,
        Err(error) => {
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

fn is_call_error(error: &LanguagePluginError) -> bool {
    matches!(error, LanguagePluginError::Call { .. })
}

fn check_imports(
    report: &mut ConformanceReport,
    plugin: &WasmLanguagePlugin,
    repository: &[RepoPath],
    importer: &RepoPath,
    manifests: Vec<Manifest>,
    watch: &mut LimitWatch,
) {
    let model = FileModel {
        runtime_imports: vec!["./lib".to_owned(), "left-pad".to_owned(), "fs".to_owned()],
        ..empty_file_model()
    };
    let file = SourceFile {
        path: importer.clone(),
        model,
    };
    let outcome =
        codeint_model::resolve_runtime_imports(plugin, repository, manifests, &[&file], &[]);
    match outcome {
        Ok(_) => report.pass(
            "resolve-imports",
            "answers every import request with repository files, a package, a built-in, or unresolved",
        ),
        Err(error) => {
            if let ResolveError::Plugin(inner) = &error {
                watch.note_language(inner);
            }
            report.fail(
                "resolve-imports",
                error.to_string(),
                "return exactly one resolution per request; a resolved import lists one or more repository files",
            );
        }
    }
}

fn empty_file_model() -> FileModel {
    FileModel {
        source_set: codeint_model::SourceSet::Production,
        source_lines: 0,
        functions: Vec::new(),
        tokens: Vec::new(),
        flagged_lines: Vec::new(),
        hazard_lines: Vec::new(),
        runtime_imports: Vec::new(),
        program_start: false,
    }
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
    ]
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
            program_start: false,
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
