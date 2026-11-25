use std::fs;
use std::path::Path;

use super::{load_settings, temp_dir};

#[test]
fn third_party_skills_are_the_installed_skills_that_are_not_project_skills() {
    let settings = match load_settings(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or(Path::new(".")),
    ) {
        Ok(settings) => settings,
        Err(error) => panic!("{error}"),
    };
    assert!(settings.is_third_party_skill(".agents/skills/wizard/template.sh"));
    assert!(!settings.is_third_party_skill(".agents/skills/correction/SKILL.md"));
    assert!(!settings.is_third_party_skill(".agents/skillset/wizard/template.sh"));
    assert!(!settings.is_third_party_skill("docs/agents/git.md"));
}

#[test]
fn performance_budget_is_read_from_checks_toml() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."));
    let settings = match load_settings(root) {
        Ok(settings) => settings,
        Err(error) => panic!("{error}"),
    };
    assert!(settings.performance.packages > 0);
    assert!(settings.performance.files_per_package > 0);
    assert!(settings.performance.functions_per_file > 0);
    assert!(settings.performance.budget_seconds > 0.0);
}

#[test]
fn performance_budget_rejects_non_positive_budget_seconds() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."));
    let original = match fs::read_to_string(root.join("checks.toml")) {
        Ok(text) => text,
        Err(error) => panic!("{error}"),
    };
    let scratch = temp_dir().join(format!("codeint-config-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    if let Err(error) = fs::create_dir_all(&scratch) {
        panic!("{error}");
    }
    let broken = original
        .lines()
        .map(|line| {
            if line.starts_with("budget_seconds =") {
                "budget_seconds = 0.0"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if let Err(error) = fs::write(scratch.join("checks.toml"), broken) {
        panic!("{error}");
    }
    let result = load_settings(&scratch);
    let _ = fs::remove_dir_all(&scratch);
    assert!(result.is_err());
}

#[test]
fn commit_scopes_cover_every_built_in_language_plugin() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."));
    let settings = match load_settings(root) {
        Ok(settings) => settings,
        Err(error) => panic!("{error}"),
    };
    let entries = match fs::read_dir(root.join("plugins/languages")) {
        Ok(entries) => entries,
        Err(error) => panic!("cannot read plugins/languages: {error}"),
    };
    let mut plugin_dirs = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => panic!("cannot read plugin entry: {error}"),
        };
        if entry.path().is_dir()
            && let Some(name) = entry.file_name().to_str()
        {
            plugin_dirs.push(name.to_owned());
        }
    }
    plugin_dirs.sort();
    assert!(
        !plugin_dirs.is_empty(),
        "expected built-in language plugin directories"
    );
    for dir in plugin_dirs {
        let expected_scope = format!("lang-{dir}");
        assert!(
            settings.commit_scopes.contains(&expected_scope),
            "checks.toml commit.scopes is missing {expected_scope:?}"
        );
    }
}
