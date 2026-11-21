use super::{
    cargo_manifest_problems, is_exact_version, npm_manifest_problems, requires_npm_lockfile,
};

#[test]
fn caret_requirement_fails() {
    let manifest = "[dependencies]\nserde = \"2.1\"\n";
    assert_eq!(
        cargo_manifest_problems("Cargo.toml", manifest).map(|problems| problems.len()),
        Ok(1)
    );
}

#[test]
fn exact_requirement_workspace_inheritance_and_paths_pass() {
    let manifest = "[workspace.dependencies]\nserde = \"=2.1.4\"\n[dependencies]\nserde = { workspace = true }\nlocal = { path = \"../local\" }\n[target.'cfg(unix)'.dependencies]\nlibc = { version = \"=0.2.170\" }\n";
    assert_eq!(
        cargo_manifest_problems("Cargo.toml", manifest),
        Ok(Vec::new())
    );
}

#[test]
fn git_dependency_fails() {
    let manifest = "[dependencies]\nthing = { git = \"https://example.com/thing\" }\n";
    assert_eq!(
        cargo_manifest_problems("Cargo.toml", manifest).map(|problems| problems.len()),
        Ok(1)
    );
}

#[test]
fn npm_ranges_fail_and_exact_versions_pass() {
    let manifest = r#"{"devDependencies": {"a": "^1.2.3", "b": "1.2.3", "c": "latest"}}"#;
    assert_eq!(
        npm_manifest_problems("package.json", manifest).map(|problems| problems.len()),
        Ok(2)
    );
}

#[test]
fn prerelease_and_build_metadata_are_exact() {
    assert!(is_exact_version("1.2.1+spec-1.1.0"));
    assert!(is_exact_version("0.1.0-beta.2"));
    assert!(!is_exact_version("1.2"));
}

#[test]
fn a_range_that_starts_like_an_exact_version_fails() {
    assert!(!is_exact_version("1.0.0-0 || >=0"));
    assert!(!is_exact_version("1.0.0 - 2.0.0"));
    assert!(is_exact_version("1.0.0-rc.1+build.5"));
    assert!(is_exact_version("0.24.2"));
}

#[test]
fn published_npm_packages_do_not_require_a_committed_lockfile() {
    assert!(requires_npm_lockfile("package.json"));
    assert!(!requires_npm_lockfile("npm/codeint/package.json"));
    assert!(!requires_npm_lockfile(
        "npm/platforms/darwin-arm64/package.json"
    ));
}
