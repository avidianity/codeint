use std::collections::HashMap;
use std::fmt;

use codeint_model::{Metric, RepoPath, StableHasher};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FindingId(u64);

impl fmt::Display for FindingId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:016x}", self.0)
    }
}

pub(crate) struct FindingIds {
    metric: Metric,
    occurrences: HashMap<u64, u64>,
}

impl FindingIds {
    pub(crate) fn new(metric: Metric) -> FindingIds {
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
        let with_metric = length_prefixed(StableHasher::new(), self.metric.key().as_bytes());
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
        FindingId(id)
    }
}

fn length_prefixed(hasher: StableHasher, bytes: &[u8]) -> StableHasher {
    hasher.write_u64(bytes.len() as u64).write_bytes(bytes)
}
