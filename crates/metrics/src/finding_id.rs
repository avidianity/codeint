use std::collections::{BTreeMap, HashMap};
use std::fmt;

use codeint_model::language::Function;
use codeint_model::{CodeModel, Metric, Package, RepoPath, StableHasher, Token};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FindingId(u64);

impl fmt::Display for FindingId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:016x}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FindingContext {
    pub content_hash: u64,
    pub before: Option<u64>,
    pub after: Option<u64>,
    pub line: Option<u32>,
}

impl FindingContext {
    pub fn content_only(content_hash: u64) -> FindingContext {
        FindingContext {
            content_hash,
            before: None,
            after: None,
            line: None,
        }
    }

    pub(crate) fn for_span(
        content_hash: u64,
        tokens: &[Token],
        start_line: u32,
        end_line: u32,
    ) -> FindingContext {
        let (before, after) = surrounding_line_hashes(tokens, start_line, end_line);
        FindingContext {
            content_hash,
            before,
            after,
            line: (start_line > 0).then_some(start_line),
        }
    }

    pub(crate) fn for_spans<'a>(
        content_hash: u64,
        spans: impl IntoIterator<Item = (&'a [Token], u32, u32)>,
    ) -> FindingContext {
        let mut before_hashes = Vec::new();
        let mut after_hashes = Vec::new();
        let mut first_line = None;
        for (tokens, start_line, end_line) in spans {
            if first_line.is_none() && start_line > 0 {
                first_line = Some(start_line);
            }
            let (before, after) = surrounding_line_hashes(tokens, start_line, end_line);
            before_hashes.push(before);
            after_hashes.push(after);
        }
        FindingContext {
            content_hash,
            before: combine_optional_hashes(&before_hashes),
            after: combine_optional_hashes(&after_hashes),
            line: first_line,
        }
    }
}

pub(crate) fn surrounding_line_hashes(
    tokens: &[Token],
    start_line: u32,
    end_line: u32,
) -> (Option<u64>, Option<u64>) {
    let start_idx = tokens.partition_point(|token| token.line < start_line);
    let before = if start_idx > 0 {
        let prev_line = tokens[start_idx - 1].line;
        let prev_start = tokens[..start_idx].partition_point(|token| token.line < prev_line);
        Some(hash_line_tokens(&tokens[prev_start..start_idx]))
    } else {
        None
    };
    let end_idx = tokens.partition_point(|token| token.line <= end_line);
    let after = if end_idx < tokens.len() {
        let next_line = tokens[end_idx].line;
        let next_end = end_idx + tokens[end_idx..].partition_point(|token| token.line <= next_line);
        Some(hash_line_tokens(&tokens[end_idx..next_end]))
    } else {
        None
    };
    (before, after)
}

fn hash_line_tokens(tokens: &[Token]) -> u64 {
    tokens
        .iter()
        .fold(
            StableHasher::new().write_u64(tokens.len() as u64),
            |hasher, token| hasher.write_u64(token.text_hash),
        )
        .finish()
}

fn combine_optional_hashes(hashes: &[Option<u64>]) -> Option<u64> {
    match hashes {
        [] => None,
        [single] => *single,
        many => {
            if many.iter().all(Option::is_none) {
                return None;
            }
            let combined = many
                .iter()
                .fold(
                    StableHasher::new().write_u64(many.len() as u64),
                    |hasher, item| match item {
                        Some(value) => hasher.write_u64(1).write_u64(*value),
                        None => hasher.write_u64(0),
                    },
                )
                .finish();
            Some(combined)
        }
    }
}

pub(crate) struct FindingIds {
    metric_key: &'static str,
    occurrences: HashMap<u64, u64>,
}

impl FindingIds {
    pub(crate) fn new(metric: Metric) -> FindingIds {
        FindingIds::for_key(metric.key())
    }

    pub(crate) fn for_key(metric_key: &'static str) -> FindingIds {
        FindingIds {
            metric_key,
            occurrences: HashMap::new(),
        }
    }

    pub(crate) fn next_with_content_hash<'path>(
        &mut self,
        paths: impl IntoIterator<Item = &'path RepoPath>,
        content: impl IntoIterator<Item = u64>,
    ) -> (FindingId, u64) {
        let with_metric = length_prefixed(StableHasher::new(), self.metric_key.as_bytes());
        let mut path_count = 0;
        let with_paths = paths.into_iter().fold(with_metric, |hasher, path| {
            path_count += 1;
            length_prefixed(hasher, path.as_str().as_bytes())
        });
        let mut content_count = 0;
        let with_content =
            content
                .into_iter()
                .fold(with_paths.write_u64(path_count), |hasher, value| {
                    content_count += 1;
                    hasher.write_u64(value)
                });
        let same_content = with_content.write_u64(content_count).finish();
        let occurrence = self.occurrences.entry(same_content).or_insert(0);
        let id = StableHasher::new()
            .write_u64(same_content)
            .write_u64(*occurrence)
            .finish();
        *occurrence += 1;
        (FindingId(id), same_content)
    }
}

fn length_prefixed(hasher: StableHasher, bytes: &[u8]) -> StableHasher {
    hasher.write_u64(bytes.len() as u64).write_bytes(bytes)
}

pub(crate) fn group_by_package<'model, T: Default + Copy>(
    model: &'model CodeModel,
    by_file: &BTreeMap<RepoPath, T>,
    merge: fn(&mut T, T),
) -> BTreeMap<Package<'model>, T> {
    let workspace = &model.workspace;
    let mut totals: BTreeMap<Package<'model>, T> = workspace
        .packages()
        .map(|package| (package, T::default()))
        .collect();
    for (path, file_value) in by_file {
        merge(
            totals.entry(workspace.package_of(path)).or_default(),
            *file_value,
        );
    }
    totals
}

pub(crate) fn function_finding_identity(
    ids: &mut FindingIds,
    path: &RepoPath,
    function: &Function,
    tokens: &[Token],
) -> (FindingId, FindingContext) {
    let (id, content_hash) = ids.next_with_content_hash(
        [path],
        tokens_of(function, tokens)
            .iter()
            .map(|token| token.text_hash),
    );
    (
        id,
        FindingContext::for_span(content_hash, tokens, function.line, function.end_line),
    )
}

pub(crate) fn tokens_of<'tokens>(
    function: &Function,
    tokens: &'tokens [Token],
) -> &'tokens [Token] {
    let first = tokens.partition_point(|token| token.line < function.line);
    let after_last = tokens.partition_point(|token| token.line <= function.end_line);
    &tokens[first..after_last.max(first)]
}
