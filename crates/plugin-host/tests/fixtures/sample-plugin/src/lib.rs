#![forbid(unsafe_code)]
#![cfg_attr(
    all(target_arch = "wasm32", target_os = "wasi", target_env = "p2"),
    no_std
)]

use codeint_plugin_sdk::fnv1a_64;
use codeint_plugin_sdk::language::{
    DeclaredDep, DepKind, FileModel, FlaggedLine, FrameworkProfileResult,
    FrameworkProfileSelection, Function, ImportRequest, LanguagePlugin, LowerError, Manifest,
    ManifestAnalysis, PackageFrameworkProfiles, ReferenceRange, Resolution, SourceSet, Token,
    TokenKind, WorkspaceError,
};
use codeint_plugin_sdk::prelude::*;

struct SampleMiniPlugin;

fn parse_manifest_dep(trimmed: &str) -> Option<(&str, DepKind)> {
    if let Some(name) = trimmed.strip_prefix("dep:") {
        Some((name.trim(), DepKind::Runtime))
    } else if let Some(name) = trimmed.strip_prefix("dev-dep:") {
        Some((name.trim(), DepKind::Dev))
    } else {
        trimmed
            .strip_prefix("opt-dep:")
            .map(|name| (name.trim(), DepKind::Optional))
    }
}

fn analyze_workspace_manifest(manifest: &Manifest, analysis: &mut ManifestAnalysis) {
    let mut packages = Vec::new();
    let mut deps = Vec::new();
    for line in manifest.text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(message) = trimmed.strip_prefix("INVALID:") {
            analysis.invalid_manifests.push(WorkspaceError {
                path: manifest.path.clone(),
                message: message.trim().to_string(),
            });
            return;
        }
        if let Some((name, kind)) = parse_manifest_dep(trimmed) {
            if !name.is_empty() {
                deps.push(DeclaredDep {
                    package_root: String::new(),
                    name: name.to_string(),
                    kind,
                });
            }
        } else if !is_workspace_directive(trimmed) {
            packages.push(trimmed.to_string());
        }
    }
    analysis.workspace_packages.extend(packages);
    analysis.declared_dependencies.extend(deps);
}

codeint_plugin_sdk::export_language_plugin!(SampleMiniPlugin);

fn strip_comment(line: &str) -> &str {
    let mut in_string = false;
    for (index, byte) in line.bytes().enumerate() {
        match byte {
            b'"' => in_string = !in_string,
            b'#' if !in_string => return &line[..index],
            _ => {}
        }
    }
    line
}

fn source_set_of(path: &str) -> SourceSet {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    if file_name.ends_with(".test.mini") || path.split('/').any(|segment| segment == "tests") {
        SourceSet::Test
    } else {
        SourceSet::Production
    }
}

fn scan_string_end(bytes: &[u8], start: usize) -> usize {
    let mut index = start + 1;
    while index < bytes.len() && bytes[index] != b'"' {
        index += 1;
    }
    if index < bytes.len() {
        index + 1
    } else {
        index
    }
}

fn scan_while(bytes: &[u8], start: usize, predicate: fn(u8) -> bool) -> usize {
    let mut index = start;
    while index < bytes.len() && predicate(bytes[index]) {
        index += 1;
    }
    index
}

fn is_ident_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn classify_ident_kind(text: &str) -> TokenKind {
    if matches!(
        text,
        "fn" | "if" | "else" | "while" | "return" | "let" | "pass"
    ) {
        TokenKind::Other
    } else {
        TokenKind::Identifier
    }
}

fn push_token(
    tokens: &mut Vec<Token>,
    text: &str,
    default_kind: TokenKind,
    is_import: bool,
    line: u32,
) {
    tokens.push(Token {
        kind: if is_import {
            TokenKind::ImportOrExport
        } else {
            default_kind
        },
        text_hash: fnv1a_64(text),
        line,
    });
}

fn tokenize_line(code: &str, line_number: u32, is_import: bool, tokens: &mut Vec<Token>) {
    let bytes = code.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_whitespace() {
            index += 1;
            continue;
        }
        let (end, kind) = if byte == b'"' {
            (scan_string_end(bytes, index), TokenKind::Literal)
        } else if byte.is_ascii_digit() {
            (
                scan_while(bytes, index, |b| b.is_ascii_digit()),
                TokenKind::Literal,
            )
        } else if byte.is_ascii_alphabetic() || byte == b'_' {
            let end = scan_while(bytes, index, is_ident_continue);
            (end, classify_ident_kind(&code[index..end]))
        } else {
            (index + 1, TokenKind::Other)
        };
        push_token(tokens, &code[index..end], kind, is_import, line_number);
        index = end;
    }
}

fn extract_import_specifier(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix("import ")?.trim();
    let inner = rest.strip_prefix('"')?.strip_suffix('"')?;
    Some(inner.to_string())
}

fn extract_function_name(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix("fn")?.trim_start();
    let end = rest
        .find(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .unwrap_or(rest.len());
    let name = &rest[..end];
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

struct OpenFunction {
    slot: usize,
    name: Option<String>,
    line: u32,
    brace_depth: u32,
    decision_points: u32,
    cognitive_complexity: u32,
    source_lines: u32,
}

fn is_workspace_directive(trimmed: &str) -> bool {
    ["dep:", "dev-dep:", "opt-dep:", "entry:"]
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
}

fn push_function_header(
    trimmed: &str,
    line_number: u32,
    functions: &mut Vec<Function>,
    stack: &mut Vec<OpenFunction>,
) {
    let opens_fn =
        (trimmed.starts_with("fn ") || trimmed.starts_with("fn(")) && trimmed.contains('{');
    if !opens_fn {
        return;
    }
    let slot = functions.len();
    let name = extract_function_name(trimmed);
    functions.push(Function {
        line: line_number,
        end_line: line_number,
        decision_points: 0,
        source_lines: 0,
        name: name.clone(),
        cognitive_complexity: 0,
        coverage: None,
    });
    stack.push(OpenFunction {
        slot,
        name,
        line: line_number,
        brace_depth: 0,
        decision_points: 0,
        cognitive_complexity: 0,
        source_lines: 0,
    });
}

fn update_function_stack(
    code: &str,
    trimmed: &str,
    line_number: u32,
    functions: &mut [Function],
    stack: &mut Vec<OpenFunction>,
) -> Result<(), LowerError> {
    let Some(current) = stack.last_mut() else {
        if code.contains('}') {
            return Err(LowerError::Unparsable(format!(
                "unmatched closing brace at line {line_number}"
            )));
        }
        return Ok(());
    };
    current.source_lines += 1;
    let nesting = current.brace_depth.saturating_sub(1);
    for word in trimmed.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_') {
        if matches!(word, "if" | "while") {
            current.decision_points += 1;
            current.cognitive_complexity += 1 + nesting;
        }
    }
    for byte in code.bytes() {
        if byte == b'{' {
            current.brace_depth += 1;
        } else if byte == b'}' {
            if current.brace_depth == 0 {
                return Err(LowerError::Unparsable(format!(
                    "unmatched closing brace at line {line_number}"
                )));
            }
            current.brace_depth -= 1;
        }
    }
    if current.brace_depth == 0 {
        let finished = stack.pop().unwrap();
        functions[finished.slot] = Function {
            line: finished.line,
            end_line: line_number,
            decision_points: finished.decision_points,
            source_lines: finished.source_lines,
            name: finished.name,
            cognitive_complexity: finished.cognitive_complexity,
            coverage: None,
        };
    }
    Ok(())
}

struct LowerState {
    source_lines: u32,
    functions: Vec<Function>,
    stack: Vec<OpenFunction>,
    tokens: Vec<Token>,
    flagged_lines: Vec<FlaggedLine>,
    runtime_imports: Vec<String>,
}

impl LowerState {
    fn new() -> Self {
        Self {
            source_lines: 0,
            functions: Vec::new(),
            stack: Vec::new(),
            tokens: Vec::new(),
            flagged_lines: Vec::new(),
            runtime_imports: Vec::new(),
        }
    }

    fn lower_line(&mut self, line_number: u32, raw_line: &str) -> Result<(), LowerError> {
        let code = strip_comment(raw_line);
        let trimmed = code.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        if trimmed.contains("SYNTAX_ERROR") {
            return Err(LowerError::Unparsable(format!(
                "syntax error at line {line_number}"
            )));
        }
        self.source_lines = self
            .source_lines
            .checked_add(1)
            .ok_or(LowerError::TooLarge)?;
        let is_import = if let Some(specifier) = extract_import_specifier(trimmed) {
            self.runtime_imports.push(specifier);
            true
        } else {
            false
        };
        if trimmed == "pass;" {
            self.flagged_lines.push(FlaggedLine {
                line: line_number,
                rule: "redundant-pass".to_string(),
            });
        }
        tokenize_line(code, line_number, is_import, &mut self.tokens);
        push_function_header(trimmed, line_number, &mut self.functions, &mut self.stack);
        update_function_stack(
            code,
            trimmed,
            line_number,
            &mut self.functions,
            &mut self.stack,
        )
    }
}

fn resolve_mini_candidate(repository_paths: &[String], candidate: String) -> Resolution {
    let with_ext = if candidate.ends_with(".mini") {
        candidate.clone()
    } else {
        format!("{candidate}.mini")
    };
    if repository_paths.iter().any(|path| path == &candidate) {
        Resolution::Resolved(vec![candidate])
    } else if repository_paths.iter().any(|path| path == &with_ext) {
        Resolution::Resolved(vec![with_ext])
    } else {
        Resolution::Unresolved(format!("cannot find {with_ext}"))
    }
}

fn resolve_single_import(
    repository_paths: &[String],
    request: ImportRequest,
    active_profiles: &[String],
) -> Resolution {
    if request.specifier.starts_with("std:") {
        return Resolution::Builtin;
    }
    if active_profiles.iter().any(|profile| profile == "mini-web")
        && let Some(alias_path) = request.specifier.strip_prefix("@/")
    {
        return resolve_mini_candidate(repository_paths, format!("src/{alias_path}"));
    }
    let Some(relative) = request.specifier.strip_prefix("./") else {
        return Resolution::External(request.specifier);
    };
    let candidate = match request.importer.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/{relative}"),
        None => relative.to_string(),
    };
    resolve_mini_candidate(repository_paths, candidate)
}

fn detect_framework_profiles(manifests: &[Manifest]) -> Vec<String> {
    ["mini-web", "mini-cli", "mini-app"]
        .into_iter()
        .filter(|profile| {
            let marker = format!("framework:{profile}");
            manifests
                .iter()
                .any(|manifest| manifest.text.lines().any(|line| line.trim() == marker))
        })
        .map(ToString::to_string)
        .collect()
}

fn select_active_profiles(
    detected: Vec<String>,
    selection: FrameworkProfileSelection,
) -> Result<Vec<String>, String> {
    match selection {
        FrameworkProfileSelection::Auto => Ok(detected),
        FrameworkProfileSelection::Disabled => Ok(Vec::new()),
        FrameworkProfileSelection::Exclude(excluded) => Ok(detected
            .into_iter()
            .filter(|profile| !excluded.contains(profile))
            .collect()),
        FrameworkProfileSelection::Only(only) => {
            if let Some(unknown) = only
                .iter()
                .find(|profile| !matches!(profile.as_str(), "mini-web" | "mini-cli" | "mini-app"))
            {
                return Err(format!("unknown Framework profile {unknown:?}"));
            }
            Ok(only)
        }
    }
}

fn filter_mini_paths(files: &[String], folder: &str) -> Vec<String> {
    files
        .iter()
        .filter(|file| file.contains(folder) && file.ends_with(".mini"))
        .cloned()
        .collect()
}

fn select_profile_ranges(
    has_web: bool,
    has_cli: bool,
    has_app: bool,
) -> (Vec<ReferenceRange>, Option<String>) {
    if has_cli && (has_web || has_app) {
        (
            Vec::new(),
            Some("multiple profiles active; falling back to base mini ranges".to_string()),
        )
    } else if has_app {
        (
            vec![ReferenceRange {
                metric: "erosion".to_string(),
                metric_version: 1,
                low: 0.02,
                high: 0.25,
                source: "mini-app framework profile".to_string(),
            }],
            None,
        )
    } else if has_web {
        (
            vec![ReferenceRange {
                metric: "erosion".to_string(),
                metric_version: 1,
                low: 0.05,
                high: 0.4,
                source: "mini-web framework profile".to_string(),
            }],
            Some("mini-web profile active".to_string()),
        )
    } else {
        (Vec::new(), None)
    }
}

fn active_profiles_for<'a>(
    package_profiles: &'a [PackageFrameworkProfiles],
    path: &str,
) -> &'a [String] {
    let mut best: Option<(usize, &'a [String])> = None;
    for entry in package_profiles {
        let matches = entry.package_root.is_empty()
            || path == entry.package_root
            || path
                .strip_prefix(entry.package_root.as_str())
                .is_some_and(|rest| rest.starts_with('/'));
        if matches && best.is_none_or(|(depth, _)| entry.package_root.len() >= depth) {
            best = Some((entry.package_root.len(), entry.active_profiles.as_slice()));
        }
    }
    best.map_or(&[], |(_, profiles)| profiles)
}

impl LanguagePlugin for SampleMiniPlugin {
    fn extensions() -> Vec<String> {
        vec!["mini".to_string()]
    }

    fn flagged_line_rules() -> Vec<String> {
        vec!["redundant-pass".to_string()]
    }

    fn lower(
        path: String,
        text: String,
        _dialect: Option<String>,
    ) -> Result<FileModel, LowerError> {
        let file_name = path.rsplit('/').next().unwrap_or(&path);
        if !file_name.ends_with(".mini") {
            return Err(LowerError::UnsupportedFile(format!(
                "{path} does not end with .mini"
            )));
        }
        if file_name.ends_with(".d.mini") {
            return Err(LowerError::DeclarationOnly);
        }
        let mut state = LowerState::new();
        for (zero_based, raw_line) in text.lines().enumerate() {
            let line_number = u32::try_from(zero_based + 1).map_err(|_| LowerError::TooLarge)?;
            state.lower_line(line_number, raw_line)?;
        }
        if !state.stack.is_empty() {
            return Err(LowerError::Unparsable(
                "unclosed function block".to_string(),
            ));
        }
        Ok(FileModel {
            source_lines: state.source_lines,
            functions: state.functions,
            tokens: state.tokens,
            flagged_lines: state.flagged_lines,
            runtime_imports: state.runtime_imports,
            ..FileModel::empty(source_set_of(&path))
        })
    }

    fn manifest_paths(repository_paths: Vec<String>) -> Vec<String> {
        repository_paths
            .into_iter()
            .filter(|path| path == "mini.workspace")
            .collect()
    }

    fn analyze_manifests(manifests: Vec<Manifest>) -> ManifestAnalysis {
        let mut analysis = ManifestAnalysis::empty();
        for manifest in manifests {
            if manifest.path == "mini.workspace" {
                analyze_workspace_manifest(&manifest, &mut analysis);
            }
        }
        analysis
    }

    fn resolve_imports(
        repository_paths: Vec<String>,
        _manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
        package_profiles: Vec<PackageFrameworkProfiles>,
    ) -> Vec<Resolution> {
        requests
            .into_iter()
            .map(|request| {
                let active = active_profiles_for(&package_profiles, &request.importer);
                resolve_single_import(&repository_paths, request, active)
            })
            .collect()
    }

    fn entry_points(repository_paths: Vec<String>, manifests: Vec<Manifest>) -> Vec<String> {
        let mut entries = Vec::new();
        for manifest in manifests {
            if manifest.path != "mini.workspace" {
                continue;
            }
            for line in manifest.text.lines() {
                if let Some(entry) = line.trim().strip_prefix("entry:") {
                    let candidate = entry.trim();
                    if repository_paths.iter().any(|path| path == candidate) {
                        entries.push(candidate.to_string());
                    }
                }
            }
        }
        entries
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        vec![ReferenceRange {
            metric: "erosion".to_string(),
            metric_version: 1,
            low: 0.0,
            high: 0.5,
            source: "sample mini language fixture".to_string(),
        }]
    }

    fn known_framework_profiles() -> Vec<String> {
        vec![
            "mini-web".to_string(),
            "mini-cli".to_string(),
            "mini-app".to_string(),
        ]
    }

    fn resolve_framework_profiles(
        _package_root: String,
        files: Vec<String>,
        manifests: Vec<Manifest>,
        selection: FrameworkProfileSelection,
    ) -> Result<FrameworkProfileResult, String> {
        let detected = detect_framework_profiles(&manifests);
        let active_profiles = select_active_profiles(detected, selection)?;
        let has_web = active_profiles.iter().any(|profile| profile == "mini-web");
        let has_cli = active_profiles.iter().any(|profile| profile == "mini-cli");
        let has_app = active_profiles.iter().any(|profile| profile == "mini-app");
        let mut entry_points = Vec::new();
        if has_web {
            entry_points.extend(filter_mini_paths(&files, "pages/"));
        }
        if has_app {
            entry_points.extend(filter_mini_paths(&files, "routes/"));
        }
        let mut generated_paths = Vec::new();
        if has_web {
            generated_paths.extend(filter_mini_paths(&files, "gen/"));
        }
        if has_app {
            generated_paths.extend(filter_mini_paths(&files, "dist-gen/"));
        }
        let (reference_ranges, reference_range_note) =
            select_profile_ranges(has_web, has_cli, has_app);
        Ok(FrameworkProfileResult {
            active_profiles,
            entry_points,
            generated_paths,
            reference_ranges,
            reference_range_note,
        })
    }

    fn has_imports() -> bool {
        true
    }
}
