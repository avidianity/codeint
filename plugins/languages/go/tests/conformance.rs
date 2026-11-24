use codeint_lang_go::{Go, flagged_line_rules};
use codeint_model::language::{DeclaredDep, DepKind, Function, ImportRequest, Resolution};
use codeint_model::{
    LanguagePlugin, LowerError, Manifest, RepoPath, SourceSet, TokenKind, WorkspaceError,
};

fn lower_ok(path: &str, src: &str) -> codeint_model::FileModel {
    Go.lower(&RepoPath::new(path).unwrap(), src)
        .unwrap_or_else(|err| panic!("expected {path} to lower cleanly: {err}"))
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
    assert!(
        Go.reference_ranges()
            .unwrap()
            .for_metric("erosion")
            .is_none()
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
    let names: Vec<Option<&str>> = model.functions.iter().map(|f| f.name.as_deref()).collect();
    assert_eq!(
        names,
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

    let resolutions = Go.resolve_imports(&repo, manifests.clone(), reqs).unwrap();
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
