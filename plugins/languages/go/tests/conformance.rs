use codeint_lang_go::{Go, flagged_line_rules};
use codeint_model::language::{DeclaredDep, DepKind, Function, ImportRequest, Resolution};
use codeint_model::{
    LanguagePlugin, LowerError, Manifest, RepoPath, SourceSet, TokenKind, WorkspaceError,
};

fn lower_ok(path: &str, src: &str) -> codeint_model::FileModel {
    Go.lower(&RepoPath::new(path).unwrap(), src)
        .unwrap_or_else(|err| panic!("expected {path} to lower cleanly: {err}"))
}

fn fn_names(model: &codeint_model::FileModel) -> Vec<Option<&str>> {
    model.functions.iter().map(|f| f.name.as_deref()).collect()
}

fn to_repo_paths(items: &[&str]) -> Vec<RepoPath> {
    items.iter().map(|p| RepoPath::new(*p).unwrap()).collect()
}

fn make_requests(pairs: &[(&str, &str)]) -> Vec<ImportRequest> {
    pairs
        .iter()
        .map(|(importer, specifier)| ImportRequest {
            importer: (*importer).to_owned(),
            specifier: (*specifier).to_owned(),
        })
        .collect()
}

fn assert_rule_hit(rule: &str, src: &str) {
    let model = lower_ok("bad.go", src);
    assert!(
        model.flagged_lines.iter().any(|l| l.rule == rule),
        "expected rule {rule} on:\n{src}\ngot: {:?}",
        model.flagged_lines
    );
}

fn assert_rule_silent(label: &str, src: &str) {
    let model = lower_ok("good.go", src);
    assert!(
        model.flagged_lines.is_empty(),
        "expected no flagged lines for {label}, got {:?}",
        model.flagged_lines
    );
}

#[test]
fn go_plugin_shell_lexer_literals_comments_and_unparsable_files() {
    assert_eq!(Go.extensions(), ["go"]);
    assert!(Go.has_imports());
    assert!(!Go.has_hazard_rules());
    assert!(Go.hazard_rules().is_empty());
    let ranges = Go.reference_ranges().unwrap();
    let src_label = "codeint stratified panel of 100 Go repositories, ADR 0035, codeint 0.8.0";
    let actual: Vec<(&str, f64, f64, &str)> = [
        "erosion",
        "cognitive_complexity",
        "verbosity",
        "duplication",
        "import_cycles",
    ]
    .into_iter()
    .map(|key| {
        let r = ranges.for_metric(key).expect("calibrated Go range");
        (key, r.low, r.high, r.source.as_str())
    })
    .collect();
    assert_eq!(
        actual,
        [
            ("erosion", 0.304, 0.746, src_label),
            ("cognitive_complexity", 0.306, 0.671, src_label),
            ("verbosity", 0.000, 0.004, src_label),
            ("duplication", 0.034, 0.243, src_label),
            ("import_cycles", 0.000, 0.000, src_label),
        ]
    );

    let unsupported = Go.lower(&RepoPath::new("notes.txt").unwrap(), "package main\n");
    assert!(matches!(
        unsupported,
        Err(LowerError::UnsupportedFile { .. })
    ));

    let empty = lower_ok("empty.go", "");
    assert_eq!(empty.source_lines, 0);
    assert!(empty.functions.is_empty());
    assert!(empty.tokens.is_empty());
    assert_eq!(empty.source_set, SourceSet::Production);
    assert!(!empty.program_start);

    for (path, src) in [
        ("unclosed_str.go", "package p\nvar s = \"abc\n"),
        ("newline_str.go", "package p\nvar s = \"a\nb\"\n"),
        ("unclosed_raw.go", "package p\nvar s = `abc\ndef"),
        ("unclosed_rune.go", "package p\nvar r = 'a\n"),
        ("empty_rune.go", "package p\nvar r = ''\n"),
        ("unclosed_block.go", "package p\n/* never closed\n"),
        ("unbalanced_brace.go", "package p\nfunc f() {\n"),
        ("mismatch_delim.go", "package p\nfunc f() { (] }\n"),
        ("illegal_char.go", "package p\nvar x = @1\n"),
    ] {
        let res = Go.lower(&RepoPath::new(path).unwrap(), src);
        assert!(
            matches!(res, Err(LowerError::Unparsable { .. })),
            "expected Unparsable for {path}, got {res:?}"
        );
    }

    let rich_src = r#"// leading comment
/* block
   comment */

package demo

func literals(ch chan int) int {
	s := "hello \"world\""
	raw := `line one
line two`
	r := '\n'
	a := 0x1_F + 0b1010 + 0o755 + 3.14e-2 + .5 + 0x1.fp-2
	imag := 2i
	val := <-ch
	val &^= 1
	_ = len(s) + len(raw) + int(r) + int(a) + int(real(imag))
	return val
}
"#;
    let model = lower_ok("pkg/rich.go", rich_src);
    assert_eq!(model.source_lines, 13);
    assert_eq!(model.functions.len(), 1);
    assert_eq!(model.functions[0].name.as_deref(), Some("literals"));
    assert_eq!(model.functions[0].line, 7);
    assert_eq!(model.functions[0].end_line, 18);
    assert_eq!(model.functions[0].source_lines, 12);

    let crlf = lower_ok("pkg/rich.go", &rich_src.replace('\n', "\r\n"));
    assert_eq!(crlf.source_lines, model.source_lines);
    assert_eq!(crlf.functions, model.functions);
}

#[test]
fn go_functions_brace_traps_bodyless_decls_methods_and_type_params() {
    let src = r#"package core

type Box[T interface{ ~int | ~string }] struct {
	Value T
}

type Reader interface {
	Read(p []byte) (n int, err error)
}

func forwardDecl(a, b int) int

func makeStruct(in struct{ X int }) struct{ Y int } {
	return struct{ Y int }{Y: in.X}
}

func makeInterface() interface{ Run() } {
	return nil
}

func headerTraps(v any) int {
	if xs := []int{1, 2}; len(xs) > 0 {
		for _, item := range []struct{ N int }{{N: 1}} {
			switch s := struct{ K int }{K: item.N}; s.K {
			case 1:
				if _, ok := v.(interface{ Close() error }); ok {
					return xs[0]
				}
			}
		}
	}
	return 0
}

func (b Box[T]) Read() T {
	return b.Value
}

func (b *Box[T]) Write(v T) {
	b.Value = v
}

func (*Box[T]) Reset() {}

func (b *Box[T]) Transform[U any](fn func(T) U) U {
	return fn(b.Value)
}

func Map[T any, U ~int | ~string](items []T, fn func(T) U) []U {
	return nil
}
"#;

    let model = lower_ok("pkg/core.go", src);
    assert_eq!(
        fn_names(&model),
        [
            Some("makeStruct"),
            Some("makeInterface"),
            Some("headerTraps"),
            Some("Box.Read"),
            Some("Box.Write"),
            Some("Box.Reset"),
            Some("Box.Transform"),
            Some("Map"),
        ]
    );
    assert_eq!(model.functions[2].decision_points, 4);
    assert_eq!(model.functions[2].cognitive_complexity, 10);
}

#[test]
fn go_function_literals_decisions_and_cognitive_complexity() {
    let src = r#"package metrics

var pkgLit = func(x int) int {
	if x > 0 {
		return x
	}
	return 0
}

func flatSwitch(x int) int {
	switch x {
	case 1, 2:
		return 10
	case 3:
		return 20
	default:
		return 0
	}
}

func nestedAndLiteral(items []int, a, b, c bool) int {
	if a && b || c {
		fn := func(v int) int {
			if v > 0 {
				return v
			}
			return 0
		}
		for _, item := range items {
			if item < 0 {
				return -1
			} else if item == 0 {
				continue
			} else {
				return fn(item)
			}
		}
	}
	return nestedAndLiteral(nil, false, b, c)
}

func jumpsAndSelect(ch1, ch2 chan int) {
outer:
	for {
		select {
		case <-ch1:
			break outer
		case <-ch2:
			continue outer
		default:
			goto done
		}
	}
done:
}

func keywordNoise() string {
	// if for case && ||
	return "if for case && ||"
}
"#;

    let model = lower_ok("pkg/metrics.go", src);
    let summary: Vec<(Option<&str>, u32, u32)> = model
        .functions
        .iter()
        .map(|f| (f.name.as_deref(), f.decision_points, f.cognitive_complexity))
        .collect();
    assert_eq!(
        summary,
        [
            (Some("flatSwitch"), 2, 1),
            (Some("nestedAndLiteral"), 7, 14),
            (Some("jumpsAndSelect"), 3, 6),
            (Some("keywordNoise"), 0, 0),
        ]
    );
    assert_eq!(
        model.functions[1],
        Function {
            line: 21,
            end_line: 40,
            decision_points: 7,
            source_lines: 20,
            name: Some("nestedAndLiteral".to_owned()),
            cognitive_complexity: 14,
            coverage: None,
        }
    );
}

#[test]
fn go_source_set_classification_and_program_start() {
    let cases = [
        (
            "pkg/app.go",
            "package app\nfunc f() {}\n",
            SourceSet::Production,
            false,
        ),
        (
            "cmd/cli/main.go",
            "package main\nfunc main() {}\n",
            SourceSet::Production,
            true,
        ),
        (
            "pkg/app_linux.go",
            "//go:build linux\npackage app\nfunc f() {}\n",
            SourceSet::Production,
            false,
        ),
        (
            "pkg/app_test.go",
            "package app\nfunc TestF() {}\n",
            SourceSet::Test,
            false,
        ),
        (
            "testdata/sample.go",
            "package sample\nfunc f() {}\n",
            SourceSet::Test,
            false,
        ),
        (
            "pkg/testdata/nested/sample.go",
            "package sample\nfunc f() {}\n",
            SourceSet::Test,
            false,
        ),
        (
            "pkg/gen.go",
            "// Code generated by protoc-gen-go. DO NOT EDIT.\npackage pkg\nfunc f() {}\n",
            SourceSet::Generated,
            false,
        ),
        (
            "pkg/gen_after_build.go",
            "//go:build !ignore\n\n// Code generated by stringer; DO NOT EDIT.\npackage pkg\nfunc f() {}\n",
            SourceSet::Generated,
            false,
        ),
        (
            "pkg/not_gen_block.go",
            "/* Code generated by tool. DO NOT EDIT. */\npackage pkg\nfunc f() {}\n",
            SourceSet::Production,
            false,
        ),
        (
            "pkg/not_gen_after_code.go",
            "package pkg\n// Code generated by tool. DO NOT EDIT.\nfunc f() {}\n",
            SourceSet::Production,
            false,
        ),
        (
            "pkg/not_gen_indented.go",
            " // Code generated by tool. DO NOT EDIT.\npackage pkg\nfunc f() {}\n",
            SourceSet::Production,
            false,
        ),
        (
            "vendor/example.com/lib/lib.go",
            "package lib\nfunc f() {}\n",
            SourceSet::Vendored,
            false,
        ),
    ];

    for (path, src, expected_set, expected_start) in cases {
        let model = lower_ok(path, src);
        assert_eq!(model.source_set, expected_set, "source_set for {path}");
        assert_eq!(
            model.program_start, expected_start,
            "program_start for {path}"
        );
    }
}

#[test]
fn go_five_flagged_line_rules_matches_and_near_misses() {
    let mut ids: Vec<_> = flagged_line_rules().collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        [
            "conditional-yields-boolean",
            "else-after-return",
            "if-else-returns-boolean",
            "redundant-break",
            "trailing-bare-return",
        ]
    );

    assert_rule_hit(
        "else-after-return",
        "package p\nfunc f(x int) int {\n\tif x > 0 {\n\t\treturn 1\n\t} else {\n\t\treturn 2\n\t}\n}\n",
    );
    assert_rule_hit(
        "else-after-return",
        "package p\nfunc f(x int) int {\n\tif y := x; y > 0 {\n\t\treturn y\n\t} else {\n\t\treturn 0\n\t}\n}\n",
    );
    assert_rule_hit(
        "else-after-return",
        "package p\nfunc f(x int) int {\n\tif x < 0 {\n\t\treturn -1\n\t} else if x == 0 {\n\t\treturn 0\n\t} else {\n\t\treturn 1\n\t}\n}\n",
    );
    assert_rule_hit(
        "if-else-returns-boolean",
        "package p\nfunc f(x int) bool {\n\tif x > 0 {\n\t\treturn true\n\t} else {\n\t\treturn false\n\t}\n}\n",
    );
    assert_rule_hit(
        "if-else-returns-boolean",
        "package p\nfunc f(x int) bool {\n\tif x > 0 {\n\t\treturn false\n\t}\n\treturn true\n}\n",
    );
    assert_rule_hit(
        "conditional-yields-boolean",
        "package p\nfunc f(x int) bool {\n\tvar ok bool\n\tif x > 0 {\n\t\tok = true\n\t} else {\n\t\tok = false\n\t}\n\treturn ok\n}\n",
    );
    assert_rule_hit(
        "trailing-bare-return",
        "package p\nfunc f() {\n\twork()\n\treturn\n}\n",
    );
    assert_rule_hit(
        "redundant-break",
        "package p\nfunc f(x int) {\n\tswitch x {\n\tcase 1:\n\t\twork()\n\t\tbreak\n\t}\n}\n",
    );
    assert_rule_hit(
        "redundant-break",
        "package p\nfunc f(ch chan int) {\n\tselect {\n\tcase <-ch:\n\t\tbreak\n\t}\n}\n",
    );

    assert_rule_silent(
        "idiomatic_if_err",
        "package p\nfunc f() (int, error) {\n\tv, err := load()\n\tif err != nil {\n\t\treturn 0, err\n\t}\n\treturn v, nil\n}\n",
    );
    assert_rule_silent(
        "else_uses_if_short_var",
        "package p\nfunc f() int {\n\tif v, err := load(); err != nil {\n\t\treturn 0\n\t} else {\n\t\treturn v\n\t}\n}\n",
    );
    assert_rule_silent(
        "else_if_chain_uses_outer_short_var",
        "package p\nfunc f() int {\n\tif v := load(); v < 0 {\n\t\treturn -1\n\t} else if v == 0 {\n\t\treturn 0\n\t} else {\n\t\treturn v\n\t}\n}\n",
    );
    assert_rule_silent(
        "else_if_returns_after_non_returning_if",
        "package p\nfunc f(x int) int {\n\tout := 0\n\tif x < 0 {\n\t\tout = -1\n\t} else if x == 0 {\n\t\treturn 0\n\t} else {\n\t\tout = 1\n\t}\n\treturn out\n}\n",
    );
    assert_rule_silent(
        "named_result_bool_and_bare_return",
        "package p\nfunc f(x int) (ok bool) {\n\tif x > 0 {\n\t\treturn true\n\t}\n\treturn false\n}\nfunc g() (n int) {\n\tn = 1\n\treturn\n}\n",
    );
    assert_rule_silent(
        "different_vars_assign_bool",
        "package p\nfunc f(x int) (bool, bool) {\n\tvar a, b bool\n\tif x > 0 {\n\t\ta = true\n\t} else {\n\t\tb = false\n\t}\n\treturn a, b\n}\n",
    );
    assert_rule_silent(
        "non_trailing_bare_return",
        "package p\nfunc f(x bool) {\n\tif x {\n\t\treturn\n\t}\n\twork()\n}\n",
    );
    assert_rule_silent(
        "labeled_and_loop_break_in_case",
        "package p\nfunc f(x int) {\nouter:\n\tfor {\n\t\tswitch x {\n\t\tcase 1:\n\t\t\tbreak outer\n\t\tcase 2:\n\t\t\tfor i := 0; i < 3; i++ {\n\t\t\t\tbreak\n\t\t\t}\n\t\tcase 3:\n\t\t\tif x == 3 {\n\t\t\t\tbreak\n\t\t\t}\n\t\t\twork()\n\t\t}\n\t}\n}\n",
    );
    assert_rule_silent(
        "if_init_bool_return",
        "package p\nfunc f() bool {\n\tif err := check(); err != nil {\n\t\treturn false\n\t}\n\treturn true\n}\n",
    );

    let named_if_else = lower_ok(
        "named_if_else.go",
        "package p\nfunc f(x int) (ok bool) {\n\tif x > 0 {\n\t\treturn true\n\t} else {\n\t\treturn false\n\t}\n}\n",
    );
    assert!(
        !named_if_else
            .flagged_lines
            .iter()
            .any(|l| l.rule == "if-else-returns-boolean")
    );
}

#[test]
fn go_imports_tokens_resolution_workspaces_replace_and_dependencies() {
    let src = r#"package main

import "C"
import f "fmt"
import (
	"net/http"
	_ "example.com/root/pkg/init"
	. "example.com/sub/api"
	"github.com/acme/lib/v2/sub"
	"github.com/acme/ind/pkg"
	"github.com/acme/tool/cmd/gen"
	"github.com/acme/unlisted/sub"
	"example.com/replaced/util"
)

func main() {
	s := `import "fake/string"`
	// import "fake/comment"
	f.Println(s, http.StatusOK)
}
"#;
    let model = lower_ok("main.go", src);
    assert!(model.program_start);
    assert_eq!(
        model.runtime_imports,
        [
            "C",
            "example.com/replaced/util",
            "example.com/root/pkg/init",
            "example.com/sub/api",
            "fmt",
            "github.com/acme/ind/pkg",
            "github.com/acme/lib/v2/sub",
            "github.com/acme/tool/cmd/gen",
            "github.com/acme/unlisted/sub",
            "net/http",
        ]
    );
    assert!(
        model
            .tokens
            .iter()
            .filter(|t| t.line <= 14)
            .all(|t| t.kind == TokenKind::ImportOrExport)
    );

    let repo = to_repo_paths(&[
        "go.mod",
        "go.work",
        "main.go",
        "pkg/init/a.go",
        "pkg/init/b.go",
        "pkg/init/a_test.go",
        "services/sub/go.mod",
        "services/sub/api/api.go",
        "local/replaced/util/u.go",
        "testdata/broken/go.mod",
        "vendor/github.com/acme/lib/go.mod",
    ]);

    assert_eq!(Go.manifest_paths(&repo), ["go.mod", "services/sub/go.mod"]);

    let root_mod = r#"module example.com/root
go 1.24
require (
	github.com/acme/lib/v2 v2.1.0
	github.com/acme/ind v1.0.0 // indirect
	github.com/acme/tool v0.4.0 // indirect
	example.com/sub v0.1.0
	example.com/replaced v0.0.0
)
require github.com/acme/direct v1.2.3
tool (
	github.com/acme/tool/cmd/gen
)
tool github.com/acme/direct/cmd/cli
replace (
	example.com/replaced => ./local/replaced
	github.com/acme/outside => ../outside
	github.com/acme/ver v1.0.0 => github.com/acme/ver v1.1.0
)
"#;
    let sub_mod = "module example.com/sub\n\ngo 1.24\nrequire github.com/acme/subdep v1.0.0\n";
    let manifests = vec![
        Manifest {
            path: "go.mod".into(),
            text: root_mod.into(),
        },
        Manifest {
            path: "services/sub/go.mod".into(),
            text: sub_mod.into(),
        },
    ];

    assert_eq!(
        Go.workspace_packages(manifests.clone()).unwrap(),
        [RepoPath::new("services/sub").unwrap()]
    );
    let bad = Go.workspace_packages(vec![Manifest {
        path: "go.mod".into(),
        text: "module example.com/root\nrequire (\n".into(),
    }]);
    assert!(matches!(bad, Err(WorkspaceError::Manifest { path, .. }) if path == "go.mod"));

    let reqs = make_requests(&[
        ("main.go", "C"),
        ("main.go", "fmt"),
        ("main.go", "net/http"),
        ("main.go", "example.com/root/pkg/init"),
        ("main.go", "example.com/root/pkg/missing"),
        ("main.go", "example.com/sub/api"),
        ("main.go", "example.com/replaced/util"),
        ("main.go", "github.com/acme/lib/v2/sub"),
        ("main.go", "github.com/acme/ind/pkg"),
        ("main.go", "github.com/acme/tool/cmd/gen"),
        ("main.go", "github.com/acme/unlisted/sub"),
        ("main.go", "github.com/acme/outside/pkg"),
    ]);

    let resolutions = Go
        .resolve_imports(&repo, manifests.clone(), reqs, &[])
        .unwrap();
    assert_eq!(resolutions[0], Resolution::Builtin);
    assert_eq!(resolutions[1], Resolution::Builtin);
    assert_eq!(resolutions[2], Resolution::Builtin);
    assert_eq!(
        resolutions[3],
        Resolution::Resolved(vec!["pkg/init/a.go".into(), "pkg/init/b.go".into()])
    );
    assert!(matches!(resolutions[4], Resolution::Unresolved(_)));
    assert_eq!(
        resolutions[5],
        Resolution::Resolved(vec!["services/sub/api/api.go".into()])
    );
    assert_eq!(
        resolutions[6],
        Resolution::Resolved(vec!["local/replaced/util/u.go".into()])
    );
    assert_eq!(
        resolutions[7],
        Resolution::External("github.com/acme/lib/v2".into())
    );
    assert_eq!(
        resolutions[8],
        Resolution::External("github.com/acme/ind".into())
    );
    assert_eq!(
        resolutions[9],
        Resolution::External("github.com/acme/tool".into())
    );
    assert_eq!(
        resolutions[10],
        Resolution::External("github.com/acme/unlisted".into())
    );
    assert_eq!(
        resolutions[11],
        Resolution::External("github.com/acme/outside".into())
    );

    let self_pkg_res = Go
        .resolve_imports(
            &repo,
            manifests.clone(),
            make_requests(&[("pkg/init/a.go", "example.com/root/pkg/init")]),
            &[],
        )
        .unwrap();
    assert_eq!(
        self_pkg_res,
        vec![Resolution::Resolved(vec!["pkg/init/b.go".into()])]
    );

    let deps = Go.declared_dependencies(manifests).unwrap();
    assert_eq!(
        deps,
        [
            DeclaredDep {
                package_root: "".into(),
                name: "github.com/acme/direct".into(),
                kind: DepKind::Runtime,
            },
            DeclaredDep {
                package_root: "".into(),
                name: "github.com/acme/lib/v2".into(),
                kind: DepKind::Runtime,
            },
            DeclaredDep {
                package_root: "".into(),
                name: "github.com/acme/tool".into(),
                kind: DepKind::Dev,
            },
            DeclaredDep {
                package_root: "".into(),
                name: "github.com/acme/ind".into(),
                kind: DepKind::Optional,
            },
            DeclaredDep {
                package_root: "services/sub".into(),
                name: "github.com/acme/subdep".into(),
                kind: DepKind::Runtime,
            },
        ]
    );
}

#[test]
fn go_handles_escaped_utf8_go_mod_parenthesized_receivers_func_type_slices_and_mixed_replaces() {
    let utf8_mod = vec![Manifest {
        path: "go.mod".into(),
        text: "module \"foo\\é\"\n".into(),
    }];
    assert!(Go.workspace_packages(utf8_mod).is_ok());

    let src = "\
package p

type Widget struct{}

func (w (*Widget)) ParenPtr(x int) int { return x }
func (w (Widget)) ParenVal(x int) int { return x }

func Direct(a, b, c, d bool) func() int {
	return func() int {
		if a {
			if b {
				if c {
					if d {
						return 1
					}
				}
			}
		}
		return 0
	}
}

func SliceOfFuncs(a, b, c, d bool) []func() int {
	return []func() int{
		func() int {
			if a {
				if b {
					if c {
						if d {
							return 1
						}
					}
				}
			}
			return 0
		},
	}
}

func SliceOfReturningFunc(a, b, c, d bool) []func() func() int {
	return []func() func() int{
		func() func() int {
			if a {
				if b {
					if c {
						if d {
							return nil
						}
					}
				}
			}
			return nil
		},
	}
}

func SliceOfPtrFunc(a, b, c, d bool) []*func() int {
	return []*func() int{
		ptr(func() int {
			if a {
				if b {
					if c {
						if d {
							return 1
						}
					}
				}
			}
			return 0
		}),
	}
}

func MapOfReturningFunc(a, b, c, d bool) map[string]func() func() int {
	return map[string]func() func() int{
		\"k\": func() func() int {
			if a {
				if b {
					if c {
						if d {
							return nil
						}
					}
				}
			}
			return nil
		},
	}
}

func MapOfPtrFunc(a, b, c, d bool) map[string]*func() int {
	return map[string]*func() int{
		\"k\": ptr(func() int {
			if a {
				if b {
					if c {
						if d {
							return 1
						}
					}
				}
			}
			return 0
		}),
	}
}

func ArrayOfPtrFunc(a, b, c, d bool) [1]*func() int {
	return [1]*func() int{
		ptr(func() int {
			if a {
				if b {
					if c {
						if d {
							return 1
						}
					}
				}
			}
			return 0
		}),
	}
}

func ChainedArrayOfPtrFunc(a, b, c, d bool) [2][1]*func() func() int {
	return [2][1]*func() func() int{
		{ptr(func() func() int {
			if a {
				if b {
					if c {
						if d {
							return nil
						}
					}
				}
			}
			return nil
		})},
	}
}

func SliceOfArrayOfPtrFunc(a, b, c, d bool) [][1]*func() int {
	return [][1]*func() int{
		{ptr(func() int {
			if a {
				if b {
					if c {
						if d {
							return 1
						}
					}
				}
			}
			return 0
		})},
	}
}

func MapOfArrayOfPtrFunc(a, b, c, d bool) map[string][1]*func() int {
	return map[string][1]*func() int{
		\"k\": {ptr(func() int {
			if a {
				if b {
					if c {
						if d {
							return 1
						}
					}
				}
			}
			return 0
		})},
	}
}

func ChainedIndexMulFuncLit(arr [][]int, a, b, c, d bool) int {
	return arr[0][1] * func() int {
		if a {
			if b {
				if c {
					if d {
						return 1
					}
				}
			}
		}
		return 0
	}()
}

func IndexMulFuncLit(arr []int, a, b, c, d bool) int {
	return arr[0] * func() int {
		if a {
			if b {
				if c {
					if d {
						return 1
					}
				}
			}
		}
		return 0
	}()
}

func NumMulFuncLit(a, b, c, d bool) int {
	return 2 * func() int {
		if a {
			if b {
				if c {
					if d {
						return 1
					}
				}
			}
		}
		return 0
	}()
}
";
    let model = lower_ok("p.go", src);
    assert_eq!(
        fn_names(&model),
        [
            Some("Widget.ParenPtr"),
            Some("Widget.ParenVal"),
            Some("Direct"),
            Some("SliceOfFuncs"),
            Some("SliceOfReturningFunc"),
            Some("SliceOfPtrFunc"),
            Some("MapOfReturningFunc"),
            Some("MapOfPtrFunc"),
            Some("ArrayOfPtrFunc"),
            Some("ChainedArrayOfPtrFunc"),
            Some("SliceOfArrayOfPtrFunc"),
            Some("MapOfArrayOfPtrFunc"),
            Some("ChainedIndexMulFuncLit"),
            Some("IndexMulFuncLit"),
            Some("NumMulFuncLit"),
        ]
    );
    let baseline = model.functions[2].cognitive_complexity;
    for func in &model.functions[3..] {
        assert_eq!(
            func.cognitive_complexity, baseline,
            "unexpected cognitive complexity for {:?}",
            func.name
        );
    }
    assert_rule_hit(
        "else-after-return",
        "package p\nfunc Rows(x bool) [2][1]*func() int {\n\treturn [2][1]*func() int{\n\t\t{ptr(func() int { return 0 })},\n\t\t{ptr(func() int {\n\t\t\tif x {\n\t\t\t\treturn 1\n\t\t\t} else {\n\t\t\t\treturn 2\n\t\t\t}\n\t\t})},\n\t}\n}\n",
    );

    let manifests = vec![Manifest {
        path: "go.mod".into(),
        text: "module example.org/app\n\nrequire example.org/lib v1.0.0\n\nreplace (\n\texample.org/lib => ./local/lib\n\texample.org/lib v1.0.0 => example.org/fork v1.0.1\n)\n".into(),
    }];
    let repo: Vec<RepoPath> = ["go.mod", "main.go", "local/lib/lib.go"]
        .into_iter()
        .map(|p| RepoPath::new(p).unwrap())
        .collect();
    let resolved = Go
        .resolve_imports(
            &repo,
            manifests.clone(),
            make_requests(&[("main.go", "example.org/lib")]),
            &[],
        )
        .unwrap();
    assert_eq!(
        resolved,
        vec![Resolution::Resolved(vec!["local/lib/lib.go".into()])]
    );
    assert!(Go.declared_dependencies(manifests).unwrap().is_empty());
}

#[test]
fn go_cognitive_excludes_chained_delegation_and_inline_type_bodies() {
    let src = r#"package delegation

type Node struct {
	n     *Node
	inner *Node
}

func (n *Node) Read() int {
	return n.n.Read()
}

func (n *Node) DeepDelegate() int {
	return n.inner.n.DeepDelegate()
}

func (n *Node) Recurse(k int) int {
	if k <= 0 {
		return 0
	}
	return n.Recurse(k - 1)
}

func CheckMatch(x any) bool {
	type matcher interface {
		CheckMatch() bool
	}
	if v, ok := x.(interface{ CheckMatch() bool }); ok {
		return v.CheckMatch()
	}
	_, ok := x.(matcher)
	return ok
}

func StructLitInBody(a, b bool) int {
	s := struct {
		Val int
	}{
		Val: func() int {
			if a && b {
				return 1
			}
			return 0
		}(),
	}
	return s.Val
}
"#;
    let model = lower_ok("pkg/delegation.go", src);
    let scores: Vec<(Option<&str>, u32)> = model
        .functions
        .iter()
        .map(|f| (f.name.as_deref(), f.cognitive_complexity))
        .collect();
    assert_eq!(
        scores,
        [
            (Some("Node.Read"), 0),
            (Some("Node.DeepDelegate"), 0),
            (Some("Node.Recurse"), 2),
            (Some("CheckMatch"), 1),
            (Some("StructLitInBody"), 3),
        ]
    );
}

#[test]
fn go_unquotes_escaped_and_raw_cr_imports_and_handles_utf8_escape_in_strings() {
    let src = "package imp\n\nimport (\n\t\"\\x66mt\"\n\t\"\\u0069o\"\n\t\"\\U00000073ort\"\n\t\"\\157s\"\n\t`net/http\r`\n)\n\nfunc Use() string {\n\treturn \"\\é\"\n}\n";
    let model = lower_ok("pkg/imp.go", src);
    assert_eq!(
        model.runtime_imports,
        ["fmt", "io", "net/http", "os", "sort"]
    );
    assert_eq!(fn_names(&model), [Some("Use")]);
}

#[test]
fn go_modern_syntax_go118_to_go124_and_planted_defect_recall() {
    let src = r#"//go:build linux && amd64

package modern

import (
	_ "embed"
	"fmt"
	f `io`
)

//go:embed data.txt
var embedded string

type Pair[K ~int | ~string, V interface{ comparable; ~float64 }] struct {
	Key K
	Val V
}

type Alias[T any] = Pair[string, float64]

func (p *Pair[K, V]) Get() K {
	return p.Key
}

func (Pair[K, V]) Zero() K {
	var zero K
	return zero
}

func ((*Pair[K, V])) Reset() {}

func RangeOverIntAndFunc(items []int) int {
	sum := 0
	for i := range 10 {
		sum += i
	}
	for v := range func(yield func(int) bool) {
		for _, item := range items {
			if !yield(item) {
				return
			}
		}
	} {
		sum += v
	}
	return sum
}

func RawStringsRunesAndUnicode(ch1, ch2 chan int) string {
	raw := `func Fake() {
		if a && b || c {
			for i := range 3 {
				switch i {
				case 1:
					import "fake/pkg"
				}
			}
		}
	} // " ' /*`
	runes := []rune{'\'', '\\', '\n', '\x7f', '\u1234', '\U0001F600', '\000', '{', '}', '"', '`', 'é'}
	select {
	case v, ok := <-ch1:
		if ok && v > 0 {
			return fmt.Sprint(v, raw, runes, f.EOF, embedded)
		}
	case ch2 <- 1:
		return "sent"
	default:
		return "none"
	}
	return ""
}

func λ(α, β int) int {
	if α > 0 && β > 0 {
		return α + β
	}
	return 0
}

func PlantedDefects[T ~int | ~string](x T, flag bool, ch chan int) int {
	check := func(v bool) bool {
		if v {
			return true
		} else {
			return false
		}
	}
	var active bool
	if flag {
		active = true
	} else {
		active = false
	}
	_ = check(active)
	helper := func() {
		fmt.Println(x)
		return
	}
	helper()
	select {
	case <-ch:
		fmt.Println(x)
		break
	default:
	}
	if flag {
		return 1
	} else {
		return 2
	}
}
"#;
    let model = lower_ok("pkg/modern.go", src);
    assert_eq!(model.runtime_imports, ["embed", "fmt", "io"]);
    assert_eq!(
        fn_names(&model),
        [
            Some("Pair.Get"),
            Some("Pair.Zero"),
            Some("Pair.Reset"),
            Some("RangeOverIntAndFunc"),
            Some("RawStringsRunesAndUnicode"),
            Some("λ"),
            Some("PlantedDefects"),
        ]
    );
    assert_eq!(model.functions[3].decision_points, 4);
    assert_eq!(model.functions[4].decision_points, 4);
    assert_eq!(model.functions[5].decision_points, 2);

    let mut planted_rules: Vec<&str> = model
        .flagged_lines
        .iter()
        .map(|fl| fl.rule.as_str())
        .collect();
    planted_rules.sort_unstable();
    planted_rules.dedup();
    assert_eq!(
        planted_rules,
        [
            "conditional-yields-boolean",
            "else-after-return",
            "if-else-returns-boolean",
            "redundant-break",
            "trailing-bare-return",
        ]
    );
}
