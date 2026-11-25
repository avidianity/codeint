use super::*;

#[test]
fn versions_parse_from_each_runtime_banner() {
    assert_eq!(parse_version("Python 3.15.2"), Some((3, 15)));
    assert_eq!(
        parse_version("go version go1.27.2 darwin/arm64"),
        Some((1, 27))
    );
    assert_eq!(parse_version("8.5.11"), Some((8, 5)));
    assert_eq!(
        parse_version("Dart SDK version: 3.11.0 (stable) on \"macos_arm64\""),
        Some((3, 11))
    );
    assert_eq!(parse_version("no digits"), None);
}
