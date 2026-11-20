use super::{Package, RepoPath, Workspace};

fn path(text: &str) -> RepoPath {
    RepoPath::new(text).expect("a valid path")
}

#[test]
fn a_file_belongs_to_the_deepest_workspace_package_whose_root_folder_holds_it() {
    let workspace = Workspace::new([path("packages/app"), path("packages/app/plugins/chart")]);
    let package_of = |file: &str| workspace.package_of(&path(file)).to_string();

    assert_eq!(package_of("packages/app/src/main.ts"), "packages/app");
    assert_eq!(
        package_of("packages/app/plugins/chart/index.ts"),
        "packages/app/plugins/chart"
    );
    assert_eq!(package_of("packages/app2/index.ts"), ".");
    assert_eq!(package_of("packages/app.ts"), ".");
    assert_eq!(package_of("main.ts"), ".");
}

#[test]
fn the_packages_of_a_workspace_are_the_root_package_then_each_member_by_path() {
    let workspace = Workspace::new([path("packages/b"), path("packages/a"), path("packages/b")]);

    let packages: Vec<String> = workspace
        .packages()
        .map(|package| package.to_string())
        .collect();

    assert_eq!(packages, [".", "packages/a", "packages/b"]);
    assert!(workspace.has_members());
    assert!(!Workspace::default().has_members());
    assert_eq!(
        Workspace::default()
            .packages()
            .collect::<Vec<Package<'_>>>(),
        [Package::Root]
    );
}

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
