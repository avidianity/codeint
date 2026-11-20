use std::path::Path;

use super::load_settings;

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
