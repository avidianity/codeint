pub(crate) mod budget;
pub(crate) mod duplicates;
pub(crate) mod layout;
pub(crate) mod perf;
pub(crate) mod pinning;
pub(crate) mod pull_request;
pub(crate) mod release;
pub(crate) mod source;
pub(crate) mod tier;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

pub(crate) type Violations = Vec<String>;

pub(crate) fn glob_set(patterns: &[String]) -> Result<GlobSet, String> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .map_err(|error| format!("invalid glob {pattern}: {error}"))?;
        builder.add(glob);
    }
    builder
        .build()
        .map_err(|error| format!("cannot build glob set: {error}"))
}
