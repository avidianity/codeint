use super::*;

#[test]
fn strip_and_rel_path_handle_root_nested_and_ignored_dirs() {
    assert_eq!(strip_package_root("", "src/main.ts"), Some("src/main.ts"));
    assert_eq!(
        strip_package_root("packages/app", "packages/app/src/main.ts"),
        Some("src/main.ts")
    );
    assert_eq!(
        strip_package_root("packages/app", "packages/other/src/main.ts"),
        None
    );
    assert_eq!(
        package_rel_path(
            "packages/app",
            "packages/app/node_modules/pkg/index.js",
            &["node_modules"]
        ),
        None
    );
    assert_eq!(
        package_rel_path(
            "packages/app",
            "packages/app/src/index.ts",
            &["node_modules"]
        ),
        Some("src/index.ts")
    );
}

fn manifest(path: &str, text: &str) -> Manifest {
    Manifest {
        path: path.to_owned(),
        text: text.to_owned(),
    }
}

#[test]
fn find_package_manifest_matches_root_or_nested_and_skips_ignored() {
    let manifests = vec![
        manifest("package.json", "{}"),
        manifest("apps/web/package.json", r#"{"name":"web"}"#),
        manifest("node_modules/foo/package.json", "{}"),
    ];
    assert_eq!(
        find_package_manifest("", "package.json", &manifests, &["node_modules"])
            .map(|m| m.path.as_str()),
        Some("package.json")
    );
    assert_eq!(
        find_package_manifest("apps/web", "package.json", &manifests, &["node_modules"])
            .map(|m| m.path.as_str()),
        Some("apps/web/package.json")
    );
    assert!(
        find_package_manifest(
            "node_modules/foo",
            "package.json",
            &manifests,
            &["node_modules"]
        )
        .is_none()
    );
}

#[test]
fn select_framework_profiles_handles_all_selection_modes() {
    let profiles = ["alpha", "beta", "gamma"];
    let detect = |name: &str| name != "gamma";
    let unknown = |name: &str| format!("unknown {name}");

    assert_eq!(
        select_framework_profiles(
            &profiles,
            |n| n,
            detect,
            FrameworkProfileSelection::Disabled,
            unknown
        ),
        Ok(Vec::new())
    );
    assert_eq!(
        select_framework_profiles(
            &profiles,
            |n| n,
            detect,
            FrameworkProfileSelection::Auto,
            unknown
        ),
        Ok(vec!["alpha", "beta"])
    );
    assert_eq!(
        select_framework_profiles(
            &profiles,
            |n| n,
            detect,
            FrameworkProfileSelection::Exclude(vec!["alpha".to_owned()]),
            unknown
        ),
        Ok(vec!["beta"])
    );
    assert_eq!(
        select_framework_profiles(
            &profiles,
            |n| n,
            detect,
            FrameworkProfileSelection::Only(vec!["gamma".to_owned()]),
            unknown
        ),
        Ok(vec!["gamma"])
    );
    assert_eq!(
        select_framework_profiles(
            &profiles,
            |n| n,
            detect,
            FrameworkProfileSelection::Only(vec!["missing".to_owned()]),
            unknown
        ),
        Err("unknown missing".to_owned())
    );
}
