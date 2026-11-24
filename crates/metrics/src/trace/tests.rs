use codeint_model::{
    CodeModel, FileModel, ImportResolution, RepoPath, RuntimeImport, SourceFile, SourceSet,
};

use super::{DEFAULT_MAX_TRACE_PATHS, MultiFileImport, TraceError, trace_imports};

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
                hazard_lines: Vec::new(),
                runtime_imports: Vec::new(),
                program_start: false,
            },
        });
    }
    for (from, to) in edges {
        model.runtime_imports.push(RuntimeImport {
            importer: repo_path(from),
            specifier: (*to).to_owned(),
            resolution: ImportResolution::Resolved(vec![repo_path(to)]),
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
    assert!(result.multi_file_imports.is_empty());
    assert!(!result.truncated);
}

#[test]
fn trace_records_multi_file_imports_along_returned_paths_and_omits_single_and_unused_ones() {
    let mut model = model_with_edges(
        &[
            "src/app.fld",
            "pkg/mid/a.fld",
            "pkg/mid/b.fld",
            "pkg/mid/sibling.fld",
            "pkg/leaf/end.fld",
            "pkg/leaf/other.fld",
            "pkg/unused/x.fld",
            "pkg/unused/y.fld",
        ],
        &[],
    );
    model.runtime_imports.push(RuntimeImport {
        importer: repo_path("src/app.fld"),
        specifier: "pkg/mid".to_owned(),
        resolution: ImportResolution::Resolved(vec![
            repo_path("pkg/mid/b.fld"),
            repo_path("pkg/mid/sibling.fld"),
            repo_path("pkg/mid/a.fld"),
            repo_path("pkg/mid/a.fld"),
        ]),
    });
    model.runtime_imports.push(RuntimeImport {
        importer: repo_path("src/app.fld"),
        specifier: "pkg/unused".to_owned(),
        resolution: ImportResolution::Resolved(vec![
            repo_path("pkg/unused/x.fld"),
            repo_path("pkg/unused/y.fld"),
        ]),
    });
    model.runtime_imports.push(RuntimeImport {
        importer: repo_path("pkg/mid/a.fld"),
        specifier: "pkg/leaf".to_owned(),
        resolution: ImportResolution::Resolved(vec![
            repo_path("pkg/leaf/other.fld"),
            repo_path("pkg/leaf/end.fld"),
        ]),
    });
    model.runtime_imports.push(RuntimeImport {
        importer: repo_path("pkg/mid/b.fld"),
        specifier: "pkg/leaf/end".to_owned(),
        resolution: ImportResolution::Resolved(vec![
            repo_path("pkg/leaf/end.fld"),
            repo_path("pkg/leaf/end.fld"),
        ]),
    });

    let all = trace_imports(
        &model,
        &repo_path("src/app.fld"),
        &repo_path("pkg/leaf/end.fld"),
        true,
        DEFAULT_MAX_TRACE_PATHS,
    )
    .unwrap();
    assert!(all.connected);
    assert_eq!(all.shortest_distance, Some(2));
    assert_eq!(
        all.paths,
        vec![
            vec![
                repo_path("src/app.fld"),
                repo_path("pkg/mid/a.fld"),
                repo_path("pkg/leaf/end.fld"),
            ],
            vec![
                repo_path("src/app.fld"),
                repo_path("pkg/mid/b.fld"),
                repo_path("pkg/leaf/end.fld"),
            ],
        ]
    );
    assert_eq!(
        all.multi_file_imports,
        vec![
            MultiFileImport {
                importer: repo_path("src/app.fld"),
                specifier: "pkg/mid".to_owned(),
                targets: vec![
                    repo_path("pkg/mid/a.fld"),
                    repo_path("pkg/mid/b.fld"),
                    repo_path("pkg/mid/sibling.fld"),
                ],
            },
            MultiFileImport {
                importer: repo_path("pkg/mid/a.fld"),
                specifier: "pkg/leaf".to_owned(),
                targets: vec![
                    repo_path("pkg/leaf/end.fld"),
                    repo_path("pkg/leaf/other.fld")
                ],
            },
        ]
    );

    let direct = trace_imports(
        &model,
        &repo_path("src/app.fld"),
        &repo_path("pkg/mid/sibling.fld"),
        false,
        DEFAULT_MAX_TRACE_PATHS,
    )
    .unwrap();
    assert_eq!(
        direct.paths,
        vec![vec![
            repo_path("src/app.fld"),
            repo_path("pkg/mid/sibling.fld")
        ]]
    );
    assert_eq!(
        direct.multi_file_imports,
        vec![MultiFileImport {
            importer: repo_path("src/app.fld"),
            specifier: "pkg/mid".to_owned(),
            targets: vec![
                repo_path("pkg/mid/a.fld"),
                repo_path("pkg/mid/b.fld"),
                repo_path("pkg/mid/sibling.fld"),
            ],
        }]
    );
}
