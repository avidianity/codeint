use codeint_lang_dart::{Dart, flagged_line_rules};
use codeint_model::language::{Function, ImportRequest, Resolution};
use codeint_model::{LanguagePlugin, LowerError, Manifest, RepoPath, SourceSet, TokenKind};

fn lower(path: &str, src: &str) -> Result<codeint_model::FileModel, LowerError> {
    Dart.lower(&RepoPath::new(path).unwrap(), src)
}

#[test]
fn dart_contract_basics() {
    assert_eq!(Dart.extensions(), ["dart"]);
    assert!(
        Dart.reference_ranges()
            .unwrap()
            .for_metric("erosion")
            .is_some()
    );
    assert!(matches!(
        lower("a.rs", "fn f() {}"),
        Err(LowerError::UnsupportedFile { .. })
    ));
    assert!(matches!(
        lower("a.dart", "void f() {\n  var s = 'x;\n}\n"),
        Err(LowerError::Unparsable { message }) if message.contains("line 2")
    ));
    let empty = lower("a.dart", "").unwrap();
    assert_eq!(
        (empty.source_set, empty.source_lines),
        (SourceSet::Production, 0)
    );
}

#[test]
fn dart_functions_and_decisions() {
    let model = lower(
        "lib/eval.dart",
        "int eval(int? x) {\n\
           if (x != null && x > 0) {\n\
             for (var i = 0; i < x; i++) { x += i; }\n\
           }\n\
           final f = (int v) => v > 0 ? v : 0;\n\
           return switch (x ?? 0) { 0 => f(0), 1 => f(1), _ => 2 };\n\
         }\n",
    )
    .unwrap();
    assert_eq!(
        model.functions,
        [
            Function {
                line: 1,
                end_line: 7,
                decision_points: 6,
                source_lines: 7
            },
            Function {
                line: 5,
                end_line: 5,
                decision_points: 1,
                source_lines: 1
            },
        ]
    );
}

#[test]
fn dart_directives_and_resolution() {
    let model = lower(
        "lib/main.dart",
        "library a;\nimport 'dart:async';\nimport 'package:demo/u.dart';\npart 'h.dart';\npart of 'p.dart';\nint r() => 1;\n",
    )
    .unwrap();
    assert_eq!(
        model.runtime_imports,
        ["dart:async", "h.dart", "package:demo/u.dart"]
    );
    assert!(
        model
            .tokens
            .iter()
            .filter(|t| t.line <= 5)
            .all(|t| t.kind == TokenKind::ImportOrExport)
    );

    let files = ["pubspec.yaml", "lib/main.dart", "lib/h.dart", "lib/u.dart"]
        .map(|p| RepoPath::new(p).unwrap());
    let reqs = [
        "package:demo/u.dart",
        "h.dart",
        "dart:async",
        "package:fl/m.dart",
        "x.dart",
    ]
    .into_iter()
    .map(|s| ImportRequest {
        importer: "lib/main.dart".into(),
        specifier: s.into(),
    })
    .collect();
    let res = Dart.resolve_imports(
        &files,
        vec![Manifest {
            path: "pubspec.yaml".into(),
            text: "name: demo\n".into(),
        }],
        reqs,
    );
    assert_eq!(res[0], Resolution::Resolved("lib/u.dart".into()));
    assert_eq!(res[1], Resolution::Resolved("lib/h.dart".into()));
    assert_eq!(res[2], Resolution::External);
    assert_eq!(res[3], Resolution::External);
    assert!(matches!(res[4], Resolution::Unresolved(_)));
}

#[test]
fn dart_verbosity_rules() {
    let mut ids: Vec<_> = flagged_line_rules().collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        [
            "catch-only-rethrows",
            "else-after-return",
            "if-else-returns-boolean",
            "ternary-yields-boolean",
            "trailing-bare-return"
        ]
    );
    let check = |rule: &str, bad: &str, good: &str| {
        assert!(
            lower("lib/a.dart", bad)
                .unwrap()
                .flagged_lines
                .iter()
                .any(|l| l.rule == rule)
        );
        assert!(
            !lower("lib/b.dart", good)
                .unwrap()
                .flagged_lines
                .iter()
                .any(|l| l.rule == rule)
        );
    };
    check(
        "else-after-return",
        "int f(int x){if(x>0){return 1;}else{return 2;}}",
        "int f(int x){var y=0;if(x>0){y=1;}else{y=2;}return y;}",
    );
    check(
        "if-else-returns-boolean",
        "bool f(int x){if(x>0){return true;}else{return false;}}",
        "int f(int x){if(x>0){return 1;}else{return 0;}}",
    );
    check(
        "catch-only-rethrows",
        "void f(){try{w();}catch(e){rethrow;}}",
        "void f(){try{w();}catch(e){log(e);rethrow;}}",
    );
    check(
        "ternary-yields-boolean",
        "bool f(int x)=>x>0?true:false;",
        "String f(int x)=>x>0?'a':'b';",
    );
    check(
        "trailing-bare-return",
        "void f(){w();return;}",
        "void f(bool x){if(x){return;}w();}",
    );
}
