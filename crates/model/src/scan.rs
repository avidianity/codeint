use std::collections::{BTreeSet, HashSet};

use crate::language::{
    DeclaredDep, DependencyManifestOwner, FlaggedLine, Function, LowerError, Manifest,
    ManifestAnalysis, ReferenceRange, WorkspaceError,
};

#[cfg(test)]
mod tests;

pub trait ScanToken {
    fn text(&self) -> &str;
    fn is_keyword(&self, kw: &str) -> bool;
    fn line(&self) -> u32;
}

fn delim_pair(open: &str) -> Option<(&'static str, &'static str)> {
    match open {
        "(" => Some(("(", ")")),
        "{" => Some(("{", "}")),
        "[" | "#[" => Some(("[", "]")),
        _ => None,
    }
}

pub fn find_matching_delim<T: ScanToken>(tokens: &[T], open_idx: usize) -> Option<usize> {
    let (inc, dec) = delim_pair(tokens.get(open_idx)?.text())?;
    let mut depth = 0usize;
    for (idx, token) in tokens.iter().enumerate().skip(open_idx) {
        let text = token.text();
        if text == inc || (inc == "[" && text == "#[") {
            depth += 1;
        } else if text == dec {
            depth -= 1;
            if depth == 0 {
                return Some(idx);
            }
        }
    }
    None
}

pub fn paren_then_brace<T: ScanToken>(
    tokens: &[T],
    kw_idx: usize,
) -> Option<(usize, usize, usize, usize)> {
    let paren_open = kw_idx + 1;
    if tokens.get(paren_open)?.text() != "(" {
        return None;
    }
    let paren_close = find_matching_delim(tokens, paren_open)?;
    let brace_open = paren_close + 1;
    if tokens.get(brace_open)?.text() != "{" {
        return None;
    }
    let brace_close = find_matching_delim(tokens, brace_open)?;
    Some((paren_open, paren_close, brace_open, brace_close))
}

pub fn flag_range(
    start_line: u32,
    end_line: u32,
    rule: &'static str,
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    for &line in code_lines.range(start_line..=end_line) {
        out.insert((line, rule));
    }
}

pub fn finalize_flagged(flagged: BTreeSet<(u32, &'static str)>) -> Vec<FlaggedLine> {
    flagged
        .into_iter()
        .map(|(line, rule)| FlaggedLine {
            line,
            rule: rule.to_owned(),
        })
        .collect()
}

pub fn check_c_style_if_else<T: ScanToken>(
    tokens: &[T],
    if_idx: usize,
    exit_keywords: &[&str],
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    let Some((_, _, then_open, then_close)) = paren_then_brace(tokens, if_idx) else {
        return;
    };
    let else_idx = then_close + 1;
    if tokens.get(else_idx).is_none_or(|t| !t.is_keyword("else")) {
        return;
    }
    let else_open = else_idx + 1;
    if tokens.get(else_open).is_none_or(|t| t.text() != "{") {
        return;
    }
    let Some(else_close) = find_matching_delim(tokens, else_open) else {
        return;
    };
    let then_slice = &tokens[then_open + 1..then_close];
    let else_slice = &tokens[else_open + 1..else_close];
    if let (Some(a), Some(b)) = (
        single_bool_return(then_slice),
        single_bool_return(else_slice),
    ) && a != b
    {
        flag_range(
            tokens[if_idx].line(),
            tokens[else_close].line(),
            "if-else-returns-boolean",
            code_lines,
            out,
        );
        return;
    }
    if ends_with_unconditional_exit(then_slice, exit_keywords) {
        flag_range(
            tokens[else_idx].line(),
            tokens[else_close].line(),
            "else-after-return",
            code_lines,
            out,
        );
    }
}

fn single_bool_return<T: ScanToken>(slice: &[T]) -> Option<bool> {
    if slice.len() == 3 && slice[0].is_keyword("return") && slice[2].text() == ";" {
        if slice[1].is_keyword("true") {
            Some(true)
        } else if slice[1].is_keyword("false") {
            Some(false)
        } else {
            None
        }
    } else {
        None
    }
}

fn ends_with_unconditional_exit<T: ScanToken>(slice: &[T], exit_keywords: &[&str]) -> bool {
    if slice.last().is_none_or(|t| t.text() != ";") {
        return false;
    }
    let mut depth = 0usize;
    let mut stmt_start = 0usize;
    for (idx, token) in slice.iter().enumerate() {
        match token.text() {
            "(" | "[" | "{" | "#[" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            ";" if depth == 0 && idx + 1 < slice.len() => stmt_start = idx + 1,
            _ => {}
        }
    }
    slice
        .get(stmt_start)
        .is_some_and(|t| exit_keywords.iter().any(|kw| t.is_keyword(kw)))
}

pub fn check_ternary_bool<T: ScanToken>(
    tokens: &[T],
    q_idx: usize,
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    if q_idx + 3 >= tokens.len() || tokens[q_idx + 2].text() != ":" {
        return;
    }
    let a_true = tokens[q_idx + 1].is_keyword("true") && tokens[q_idx + 3].is_keyword("false");
    let a_false = tokens[q_idx + 1].is_keyword("false") && tokens[q_idx + 3].is_keyword("true");
    if a_true || a_false {
        flag_range(
            tokens[q_idx].line(),
            tokens[q_idx + 3].line(),
            "ternary-yields-boolean",
            code_lines,
            out,
        );
    }
}

pub fn score_function_spans<T: ScanToken>(
    tokens: &[T],
    code_lines: &BTreeSet<u32>,
    spans: &[(usize, usize, u32, u32)],
    decision_weight: impl Fn(&[T], usize) -> u32,
) -> Vec<Function> {
    let mut result: Vec<Function> = spans
        .iter()
        .enumerate()
        .map(|(owner, &(body_start, body_end, start_line, end_line))| {
            let mut decisions = 0u32;
            let mut cursor = body_start;
            while cursor <= body_end {
                if let Some(child_end) = nested_end(spans, owner, cursor) {
                    cursor = child_end + 1;
                    continue;
                }
                decisions += decision_weight(tokens, cursor);
                cursor += 1;
            }
            Function {
                line: start_line,
                end_line,
                decision_points: decisions,
                source_lines: code_lines.range(start_line..=end_line).count() as u32,
                name: None,
                cognitive_complexity: 0,
                coverage: None,
            }
        })
        .collect();
    result.sort_by_key(|f| (f.line, f.end_line));
    result
}

fn nested_end(spans: &[(usize, usize, u32, u32)], owner: usize, cursor: usize) -> Option<usize> {
    spans
        .iter()
        .enumerate()
        .find(|&(idx, &(start, _, _, _))| idx != owner && start == cursor)
        .map(|(_, &(_, end, _, _))| end)
}

pub fn normalize_relative_path(base_dir: &str, rel: &str) -> Option<String> {
    let mut segments: Vec<&str> = if base_dir.is_empty() {
        Vec::new()
    } else {
        base_dir.split('/').collect()
    };
    for seg in rel.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                segments.pop()?;
            }
            other => segments.push(other),
        }
    }
    Some(segments.join("/"))
}

pub fn find_manifests<P: AsRef<str>>(repository_paths: &[P], filename: &str) -> Vec<String> {
    let nested = format!("/{filename}");
    repository_paths
        .iter()
        .map(AsRef::as_ref)
        .filter(|path| *path == filename || path.ends_with(&nested))
        .map(str::to_owned)
        .collect()
}

pub fn track_delim(stack: &mut Vec<(u8, u32)>, op: &str, line: u32) -> Result<(), LowerError> {
    match op {
        "#[" | "[" => stack.push((b'[', line)),
        "{" => stack.push((b'{', line)),
        "(" => stack.push((b'(', line)),
        "}" | ")" | "]" => {
            let expected = match op {
                "}" => b'{',
                ")" => b'(',
                _ => b'[',
            };
            if !matches!(stack.pop(), Some((open, _)) if open == expected) {
                return Err(LowerError::Unparsable(format!(
                    "unexpected {op:?} at line {line}"
                )));
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn find_arrow_expr_end<T: ScanToken>(tokens: &[T], start: usize) -> usize {
    let mut depth = 0usize;
    for (idx, tok) in tokens.iter().enumerate().skip(start) {
        match tok.text() {
            "(" | "[" | "{" | "#[" => depth += 1,
            ")" | "]" | "}" if depth == 0 => return idx.saturating_sub(1),
            ")" | "]" | "}" => depth -= 1,
            ";" | "," if depth == 0 => return idx.saturating_sub(1),
            _ => {}
        }
    }
    tokens.len().saturating_sub(1)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelativeFileResolution {
    Resolved(String),
    Ignored,
    Unresolved(String),
}

pub fn resolve_relative_file(
    files: &HashSet<String>,
    importer: &str,
    rel_uri: &str,
    ignore_vendor: bool,
) -> RelativeFileResolution {
    if rel_uri.starts_with('/') {
        return RelativeFileResolution::Unresolved(format!(
            "{rel_uri:?} is an absolute path, which depends on the machine"
        ));
    }
    let parent = importer.rsplit_once('/').map_or("", |(dir, _)| dir);
    let Some(joined) = normalize_relative_path(parent, rel_uri) else {
        return RelativeFileResolution::Unresolved(format!(
            "{rel_uri:?} leads outside the repository"
        ));
    };
    if ignore_vendor && joined.split('/').any(|seg| seg == "vendor") {
        return RelativeFileResolution::Ignored;
    }
    if files.contains(&joined) {
        if joined == importer {
            RelativeFileResolution::Ignored
        } else {
            RelativeFileResolution::Resolved(joined)
        }
    } else {
        RelativeFileResolution::Unresolved(format!("no file of the repository matches {rel_uri:?}"))
    }
}

pub struct CoreReferenceRanges {
    pub erosion: (f64, f64),
    pub cognitive_complexity: (f64, f64),
    pub verbosity: (f64, f64),
    pub duplication: (f64, f64),
    pub import_cycles: Option<(f64, f64)>,
}

impl CoreReferenceRanges {
    pub const fn new(
        erosion: (f64, f64),
        cognitive_complexity: (f64, f64),
        verbosity: (f64, f64),
        duplication: (f64, f64),
        import_cycles: Option<(f64, f64)>,
    ) -> Self {
        Self {
            erosion,
            cognitive_complexity,
            verbosity,
            duplication,
            import_cycles,
        }
    }

    pub fn build(self, source: &str) -> Vec<ReferenceRange> {
        let mut ranges = build_reference_ranges(
            source,
            [
                ("erosion", 1, self.erosion.0, self.erosion.1),
                (
                    "cognitive_complexity",
                    1,
                    self.cognitive_complexity.0,
                    self.cognitive_complexity.1,
                ),
                ("verbosity", 2, self.verbosity.0, self.verbosity.1),
                ("duplication", 2, self.duplication.0, self.duplication.1),
            ],
        );
        if let Some((low, high)) = self.import_cycles {
            ranges.push(ReferenceRange {
                metric: "import_cycles".to_owned(),
                metric_version: 1,
                low,
                high,
                source: source.to_owned(),
            });
        }
        ranges
    }
}

pub fn paths_with_suffix<'a, P: AsRef<str>>(
    repository_paths: &'a [P],
    suffix: &str,
) -> HashSet<&'a str> {
    repository_paths
        .iter()
        .map(AsRef::as_ref)
        .filter(|path| path.ends_with(suffix))
        .collect()
}

pub fn build_reference_ranges<const N: usize>(
    source: &str,
    entries: [(&str, u32, f64, f64); N],
) -> Vec<ReferenceRange> {
    entries
        .into_iter()
        .map(|(metric, metric_version, low, high)| ReferenceRange {
            metric: metric.to_owned(),
            metric_version,
            low,
            high,
            source: source.to_owned(),
        })
        .collect()
}

pub fn manifest_root_excluding<'a>(
    path: &'a str,
    file_name: &str,
    ignored_dirs: &[&str],
) -> Option<&'a str> {
    if path == file_name {
        return Some("");
    }
    let dir = path.strip_suffix(file_name)?.strip_suffix('/')?;
    (!dir.split('/').any(|seg| ignored_dirs.contains(&seg))).then_some(dir)
}

pub fn simple_manifest_analysis(
    manifests: &[Manifest],
    root_manifest_path: &str,
    discover: impl FnOnce(&[Manifest]) -> Result<Vec<String>, WorkspaceError>,
    extract: impl FnOnce(&[&Manifest]) -> (Vec<DependencyManifestOwner>, Vec<DeclaredDep>),
) -> ManifestAnalysis {
    let mut invalid_manifests = Vec::new();
    let mut workspace_packages = Vec::new();
    let mut root_invalid = false;
    if manifests
        .iter()
        .any(|manifest| manifest.path == root_manifest_path)
    {
        match discover(manifests) {
            Ok(packages) => workspace_packages = packages,
            Err(error) => {
                invalid_manifests.push(error);
                root_invalid = true;
            }
        }
    }
    let valid: Vec<&Manifest> = manifests
        .iter()
        .filter(|manifest| !(root_invalid && manifest.path == root_manifest_path))
        .collect();
    let (dependency_manifests, declared_dependencies) = extract(&valid);
    ManifestAnalysis {
        workspace_packages,
        invalid_manifests,
        dependency_manifests,
        declared_dependencies,
        manifest_used_dependencies: Vec::new(),
        root_dependency_inheritance: Vec::new(),
    }
}
