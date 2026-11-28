use std::collections::HashSet;

use codeint_model::{
    FileModel, RepoPath, SourceFile, SourceSet, SourceSetModel, Token, TokenKind, token_text_hash,
};

use super::{CandidateBlock, MINHASH_SIZE, TokenSpan, record_bucket_pairs};

fn synthetic_model(file_count: usize, same_content_prefix: usize) -> SourceSetModel {
    let files = (0..file_count)
        .map(|idx| {
            let group = if idx < same_content_prefix { 0 } else { idx };
            let tokens = vec![
                Token {
                    kind: TokenKind::Other,
                    text_hash: token_text_hash(&format!("kw_{group}")),
                    line: 1,
                },
                Token {
                    kind: TokenKind::Other,
                    text_hash: token_text_hash("+"),
                    line: 1,
                },
            ];
            SourceFile {
                path: RepoPath::new(format!("src/f{idx:04}.ts")).expect("valid path"),
                model: FileModel {
                    source_set: SourceSet::Production,
                    source_lines: 1,
                    functions: Vec::new(),
                    tokens,
                    flagged_lines: Vec::new(),
                    hazard_lines: Vec::new(),
                    runtime_imports: Vec::new(),
                    program_start: false,
                    is_tooling_file: false,
                    diagnostics: Vec::new(),
                    scope_artifact: None,
                },
            }
        })
        .collect();
    SourceSetModel {
        files,
        ..SourceSetModel::default()
    }
}

fn synthetic_candidate(
    file: usize,
    content_hash: u64,
    signature: [u64; MINHASH_SIZE],
) -> CandidateBlock {
    CandidateBlock {
        span: TokenSpan {
            file,
            start_token: 0,
            end_token: 2,
        },
        shingles: Vec::new(),
        signature,
        content_hash,
    }
}

fn collect_bucket_pairs(
    model: &SourceSetModel,
    candidates: &[CandidateBlock],
) -> HashSet<(usize, usize)> {
    let bucket: Vec<(u64, usize)> = (0..candidates.len()).map(|idx| (42u64, idx)).collect();
    let mut seen_pairs = HashSet::new();
    let mut partner_counts = vec![0usize; candidates.len()];
    record_bucket_pairs(
        model,
        candidates,
        &bucket,
        &mut seen_pairs,
        &mut partner_counts,
    );
    seen_pairs
}

#[test]
fn oversized_lsh_bucket_ranks_pairs_by_minhash_similarity_not_alphabetical_order() {
    let model = synthetic_model(40, 0);
    let mut candidates = Vec::new();
    for idx in 0..40usize {
        let mut sig = [0u64; MINHASH_SIZE];
        for (row, slot) in sig.iter_mut().enumerate() {
            *slot = ((idx as u64 + 1) << 16) | row as u64;
        }
        candidates.push(synthetic_candidate(idx, idx as u64 + 1, sig));
    }
    let shared_sig = [777u64; MINHASH_SIZE];
    candidates[15].signature = shared_sig;
    candidates[39].signature = shared_sig;

    let seen_pairs = collect_bucket_pairs(&model, &candidates);

    assert!(
        seen_pairs.contains(&(15, 39)),
        "high-similarity pair (15, 39) must be selected in oversized bucket"
    );
}

#[test]
fn very_large_lsh_bucket_keeps_matching_signatures_adjacent_and_skips_identical_copies() {
    let model = synthetic_model(520, 518);
    let mut candidates = Vec::new();
    let copy_sig = [111u64; MINHASH_SIZE];
    for idx in 0..518usize {
        candidates.push(synthetic_candidate(idx, 1, copy_sig));
    }
    let mut variant_sig = [111u64; MINHASH_SIZE];
    variant_sig[0] = 999;
    variant_sig[1] = 999;
    candidates.push(synthetic_candidate(518, 2, variant_sig));
    candidates.push(synthetic_candidate(519, 3, variant_sig));

    let seen_pairs = collect_bucket_pairs(&model, &candidates);

    assert!(
        seen_pairs.contains(&(518, 519)),
        "near-identical variants in >512 bucket must be paired"
    );
    assert!(
        !seen_pairs.contains(&(0, 1)),
        "identical copies must be skipped before ranking and capping"
    );
}
