use codeint_model::{
    CodeModel, FileModel, ImportResolution, RepoPath, RuntimeImport, SourceFile, SourceSet,
};

use super::{ImportCycles, UnresolvedImport};

fn path(text: &str) -> RepoPath {
    RepoPath::new(text).expect("a valid path")
}

fn module(name: &str) -> SourceFile {
    SourceFile {
        path: path(name),
        model: FileModel {
            source_set: SourceSet::Production,
            source_lines: 1,
            functions: Vec::new(),
            tokens: Vec::new(),
            flagged_lines: Vec::new(),
            runtime_imports: Vec::new(),
        },
    }
}

fn resolved(importer: &str, target: &str) -> RuntimeImport {
    RuntimeImport {
        importer: path(importer),
        specifier: format!("./{target}"),
        resolution: ImportResolution::Resolved(path(target)),
    }
}

fn graph(modules: &[&str], imports: &[(&str, &str)]) -> CodeModel {
    CodeModel {
        files: modules.iter().map(|name| module(name)).collect(),
        runtime_imports: imports
            .iter()
            .map(|(importer, target)| resolved(importer, target))
            .collect(),
        ..CodeModel::default()
    }
}

fn cycle_members(cycles: &ImportCycles) -> Vec<Vec<&str>> {
    cycles
        .cycles()
        .iter()
        .map(|cycle| cycle.modules().iter().map(RepoPath::as_str).collect())
        .collect()
}

#[test]
fn a_module_that_imports_itself_is_an_import_cycle_of_one() {
    let cycles = ImportCycles::measure(&graph(&["a.ts", "b.ts"], &[("a.ts", "a.ts")]));

    assert_eq!(cycle_members(&cycles), [["a.ts"]]);
    assert_eq!(cycles.modules_in_import_cycles(), 1);
    assert_eq!(cycles.modules(), 2);
}

#[test]
fn two_rings_joined_by_a_one_way_import_are_two_import_cycles() {
    let cycles = ImportCycles::measure(&graph(
        &["a.ts", "b.ts", "c.ts", "d.ts"],
        &[
            ("a.ts", "b.ts"),
            ("b.ts", "a.ts"),
            ("b.ts", "c.ts"),
            ("c.ts", "d.ts"),
            ("d.ts", "c.ts"),
        ],
    ));

    assert_eq!(cycle_members(&cycles), [["a.ts", "b.ts"], ["c.ts", "d.ts"]]);
    assert_eq!(cycles.modules_in_import_cycles(), 4);
}

#[test]
fn a_larger_import_cycle_comes_before_a_smaller_one() {
    let cycles = ImportCycles::measure(&graph(
        &["a.ts", "b.ts", "c.ts", "d.ts", "e.ts"],
        &[
            ("a.ts", "b.ts"),
            ("b.ts", "a.ts"),
            ("c.ts", "d.ts"),
            ("d.ts", "e.ts"),
            ("e.ts", "c.ts"),
        ],
    ));

    assert_eq!(
        cycle_members(&cycles),
        [vec!["c.ts", "d.ts", "e.ts"], vec!["a.ts", "b.ts"]]
    );
}

#[test]
fn imports_of_files_that_are_not_modules_and_unresolved_imports_add_no_edge() {
    let mut model = graph(&["a.ts", "b.ts"], &[("a.ts", "b.ts"), ("b.ts", "a.json")]);
    model.runtime_imports.push(RuntimeImport {
        importer: path("b.ts"),
        specifier: "./a".to_owned(),
        resolution: ImportResolution::Unresolved {
            reason: "no file".to_owned(),
        },
    });
    model.runtime_imports.push(RuntimeImport {
        importer: path("b.ts"),
        specifier: "a".to_owned(),
        resolution: ImportResolution::External,
    });

    let cycles = ImportCycles::measure(&model);

    assert!(cycles.cycles().is_empty());
    assert_eq!(cycles.modules_in_import_cycles(), 0);
}

#[test]
fn a_ring_of_a_hundred_thousand_modules_is_one_import_cycle_without_overflowing_the_stack() {
    let names: Vec<String> = (0..100_000)
        .map(|index| format!("m{index:06}.ts"))
        .collect();
    let modules: Vec<&str> = names.iter().map(String::as_str).collect();
    let imports: Vec<(&str, &str)> = modules
        .iter()
        .zip(modules.iter().cycle().skip(1))
        .map(|(importer, target)| (*importer, *target))
        .collect();

    let cycles = ImportCycles::measure(&graph(&modules, &imports));

    assert_eq!(cycles.cycles().len(), 1);
    assert_eq!(cycles.modules_in_import_cycles(), 100_000);
}

#[test]
fn an_empty_code_model_has_no_modules_and_no_import_cycles() {
    let cycles = ImportCycles::measure(&CodeModel::default());

    assert_eq!(cycles.modules(), 0);
    assert!(cycles.cycles().is_empty());
}

fn unresolved(importer: &str, specifier: &str) -> RuntimeImport {
    RuntimeImport {
        importer: path(importer),
        specifier: specifier.to_owned(),
        resolution: ImportResolution::Unresolved {
            reason: format!("no file matches {specifier}"),
        },
    }
}

#[test]
fn unresolved_imports_are_listed_by_importer_in_source_order_and_others_are_not() {
    let mut model = graph(&["a.ts", "b.ts"], &[("a.ts", "b.ts"), ("b.ts", "a.ts")]);
    model.runtime_imports.extend([
        unresolved("b.ts", "./second"),
        unresolved("a.ts", "./third"),
        RuntimeImport {
            importer: path("a.ts"),
            specifier: "lodash".to_owned(),
            resolution: ImportResolution::External,
        },
        unresolved("b.ts", "./first"),
    ]);

    let cycles = ImportCycles::measure(&model);

    assert_eq!(cycle_members(&cycles), [["a.ts", "b.ts"]]);
    assert_eq!(
        cycles.unresolved_imports(),
        [
            UnresolvedImport {
                importer: path("a.ts"),
                specifier: "./third".to_owned(),
                reason: "no file matches ./third".to_owned(),
            },
            UnresolvedImport {
                importer: path("b.ts"),
                specifier: "./second".to_owned(),
                reason: "no file matches ./second".to_owned(),
            },
            UnresolvedImport {
                importer: path("b.ts"),
                specifier: "./first".to_owned(),
                reason: "no file matches ./first".to_owned(),
            },
        ]
    );
}
