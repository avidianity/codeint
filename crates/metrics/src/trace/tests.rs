use codeint_model::{
    CodeModel, FileModel, ImportResolution, RepoPath, RuntimeImport, SourceFile, SourceSet,
};

use super::{DEFAULT_MAX_TRACE_PATHS, TraceError, trace_imports};

fn repo_path(path: &str) -> RepoPath {
    RepoPath::new(path).unwrap()
}

fn model_with_edges(files: &[&str], edges: &[(&str, &str)]) -> CodeModel {
    let mut model = CodeModel::default();
    for path in files {
        model.files.push(SourceFile {
            path: repo_path(path),
            model: FileModel {
                source_lines: 1,
                source_set: SourceSet::Production,
                functions: Vec::new(),
                tokens: Vec::new(),
                flagged_lines: Vec::new(),
                runtime_imports: Vec::new(),
                program_start: false,
            },
        });
    }
    for (from, to) in edges {
        model.runtime_imports.push(RuntimeImport {
            importer: repo_path(from),
            specifier: (*to).to_owned(),
            resolution: ImportResolution::Resolved(repo_path(to)),
        });
    }
    model
}

#[test]
fn trace_same_module_returns_zero_distance_single_path() {
    let model = model_with_edges(&["a.ts"], &[("a.ts", "a.ts")]);
    let result = trace_imports(
        &model,
        &repo_path("a.ts"),
        &repo_path("a.ts"),
        true,
        DEFAULT_MAX_TRACE_PATHS,
    )
    .unwrap();
    assert!(result.connected);
    assert_eq!(result.shortest_distance, Some(0));
    assert_eq!(result.paths, vec![vec![repo_path("a.ts")]]);
    assert!(!result.truncated);
}

#[test]
fn trace_single_mode_sets_truncated_false_even_when_multiple_shortest_paths_exist() {
    let model = model_with_edges(
        &["a.ts", "b.ts", "c.ts", "d.ts"],
        &[
            ("a.ts", "c.ts"),
            ("a.ts", "b.ts"),
            ("b.ts", "d.ts"),
            ("c.ts", "d.ts"),
        ],
    );
    let result = trace_imports(
        &model,
        &repo_path("a.ts"),
        &repo_path("d.ts"),
        false,
        DEFAULT_MAX_TRACE_PATHS,
    )
    .unwrap();
    assert!(result.connected);
    assert_eq!(result.shortest_distance, Some(2));
    assert_eq!(
        result.paths,
        vec![vec![
            repo_path("a.ts"),
            repo_path("b.ts"),
            repo_path("d.ts")
        ]]
    );
    assert!(!result.truncated);
}

#[test]
fn trace_rejects_zero_max_paths_and_unknown_modules() {
    let model = model_with_edges(&["a.ts"], &[]);
    assert_eq!(
        trace_imports(&model, &repo_path("a.ts"), &repo_path("a.ts"), true, 0),
        Err(TraceError::InvalidMaxPaths)
    );
    assert_eq!(
        trace_imports(
            &model,
            &repo_path("missing.ts"),
            &repo_path("a.ts"),
            false,
            10
        ),
        Err(TraceError::UnknownModule {
            path: repo_path("missing.ts")
        })
    );
}

#[test]
fn trace_terminates_on_dense_cycle_without_looping() {
    let model = model_with_edges(
        &["a.ts", "b.ts", "c.ts", "d.ts"],
        &[
            ("a.ts", "b.ts"),
            ("b.ts", "a.ts"),
            ("b.ts", "c.ts"),
            ("c.ts", "b.ts"),
            ("c.ts", "a.ts"),
            ("a.ts", "a.ts"),
        ],
    );
    let result = trace_imports(&model, &repo_path("a.ts"), &repo_path("d.ts"), true, 10).unwrap();
    assert!(!result.connected);
    assert_eq!(result.shortest_distance, None);
    assert!(result.paths.is_empty());
    assert!(!result.truncated);
}
