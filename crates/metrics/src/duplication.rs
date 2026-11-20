use std::num::NonZeroUsize;

use codeint_model::{CodeModel, RepoPath};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MinimumCloneTokens(NonZeroUsize);

impl MinimumCloneTokens {
    pub const PROVISIONAL_DEFAULT: MinimumCloneTokens =
        MinimumCloneTokens(NonZeroUsize::new(50).expect("50 is not zero"));

    pub fn new(tokens: NonZeroUsize) -> MinimumCloneTokens {
        MinimumCloneTokens(tokens)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Duplication {
    pub clone_lines: u64,
    pub source_lines: u64,
    pub clone_groups: Vec<CloneGroup>,
}

impl Duplication {
    pub fn ratio(&self) -> f64 {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloneGroup {
    pub tokens: usize,
    pub fragments: Vec<Fragment>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fragment {
    pub path: RepoPath,
    pub start_line: u32,
    pub end_line: u32,
}

pub fn find_clone_groups(model: &CodeModel, minimum: MinimumCloneTokens) -> Duplication {
    let _ = (model, minimum);
    todo!()
}
