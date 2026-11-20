use std::collections::HashMap;
use std::fmt;

use codeint_model::RepoPath;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FindingId(u64);

impl fmt::Display for FindingId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:016x}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MetricKey {
    Erosion,
    Duplication,
    Verbosity,
    ImportCycles,
}

pub(crate) struct FindingIds {
    metric: MetricKey,
    occurrences: HashMap<u64, u64>,
}

impl FindingIds {
    pub(crate) fn new(metric: MetricKey) -> FindingIds {
        FindingIds {
            metric,
            occurrences: HashMap::new(),
        }
    }

    pub(crate) fn next_in_source_order<'path>(
        &mut self,
        paths: impl IntoIterator<Item = &'path RepoPath>,
        content: impl IntoIterator<Item = u64>,
    ) -> FindingId {
        let _ = (self.metric, &self.occurrences);
        let _ = (paths.into_iter().count(), content.into_iter().count());
        todo!()
    }
}
