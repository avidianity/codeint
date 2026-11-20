use super::RepoPath;

#[test]
fn a_repository_path_is_relative_and_uses_forward_slashes() {
    for valid in ["main.ts", "src/app/main.ts", ".github/ci.js"] {
        assert!(RepoPath::new(valid).is_ok(), "{valid} should be valid");
    }
    for invalid in [
        "",
        "/src/main.ts",
        "src\\main.ts",
        "src//main.ts",
        "src/",
        "./main.ts",
        "src/../main.ts",
    ] {
        assert!(
            RepoPath::new(invalid).is_err(),
            "{invalid} should be invalid"
        );
    }
}

#[test]
fn the_extension_is_the_text_after_the_last_dot_of_the_file_name() {
    let extension_of = |path: &str| {
        RepoPath::new(path)
            .expect("a valid path")
            .extension()
            .map(str::to_owned)
    };
    assert_eq!(extension_of("src/main.ts"), Some("ts".to_owned()));
    assert_eq!(extension_of("types/index.d.ts"), Some("ts".to_owned()));
    assert_eq!(extension_of("v1.2/Makefile"), None);
    assert_eq!(extension_of("src/.eslintrc"), None);
}
