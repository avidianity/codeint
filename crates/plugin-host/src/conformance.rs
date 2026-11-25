use std::collections::HashSet;
use std::path::Path;

use codeint_config::{PluginKind, PluginLimits};
use codeint_model::{
    CodeModel, FileModel, FrameworkProfileSelection, LanguageFacts, LanguagePlugin, LowerError,
    Manifest, MetricPlugin, MetricPluginError, RepoPath, SourceFile, validate_hazard_lines,
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
}

#[derive(Clone, Debug)]
pub struct ConformanceSource {
    pub file_name: String,
    pub text: String,
}

const LANGUAGE_CHECKS: [&str; 14] = [
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

const METRIC_CHECKS: [&str; 11] = [
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
        PluginKind::Language => run_language(&mut report, name, path, &bytes, limits, sources),
        PluginKind::Metric => run_metric(&mut report, name, path, &bytes, limits),
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
            report.skip_remaining(checks, "skipped because an earlier check failed");
            None
        }
    }
}

fn pass_structural_checks(report: &mut ConformanceReport) {
    report.pass("component", "valid WebAssembly component");
    report.pass("wit-contract", "WIT contract matches codeint:plugin@0.4.0");
    report.pass("zero-imports", "no WASI or host imports");
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
            pass_structural_checks(report);
            report.fail(
                "metric-key",
                error.to_string(),
                "return a lowercase snake_case key that is not a built-in Metric key or a reserved report key",
            );
        }
        PluginHostError::InvalidMetricVersion { .. } => {
            pass_structural_checks(report);
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
    let extensions = plugin.extensions();
    check_extensions(report, &extensions);
    let hazard_rules = plugin.hazard_rules();
    check_hazard_rules(report, &plugin, &hazard_rules);
    check_framework_profiles(report, &plugin);
    let probe_extension = extensions
        .iter()
        .find(|extension| !extension.is_empty())
        .cloned()
        .unwrap_or_else(|| "txt".to_owned());
    check_lower(
        report,
        &plugin,
        &name,
        &probe_extension,
        &hazard_rules,
        sources,
    );
    check_repository_queries(report, &plugin, &probe_extension);
    report.pass(
        "limits",
        format!(
            "peak memory {} bytes within {} MiB and {} ms per call",
            plugin.peak_memory_bytes(),
            limits.memory_mb,
            limits.timeout_ms
        ),
    );
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

fn check_framework_profiles(report: &mut ConformanceReport, plugin: &WasmLanguagePlugin) {
    let profiles = plugin.known_framework_profiles();
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
                report.fail(
                    "framework-profiles",
                    format!("{label} selection failed: {error}"),
                    "resolve-framework-profiles must answer disabled, auto, exclude, and only selections without an error",
                );
                return;
            }
        }
    }
    report.pass(
        "framework-profiles",
        format!("{} Framework profiles resolve", profiles.len()),
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
    plugin_name: &str,
    extension: &str,
    hazard_rules: &[String],
    sources: &[ConformanceSource],
) {
    let listed: HashSet<String> = hazard_rules.iter().cloned().collect();
    let mut lowered_count = 0;
    for probe in built_in_probes(extension, sources) {
        let path = repo_path(&format!("src/{}", probe.file_name));
        let first = plugin.lower(&path, &probe.text);
        let second = plugin.lower(&path, &probe.text);
        if let Err(error) = &first
            && is_limit_error(error)
        {
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
                plugin_name,
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

fn check_repository_queries(
    report: &mut ConformanceReport,
    plugin: &WasmLanguagePlugin,
    extension: &str,
) {
    let source = repo_path(&format!("src/main.{extension}"));
    let other = repo_path(&format!("src/lib.{extension}"));
    let repository = vec![
        source.clone(),
        other,
        repo_path("package.json"),
        repo_path("Cargo.toml"),
    ];
    let manifests = || {
        vec![
            Manifest {
                path: "package.json".to_owned(),
                text: "{\"name\":\"probe\"}".to_owned(),
            },
            Manifest {
                path: "Cargo.toml".to_owned(),
                text: "not a valid manifest \u{0}\n[[[".to_owned(),
            },
        ]
    };

    let known: HashSet<&str> = repository.iter().map(RepoPath::as_str).collect();
    let wanted = plugin.manifest_paths(&repository);
    if let Some(unknown) = wanted.iter().find(|path| !known.contains(path.as_str())) {
        report.fail(
            "manifest-paths",
            format!("manifest-paths asked for {unknown:?}, which is not a repository file"),
            "return only paths taken from the repository paths the host passes in",
        );
    } else {
        report.pass("manifest-paths", "asks only for repository files");
    }

    match plugin.workspace_packages(manifests()) {
        Ok(packages) if packages.iter().any(|package| package.as_str().is_empty()) => {
            report.fail(
                "workspace-packages",
                "workspace-packages returned the repository root",
                "return only member package folders; the root package is implicit",
            );
        }
        Ok(_) | Err(_) => report.pass(
            "workspace-packages",
            "answers valid and invalid manifests without an invalid path",
        ),
    }

    if plugin.has_imports() {
        check_imports(report, plugin, &repository, &source, manifests());
    } else {
        report.skip("resolve-imports", "has-imports is false");
    }

    match plugin.declared_dependencies(manifests()) {
        Ok(_) => report.pass("declared-dependencies", "dependency declarations are valid"),
        Err(error) if is_call_error(&error) => report.fail(
            "declared-dependencies",
            error.to_string(),
            "declared-dependencies must never trap; return an empty list for manifests it does not understand",
        ),
        Err(error) => report.fail(
            "declared-dependencies",
            error.to_string(),
            "return dependencies with a repository-relative package root and a non-empty name",
        ),
    }

    match plugin.entry_points(&repository, manifests()) {
        Ok(_) => report.pass("entry-points", "entry points are repository files"),
        Err(error) => report.fail(
            "entry-points",
            error.to_string(),
            "return only entry points that are files from the repository paths the host passes in",
        ),
    }

    match plugin.reference_ranges() {
        Ok(ranges) => report.pass(
            "reference-ranges",
            format!("{} Reference ranges within 0..=1", ranges.iter().count()),
        ),
        Err(error) => report.fail(
            "reference-ranges",
            error.to_string(),
            "return at most one Reference range per Metric with 0 <= low <= high <= 1",
        ),
    }
}

fn is_call_error(error: &codeint_model::LanguagePluginError) -> bool {
    matches!(error, codeint_model::LanguagePluginError::Call { .. })
}

fn check_imports(
    report: &mut ConformanceReport,
    plugin: &WasmLanguagePlugin,
    repository: &[RepoPath],
    importer: &RepoPath,
    manifests: Vec<Manifest>,
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
        Err(error) => report.fail(
            "resolve-imports",
            error.to_string(),
            "return exactly one resolution per request; a resolved import lists one or more repository files",
        ),
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

fn check_measure(report: &mut ConformanceReport, plugin: &WasmMetricPlugin, limits: PluginLimits) {
    let models = [CodeModel::default(), synthetic_model()];
    let mut failed = false;
    for model in &models {
        let first = plugin.measure(model);
        let second = plugin.measure(model);
        match (&first, &second) {
            (Ok(a), Ok(b)) if a != b => {
                report.fail(
                    "measurement-bounds",
                    "two measure calls on the same Code model differ",
                    "make measure deterministic and stateless",
                );
                failed = true;
            }
            (Err(error), _) | (_, Err(error)) => {
                record_measure_error(report, error);
                failed = true;
            }
            _ => {}
        }
        if failed {
            break;
        }
    }
    if failed {
        for id in [
            "measurement-bounds",
            "finding-ids",
            "finding-lines",
            "finding-paths",
        ] {
            if !report.checks.iter().any(|check| check.id == id) {
                report.skip(id, "an earlier measurement error stopped the run");
            }
        }
        return;
    }
    report.pass(
        "measurement-bounds",
        "ratio within 0..=1, numbers finite and non-negative",
    );
    report.pass("finding-ids", "Finding ids are non-empty");
    report.pass("finding-lines", "Finding lines start at 1");
    report.pass("finding-paths", "Finding paths are repository-relative");
    report.pass(
        "limits",
        format!(
            "peak memory {} bytes within {} MiB and {} ms per call",
            plugin.peak_memory_bytes(),
            limits.memory_mb,
            limits.timeout_ms
        ),
    );
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
        | MetricPluginError::TimeLimitExceeded { .. } => report.fail(
            "limits",
            message,
            "keep measure within the memory and time limits, or raise them with --plugin-memory-mb and --plugin-timeout-ms",
        ),
        _ => report.fail(
            "measurement-bounds",
            message,
            "measure must return a Measurement for every Code model without trapping",
        ),
    }
}

fn synthetic_model() -> CodeModel {
    let production = SourceFile {
        path: repo_path("src/app.ts"),
        model: FileModel {
            source_lines: 4,
            ..empty_file_model()
        },
    };
    let test = SourceFile {
        path: repo_path("tests/app.test.ts"),
        model: FileModel {
            source_set: codeint_model::SourceSet::Test,
            source_lines: 2,
            ..empty_file_model()
        },
    };
    CodeModel {
        files: vec![production, test],
        languages: vec![LanguageFacts {
            language: "typescript-javascript".to_owned(),
            has_coverage: false,
            has_hazard_rules: false,
            has_imports: false,
        }],
        ..CodeModel::default()
    }
}
