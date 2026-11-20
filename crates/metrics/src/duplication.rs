use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::num::NonZeroUsize;

use codeint_model::{CodeModel, Metric, Package, RepoPath, Token, TokenKind};

use crate::line_share::{LineShare, line_shares_by_package};

use crate::FindingId;
use crate::finding_id::FindingIds;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MinimumCloneTokens(NonZeroUsize);

impl MinimumCloneTokens {
    pub const DEFAULT: MinimumCloneTokens =
        MinimumCloneTokens(NonZeroUsize::new(75).expect("75 is not zero"));

    pub fn new(tokens: NonZeroUsize) -> MinimumCloneTokens {
        MinimumCloneTokens(tokens)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MinimumDistinctTokens(usize);

impl MinimumDistinctTokens {
    pub const DEFAULT: MinimumDistinctTokens = MinimumDistinctTokens(14);

    pub fn new(tokens: usize) -> MinimumDistinctTokens {
        MinimumDistinctTokens(tokens)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Duplication {
    pub clone_lines: CloneLines,
    pub source_lines: u64,
    pub clone_groups: Vec<CloneGroup>,
}

impl Duplication {
    pub fn measure(
        model: &CodeModel,
        minimum: MinimumCloneTokens,
        minimum_distinct: MinimumDistinctTokens,
    ) -> Duplication {
        let window = minimum.0.get();
        let classes = equal_window_classes(model, &searched_runs(model), window);
        let mut clone_lines_by_file: BTreeMap<usize, BTreeSet<u32>> = BTreeMap::new();
        let mut unidentified_groups = Vec::new();
        for (tokens, starts) in maximal_runs(&classes, window) {
            let starts = without_overlaps(starts, tokens);
            if starts.len() < 2 {
                continue;
            }
            let first = starts[0];
            let first_fragment =
                &model.files[first.file].model.tokens[first.token..first.token + tokens];
            if distinct_normalized_tokens(first_fragment) < minimum_distinct.0 {
                continue;
            }
            let normalized_content: Vec<u64> = model.files[first.file].model.tokens
                [first.token..first.token + tokens]
                .iter()
                .map(|token| normalized(token).hash_input())
                .collect();
            let mut fragments = Vec::with_capacity(starts.len());
            for Position { file, token } in starts {
                let covered = &model.files[file].model.tokens[token..token + tokens];
                clone_lines_by_file
                    .entry(file)
                    .or_default()
                    .extend(covered.iter().map(|covered| covered.line));
                fragments.push(fragment(&model.files[file].path, covered));
            }
            fragments.sort();
            unidentified_groups.push((tokens, fragments, normalized_content));
        }
        unidentified_groups
            .sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
        let mut ids = FindingIds::new(Metric::Duplication);
        let clone_groups = unidentified_groups
            .into_iter()
            .map(|(tokens, fragments, normalized_content)| CloneGroup {
                id: ids.next_in_source_order(
                    fragments.iter().map(|fragment| &fragment.path),
                    normalized_content,
                ),
                tokens,
                fragments,
            })
            .collect();
        Duplication {
            clone_lines: CloneLines::of_files(model, clone_lines_by_file),
            source_lines: model
                .files
                .iter()
                .map(|file| u64::from(file.model.source_lines))
                .sum(),
            clone_groups,
        }
    }

    pub fn ratio(&self) -> f64 {
        if self.source_lines == 0 {
            0.0
        } else {
            self.clone_lines.count() as f64 / self.source_lines as f64
        }
    }

    pub fn by_package<'model>(
        &self,
        model: &'model CodeModel,
    ) -> BTreeMap<Package<'model>, LineShare> {
        line_shares_by_package(model, self.clone_lines.count_by_file())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CloneLines(BTreeMap<RepoPath, BTreeSet<u32>>);

impl CloneLines {
    fn of_files(model: &CodeModel, lines_by_file: BTreeMap<usize, BTreeSet<u32>>) -> CloneLines {
        CloneLines(
            lines_by_file
                .into_iter()
                .map(|(file, lines)| (model.files[file].path.clone(), lines))
                .collect(),
        )
    }

    pub fn count(&self) -> u64 {
        self.0.values().map(|lines| lines.len() as u64).sum()
    }

    pub(crate) fn count_by_file(&self) -> impl Iterator<Item = (&RepoPath, u64)> {
        self.0
            .iter()
            .map(|(path, lines)| (path, lines.len() as u64))
    }

    pub fn contains(&self, path: &RepoPath, line: u32) -> bool {
        self.0.get(path).is_some_and(|lines| lines.contains(&line))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloneGroup {
    pub id: FindingId,
    pub tokens: usize,
    pub fragments: Vec<Fragment>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fragment {
    pub path: RepoPath,
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Position {
    file: usize,
    token: usize,
}

impl Position {
    fn next(self) -> Position {
        Position {
            file: self.file,
            token: self.token + 1,
        }
    }
}

struct WindowClasses {
    starts: Vec<Vec<Position>>,
    class_at: HashMap<Position, usize>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum NormalizedToken {
    Identifier,
    Literal,
    Other { text_hash: u64 },
}

fn normalized(token: &Token) -> NormalizedToken {
    match token.kind {
        TokenKind::Identifier => NormalizedToken::Identifier,
        TokenKind::Literal => NormalizedToken::Literal,
        TokenKind::Other | TokenKind::ImportOrExport => NormalizedToken::Other {
            text_hash: token.text_hash,
        },
    }
}

impl NormalizedToken {
    fn hash_input(self) -> u64 {
        match self {
            NormalizedToken::Identifier => 0,
            NormalizedToken::Literal => 1,
            NormalizedToken::Other { text_hash } => text_hash,
        }
    }
}

struct SearchedRun<'model> {
    file: usize,
    first_token: usize,
    tokens: &'model [Token],
}

fn searched_runs(model: &CodeModel) -> Vec<SearchedRun<'_>> {
    let mut runs = Vec::new();
    for (file, source_file) in model.files.iter().enumerate() {
        let mut first_token = 0;
        for searched in source_file
            .model
            .tokens
            .split(|token| token.kind == TokenKind::ImportOrExport)
        {
            if !searched.is_empty() {
                runs.push(SearchedRun {
                    file,
                    first_token,
                    tokens: searched,
                });
            }
            first_token += searched.len() + 1;
        }
    }
    runs
}

fn distinct_normalized_tokens(tokens: &[Token]) -> usize {
    tokens.iter().map(normalized).collect::<HashSet<_>>().len()
}

struct FirstTokenIndexes(Vec<usize>);

impl FirstTokenIndexes {
    fn new(runs: &[SearchedRun<'_>]) -> FirstTokenIndexes {
        FirstTokenIndexes(
            runs.iter()
                .scan(0, |next, run| {
                    let first = *next;
                    *next += run.tokens.len();
                    Some(first)
                })
                .collect(),
        )
    }

    fn position(&self, runs: &[SearchedRun<'_>], index: usize) -> Position {
        let run = self.0.partition_point(|first| *first <= index) - 1;
        Position {
            file: runs[run].file,
            token: runs[run].first_token + index - self.0[run],
        }
    }
}

fn equal_window_classes(
    model: &CodeModel,
    runs: &[SearchedRun<'_>],
    window: usize,
) -> WindowClasses {
    let window_count = runs
        .iter()
        .map(|run| (run.tokens.len() + 1).saturating_sub(window))
        .sum();
    let first_token_indexes = FirstTokenIndexes::new(runs);
    let mut windows: Vec<(u64, usize)> = Vec::with_capacity(window_count);
    for (run, first_token_index) in runs.iter().zip(&first_token_indexes.0) {
        windows.extend(
            rolling_window_hashes(run.tokens, window)
                .enumerate()
                .map(|(token, hash)| (hash, first_token_index + token)),
        );
    }
    windows.sort_unstable();
    let mut classes = WindowClasses {
        starts: Vec::new(),
        class_at: HashMap::new(),
    };
    for same_hash in windows.chunk_by(|left, right| left.0 == right.0) {
        if same_hash.len() < 2 {
            continue;
        }
        let normalized_window = |position: Position| {
            model.files[position.file].model.tokens[position.token..position.token + window]
                .iter()
                .map(normalized)
        };
        let mut equal_text: Vec<Vec<Position>> = Vec::new();
        for position in same_hash
            .iter()
            .map(|&(_, index)| first_token_indexes.position(runs, index))
        {
            match equal_text
                .iter_mut()
                .find(|starts| normalized_window(starts[0]).eq(normalized_window(position)))
            {
                Some(starts) => starts.push(position),
                None => equal_text.push(vec![position]),
            }
        }
        for starts in equal_text.into_iter().filter(|starts| starts.len() >= 2) {
            let class = classes.starts.len();
            classes
                .class_at
                .extend(starts.iter().map(|&position| (position, class)));
            classes.starts.push(starts);
        }
    }
    classes
}

fn rolling_window_hashes(stream: &[Token], window: usize) -> impl Iterator<Item = u64> + '_ {
    const BASE: u64 = 0x100_0000_01b3;
    let leading_weight = (1..window).fold(1u64, |weight, _| weight.wrapping_mul(BASE));
    let mixed_at = |index: usize| mixed(normalized(&stream[index]).hash_input());
    let first = (0..window.min(stream.len())).fold(0u64, |hash, index| {
        hash.wrapping_mul(BASE).wrapping_add(mixed_at(index))
    });
    let count = (stream.len() + 1).saturating_sub(window);
    (0..count).scan(first, move |hash, start| {
        let current = *hash;
        if start + window < stream.len() {
            *hash = hash
                .wrapping_sub(mixed_at(start).wrapping_mul(leading_weight))
                .wrapping_mul(BASE)
                .wrapping_add(mixed_at(start + window));
        }
        Some(current)
    })
}

fn mixed(id: u64) -> u64 {
    let mut value = id.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn maximal_runs(classes: &WindowClasses, window: usize) -> Vec<(usize, Vec<Position>)> {
    let successor: Vec<Option<usize>> = classes
        .starts
        .iter()
        .map(|starts| {
            let next = *classes.class_at.get(&starts[0].next())?;
            let next_starts = &classes.starts[next];
            let shifted_by_one = next_starts.len() == starts.len()
                && starts
                    .iter()
                    .zip(next_starts)
                    .all(|(start, next_start)| start.next() == *next_start);
            shifted_by_one.then_some(next)
        })
        .collect();
    let mut has_predecessor = vec![false; classes.starts.len()];
    for next in successor.iter().flatten() {
        has_predecessor[*next] = true;
    }
    (0..classes.starts.len())
        .filter(|class| !has_predecessor[*class])
        .map(|first| {
            let mut tokens = window;
            let mut class = first;
            while let Some(next) = successor[class] {
                tokens += 1;
                class = next;
            }
            (tokens, classes.starts[first].clone())
        })
        .collect()
}

fn without_overlaps(starts: Vec<Position>, tokens: usize) -> Vec<Position> {
    let mut kept: Vec<Position> = Vec::with_capacity(starts.len());
    for start in starts {
        let overlaps_previous = kept
            .last()
            .is_some_and(|last| last.file == start.file && start.token < last.token + tokens);
        if !overlaps_previous {
            kept.push(start);
        }
    }
    kept
}

fn fragment(path: &RepoPath, covered: &[Token]) -> Fragment {
    Fragment {
        path: path.clone(),
        start_line: covered.first().map_or(0, |token| token.line),
        end_line: covered.last().map_or(0, |token| token.line),
    }
}
