use codeint_model::language::Manifest;

use super::workspace_packages;

fn pnpm_workspace_yaml(patterns: &[&str]) -> String {
    patterns
        .iter()
        .fold(String::from("packages:\n"), |yaml, pattern| {
            yaml + &format!("  - '{}'\n", pattern.replace('\'', "''"))
        })
}

fn manifest(path: &str, text: &str) -> Manifest {
    Manifest {
        path: path.to_owned(),
        text: text.to_owned(),
    }
}

fn pnpm_package_roots(patterns: &[&str], package_folders: &[&str]) -> Vec<String> {
    let mut manifests = vec![
        manifest("package.json", "{}"),
        manifest("pnpm-workspace.yaml", &pnpm_workspace_yaml(patterns)),
    ];
    manifests.extend(
        package_folders
            .iter()
            .map(|folder| manifest(&format!("{folder}/package.json"), "{}")),
    );
    let mut roots = match workspace_packages(&manifests) {
        Ok(roots) => roots,
        Err(error) => panic!("workspace packages not found: {error:?}"),
    };
    roots.sort();
    roots
}

#[test]
fn each_folder_a_pnpm_package_pattern_matches_is_a_workspace_package() {
    let roots = pnpm_package_roots(
        &["my-app", "packages/*", "components/**"],
        &[
            "my-app",
            "packages/core",
            "packages/ui",
            "components/button",
            "components/forms/input",
            "docs",
        ],
    );

    assert_eq!(
        roots,
        [
            "components/button",
            "components/forms/input",
            "my-app",
            "packages/core",
            "packages/ui",
        ]
    );
}

#[test]
fn a_negated_pnpm_package_pattern_excludes_the_folders_it_matches() {
    let roots = pnpm_package_roots(
        &["packages/**", "!**/test/**", "!./packages/legacy"],
        &[
            "packages/app",
            "packages/legacy",
            "packages/test",
            "packages/app/test/fixture",
        ],
    );

    assert_eq!(roots, ["packages/app"]);
}

#[test]
fn a_negated_pnpm_package_pattern_excludes_whatever_order_it_is_written_in() {
    let roots = pnpm_package_roots(
        &["!packages/legacy", "packages/*"],
        &["packages/app", "packages/legacy"],
    );

    assert_eq!(roots, ["packages/app"]);
}

fn package_roots_with_both_manifests(pnpm_workspace_manifest_path: &str) -> Vec<String> {
    workspace_packages(&[
        manifest("package.json", r#"{"workspaces": ["apps/*"]}"#),
        manifest(pnpm_workspace_manifest_path, "packages:\n  - packages/*\n"),
        manifest("apps/web/package.json", "{}"),
        manifest("packages/core/package.json", "{}"),
    ])
    .expect("workspace packages")
}

#[test]
fn pnpm_workspace_yaml_decides_the_packages_over_the_workspaces_field_of_package_json() {
    assert_eq!(
        package_roots_with_both_manifests("pnpm-workspace.yaml"),
        ["packages/core"]
    );
}

#[test]
fn a_pnpm_workspace_yaml_below_the_repository_root_does_not_decide_the_packages() {
    assert_eq!(
        package_roots_with_both_manifests("tools/pnpm-workspace.yaml"),
        ["apps/web"]
    );
}

#[test]
fn a_pnpm_workspace_yaml_without_package_patterns_means_no_workspace_packages() {
    assert!(pnpm_package_roots(&[], &["packages/app"]).is_empty());
}

#[test]
fn a_pnpm_wildcard_skips_folders_whose_names_begin_with_a_dot_unless_the_pattern_names_them() {
    let roots = pnpm_package_roots(
        &["packages/*", "tools/**", "hidden/.dev"],
        &[
            "packages/app",
            "packages/.cache",
            "tools/.bin/runner",
            "tools/lint",
            "hidden/.dev",
        ],
    );

    assert_eq!(roots, ["hidden/.dev", "packages/app", "tools/lint"]);
}

#[test]
fn a_negated_pnpm_wildcard_excludes_folders_whose_names_begin_with_a_dot() {
    let roots = pnpm_package_roots(
        &["packages/.dev/tool", "packages/app", "!packages/**"],
        &["packages/.dev/tool", "packages/app"],
    );

    assert!(roots.is_empty());
}

#[test]
fn a_pnpm_package_pattern_matches_after_its_dot_segments_and_repeated_slashes_are_resolved() {
    let roots = pnpm_package_roots(
        &["./apps/*", "packages//*", "libs/../tools/", "../outside/*"],
        &["apps/web", "packages/core", "tools", "libs", "outside/app"],
    );

    assert_eq!(roots, ["apps/web", "packages/core", "tools"]);
}

#[test]
fn an_absolute_pnpm_package_pattern_selects_and_excludes_nothing() {
    let roots = pnpm_package_roots(
        &["/packages/*", "apps/*", "!/apps/web"],
        &["packages/core", "apps/web"],
    );

    assert_eq!(roots, ["apps/web"]);
}

#[test]
fn a_package_inside_node_modules_or_bower_components_is_never_a_pnpm_workspace_package() {
    let roots = pnpm_package_roots(
        &["**"],
        &[
            "app",
            "app/node_modules/dependency",
            "node_modules/tool",
            "bower_components/widget",
        ],
    );

    assert_eq!(roots, ["app"]);
}

#[test]
fn a_pnpm_workspace_yaml_with_comments_and_other_settings_reads_only_its_package_patterns() {
    let roots = workspace_packages(&[
        manifest(
            "pnpm-workspace.yaml",
            concat!(
                "# The apps and the shared libraries.\n",
                "packages:\n",
                "  - apps/*   # every app\n",
                "  - \"!apps/legacy\"\n",
                "catalog:\n",
                "  react: ^19.0.0\n",
                "onlyBuiltDependencies:\n",
                "  - esbuild\n",
            ),
        ),
        manifest("apps/web/package.json", "{}"),
        manifest("apps/legacy/package.json", "{}"),
    ])
    .expect("workspace packages");

    assert_eq!(roots, ["apps/web"]);
}

#[test]
fn an_empty_pnpm_workspace_yaml_or_one_with_an_empty_packages_field_means_no_workspace_packages() {
    for pnpm_workspace_manifest in ["", "# only a comment\n", "packages:\n", "packages: []\n"] {
        let roots = workspace_packages(&[
            manifest("pnpm-workspace.yaml", pnpm_workspace_manifest),
            manifest("packages/app/package.json", "{}"),
        ])
        .expect("workspace packages");

        assert!(roots.is_empty(), "{pnpm_workspace_manifest:?}");
    }
}

#[test]
fn repeated_any_depth_segments_in_a_pnpm_pattern_do_not_backtrack_exponentially() {
    let deep_folder = (0..20).map(|_| "a").collect::<Vec<_>>().join("/");
    let pattern = format!("{}nomatch", "**/".repeat(15));
    let roots = pnpm_package_roots(&[&pattern], &[&deep_folder]);

    assert!(roots.is_empty());
}
