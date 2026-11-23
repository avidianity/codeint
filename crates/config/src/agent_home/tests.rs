use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::{AGENT_HOME_VARIABLE, resolve_agent_home};

fn env_from<'a>(
    entries: &'a [(&'a str, &'a str)],
) -> impl Fn(&'static str) -> Option<OsString> + 'a {
    move |name| {
        entries
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| OsString::from(value))
    }
}

#[test]
fn explicit_config_root_wins_over_env_and_home() {
    let cwd = Path::new("/work/repo");
    let resolved = resolve_agent_home(
        cwd,
        Some(Path::new("isolated-home")),
        false,
        env_from(&[(AGENT_HOME_VARIABLE, "/env/home"), ("HOME", "/real/home")]),
    );
    assert_eq!(resolved, Some(PathBuf::from("/work/repo/isolated-home")));
}

#[test]
fn agent_home_env_var_wins_over_platform_home() {
    let cwd = Path::new("/work/repo");
    let resolved = resolve_agent_home(
        cwd,
        None,
        false,
        env_from(&[(AGENT_HOME_VARIABLE, ".agent-home"), ("HOME", "/real/home")]),
    );
    assert_eq!(resolved, Some(PathBuf::from("/work/repo/.agent-home")));
}

#[test]
fn falls_back_to_platform_home_when_override_is_unset_or_empty() {
    let cwd = Path::new("/work/repo");
    let unix = resolve_agent_home(
        cwd,
        None,
        false,
        env_from(&[(AGENT_HOME_VARIABLE, ""), ("HOME", "/Users/dev")]),
    );
    assert_eq!(unix, Some(PathBuf::from("/Users/dev")));

    let windows = resolve_agent_home(
        cwd,
        None,
        true,
        env_from(&[("USERPROFILE", "C:\\Users\\dev")]),
    );
    assert_eq!(windows, Some(PathBuf::from("C:\\Users\\dev")));
}

#[test]
fn returns_none_when_no_home_source_is_present() {
    let cwd = Path::new("/work/repo");
    let resolved = resolve_agent_home(cwd, None, false, env_from(&[]));
    assert_eq!(resolved, None);
}
