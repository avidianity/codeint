use std::collections::{BTreeMap, HashMap, HashSet};

use codeint_model::{CodeModel, StableHasher, Token};

use super::{
    CloneGroup, DisjointSet, Fragment, FragmentIdentifier, GroupClusterInput, SearchedRun,
    TokenSpan, content_digest, distinct_normalized_tokens, fragment, merge_overlapping_spans,
    mixed, normalized,
};

const SHINGLE_LEN: usize = 3;
const CDC_WINDOW: usize = 4;
const MINHASH_SIZE: usize = 64;
const BUCKET_ALL_PAIRS_LIMIT: usize = 32;
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
    let pairs = collect_candidate_pairs(&candidates, config.min_similarity);
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
    let mut spans = Vec::new();
    for run in runs {
        if run.tokens.len() < window {
            continue;
        }
        collect_run_candidate_spans(model, run, window, minimum_distinct, &mut spans);
    }
    spans.sort_unstable();
    spans.dedup();
    let shingle_len = SHINGLE_LEN.min(window).max(1);
    let seeds = minhash_seeds();
    spans
        .into_iter()
        .filter_map(|span| {
            let tokens = &model.files[span.file].model.tokens[span.start_token..span.end_token];
            let shingles = tagged_shingles(tokens, shingle_len);
            if shingles.is_empty() {
                return None;
            }
            let signature = minhash_signature(&shingles, &seeds);
            Some(CandidateBlock {
                span,
                shingles,
                signature,
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

fn collect_candidate_pairs(
    candidates: &[CandidateBlock],
    min_similarity: f64,
) -> Vec<(usize, usize)> {
    let (bands, rows) = lsh_band_shape(min_similarity);
    let mut seen_pairs: HashSet<(usize, usize)> = HashSet::new();
    let mut partner_counts = vec![0usize; candidates.len()];
    let mut bucket_entries: Vec<(u64, usize)> = Vec::with_capacity(candidates.len());
    for band in 0..bands {
        bucket_entries.clear();
        let start = band * rows;
        for (idx, candidate) in candidates.iter().enumerate() {
            let key = candidate.signature[start..start + rows]
                .iter()
                .fold(
                    StableHasher::new().write_u64(band as u64),
                    |hasher, &val| hasher.write_u64(val),
                )
                .finish();
            bucket_entries.push((key, idx));
        }
        bucket_entries.sort_unstable();
        for bucket in bucket_entries.chunk_by(|left, right| left.0 == right.0) {
            if bucket.len() < 2 {
                continue;
            }
            record_bucket_pairs(bucket, &mut seen_pairs, &mut partner_counts);
        }
    }
    let mut pairs: Vec<(usize, usize)> = seen_pairs.into_iter().collect();
    pairs.sort_unstable();
    pairs
}

fn record_bucket_pairs(
    bucket: &[(u64, usize)],
    seen_pairs: &mut HashSet<(usize, usize)>,
    partner_counts: &mut [usize],
) {
    if bucket.len() <= BUCKET_ALL_PAIRS_LIMIT {
        for (pos, &(_, left)) in bucket.iter().enumerate() {
            for &(_, right) in &bucket[pos + 1..] {
                try_insert_pair(left, right, seen_pairs, partner_counts);
            }
        }
        return;
    }
    let anchor = bucket[0].1;
    for pos in 1..bucket.len() {
        let current = bucket[pos].1;
        try_insert_pair(anchor, current, seen_pairs, partner_counts);
        let window_start = pos.saturating_sub(BUCKET_WINDOW_NEIGHBORS);
        for &(_, prev) in &bucket[window_start..pos] {
            try_insert_pair(prev, current, seen_pairs, partner_counts);
        }
    }
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
    let mut matched = Vec::new();
    for &(left, right) in pairs {
        let a = &candidates[left];
        let b = &candidates[right];
        if spans_overlap_same_file(a.span, b.span) {
            continue;
        }
        if !length_ratio_admits(a.shingles.len(), b.shingles.len(), min_similarity) {
            continue;
        }
        let similarity = jaccard_sorted(&a.shingles, &b.shingles);
        if similarity < min_similarity {
            continue;
        }
        if normalized_tokens_equal(model, a.span, b.span)
            || covered_by_same_exact_group(a.span, b.span, exact_by_file)
        {
            continue;
        }
        matched.push(MatchedPair {
            left,
            right,
            similarity,
        });
    }
    matched
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
    let mut dsu = DisjointSet::new(candidates.len());
    for pair in pairs {
        dsu.union(pair.left, pair.right);
    }
    let mut components: BTreeMap<usize, Vec<MatchedPair>> = BTreeMap::new();
    for &pair in pairs {
        components
            .entry(dsu.find(pair.left))
            .or_default()
            .push(pair);
    }
    let mut unidentified: Vec<UnidentifiedNearGroup> = components
        .into_values()
        .filter_map(|component_pairs| unidentified_near_group(model, candidates, &component_pairs))
        .collect();
    unidentified.sort_by(|left, right| left.fragments.cmp(&right.fragments));
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
