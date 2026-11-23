use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

pub(crate) const AGENT_HOME_VARIABLE: &str = "CODEINT_AGENT_HOME";

pub(crate) fn resolve_agent_home<F>(
    current_folder: &Path,
    explicit: Option<&Path>,
    is_windows: bool,
    read_env: F,
) -> Option<PathBuf>
where
    F: Fn(&'static str) -> Option<OsString>,
{
    if let Some(override_dir) = explicit {
        return Some(current_folder.join(override_dir));
    }
    if let Some(configured) = non_empty_var(&read_env, AGENT_HOME_VARIABLE) {
        return Some(current_folder.join(configured));
    }
    let fallback = if is_windows {
        non_empty_var(&read_env, "USERPROFILE").or_else(|| non_empty_var(&read_env, "HOME"))
    } else {
        non_empty_var(&read_env, "HOME")
    };
    fallback.map(PathBuf::from)
}

fn non_empty_var<F>(read_env: &F, key: &'static str) -> Option<OsString>
where
    F: Fn(&'static str) -> Option<OsString>,
{
    read_env(key).filter(|val| !val.is_empty())
}
