use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::num::NonZeroUsize;

use codeint_model::{CodeModel, Metric, Package, RepoPath, StableHasher, Token, TokenKind};

use crate::line_share::{LineShare, line_shares_by_package};

use crate::FindingId;
use crate::finding_id::FindingIds;

mod near_clones;
#[cfg(test)]
mod tests;

pub use near_clones::NearCloneConfig;

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
    pub exact_clone_lines: CloneLines,
    pub source_lines: u64,
    pub clone_groups: Vec<CloneGroup>,
    pub clone_families: Vec<CloneFamily>,
}

impl Duplication {
    pub fn measure(
        model: &CodeModel,
        minimum: MinimumCloneTokens,
        minimum_distinct: MinimumDistinctTokens,
    ) -> Duplication {
        Duplication::measure_with_near_clones(model, minimum, minimum_distinct, None)
    }

    pub fn measure_with_near_clones(
        model: &CodeModel,
        minimum: MinimumCloneTokens,
        minimum_distinct: MinimumDistinctTokens,
        near_clones: Option<NearCloneConfig>,
    ) -> Duplication {
        let window = minimum.0.get();
        let runs = searched_runs(model);
        let classes = equal_window_classes(model, &runs, window);
        let mut exact_lines_by_file: BTreeMap<usize, BTreeSet<u32>> = BTreeMap::new();
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
            let mut spans = Vec::with_capacity(starts.len());
            for Position { file, token } in starts {
                let covered = &model.files[file].model.tokens[token..token + tokens];
                exact_lines_by_file
                    .entry(file)
                    .or_default()
                    .extend(covered.iter().map(|covered| covered.line));
                fragments.push(fragment(&model.files[file].path, covered));
                spans.push(TokenSpan {
                    file,
                    start_token: token,
                    end_token: token + tokens,
                });
            }
            fragments.sort();
            unidentified_groups.push(UnidentifiedGroup {
                tokens,
                fragments,
                spans,
                normalized_content,
            });
        }
        unidentified_groups.sort_by(|left, right| {
            right
                .tokens
                .cmp(&left.tokens)
                .then_with(|| left.fragments.cmp(&right.fragments))
        });
        let mut ids = FindingIds::new(Metric::Duplication);
        let mut clone_groups = Vec::with_capacity(unidentified_groups.len());
        let mut cluster_inputs = Vec::with_capacity(unidentified_groups.len());
        for group in unidentified_groups {
            let digest = content_digest(&group.normalized_content);
            let id = ids.next_in_source_order(
                group.fragments.iter().map(|fragment| &fragment.path),
                group.normalized_content,
            );
            clone_groups.push(CloneGroup {
                id,
                tokens: group.tokens,
                similarity: None,
                fragments: group.fragments,
            });
            cluster_inputs.push(GroupClusterInput {
                id,
                spans: group.spans,
                content_digest: digest,
            });
        }
        let mut total_lines_by_file = exact_lines_by_file.clone();
        if let Some(config) = near_clones {
            let near = near_clones::detect_near_clones(
                model,
                &runs,
                window,
                minimum_distinct.0,
                &cluster_inputs,
                config,
            );
            for span in near.spans {
                let covered =
                    &model.files[span.file].model.tokens[span.start_token..span.end_token];
                total_lines_by_file
                    .entry(span.file)
                    .or_default()
                    .extend(covered.iter().map(|token| token.line));
            }
            clone_groups.extend(near.groups);
            cluster_inputs.extend(near.cluster_inputs);
            clone_groups.sort_by(|left, right| {
                right
                    .tokens
                    .cmp(&left.tokens)
                    .then_with(|| left.fragments.cmp(&right.fragments))
            });
        }
        let clone_families = build_clone_families(model, &cluster_inputs);
        Duplication {
            clone_lines: CloneLines::of_files(model, total_lines_by_file),
            exact_clone_lines: CloneLines::of_files(model, exact_lines_by_file),
            source_lines: model
                .files
                .iter()
                .map(|file| u64::from(file.model.source_lines))
                .sum(),
            clone_groups,
            clone_families,
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

#[derive(Clone, Debug, PartialEq)]
pub struct CloneGroup {
    pub id: FindingId,
    pub tokens: usize,
    pub similarity: Option<f64>,
    pub fragments: Vec<Fragment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloneFamily {
    pub id: FindingId,
    pub duplicated_lines: u64,
    pub clone_group_ids: Vec<FindingId>,
    pub fragments: Vec<Fragment>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fragment {
    pub path: RepoPath,
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct TokenSpan {
    file: usize,
    start_token: usize,
    end_token: usize,
}

struct UnidentifiedGroup {
    tokens: usize,
    fragments: Vec<Fragment>,
    spans: Vec<TokenSpan>,
    normalized_content: Vec<u64>,
}

struct GroupClusterInput {
    id: FindingId,
    spans: Vec<TokenSpan>,
    content_digest: u64,
}

struct UnidentifiedFamily {
    duplicated_lines: u64,
    clone_group_ids: Vec<FindingId>,
    fragments: Vec<Fragment>,
    content_digests: Vec<u64>,
}

struct DisjointSet {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl DisjointSet {
    fn new(len: usize) -> DisjointSet {
        DisjointSet {
            parent: (0..len).collect(),
            rank: vec![0; len],
        }
    }

    fn find(&mut self, mut node: usize) -> usize {
        while self.parent[node] != node {
            let grandparent = self.parent[self.parent[node]];
            self.parent[node] = grandparent;
            node = grandparent;
        }
        node
    }

    fn union(&mut self, left: usize, right: usize) {
        let left_root = self.find(left);
        let right_root = self.find(right);
        if left_root == right_root {
            return;
        }
        match self.rank[left_root].cmp(&self.rank[right_root]) {
            std::cmp::Ordering::Less => self.parent[left_root] = right_root,
            std::cmp::Ordering::Greater => self.parent[right_root] = left_root,
            std::cmp::Ordering::Equal => {
                self.parent[right_root] = left_root;
                self.rank[left_root] += 1;
            }
        }
    }
}

const MINIMUM_CLONE_FAMILY_LOCATIONS: usize = 3;

fn content_digest(normalized_content: &[u64]) -> u64 {
    normalized_content
        .iter()
        .fold(
            StableHasher::new().write_u64(normalized_content.len() as u64),
            |hasher, &token| hasher.write_u64(token),
        )
        .finish()
}

fn build_clone_families(model: &CodeModel, groups: &[GroupClusterInput]) -> Vec<CloneFamily> {
    if groups.is_empty() {
        return Vec::new();
    }
    let components = overlapping_group_components(groups);
    let mut unidentified: Vec<UnidentifiedFamily> = components
        .into_values()
        .filter_map(|group_indices| family_from_component(model, groups, &group_indices))
        .collect();
    unidentified.sort_by(|left, right| left.fragments.cmp(&right.fragments));
    let mut ids = FindingIds::for_key("clone_family");
    let mut families: Vec<CloneFamily> = unidentified
        .into_iter()
        .map(|family| CloneFamily {
            id: ids.next_in_source_order(
                family.fragments.iter().map(|fragment| &fragment.path),
                family.content_digests,
            ),
            duplicated_lines: family.duplicated_lines,
            clone_group_ids: family.clone_group_ids,
            fragments: family.fragments,
        })
        .collect();
    families.sort_by(|left, right| {
        right
            .duplicated_lines
            .cmp(&left.duplicated_lines)
            .then_with(|| right.fragments.len().cmp(&left.fragments.len()))
            .then_with(|| left.fragments.cmp(&right.fragments))
    });
    families
}

fn overlapping_group_components(groups: &[GroupClusterInput]) -> BTreeMap<usize, Vec<usize>> {
    let mut occurrences: Vec<(usize, usize, usize, usize)> = groups
        .iter()
        .enumerate()
        .flat_map(|(group_index, group)| {
            group
                .spans
                .iter()
                .map(move |span| (span.file, span.start_token, span.end_token, group_index))
        })
        .collect();
    occurrences
        .sort_unstable_by_key(|&(file, start, end, group)| (file, start, Reverse(end), group));
    let mut dsu = DisjointSet::new(groups.len());
    let mut active: Option<(usize, usize, usize)> = None;
    for (file, start, end, group) in occurrences {
        match active {
            Some((active_file, active_end, active_group))
                if active_file == file && start < active_end =>
            {
                dsu.union(active_group, group);
                if end > active_end {
                    active = Some((file, end, group));
                }
            }
            _ => active = Some((file, end, group)),
        }
    }
    let mut components: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for group_index in 0..groups.len() {
        components
            .entry(dsu.find(group_index))
            .or_default()
            .push(group_index);
    }
    components
}

fn family_from_component(
    model: &CodeModel,
    groups: &[GroupClusterInput],
    group_indices: &[usize],
) -> Option<UnidentifiedFamily> {
    let raw_spans: Vec<TokenSpan> = group_indices
        .iter()
        .flat_map(|&index| groups[index].spans.iter().copied())
        .collect();
    let merged_spans = merge_overlapping_spans(raw_spans);
    let mut fragments: Vec<Fragment> = merged_spans
        .iter()
        .map(|span| {
            let file = &model.files[span.file];
            fragment(
                &file.path,
                &file.model.tokens[span.start_token..span.end_token],
            )
        })
        .collect();
    fragments.sort();
    if fragments.len() < MINIMUM_CLONE_FAMILY_LOCATIONS {
        return None;
    }
    let duplicated_lines = unique_line_count(model, &merged_spans);
    let clone_group_ids: Vec<FindingId> = group_indices
        .iter()
        .map(|&index| groups[index].id)
        .collect();
    let mut content_digests: Vec<u64> = group_indices
        .iter()
        .map(|&index| groups[index].content_digest)
        .collect();
    content_digests.sort_unstable();
    Some(UnidentifiedFamily {
        duplicated_lines,
        clone_group_ids,
        fragments,
        content_digests,
    })
}

fn merge_overlapping_spans(mut spans: Vec<TokenSpan>) -> Vec<TokenSpan> {
    spans.sort_unstable();
    let mut merged: Vec<TokenSpan> = Vec::with_capacity(spans.len());
    for span in spans {
        match merged.last_mut() {
            Some(last) if last.file == span.file && span.start_token < last.end_token => {
                last.end_token = last.end_token.max(span.end_token);
            }
            _ => merged.push(span),
        }
    }
    merged
}

fn unique_line_count(model: &CodeModel, spans: &[TokenSpan]) -> u64 {
    let mut lines_by_file: BTreeMap<usize, BTreeSet<u32>> = BTreeMap::new();
    for span in spans {
        let covered = &model.files[span.file].model.tokens[span.start_token..span.end_token];
        lines_by_file
            .entry(span.file)
            .or_default()
            .extend(covered.iter().map(|token| token.line));
    }
    lines_by_file.values().map(|lines| lines.len() as u64).sum()
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
