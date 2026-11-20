use std::collections::HashSet;

use codeint_model::language::{ImportRequest, Resolution};

pub(crate) struct RelativeResolver<'paths> {
    repository_paths: HashSet<&'paths str>,
}

impl<'paths> RelativeResolver<'paths> {
    pub(crate) fn new(_repository_paths: &'paths [String]) -> RelativeResolver<'paths> {
        todo!()
    }

    pub(crate) fn resolve(&self, _request: &ImportRequest) -> Resolution {
        todo!("resolve against {} paths", self.repository_paths.len())
    }
}
