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
    }
}

#[test]
fn entries_need_a_built_in_language_and_a_full_commit_sha() {
    let sha = "0123456789abcdef0123456789abcdef01234567";
    assert_eq!(validate_entry(&entry("python", sha)), Ok(()));
    assert!(validate_entry(&entry("cobol", sha)).is_err_and(|message| message.contains("cobol")));
    assert!(validate_entry(&entry("python", "main")).is_err_and(|message| message.contains("40")));
    assert!(validate_entry(&entry("python", &sha.to_uppercase())).is_err());
}
