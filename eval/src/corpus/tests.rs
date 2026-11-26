use super::*;

#[test]
fn size_buckets_follow_the_production_line_bands() {
    assert_eq!(SizeBucket::for_production_lines(1_999), None);
    assert_eq!(
        SizeBucket::for_production_lines(2_000),
        Some(SizeBucket::Small)
    );
    assert_eq!(
        SizeBucket::for_production_lines(9_999),
        Some(SizeBucket::Small)
    );
    assert_eq!(
        SizeBucket::for_production_lines(10_000),
        Some(SizeBucket::Medium)
    );
    assert_eq!(
        SizeBucket::for_production_lines(49_999),
        Some(SizeBucket::Medium)
    );
    assert_eq!(
        SizeBucket::for_production_lines(50_000),
        Some(SizeBucket::Large)
    );
}

fn entry(language: &str, commit: &str) -> CorpusEntry {
    CorpusEntry {
        name: "owner/name".to_owned(),
        language: language.to_owned(),
        bucket: SizeBucket::Small,
        url: "https://github.com/owner/name".to_owned(),
        commit: commit.to_owned(),
        panel: None,
    }
}

fn manifest(repositories: &str, case_studies: &str) -> String {
    format!(
        r#"{{"measured_on":"2026-11-25","repositories":[{repositories}],"case_studies":[{case_studies}]}}"#
    )
}

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

fn repository(panel: &str) -> String {
    format!(
        r#"{{"name":"owner/name","language":"typescript-javascript","bucket":"small","url":"https://github.com/owner/name","commit":"{SHA}"{panel}}}"#
    )
}

fn load(text: &str) -> Result<Corpus, EvaluationError> {
    let folder = tempfile::tempdir().expect("temporary folder");
    let path = folder.path().join("panels.json");
    std::fs::write(&path, text).expect("manifest written");
    load_corpus(&path)
}

#[test]
fn one_repository_may_join_several_panels_but_not_the_same_panel_twice() {
    let react = repository(r#","panel":"react""#);
    let vue = repository(r#","panel":"vue3""#);
    let both = format!("{react},{vue},{}", repository(""));
    assert_eq!(
        load(&manifest(&both, ""))
            .expect("valid")
            .repositories
            .len(),
        3
    );
    let twice = format!("{react},{react}");
    assert!(load(&manifest(&twice, "")).is_err_and(|error| error.to_string().contains("(react)")));
}

#[test]
fn case_studies_pin_both_commits() {
    let study = |after: &str| {
        format!(
            r#"{{"name":"study","url":"https://github.com/owner/name","before":"{SHA}","after":"{after}","change":"split a module"}}"#
        )
    };
    let valid = load(&manifest(&repository(""), &study(SHA))).expect("valid");
    assert_eq!(valid.case_studies.len(), 1);
    assert!(load(&manifest(&repository(""), &study("main"))).is_err());
}

#[test]
fn entries_need_a_built_in_language_and_a_full_commit_sha() {
    let sha = "0123456789abcdef0123456789abcdef01234567";
    assert_eq!(validate_entry(&entry("python", sha)), Ok(()));
    assert!(validate_entry(&entry("cobol", sha)).is_err_and(|message| message.contains("cobol")));
    assert!(validate_entry(&entry("python", "main")).is_err_and(|message| message.contains("40")));
    assert!(validate_entry(&entry("python", &sha.to_uppercase())).is_err());
}

#[test]
fn entries_and_case_studies_reject_option_like_urls() {
    let mut bad = entry("python", SHA);
    bad.url = "--upload-pack=touch /tmp/pwned".to_owned();
    assert!(validate_entry(&bad).is_err());
    let study = CaseStudy {
        name: "study".to_owned(),
        url: "--upload-pack=id".to_owned(),
        before: SHA.to_owned(),
        after: SHA.to_owned(),
        change: "change".to_owned(),
    };
    assert!(validate_case_study(&study).is_err());
}
