use std::collections::{BTreeMap, HashMap, VecDeque};

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

const MAX_EXACT_MATCH_GROUP: usize = 64;
const MAX_LINE_DISTANCE: u32 = 1_000_000;
const MATCH_WEIGHT: i64 = 1 << 44;
const CONTEXT_SHIFT: u32 = 34;
const LINE_SHIFT: u32 = 7;

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FindingAnchor {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

impl FindingAnchor {
    pub fn id_only(id: impl Into<String>) -> FindingAnchor {
        FindingAnchor {
            id: id.into(),
            content: None,
            before: None,
            after: None,
            line: None,
        }
    }

    pub fn from_hashes(
        id: String,
        content: u64,
        before: Option<u64>,
        after: Option<u64>,
        line: Option<u32>,
    ) -> FindingAnchor {
        FindingAnchor {
            id,
            content: Some(format!("{content:016x}")),
            before: before.map(|value| format!("{value:016x}")),
            after: after.map(|value| format!("{value:016x}")),
            line,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnchorMatch {
    pub matched_pairs: Vec<(usize, usize)>,
}

impl AnchorMatch {
    pub fn matched_references(&self, len: usize) -> Vec<bool> {
        let mut matched = vec![false; len];
        for &(reference_index, _) in &self.matched_pairs {
            matched[reference_index] = true;
        }
        matched
    }

    pub fn matched_candidates(&self, len: usize) -> Vec<bool> {
        let mut matched = vec![false; len];
        for &(_, candidate_index) in &self.matched_pairs {
            matched[candidate_index] = true;
        }
        matched
    }
}

pub fn match_anchors(references: &[FindingAnchor], candidates: &[FindingAnchor]) -> AnchorMatch {
    if references.is_empty() || candidates.is_empty() {
        return AnchorMatch::default();
    }
    let mut matched_pairs = Vec::new();
    let mut used_references = vec![false; references.len()];
    let mut used_candidates = vec![false; candidates.len()];

    match_id_only_references(
        references,
        candidates,
        &mut used_references,
        &mut used_candidates,
        &mut matched_pairs,
    );

    let reference_groups = group_unused_by_content(references, &used_references);
    let candidate_groups = group_unused_by_content(candidates, &used_candidates);

    for (content, ref_indices) in reference_groups {
        let Some(cand_indices) = candidate_groups.get(content) else {
            continue;
        };
        match_content_group(
            references,
            candidates,
            &ref_indices,
            cand_indices,
            &mut matched_pairs,
        );
    }

    matched_pairs.sort_unstable();
    AnchorMatch { matched_pairs }
}

fn group_unused_by_content<'a>(
    anchors: &'a [FindingAnchor],
    used: &[bool],
) -> BTreeMap<&'a str, Vec<usize>> {
    let mut groups: BTreeMap<&'a str, Vec<usize>> = BTreeMap::new();
    for (index, anchor) in anchors.iter().enumerate() {
        if !used[index]
            && let Some(content) = anchor.content.as_deref()
        {
            groups.entry(content).or_default().push(index);
        }
    }
    groups
}

fn match_id_only_references(
    references: &[FindingAnchor],
    candidates: &[FindingAnchor],
    used_references: &mut [bool],
    used_candidates: &mut [bool],
    matched_pairs: &mut Vec<(usize, usize)>,
) {
    let mut candidates_by_id: HashMap<&str, VecDeque<usize>> = HashMap::new();
    for (index, candidate) in candidates.iter().enumerate() {
        candidates_by_id
            .entry(candidate.id.as_str())
            .or_default()
            .push_back(index);
    }
    for (ref_index, reference) in references.iter().enumerate() {
        if reference.content.is_some() {
            continue;
        }
        if let Some(queue) = candidates_by_id.get_mut(reference.id.as_str())
            && let Some(cand_index) = queue.pop_front()
        {
            used_references[ref_index] = true;
            used_candidates[cand_index] = true;
            matched_pairs.push((ref_index, cand_index));
        }
    }
}

fn match_content_group(
    references: &[FindingAnchor],
    candidates: &[FindingAnchor],
    ref_indices: &[usize],
    cand_indices: &[usize],
    matched_pairs: &mut Vec<(usize, usize)>,
) {
    if ref_indices.len() == 1 && cand_indices.len() == 1 {
        matched_pairs.push((ref_indices[0], cand_indices[0]));
        return;
    }
    if ref_indices.len().max(cand_indices.len()) <= MAX_EXACT_MATCH_GROUP {
        match_group_exact(
            references,
            candidates,
            ref_indices,
            cand_indices,
            matched_pairs,
        );
    } else {
        match_group_bucketed(
            references,
            candidates,
            ref_indices,
            cand_indices,
            matched_pairs,
        );
    }
}

fn match_group_exact(
    references: &[FindingAnchor],
    candidates: &[FindingAnchor],
    ref_indices: &[usize],
    cand_indices: &[usize],
    matched_pairs: &mut Vec<(usize, usize)>,
) {
    let size = ref_indices.len().max(cand_indices.len());
    let mut weights = vec![vec![0i64; size]; size];
    for (row, &ref_index) in ref_indices.iter().enumerate() {
        for (col, &cand_index) in cand_indices.iter().enumerate() {
            weights[row][col] = pair_weight(&references[ref_index], &candidates[cand_index]);
        }
    }
    let assignment = hungarian_max_weight(&weights);
    for (row, &col) in assignment.iter().enumerate() {
        if row < ref_indices.len() && col < cand_indices.len() {
            matched_pairs.push((ref_indices[row], cand_indices[col]));
        }
    }
}

fn pair_weight(reference: &FindingAnchor, candidate: &FindingAnchor) -> i64 {
    let context_score = context_match_score(reference, candidate);
    let line_bonus = line_proximity_bonus(reference.line, candidate.line);
    let same_id = i64::from(reference.id == candidate.id);
    MATCH_WEIGHT + (context_score << CONTEXT_SHIFT) + (line_bonus << LINE_SHIFT) + same_id
}

fn context_match_score(reference: &FindingAnchor, candidate: &FindingAnchor) -> i64 {
    let has_line_bounds = reference.line.is_some() && candidate.line.is_some();
    side_match_score(
        reference.before.as_deref(),
        candidate.before.as_deref(),
        has_line_bounds,
    ) + side_match_score(
        reference.after.as_deref(),
        candidate.after.as_deref(),
        has_line_bounds,
    )
}

fn side_match_score(
    reference_side: Option<&str>,
    candidate_side: Option<&str>,
    has_line_bounds: bool,
) -> i64 {
    match (reference_side, candidate_side) {
        (Some(left), Some(right)) if left == right => 3,
        (None, None) if has_line_bounds => 2,
        _ => 0,
    }
}

fn line_proximity_bonus(reference_line: Option<u32>, candidate_line: Option<u32>) -> i64 {
    let distance = match (reference_line, candidate_line) {
        (Some(left), Some(right)) => left.abs_diff(right).min(MAX_LINE_DISTANCE),
        _ => MAX_LINE_DISTANCE,
    };
    i64::from(MAX_LINE_DISTANCE - distance)
}

struct HungarianSolver<'a> {
    weights: &'a [Vec<i64>],
    max_weight: i64,
    u: Vec<i64>,
    v: Vec<i64>,
    p: Vec<usize>,
    way: Vec<usize>,
}

impl<'a> HungarianSolver<'a> {
    fn new(weights: &'a [Vec<i64>]) -> HungarianSolver<'a> {
        let n = weights.len();
        let max_weight = weights
            .iter()
            .flat_map(|row| row.iter().copied())
            .max()
            .unwrap_or(0);
        HungarianSolver {
            weights,
            max_weight,
            u: vec![0; n + 1],
            v: vec![0; n + 1],
            p: vec![0; n + 1],
            way: vec![0; n + 1],
        }
    }

    fn solve(mut self) -> Vec<usize> {
        let n = self.weights.len();
        for row in 1..=n {
            self.augment_row(row);
        }
        let mut assignment = vec![0usize; n];
        for col in 1..=n {
            if self.p[col] > 0 {
                assignment[self.p[col] - 1] = col - 1;
            }
        }
        assignment
    }

    fn augment_row(&mut self, row: usize) {
        let n = self.weights.len();
        self.p[0] = row;
        let mut col0 = 0usize;
        let mut min_v = vec![i64::MAX; n + 1];
        let mut used = vec![false; n + 1];
        loop {
            used[col0] = true;
            let (delta, next_col) = self.relax_columns(col0, &used, &mut min_v);
            self.shift_potentials(&used, &mut min_v, delta);
            col0 = next_col;
            if self.p[col0] == 0 {
                break;
            }
        }
        self.unwind_path(col0);
    }

    fn relax_columns(&mut self, col0: usize, used: &[bool], min_v: &mut [i64]) -> (i64, usize) {
        let row0 = self.p[col0];
        let mut delta = i64::MAX;
        let mut best_col = 0usize;
        for col in 1..=self.weights.len() {
            if used[col] {
                continue;
            }
            let cost = self.max_weight - self.weights[row0 - 1][col - 1];
            let current = cost - self.u[row0] - self.v[col];
            if current < min_v[col] {
                min_v[col] = current;
                self.way[col] = col0;
            }
            if min_v[col] < delta {
                delta = min_v[col];
                best_col = col;
            }
        }
        (delta, best_col)
    }

    fn shift_potentials(&mut self, used: &[bool], min_v: &mut [i64], delta: i64) {
        for col in 0..=self.weights.len() {
            if used[col] {
                self.u[self.p[col]] += delta;
                self.v[col] -= delta;
            } else {
                min_v[col] -= delta;
            }
        }
    }

    fn unwind_path(&mut self, mut col0: usize) {
        while col0 != 0 {
            let col1 = self.way[col0];
            self.p[col0] = self.p[col1];
            col0 = col1;
        }
    }
}

fn hungarian_max_weight(weights: &[Vec<i64>]) -> Vec<usize> {
    HungarianSolver::new(weights).solve()
}

struct BucketedMatcher<'a> {
    references: &'a [FindingAnchor],
    candidates: &'a [FindingAnchor],
    ref_indices: &'a [usize],
    cand_indices: &'a [usize],
    ref_used: Vec<bool>,
    cand_used: Vec<bool>,
}

impl<'a> BucketedMatcher<'a> {
    fn pair_by_key<K: Eq + std::hash::Hash>(
        &mut self,
        matched_pairs: &mut Vec<(usize, usize)>,
        key_of: impl Fn(&'a FindingAnchor) -> Option<K>,
    ) {
        let mut buckets: HashMap<K, VecDeque<usize>> = HashMap::new();
        for (cand_pos, &cand_index) in self.cand_indices.iter().enumerate() {
            if !self.cand_used[cand_pos]
                && let Some(key) = key_of(&self.candidates[cand_index])
            {
                buckets.entry(key).or_default().push_back(cand_pos);
            }
        }
        for (ref_pos, &ref_index) in self.ref_indices.iter().enumerate() {
            if self.ref_used[ref_pos] {
                continue;
            }
            if let Some(key) = key_of(&self.references[ref_index])
                && let Some(queue) = buckets.get_mut(&key)
                && let Some(cand_pos) = queue.pop_front()
            {
                self.ref_used[ref_pos] = true;
                self.cand_used[cand_pos] = true;
                matched_pairs.push((ref_index, self.cand_indices[cand_pos]));
            }
        }
    }
}

fn match_group_bucketed(
    references: &[FindingAnchor],
    candidates: &[FindingAnchor],
    ref_indices: &[usize],
    cand_indices: &[usize],
    matched_pairs: &mut Vec<(usize, usize)>,
) {
    let mut matcher = BucketedMatcher {
        references,
        candidates,
        ref_indices,
        cand_indices,
        ref_used: vec![false; ref_indices.len()],
        cand_used: vec![false; cand_indices.len()],
    };
    matcher.pair_by_key(matched_pairs, |anchor| {
        Some((anchor.before.as_deref(), anchor.after.as_deref()))
    });
    matcher.pair_by_key(matched_pairs, |anchor| anchor.before.as_deref());
    matcher.pair_by_key(matched_pairs, |anchor| anchor.after.as_deref());
    let remaining_refs = ref_indices
        .iter()
        .enumerate()
        .filter(|&(pos, _)| !matcher.ref_used[pos])
        .map(|(_, &index)| index);
    let remaining_cands = cand_indices
        .iter()
        .enumerate()
        .filter(|&(pos, _)| !matcher.cand_used[pos])
        .map(|(_, &index)| index);
    matched_pairs.extend(remaining_refs.zip(remaining_cands));
}
