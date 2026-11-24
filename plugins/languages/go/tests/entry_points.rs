use codeint_lang_go::Go;
use codeint_model::{LanguagePlugin, Manifest, RepoPath};

fn assert_entry_points(paths: &[&str], mods: &[(&str, &str)], expected: &[&str]) {
    let mut repo = Vec::with_capacity(paths.len());
    for &path in paths {
        repo.push(RepoPath::new(path).unwrap());
    }
    let mut manifests = Vec::with_capacity(mods.len());
    for &(path, text) in mods {
        manifests.push(Manifest {
            path: path.to_owned(),
            text: text.to_owned(),
        });
    }
    let actual = Go.entry_points(&repo, manifests).unwrap();
    let expected_paths: Vec<RepoPath> = expected
        .iter()
        .map(|path| RepoPath::new(*path).unwrap())
        .collect();
    assert_eq!(actual, expected_paths);
}

#[test]
fn public_go_packages_outside_internal_are_entry_points() {
    assert_entry_points(
        &[
            "go.mod",
            "root.go",
            "root_test.go",
            "pkg/api/handler.go",
            "pkg/api/handler_test.go",
            "pkg/internal.go",
            "internal/secret/secret.go",
            "pkg/internal/hidden/hidden.go",
            "internal_helpers/util.go",
            "testdata/fixture.go",
            "vendor/example.com/lib/lib.go",
        ],
        &[("go.mod", "module example.com/lib\n\ngo 1.24\n")],
        &[
            "internal_helpers/util.go",
            "pkg/api/handler.go",
            "pkg/internal.go",
            "root.go",
        ],
    );
}

#[test]
fn nested_module_counts_internal_relative_to_its_own_module_root() {
    assert_entry_points(
        &[
            "go.mod",
            "internal/nested/go.mod",
            "internal/nested/public/api.go",
            "internal/nested/internal/priv/priv.go",
            "internal/rootonly/dead.go",
        ],
        &[
            ("go.mod", "module example.com/root\n"),
            ("internal/nested/go.mod", "module example.com/nested\n"),
        ],
        &["internal/nested/public/api.go"],
    );
}
