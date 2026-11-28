use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

use codeint_model::{CodeModel, StableHasher, Token};
use rayon::prelude::*;

use super::{
    CloneGroup, Fragment, FragmentIdentifier, GroupClusterInput, SearchedRun, TokenSpan,
    content_digest, distinct_normalized_tokens, fragment, merge_overlapping_spans, mixed,
    normalized,
};

#[cfg(test)]
mod tests;

const SHINGLE_LEN: usize = 3;
const CDC_WINDOW: usize = 4;
const MINHASH_SIZE: usize = 64;
const BUCKET_ALL_PAIRS_LIMIT: usize = 32;
const BUCKET_RANKED_ALL_PAIRS_LIMIT: usize = 512;
const BUCKET_WINDOW_NEIGHBORS: usize = 8;
const MAX_CANDIDATES_PER_BLOCK: usize = 64;
const MAX_SINGLE_RUN_TOKENS: usize = 256;
const MIN_CHUNK_TOKENS: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NearCloneConfig {
    min_similarity: f64,
}

impl NearCloneConfig {
    pub fn new(min_similarity: f64) -> Option<NearCloneConfig> {
        (min_similarity.is_finite() && min_similarity > 0.0 && min_similarity <= 1.0)
            .then_some(NearCloneConfig { min_similarity })
    }

    pub fn min_similarity(self) -> f64 {
        self.min_similarity
    }
}

pub(super) struct NearCloneOutput {
    pub(super) groups: Vec<CloneGroup>,
    pub(super) cluster_inputs: Vec<GroupClusterInput>,
    pub(super) spans: Vec<TokenSpan>,
}

struct CandidateBlock {
    span: TokenSpan,
    shingles: Vec<u64>,
    signature: [u64; MINHASH_SIZE],
    content_hash: u64,
}

#[derive(Clone, Copy)]
struct MatchedPair {
    left: usize,
    right: usize,
    similarity: f64,
}

struct UnidentifiedNearGroup {
    tokens: usize,
    similarity: f64,
    fragments: Vec<Fragment>,
    spans: Vec<TokenSpan>,
    content_digests: Vec<u64>,
}

pub(super) fn detect_near_clones(
    model: &CodeModel,
    runs: &[SearchedRun<'_>],
    window: usize,
    minimum_distinct: usize,
    exact_groups: &[GroupClusterInput],
    config: NearCloneConfig,
) -> NearCloneOutput {
    let candidates = extract_candidates(model, runs, window, minimum_distinct);
    if candidates.len() < 2 {
        return NearCloneOutput {
            groups: Vec::new(),
            cluster_inputs: Vec::new(),
            spans: Vec::new(),
        };
    }
    let pairs = collect_candidate_pairs(model, &candidates, config.min_similarity);
    let exact_by_file = index_exact_spans(exact_groups);
    let matched = verify_pairs(
        model,
        &candidates,
        &pairs,
        &exact_by_file,
        config.min_similarity,
    );
    let kept = suppress_subsumed_pairs(&candidates, matched);
    build_near_clone_groups(model, &candidates, &kept)
}

fn extract_candidates(
    model: &CodeModel,
    runs: &[SearchedRun<'_>],
    window: usize,
    minimum_distinct: usize,
) -> Vec<CandidateBlock> {
    let mut spans: Vec<TokenSpan> = runs
        .par_iter()
        .filter(|run| run.tokens.len() >= window)
        .flat_map_iter(|run| {
            let mut run_spans = Vec::new();
            collect_run_candidate_spans(model, run, window, minimum_distinct, &mut run_spans);
            run_spans
        })
        .collect();
    spans.par_sort_unstable();
    spans.dedup();
    let shingle_len = SHINGLE_LEN.min(window).max(1);
    let seeds = minhash_seeds();
    spans
        .into_par_iter()
        .filter_map(|span| {
            let tokens = &model.files[span.file].model.tokens[span.start_token..span.end_token];
            let shingles = tagged_shingles(tokens, shingle_len);
            if shingles.is_empty() {
                return None;
            }
            let signature = minhash_signature(&shingles, &seeds);
            let content_hash = normalized_content_hash(tokens);
            Some(CandidateBlock {
                span,
                shingles,
                signature,
                content_hash,
            })
        })
        .collect()
}

fn collect_run_candidate_spans(
    model: &CodeModel,
    run: &SearchedRun<'_>,
    window: usize,
    minimum_distinct: usize,
    spans: &mut Vec<TokenSpan>,
) {
    let mut function_ranges = Vec::new();
    for function in &model.files[run.file].model.functions {
        let lo = run
            .tokens
            .partition_point(|token| token.line < function.line);
        let hi = run
            .tokens
            .partition_point(|token| token.line <= function.end_line);
        if hi > lo {
            function_ranges.push((lo, hi));
            push_if_qualifying(run, lo, hi, window, minimum_distinct, spans);
        }
    }
    for (gap_lo, gap_hi) in uncovered_gaps(run.tokens.len(), function_ranges) {
        collect_gap_spans(run, gap_lo, gap_hi, window, minimum_distinct, spans);
    }
}

fn uncovered_gaps(len: usize, mut ranges: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    if ranges.is_empty() {
        return vec![(0, len)];
    }
    ranges.sort_unstable();
    let mut gaps = Vec::new();
    let mut cursor = 0;
    for (lo, hi) in ranges {
        if lo > cursor {
            gaps.push((cursor, lo));
        }
        cursor = cursor.max(hi);
    }
    if cursor < len {
        gaps.push((cursor, len));
    }
    gaps
}

fn collect_gap_spans(
    run: &SearchedRun<'_>,
    gap_lo: usize,
    gap_hi: usize,
    window: usize,
    minimum_distinct: usize,
    spans: &mut Vec<TokenSpan>,
) {
    let gap_len = gap_hi - gap_lo;
    if gap_len < window {
        return;
    }
    let single_limit = MAX_SINGLE_RUN_TOKENS.max(window.saturating_mul(4));
    if gap_len <= single_limit {
        push_if_qualifying(run, gap_lo, gap_hi, window, minimum_distinct, spans);
        return;
    }
    let chunk = MIN_CHUNK_TOKENS.max(window.saturating_mul(2));
    let stride = (chunk / 2).max(1);
    let min_step = (stride / 2).max(CDC_WINDOW).max(16);
    let max_step = chunk;
    let mask = ((stride / 3).max(16).next_power_of_two() - 1) as u64;
    let boundaries =
        content_defined_boundaries(run.tokens, gap_lo, gap_hi, min_step, max_step, mask);
    for idx in 0..boundaries.len().saturating_sub(1) {
        let lo = boundaries[idx];
        let hi = boundaries[idx + 1];
        push_if_qualifying(run, lo, hi, window, minimum_distinct, spans);
        if let Some(&next_hi) = boundaries.get(idx + 2) {
            push_if_qualifying(run, lo, next_hi, window, minimum_distinct, spans);
        }
    }
}

fn content_defined_boundaries(
    tokens: &[Token],
    gap_lo: usize,
    gap_hi: usize,
    min_step: usize,
    max_step: usize,
    mask: u64,
) -> Vec<usize> {
    let mut boundaries = vec![gap_lo];
    let mut start = gap_lo;
    while start + max_step < gap_hi {
        let next = next_cdc_boundary(tokens, start, gap_hi, min_step, max_step, mask);
        boundaries.push(next);
        start = next;
    }
    if *boundaries.last().unwrap_or(&gap_lo) < gap_hi {
        boundaries.push(gap_hi);
    }
    boundaries
}

fn next_cdc_boundary(
    tokens: &[Token],
    start: usize,
    gap_hi: usize,
    min_step: usize,
    max_step: usize,
    mask: u64,
) -> usize {
    let limit = (start + max_step).min(gap_hi);
    let mut best_pos = limit;
    let mut best_hash = u64::MAX;
    for pos in (start + min_step)..=limit {
        let hash = cdc_boundary_hash(tokens, pos);
        if (hash & mask) == 0 {
            return pos;
        }
        if hash < best_hash {
            best_hash = hash;
            best_pos = pos;
        }
    }
    best_pos
}

fn cdc_boundary_hash(tokens: &[Token], pos: usize) -> u64 {
    tokens[pos - CDC_WINDOW..pos]
        .iter()
        .fold(StableHasher::new(), |hasher, token| {
            hasher.write_u64(normalized(token).hash_input())
        })
        .finish()
}

fn push_if_qualifying(
    run: &SearchedRun<'_>,
    lo: usize,
    hi: usize,
    window: usize,
    minimum_distinct: usize,
    spans: &mut Vec<TokenSpan>,
) {
    if hi - lo < window {
        return;
    }
    if distinct_normalized_tokens(&run.tokens[lo..hi]) < minimum_distinct {
        return;
    }
    spans.push(TokenSpan {
        file: run.file,
        start_token: run.first_token + lo,
        end_token: run.first_token + hi,
    });
}

fn tagged_shingles(tokens: &[Token], shingle_len: usize) -> Vec<u64> {
    if tokens.len() < shingle_len {
        return Vec::new();
    }
    let normalized_ids: Vec<u64> = tokens
        .iter()
        .map(|token| normalized(token).hash_input())
        .collect();
    let count = normalized_ids.len() - shingle_len + 1;
    let mut occurrences: HashMap<u64, u64> = HashMap::with_capacity(count);
    let mut shingles = Vec::with_capacity(count);
    for slice in normalized_ids.windows(shingle_len) {
        let base = slice
            .iter()
            .fold(StableHasher::new(), |hasher, &id| hasher.write_u64(id))
            .finish();
        let occ = occurrences.entry(base).or_insert(0);
        let tagged = StableHasher::new().write_u64(base).write_u64(*occ).finish();
        *occ += 1;
        shingles.push(tagged);
    }
    shingles.sort_unstable();
    shingles.dedup();
    shingles
}

fn minhash_seeds() -> [(u64, u64); MINHASH_SIZE] {
    std::array::from_fn(|index| {
        let idx = index as u64;
        let mul = mixed(idx ^ 0x517c_c1b7_2722_0a95) | 1;
        let add = mixed(idx.wrapping_add(MINHASH_SIZE as u64) ^ 0x9e37_79b9_7f4a_7c15);
        (mul, add)
    })
}

fn minhash_signature(shingles: &[u64], seeds: &[(u64, u64); MINHASH_SIZE]) -> [u64; MINHASH_SIZE] {
    let mut signature = [u64::MAX; MINHASH_SIZE];
    for &shingle in shingles {
        for (slot, &(mul, add)) in signature.iter_mut().zip(seeds) {
            let hashed = mixed(shingle.wrapping_mul(mul).wrapping_add(add));
            if hashed < *slot {
                *slot = hashed;
            }
        }
    }
    signature
}

fn lsh_band_shape(min_similarity: f64) -> (usize, usize) {
    if min_similarity >= 0.75 {
        (16, 4)
    } else if min_similarity >= 0.50 {
        (32, 2)
    } else {
        (64, 1)
    }
}

fn normalized_content_hash(tokens: &[Token]) -> u64 {
    tokens
        .iter()
        .fold(StableHasher::new(), |hasher, token| {
            hasher.write_u64(normalized(token).hash_input())
        })
        .finish()
}

fn candidates_normalized_equal(
    model: &CodeModel,
    left: &CandidateBlock,
    right: &CandidateBlock,
) -> bool {
    left.content_hash == right.content_hash && normalized_tokens_equal(model, left.span, right.span)
}

fn collect_candidate_pairs(
    model: &CodeModel,
    candidates: &[CandidateBlock],
    min_similarity: f64,
) -> Vec<(usize, usize)> {
    let (bands, rows) = lsh_band_shape(min_similarity);
    let band_entries: Vec<Vec<(u64, usize)>> = (0..bands)
        .into_par_iter()
        .map(|band| {
            let start = band * rows;
            let mut entries: Vec<(u64, usize)> = candidates
                .iter()
                .enumerate()
                .map(|(idx, candidate)| {
                    let key = candidate.signature[start..start + rows]
                        .iter()
                        .fold(
                            StableHasher::new().write_u64(band as u64),
                            |hasher, &val| hasher.write_u64(val),
                        )
                        .finish();
                    (key, idx)
                })
                .collect();
            entries.sort_unstable();
            entries
        })
        .collect();
    let mut seen_pairs: HashSet<(usize, usize)> = HashSet::new();
    let mut partner_counts = vec![0usize; candidates.len()];
    for entries in &band_entries {
        for bucket in entries.chunk_by(|left, right| left.0 == right.0) {
            if bucket.len() < 2 {
                continue;
            }
            record_bucket_pairs(
                model,
                candidates,
                bucket,
                &mut seen_pairs,
                &mut partner_counts,
            );
        }
    }
    let mut pairs: Vec<(usize, usize)> = seen_pairs.into_iter().collect();
    pairs.par_sort_unstable();
    pairs
}

fn record_bucket_pairs(
    model: &CodeModel,
    candidates: &[CandidateBlock],
    bucket: &[(u64, usize)],
    seen_pairs: &mut HashSet<(usize, usize)>,
    partner_counts: &mut [usize],
) {
    if bucket.len() <= BUCKET_ALL_PAIRS_LIMIT {
        for (pos, &(_, left)) in bucket.iter().enumerate() {
            for &(_, right) in &bucket[pos + 1..] {
                if candidates_normalized_equal(model, &candidates[left], &candidates[right]) {
                    continue;
                }
                try_insert_pair(left, right, seen_pairs, partner_counts);
            }
        }
        return;
    }
    let max_pairs = bucket.len().saturating_mul(BUCKET_WINDOW_NEIGHBORS);
    let mut scored = collect_bucket_pair_scores(model, candidates, bucket);
    scored.sort_unstable_by_key(|&(matches, left, right)| (Reverse(matches), left, right));
    for (_, left, right) in scored.into_iter().take(max_pairs) {
        try_insert_pair(left, right, seen_pairs, partner_counts);
    }
}

fn collect_bucket_pair_scores(
    model: &CodeModel,
    candidates: &[CandidateBlock],
    bucket: &[(u64, usize)],
) -> Vec<(usize, usize, usize)> {
    if bucket.len() <= BUCKET_RANKED_ALL_PAIRS_LIMIT {
        return score_all_bucket_pairs(model, candidates, bucket);
    }
    score_similarity_ordered_bucket_window(model, candidates, bucket)
}

fn push_scored_bucket_pair(
    model: &CodeModel,
    candidates: &[CandidateBlock],
    left: usize,
    right: usize,
    scored: &mut Vec<(usize, usize, usize)>,
) {
    if candidates_normalized_equal(model, &candidates[left], &candidates[right]) {
        return;
    }
    let matches = minhash_matches(&candidates[left].signature, &candidates[right].signature);
    scored.push((matches, left.min(right), left.max(right)));
}

fn score_all_bucket_pairs(
    model: &CodeModel,
    candidates: &[CandidateBlock],
    bucket: &[(u64, usize)],
) -> Vec<(usize, usize, usize)> {
    let mut scored = Vec::with_capacity(bucket.len() * (bucket.len() - 1) / 2);
    for (pos, &(_, left)) in bucket.iter().enumerate() {
        for &(_, right) in &bucket[pos + 1..] {
            push_scored_bucket_pair(model, candidates, left, right, &mut scored);
        }
    }
    scored
}

fn score_similarity_ordered_bucket_window(
    model: &CodeModel,
    candidates: &[CandidateBlock],
    bucket: &[(u64, usize)],
) -> Vec<(usize, usize, usize)> {
    let anchor = bucket[0].1;
    let mut ordered: Vec<(usize, usize)> = bucket
        .iter()
        .map(|&(_, idx)| {
            let matches =
                minhash_matches(&candidates[anchor].signature, &candidates[idx].signature);
            (matches, idx)
        })
        .collect();
    ordered.sort_unstable_by(|&(left_matches, left_idx), &(right_matches, right_idx)| {
        right_matches
            .cmp(&left_matches)
            .then_with(|| {
                candidates[left_idx]
                    .signature
                    .cmp(&candidates[right_idx].signature)
            })
            .then_with(|| left_idx.cmp(&right_idx))
    });
    let mut scored = Vec::with_capacity(ordered.len().saturating_mul(BUCKET_WINDOW_NEIGHBORS + 1));
    for (pos, &(_, left)) in ordered.iter().enumerate() {
        let end = (pos + 1 + BUCKET_WINDOW_NEIGHBORS).min(ordered.len());
        for &(_, right) in &ordered[pos + 1..end] {
            push_scored_bucket_pair(model, candidates, left, right, &mut scored);
        }
    }
    scored
}

fn minhash_matches(left: &[u64; MINHASH_SIZE], right: &[u64; MINHASH_SIZE]) -> usize {
    left.iter()
        .zip(right.iter())
        .filter(|(a, b)| a == b)
        .count()
}

fn try_insert_pair(
    left: usize,
    right: usize,
    seen_pairs: &mut HashSet<(usize, usize)>,
    partner_counts: &mut [usize],
) {
    if left == right {
        return;
    }
    let pair = if left < right {
        (left, right)
    } else {
        (right, left)
    };
    if partner_counts[pair.0] >= MAX_CANDIDATES_PER_BLOCK
        && partner_counts[pair.1] >= MAX_CANDIDATES_PER_BLOCK
    {
        return;
    }
    if seen_pairs.insert(pair) {
        partner_counts[pair.0] += 1;
        partner_counts[pair.1] += 1;
    }
}

fn index_exact_spans(
    exact_groups: &[GroupClusterInput],
) -> HashMap<usize, Vec<(usize, TokenSpan)>> {
    let mut by_file: HashMap<usize, Vec<(usize, TokenSpan)>> = HashMap::new();
    for (group_index, group) in exact_groups.iter().enumerate() {
        for &span in &group.spans {
            by_file
                .entry(span.file)
                .or_default()
                .push((group_index, span));
        }
    }
    by_file
}

fn verify_pairs(
    model: &CodeModel,
    candidates: &[CandidateBlock],
    pairs: &[(usize, usize)],
    exact_by_file: &HashMap<usize, Vec<(usize, TokenSpan)>>,
    min_similarity: f64,
) -> Vec<MatchedPair> {
    pairs
        .par_iter()
        .filter_map(|&(left, right)| {
            let a = &candidates[left];
            let b = &candidates[right];
            if spans_overlap_same_file(a.span, b.span) {
                return None;
            }
            if !length_ratio_admits(a.shingles.len(), b.shingles.len(), min_similarity) {
                return None;
            }
            let similarity = jaccard_sorted(&a.shingles, &b.shingles);
            if similarity < min_similarity {
                return None;
            }
            if normalized_tokens_equal(model, a.span, b.span)
                || covered_by_same_exact_group(a.span, b.span, exact_by_file)
            {
                return None;
            }
            Some(MatchedPair {
                left,
                right,
                similarity,
            })
        })
        .collect()
}

fn spans_overlap_same_file(left: TokenSpan, right: TokenSpan) -> bool {
    left.file == right.file
        && left.start_token < right.end_token
        && right.start_token < left.end_token
}

fn span_contains(outer: TokenSpan, inner: TokenSpan) -> bool {
    outer.file == inner.file
        && outer.start_token <= inner.start_token
        && inner.end_token <= outer.end_token
}

fn length_ratio_admits(left_len: usize, right_len: usize, min_similarity: f64) -> bool {
    let shorter = left_len.min(right_len) as f64;
    let longer = left_len.max(right_len) as f64;
    longer > 0.0 && shorter >= longer * min_similarity
}

fn jaccard_sorted(left: &[u64], right: &[u64]) -> f64 {
    let mut i = 0;
    let mut j = 0;
    let mut shared = 0usize;
    while i < left.len() && j < right.len() {
        match left[i].cmp(&right[j]) {
            std::cmp::Ordering::Equal => {
                shared += 1;
                i += 1;
                j += 1;
            }
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
        }
    }
    let union = left.len() + right.len() - shared;
    if union == 0 {
        0.0
    } else {
        shared as f64 / union as f64
    }
}

fn normalized_tokens_equal(model: &CodeModel, left: TokenSpan, right: TokenSpan) -> bool {
    if left.end_token - left.start_token != right.end_token - right.start_token {
        return false;
    }
    let left_tokens = &model.files[left.file].model.tokens[left.start_token..left.end_token];
    let right_tokens = &model.files[right.file].model.tokens[right.start_token..right.end_token];
    left_tokens
        .iter()
        .map(normalized)
        .eq(right_tokens.iter().map(normalized))
}

fn covered_by_same_exact_group(
    left: TokenSpan,
    right: TokenSpan,
    exact_by_file: &HashMap<usize, Vec<(usize, TokenSpan)>>,
) -> bool {
    let (Some(left_groups), Some(right_groups)) = (
        exact_by_file.get(&left.file),
        exact_by_file.get(&right.file),
    ) else {
        return false;
    };
    let covering_left: HashSet<usize> = left_groups
        .iter()
        .filter(|&&(_, span)| span_contains(span, left))
        .map(|&(group_index, _)| group_index)
        .collect();
    if covering_left.is_empty() {
        return false;
    }
    right_groups.iter().any(|&(group_index, span)| {
        covering_left.contains(&group_index) && span_contains(span, right)
    })
}

fn suppress_subsumed_pairs(
    candidates: &[CandidateBlock],
    mut matched: Vec<MatchedPair>,
) -> Vec<MatchedPair> {
    matched.sort_by(|left, right| {
        let left_size =
            span_len(candidates[left.left].span) + span_len(candidates[left.right].span);
        let right_size =
            span_len(candidates[right.left].span) + span_len(candidates[right.right].span);
        right_size
            .cmp(&left_size)
            .then_with(|| left.left.cmp(&right.left))
            .then_with(|| left.right.cmp(&right.right))
    });
    let mut kept: Vec<MatchedPair> = Vec::with_capacity(matched.len());
    let mut kept_by_file_pair: HashMap<(usize, usize), Vec<MatchedPair>> = HashMap::new();
    for pair in matched {
        let left_span = candidates[pair.left].span;
        let right_span = candidates[pair.right].span;
        let file_pair = (
            left_span.file.min(right_span.file),
            left_span.file.max(right_span.file),
        );
        let subsumed = kept_by_file_pair
            .get(&file_pair)
            .is_some_and(|existing_pairs| {
                existing_pairs.iter().any(|existing| {
                    let ex_left = candidates[existing.left].span;
                    let ex_right = candidates[existing.right].span;
                    (span_contains(ex_left, left_span) && span_contains(ex_right, right_span))
                        || (span_contains(ex_left, right_span)
                            && span_contains(ex_right, left_span))
                })
            });
        if !subsumed {
            kept_by_file_pair.entry(file_pair).or_default().push(pair);
            kept.push(pair);
        }
    }
    kept
}

fn span_len(span: TokenSpan) -> usize {
    span.end_token - span.start_token
}

fn build_near_clone_groups(
    model: &CodeModel,
    candidates: &[CandidateBlock],
    pairs: &[MatchedPair],
) -> NearCloneOutput {
    if pairs.is_empty() {
        return NearCloneOutput {
            groups: Vec::new(),
            cluster_inputs: Vec::new(),
            spans: Vec::new(),
        };
    }
    let clusters = cluster_pairs_by_anchor(model, candidates, pairs);
    let unidentified = collect_unidentified_near_groups(model, candidates, clusters);
    let mut identifier = FragmentIdentifier::new("near_clone", model);
    let mut groups = Vec::with_capacity(unidentified.len());
    let mut cluster_inputs = Vec::with_capacity(unidentified.len());
    let mut all_spans = Vec::new();
    for item in unidentified {
        let group_digest = content_digest(&item.content_digests);
        let (id, context) = identifier.identify(&item.fragments, item.content_digests);
        all_spans.extend(item.spans.iter().copied());
        groups.push(CloneGroup {
            id,
            context,
            tokens: item.tokens,
            similarity: Some(item.similarity),
            fragments: item.fragments,
        });
        cluster_inputs.push(GroupClusterInput {
            id,
            spans: item.spans,
            content_digest: group_digest,
        });
    }
    NearCloneOutput {
        groups,
        cluster_inputs,
        spans: all_spans,
    }
}

fn candidate_order_key<'a>(
    model: &'a CodeModel,
    candidates: &[CandidateBlock],
    idx: usize,
) -> (&'a codeint_model::RepoPath, usize, usize) {
    let span = candidates[idx].span;
    (
        &model.files[span.file].path,
        span.start_token,
        span.end_token,
    )
}

fn normalized_edge(left: usize, right: usize) -> (usize, usize) {
    (left.min(right), left.max(right))
}

#[derive(Clone, Copy)]
struct NodeRank {
    degree: usize,
    total_similarity: f64,
    content_hash: u64,
}

fn compare_node_ranks(
    model: &CodeModel,
    candidates: &[CandidateBlock],
    ranks: &HashMap<usize, NodeRank>,
    left: usize,
    right: usize,
) -> std::cmp::Ordering {
    let left_rank = ranks[&left];
    let right_rank = ranks[&right];
    right_rank
        .degree
        .cmp(&left_rank.degree)
        .then_with(|| {
            right_rank
                .total_similarity
                .total_cmp(&left_rank.total_similarity)
        })
        .then_with(|| left_rank.content_hash.cmp(&right_rank.content_hash))
        .then_with(|| {
            candidate_order_key(model, candidates, left)
                .cmp(&candidate_order_key(model, candidates, right))
        })
}

fn compute_node_ranks(
    candidates: &[CandidateBlock],
    adjacency: &HashMap<usize, Vec<(usize, f64)>>,
) -> HashMap<usize, NodeRank> {
    adjacency
        .iter()
        .map(|(&node, neighbors)| {
            let mut sims: Vec<f64> = neighbors.iter().map(|&(_, sim)| sim).collect();
            sims.sort_by(|a, b| b.total_cmp(a));
            let total_similarity = sims.into_iter().sum();
            (
                node,
                NodeRank {
                    degree: neighbors.len(),
                    total_similarity,
                    content_hash: candidates[node].content_hash,
                },
            )
        })
        .collect()
}

fn pair_similarity_for_cluster(
    model: &CodeModel,
    candidates: &[CandidateBlock],
    edge_similarity: &HashMap<(usize, usize), f64>,
    left: usize,
    right: usize,
) -> Option<f64> {
    if let Some(&similarity) = edge_similarity.get(&normalized_edge(left, right)) {
        return Some(similarity);
    }
    if candidates_normalized_equal(model, &candidates[left], &candidates[right]) {
        return Some(1.0);
    }
    None
}

fn grow_anchor_clique(
    model: &CodeModel,
    candidates: &[CandidateBlock],
    edge_similarity: &HashMap<(usize, usize), f64>,
    anchor: usize,
    uncovered: &[(usize, f64)],
) -> (Vec<usize>, Vec<MatchedPair>) {
    let mut members = vec![anchor];
    let mut cluster_pairs = Vec::new();
    for &(member, anchor_similarity) in uncovered {
        let mut min_similarity = anchor_similarity;
        let mut compatible = true;
        for &existing in &members[1..] {
            match pair_similarity_for_cluster(model, candidates, edge_similarity, member, existing)
            {
                Some(similarity) => min_similarity = min_similarity.min(similarity),
                None => {
                    compatible = false;
                    break;
                }
            }
        }
        if compatible {
            members.push(member);
            cluster_pairs.push(MatchedPair {
                left: anchor,
                right: member,
                similarity: min_similarity,
            });
        }
    }
    (members, cluster_pairs)
}

fn cluster_pairs_by_anchor(
    model: &CodeModel,
    candidates: &[CandidateBlock],
    pairs: &[MatchedPair],
) -> Vec<Vec<MatchedPair>> {
    let mut adjacency: HashMap<usize, Vec<(usize, f64)>> = HashMap::new();
    let mut edge_similarity: HashMap<(usize, usize), f64> = HashMap::with_capacity(pairs.len());
    for &pair in pairs {
        edge_similarity.insert(normalized_edge(pair.left, pair.right), pair.similarity);
        adjacency
            .entry(pair.left)
            .or_default()
            .push((pair.right, pair.similarity));
        adjacency
            .entry(pair.right)
            .or_default()
            .push((pair.left, pair.similarity));
    }
    let ranks = compute_node_ranks(candidates, &adjacency);
    let mut ordered_nodes: Vec<usize> = adjacency.keys().copied().collect();
    ordered_nodes
        .sort_by(|&left, &right| compare_node_ranks(model, candidates, &ranks, left, right));
    for neighbors in adjacency.values_mut() {
        neighbors.sort_by(|&(left, left_sim), &(right, right_sim)| {
            right_sim
                .total_cmp(&left_sim)
                .then_with(|| compare_node_ranks(model, candidates, &ranks, left, right))
        });
    }
    let mut covered_edges: HashSet<(usize, usize)> = HashSet::with_capacity(pairs.len());
    let mut clusters = Vec::new();
    for anchor in ordered_nodes {
        let Some(neighbors) = adjacency.get(&anchor) else {
            continue;
        };
        loop {
            let uncovered: Vec<(usize, f64)> = neighbors
                .iter()
                .copied()
                .filter(|&(member, _)| !covered_edges.contains(&normalized_edge(anchor, member)))
                .collect();
            if uncovered.is_empty() {
                break;
            }
            let (members, cluster_pairs) =
                grow_anchor_clique(model, candidates, &edge_similarity, anchor, &uncovered);
            mark_cluster_edges_covered(&members, &mut covered_edges);
            clusters.push(cluster_pairs);
        }
    }
    clusters
}

fn mark_cluster_edges_covered(members: &[usize], covered_edges: &mut HashSet<(usize, usize)>) {
    for (pos, &member) in members.iter().enumerate() {
        for &other in &members[pos + 1..] {
            covered_edges.insert(normalized_edge(member, other));
        }
    }
}

fn collect_unidentified_near_groups(
    model: &CodeModel,
    candidates: &[CandidateBlock],
    clusters: Vec<Vec<MatchedPair>>,
) -> Vec<UnidentifiedNearGroup> {
    let mut unidentified: Vec<UnidentifiedNearGroup> = clusters
        .into_par_iter()
        .filter_map(|component_pairs| unidentified_near_group(model, candidates, &component_pairs))
        .collect();
    unidentified.sort_by(|left, right| left.fragments.cmp(&right.fragments));
    unidentified
}

fn unidentified_near_group(
    model: &CodeModel,
    candidates: &[CandidateBlock],
    component_pairs: &[MatchedPair],
) -> Option<UnidentifiedNearGroup> {
    let mut raw_spans = Vec::with_capacity(component_pairs.len() * 2);
    let mut min_similarity = 1.0_f64;
    for pair in component_pairs {
        raw_spans.push(candidates[pair.left].span);
        raw_spans.push(candidates[pair.right].span);
        min_similarity = min_similarity.min(pair.similarity);
    }
    let spans = merge_overlapping_spans(raw_spans);
    if spans.len() < 2 {
        return None;
    }
    let tokens = spans.iter().map(|&span| span_len(span)).min().unwrap_or(0);
    let mut fragments = Vec::with_capacity(spans.len());
    let mut content_digests = Vec::with_capacity(spans.len());
    for &span in &spans {
        let file = &model.files[span.file];
        let covered = &file.model.tokens[span.start_token..span.end_token];
        fragments.push(fragment(&file.path, covered));
        let normalized_content: Vec<u64> = covered
            .iter()
            .map(|token| normalized(token).hash_input())
            .collect();
        content_digests.push(content_digest(&normalized_content));
    }
    fragments.sort();
    content_digests.sort_unstable();
    Some(UnidentifiedNearGroup {
        tokens,
        similarity: min_similarity,
        fragments,
        spans,
        content_digests,
    })
}
