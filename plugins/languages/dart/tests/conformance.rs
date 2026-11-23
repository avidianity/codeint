use codeint_lang_dart::{Dart, flagged_line_rules};
use codeint_model::language::{ImportRequest, Resolution};
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
    assert_eq!(model.functions.len(), 2);
    let eval_fn = &model.functions[0];
    assert_eq!(
        (
            eval_fn.line,
            eval_fn.end_line,
            eval_fn.decision_points,
            eval_fn.source_lines,
            eval_fn.name.as_deref(),
            eval_fn.cognitive_complexity,
        ),
        (1, 7, 6, 7, Some("eval"), 5)
    );
    let arrow_fn = &model.functions[1];
    assert_eq!(
        (
            arrow_fn.line,
            arrow_fn.end_line,
            arrow_fn.decision_points,
            arrow_fn.source_lines,
            arrow_fn.name.as_deref(),
            arrow_fn.cognitive_complexity,
        ),
        (5, 5, 1, 1, None, 1)
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
    assert_eq!(res[2], Resolution::Builtin);
    assert_eq!(res[3], Resolution::External("fl".into()));
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

#[test]
fn dart_builtins_package_normalization_and_declared_dependencies() {
    const ROOT_PUBSPEC: &str = "\
name: acme_root
workspace:
  - packages/core
  - packages/app
dependencies:
  http: ^1.2.0
  intl:
dev_dependencies:
  lints: ^5.0.0
";
    const CORE_PUBSPEC: &str = "\
name: acme_core
dependencies:
  collection: ^1.19.0
";
    const APP_PUBSPEC: &str = "\
name: acme_app
dependencies:
  acme_core:
    path: ../core
  flutter:
    sdk: flutter
  path: ^1.9.0
dev_dependencies:
  flutter_test:
    sdk: flutter
  test: ^1.25.0
";
    const APP_MAIN: &str = "\
import 'dart:async';
import 'dart:convert';
import 'package:acme_core/core.dart';
import 'package:flutter/material.dart';
import 'package:path/path.dart';
import 'package:unlisted_pkg/widget.dart';

int run() => coreValue();
";
    let pubspecs = vec![
        Manifest {
            path: "pubspec.yaml".into(),
            text: ROOT_PUBSPEC.into(),
        },
        Manifest {
            path: "packages/core/pubspec.yaml".into(),
            text: CORE_PUBSPEC.into(),
        },
        Manifest {
            path: "packages/app/pubspec.yaml".into(),
            text: APP_PUBSPEC.into(),
        },
    ];
    let entry = "packages/app/lib/main.dart";
    let model = lower(entry, APP_MAIN).unwrap();
    let files = [
        "pubspec.yaml",
        "packages/core/pubspec.yaml",
        "packages/core/lib/core.dart",
        "packages/app/pubspec.yaml",
        entry,
    ]
    .map(|p| RepoPath::new(p).unwrap());
    let mut requests = Vec::new();
    for specifier in &model.runtime_imports {
        requests.push(ImportRequest {
            importer: entry.to_owned(),
            specifier: specifier.clone(),
        });
    }
    let resolutions = Dart.resolve_imports(&files, pubspecs.clone(), requests);
    assert_eq!(
        resolutions,
        [
            Resolution::Builtin,
            Resolution::Builtin,
            Resolution::Resolved("packages/core/lib/core.dart".into()),
            Resolution::External("flutter".into()),
            Resolution::External("path".into()),
            Resolution::External("unlisted_pkg".into()),
        ]
    );

    let deps = Dart.declared_dependencies(pubspecs).unwrap();
    let rendered: Vec<String> = deps
        .iter()
        .map(|d| format!("{}|{}|{:?}", d.package_root, d.name, d.kind))
        .collect();
    assert_eq!(
        rendered,
        [
            "|http|DepKind::Runtime",
            "|intl|DepKind::Runtime",
            "|lints|DepKind::Dev",
            "packages/app|flutter|DepKind::Runtime",
            "packages/app|path|DepKind::Runtime",
            "packages/app|flutter_test|DepKind::Dev",
            "packages/app|test|DepKind::Dev",
            "packages/core|collection|DepKind::Runtime",
        ]
    );
}

#[test]
fn dart_function_names_cover_functions_generics_methods_ctors_getters_and_closures() {
    let src = "\
T identity<T>(T value) => value;
class Counter {
  int _value;
  Counter(this._value);
  Counter.zero() : _value = 0;
  Counter.fromInt(int start) : _value = start {
    _value += 0;
  }
  int get current => _value;
  set current(int next) { _value = next; }
  bool operator ==(Object other) => identical(this, other);
  List<int> transform(List<int> items) {
    int localDouble(int n) => n * 2;
    return items.map((item) => localDouble(item)).where((item) {
      return item > 0;
    }).toList();
  }
}
";
    let model = lower("lib/names.dart", src).unwrap();
    let expected = [
        Some("identity"),
        Some("Counter.fromInt"),
        Some("current"),
        Some("current"),
        Some("operator =="),
        Some("transform"),
        Some("localDouble"),
        None,
        None,
    ];
    assert_eq!(model.functions.len(), expected.len());
    for (idx, want) in expected.into_iter().enumerate() {
        assert_eq!(model.functions[idx].name.as_deref(), want, "index {idx}");
    }
}

#[test]
fn dart_cognitive_complexity_distinguishes_flat_vs_nested_and_isolates_nested_functions() {
    let src = "\
int flatSwitch(int x) {
  switch (x) {
    case 1: return 10;
    case 2: return 20;
    case 3: return 30;
    case 4: return 40;
    default: return 0;
  }
}
int flatIfs(bool a, bool b, bool c, bool d) {
  var n = 0;
  if (a) { n += 1; }
  if (b) { n += 1; }
  if (c) { n += 1; }
  if (d) { n += 1; }
  return n;
}
int deeplyNested(bool a, bool b, bool c, bool d) {
  if (a) {
    if (b) {
      if (c) {
        if (d) { return 1; }
      }
    }
  }
  return 0;
}
int bareOnAndDoWhile(int n) {
  do {
    try {
      n -= 1;
    } on FormatException {
      return -1;
    }
  } while (n > 0);
  return n;
}
int complex(bool a, bool b, bool c, List<int> items) {
  if (a && b || c) {
    outer: for (final item in items) {
      if (item < 0) {
        break outer;
      } else if (item == 0) {
        continue outer;
      } else {
        try {
          final fn = (int v) {
            if (v > 0) {
              if (v > 1) { return v; }
            }
            return 0;
          };
          return fn(item);
        } on FormatException catch (e) {
          return complex(false, b, c, []);
        }
      }
    }
  }
  return 0;
}
";
    let model = lower("lib/cognitive.dart", src).unwrap();
    let expected: &[(Option<&str>, u32)] = &[
        (Some("flatSwitch"), 1),
        (Some("flatIfs"), 4),
        (Some("deeplyNested"), 10),
        (Some("bareOnAndDoWhile"), 3),
        (Some("complex"), 17),
        (None, 3),
    ];
    assert_eq!(model.functions.len(), expected.len());
    for (got, &(want_name, want_cog)) in model.functions.iter().zip(expected) {
        assert_eq!(got.name.as_deref(), want_name);
        assert_eq!(got.cognitive_complexity, want_cog, "for {want_name:?}");
    }
}

#[test]
fn dart_switch_case_nullable_type_patterns_are_not_counted_as_ternaries() {
    let src = "\
int classify(Object? value) {
  switch (value) {
    case int? _: return 1;
    case double? _: return 2;
    case String? _: return 3;
    case bool? _: return 4;
    case List? _: return 5;
    case Map? _: return 6;
    case Set? _: return 7;
    case num? _: return 8;
    default: return 0;
  }
}
int guarded(Object? value, bool flag) {
  switch (value) {
    case int? x when flag ? x != null : false:
      return 1;
    case String? _:
    default:
      return 0;
  }
}
";
    let model = lower("lib/patterns.dart", src).unwrap();
    assert_eq!(model.functions.len(), 2);

    let classify = &model.functions[0];
    assert_eq!(classify.name.as_deref(), Some("classify"));
    assert_eq!(classify.cognitive_complexity, 1);
    assert_eq!(classify.decision_points + 1, 9);

    let guarded = &model.functions[1];
    assert_eq!(guarded.name.as_deref(), Some("guarded"));
    assert_eq!(guarded.cognitive_complexity, 3);
    assert_eq!(guarded.decision_points + 1, 4);
}

#[test]
fn dart_switch_expression_nullable_patterns_before_ternary_are_not_counted_as_ternaries() {
    let src = "\
int evaluate(Object? v, bool c) {
  return switch (v) {
    int? n => c ? 1 : 2,
    double? n => c ? 3 : 4,
    String? n => c ? 5 : 6,
    bool? n => c ? 7 : 8,
    List? n => c ? 9 : 10,
    Map? n => c ? 11 : 12,
    Set? n => c ? 13 : 14,
    num? n => c ? 15 : 16,
    _ => 0,
  };
}
";
    let model = lower("lib/switch_expr.dart", src).unwrap();
    assert_eq!(model.functions.len(), 1);
    let evaluate = &model.functions[0];
    assert_eq!(evaluate.name.as_deref(), Some("evaluate"));
    assert_eq!(evaluate.decision_points, 16);
    assert_eq!(evaluate.decision_points + 1, 17);
    assert_eq!(evaluate.cognitive_complexity, 17);
}

#[test]
fn dart_nullable_declarations_when_guards_and_nested_ternaries_count_accurately() {
    let src = "\
int assignNullable(bool c) {
  int? a = c ? 1 : 2;
  int? b = c ? 3 : 4;
  int? d = c ? 5 : 6;
  int? e = c ? 7 : 8;
  int? f = c ? 9 : 10;
  int? g = c ? 11 : 12;
  int? h = c ? 13 : 14;
  int? i = c ? 15 : 16;
  return a ?? b ?? d ?? e ?? f ?? g ?? h ?? i ?? 0;
}
int guardedSwitchExpr(Object? v, bool c) {
  return switch (v) {
    int? n when c ? n != null : false => 1,
    double? n when c ? n != null : false => 2,
    String? n when c ? n != null : false => 3,
    bool? n when c ? n != null : false => 4,
    List? n when c ? n != null : false => 5,
    Map? n when c ? n != null : false => 6,
    Set? n when c ? n != null : false => 7,
    num? n when c ? n != null : false => 8,
    _ => 0,
  };
}
int nestedParenthesized(bool a, bool b) {
  final x = a ? (b ? 1 : 2) : 3;
  return x;
}
int nestedUnparenthesized(bool a, bool b) {
  final y = a ? b ? 1 : 2 : 3;
  return y;
}
";
    let model = lower("lib/ternary_edges.dart", src).unwrap();
    let got: Vec<(Option<&str>, u32, u32)> = model
        .functions
        .iter()
        .map(|f| {
            (
                f.name.as_deref(),
                f.decision_points + 1,
                f.cognitive_complexity,
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            (Some("assignNullable"), 17, 8),
            (Some("guardedSwitchExpr"), 17, 17),
            (Some("nestedParenthesized"), 3, 3),
            (Some("nestedUnparenthesized"), 3, 3),
        ]
    );
}
