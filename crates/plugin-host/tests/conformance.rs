mod common;

use std::path::Path;

use codeint_config::PluginHostConfig;
use codeint_lang_dart::Dart;
use codeint_lang_php::Php;
use codeint_lang_python::Python;
use codeint_lang_rust::Rust;
use codeint_lang_ts::TypeScriptAndJavaScript;
use codeint_model::language::{ImportRequest, Resolution};
use codeint_model::{LanguagePlugin, LowerError, Manifest, RepoPath, SourceSet, WorkspaceError};
use codeint_plugin_host::load_language_plugin;

use common::{declaration_for, sample_plugin_wasm};

struct ConformanceFixture<'a> {
    name: &'a str,
    production_path: &'a str,
    production_source: &'a str,
    production_source_crlf: &'a str,
    test_path: &'a str,
    test_source: &'a str,
    empty_path: &'a str,
    declaration_only: Option<(&'a str, &'a str)>,
    unparsable_path: &'a str,
    unparsable_source: &'a str,
    unsupported_path: &'a str,
    repository_paths: &'a [&'a str],
    expected_manifest_path: &'a str,
    valid_manifests: &'a [(&'a str, &'a str)],
    expected_workspace_package: &'a str,
    invalid_manifest_text: &'a str,
    import_requests: &'a [(&'a str, &'a str)],
    expected_resolved_target: &'a str,
}

fn assert_seam_2_conformance(plugin: &dyn LanguagePlugin, fixture: &ConformanceFixture<'_>) {
    let label = fixture.name;

    let extensions = plugin.extensions();
    assert!(
        !extensions.is_empty(),
        "{label}: extensions must not be empty"
    );
    for extension in &extensions {
        assert!(
            !extension.is_empty() && !extension.starts_with('.'),
            "{label}: invalid extension {extension:?}"
        );
    }

    let unsupported = plugin.lower(
        &RepoPath::new(fixture.unsupported_path).unwrap(),
        "plain text\n",
    );
    assert!(
        matches!(unsupported, Err(LowerError::UnsupportedFile { .. })),
        "{label}: expected UnsupportedFile, got {unsupported:?}"
    );

    let empty = plugin
        .lower(&RepoPath::new(fixture.empty_path).unwrap(), "")
        .unwrap_or_else(|error| panic!("{label}: empty file failed to lower: {error}"));
    assert_eq!(empty.source_lines, 0, "{label}: empty source_lines");
    assert!(empty.functions.is_empty(), "{label}: empty functions");
    assert!(empty.tokens.is_empty(), "{label}: empty tokens");
    assert!(
        empty.flagged_lines.is_empty(),
        "{label}: empty flagged_lines"
    );
    assert!(
        empty.runtime_imports.is_empty(),
        "{label}: empty runtime_imports"
    );

    if let Some((decl_path, decl_source)) = fixture.declaration_only {
        let decl = plugin.lower(&RepoPath::new(decl_path).unwrap(), decl_source);
        assert!(
            matches!(decl, Err(LowerError::DeclarationOnly)),
            "{label}: expected DeclarationOnly, got {decl:?}"
        );
    }

    let unparsable = plugin.lower(
        &RepoPath::new(fixture.unparsable_path).unwrap(),
        fixture.unparsable_source,
    );
    assert!(
        matches!(&unparsable, Err(LowerError::Unparsable { message }) if !message.is_empty()),
        "{label}: expected Unparsable with message, got {unparsable:?}"
    );

    let prod_path = RepoPath::new(fixture.production_path).unwrap();
    let prod = plugin
        .lower(&prod_path, fixture.production_source)
        .unwrap_or_else(|error| panic!("{label}: production file failed to lower: {error}"));
    assert_eq!(
        prod.source_set,
        SourceSet::Production,
        "{label}: production source_set"
    );
    assert!(prod.source_lines > 0, "{label}: production source_lines");
    assert!(!prod.functions.is_empty(), "{label}: production functions");
    assert!(!prod.tokens.is_empty(), "{label}: production tokens");
    assert!(
        !prod.runtime_imports.is_empty(),
        "{label}: production runtime_imports"
    );
    for pair in prod.functions.windows(2) {
        assert!(
            pair[0].line <= pair[1].line,
            "{label}: functions out of order"
        );
    }
    for function in &prod.functions {
        assert!(
            function.line >= 1 && function.line <= function.end_line,
            "{label}: invalid function span {function:?}"
        );
        assert_eq!(
            function.coverage, None,
            "{label}: Language plugin must leave function coverage absent"
        );
    }
    for pair in prod.tokens.windows(2) {
        assert!(pair[0].line <= pair[1].line, "{label}: tokens out of order");
    }
    for flagged in &prod.flagged_lines {
        assert!(
            flagged.line >= 1 && !flagged.rule.is_empty(),
            "{label}: invalid flagged line"
        );
    }

    let crlf = plugin
        .lower(&prod_path, fixture.production_source_crlf)
        .unwrap_or_else(|error| panic!("{label}: CRLF file failed to lower: {error}"));
    assert_eq!(
        crlf.source_lines, prod.source_lines,
        "{label}: CRLF source_lines"
    );
    assert_eq!(crlf.functions, prod.functions, "{label}: CRLF functions");

    let test_model = plugin
        .lower(
            &RepoPath::new(fixture.test_path).unwrap(),
            fixture.test_source,
        )
        .unwrap_or_else(|error| panic!("{label}: test file failed to lower: {error}"));
    assert_eq!(
        test_model.source_set,
        SourceSet::Test,
        "{label}: test source_set"
    );
    for function in &test_model.functions {
        assert_eq!(
            function.coverage, None,
            "{label}: Language plugin must leave test function coverage absent"
        );
    }

    let repo_paths: Vec<RepoPath> = fixture
        .repository_paths
        .iter()
        .map(|path| RepoPath::new(*path).unwrap())
        .collect();
    let manifest_paths = plugin.manifest_paths(&repo_paths);
    assert!(
        manifest_paths.contains(&fixture.expected_manifest_path.to_owned()),
        "{label}: expected manifest {} in {manifest_paths:?}",
        fixture.expected_manifest_path
    );
    for chosen in &manifest_paths {
        assert!(
            fixture.repository_paths.contains(&chosen.as_str()),
            "{label}: manifest_paths returned {chosen:?} outside repository_paths"
        );
    }

    let valid_manifest: Vec<Manifest> = fixture
        .valid_manifests
        .iter()
        .map(|(path, text)| Manifest {
            path: (*path).to_owned(),
            text: (*text).to_owned(),
        })
        .collect();
    let packages = plugin
        .workspace_packages(valid_manifest.clone())
        .unwrap_or_else(|error| panic!("{label}: workspace_packages failed: {error}"));
    assert_eq!(
        packages,
        vec![RepoPath::new(fixture.expected_workspace_package).unwrap()],
        "{label}: workspace_packages"
    );

    let invalid_manifest = vec![Manifest {
        path: fixture.expected_manifest_path.to_owned(),
        text: fixture.invalid_manifest_text.to_owned(),
    }];
    let bad_workspace = plugin.workspace_packages(invalid_manifest);
    assert!(
        matches!(bad_workspace, Err(WorkspaceError::Manifest { .. })),
        "{label}: expected WorkspaceError::Manifest, got {bad_workspace:?}"
    );

    let requests: Vec<ImportRequest> = fixture
        .import_requests
        .iter()
        .map(|(importer, specifier)| ImportRequest {
            importer: (*importer).to_owned(),
            specifier: (*specifier).to_owned(),
        })
        .collect();
    let resolutions = plugin.resolve_imports(&repo_paths, valid_manifest.clone(), requests);
    assert_eq!(
        resolutions.len(),
        fixture.import_requests.len(),
        "{label}: resolve_imports length must match requests"
    );
    assert_eq!(
        resolutions[0],
        Resolution::Resolved(fixture.expected_resolved_target.to_owned()),
        "{label}: first import resolves"
    );
    assert!(
        matches!(
            resolutions[1],
            Resolution::External(_) | Resolution::Builtin
        ),
        "{label}: second import is external or builtin, got {:?}",
        resolutions[1]
    );
    assert!(
        matches!(resolutions[2], Resolution::Unresolved(_)),
        "{label}: third import is unresolved, got {:?}",
        resolutions[2]
    );

    let _deps = plugin
        .declared_dependencies(valid_manifest.clone())
        .unwrap_or_else(|error| panic!("{label}: declared_dependencies failed: {error}"));
    let entry_points = plugin
        .entry_points(&repo_paths, valid_manifest)
        .unwrap_or_else(|error| panic!("{label}: entry_points failed: {error}"));
    for entry in &entry_points {
        assert!(
            fixture.repository_paths.contains(&entry.as_str()),
            "{label}: entry_points returned {entry:?} outside repository_paths"
        );
    }

    let ranges = plugin
        .reference_ranges()
        .unwrap_or_else(|error| panic!("{label}: reference_ranges failed: {error}"));
    assert!(
        ranges.for_metric("erosion").is_some(),
        "{label}: expected erosion reference range"
    );

    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    plugin
                        .lower(&prod_path, fixture.production_source)
                        .expect("concurrent lower succeeds")
                })
            })
            .collect();
        for handle in handles {
            assert_eq!(
                handle.join().unwrap(),
                prod,
                "{label}: concurrent lower mismatch"
            );
        }
    });
}

#[test]
fn built_in_typescript_javascript_plugin_passes_seam_2_conformance() {
    let fixture = ConformanceFixture {
        name: "typescript-javascript",
        production_path: "packages/app/src/main.ts",
        production_source: "import { helper } from './helper';\n\n// comment\nexport function run(flag: boolean): number {\n  if (flag) {\n    return helper();\n  } else {\n    return 0;\n  }\n}\n",
        production_source_crlf: "import { helper } from './helper';\r\n\r\n// comment\r\nexport function run(flag: boolean): number {\r\n  if (flag) {\r\n    return helper();\r\n  } else {\r\n    return 0;\r\n  }\r\n}\r\n",
        test_path: "packages/app/src/main.test.ts",
        test_source: "export function check() {\n  return 1;\n}\n",
        empty_path: "packages/app/src/empty.ts",
        declaration_only: Some((
            "packages/app/src/types.d.ts",
            "export declare const version: string;\n",
        )),
        unparsable_path: "packages/app/src/broken.ts",
        unparsable_source: "export const = ;\n",
        unsupported_path: "scripts/tool.unknown",
        repository_paths: &[
            "package.json",
            "packages/app/package.json",
            "packages/app/src/main.ts",
            "packages/app/src/helper.ts",
        ],
        expected_manifest_path: "package.json",
        valid_manifests: &[
            ("package.json", r#"{"workspaces": ["packages/app"]}"#),
            ("packages/app/package.json", r#"{"name": "@sample/app"}"#),
        ],
        expected_workspace_package: "packages/app",
        invalid_manifest_text: "{not json",
        import_requests: &[
            ("packages/app/src/main.ts", "./helper"),
            ("packages/app/src/main.ts", "node:path"),
            ("packages/app/src/main.ts", "./missing"),
        ],
        expected_resolved_target: "packages/app/src/helper.ts",
    };

    assert_seam_2_conformance(&TypeScriptAndJavaScript, &fixture);
}

#[test]
fn sample_external_wasm_plugin_passes_seam_2_conformance() {
    let wasm_path = sample_plugin_wasm();
    let declaration = declaration_for("sample-mini", wasm_path);
    let plugin = load_language_plugin(Path::new("."), &declaration, &PluginHostConfig::default())
        .expect("sample external wasm plugin loads");

    let fixture = ConformanceFixture {
        name: "sample-mini",
        production_path: "packages/app/src/main.mini",
        production_source: "import \"./helper\"\n\n# comment\nfn run(flag) {\n  if flag {\n    pass;\n  }\n}\n",
        production_source_crlf: "import \"./helper\"\r\n\r\n# comment\r\nfn run(flag) {\r\n  if flag {\r\n    pass;\r\n  }\r\n}\r\n",
        test_path: "packages/app/src/main.test.mini",
        test_source: "fn check() {\n  return 1\n}\n",
        empty_path: "packages/app/src/empty.mini",
        declaration_only: Some(("packages/app/src/types.d.mini", "fn extern_only() {}\n")),
        unparsable_path: "packages/app/src/broken.mini",
        unparsable_source: "SYNTAX_ERROR\n",
        unsupported_path: "scripts/tool.unknown",
        repository_paths: &[
            "mini.workspace",
            "packages/app/src/main.mini",
            "packages/app/src/helper.mini",
        ],
        expected_manifest_path: "mini.workspace",
        valid_manifests: &[("mini.workspace", "packages/app\n")],
        expected_workspace_package: "packages/app",
        invalid_manifest_text: "INVALID: bad workspace entry\n",
        import_requests: &[
            ("packages/app/src/main.mini", "./helper"),
            ("packages/app/src/main.mini", "external-lib"),
            ("packages/app/src/main.mini", "./missing"),
        ],
        expected_resolved_target: "packages/app/src/helper.mini",
    };

    assert_seam_2_conformance(&plugin, &fixture);
}

#[test]
fn built_in_rust_plugin_passes_seam_2_conformance() {
    run_simple_conformance(
        &Rust,
        "rust",
        "mod helper;\nuse crate::helper::value;\n\n// comment\npub fn run(flag: bool) -> i32 {\n    if flag {\n        return value();\n    } else {\n        return 0;\n    }\n}\n",
        (
            "crates/app/src/lib.rs",
            "crates/app/src/helper.rs",
            "crates/app/tests/integration.rs",
            "fn check() -> i32 { 1 }\n",
            "crates/app/src/empty.rs",
            "crates/app/src/broken.rs",
            "fn broken( {\n",
        ),
        (
            "Cargo.toml",
            "crates/app",
            "[workspace\nmembers = [",
            &[
                "Cargo.toml",
                "crates/app/Cargo.toml",
                "crates/app/src/lib.rs",
                "crates/app/src/helper.rs",
            ],
            &[
                ("Cargo.toml", "[workspace]\nmembers = [\"crates/app\"]\n"),
                ("crates/app/Cargo.toml", "[package]\nname = \"app\"\n"),
            ],
            &[
                ("crates/app/src/lib.rs", "mod:helper"),
                ("crates/app/src/lib.rs", "use:std::fmt"),
                ("crates/app/src/lib.rs", "mod:missing"),
            ],
        ),
    );
}

type SimpleFiles<'a> = (
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
);
type SimpleWorkspace<'a> = (
    &'a str,
    &'a str,
    &'a str,
    &'a [&'a str],
    &'a [(&'a str, &'a str)],
    &'a [(&'a str, &'a str)],
);

fn run_simple_conformance(
    plugin: &dyn LanguagePlugin,
    name: &str,
    src: &str,
    files: SimpleFiles<'_>,
    workspace: SimpleWorkspace<'_>,
) {
    let crlf = src.replace('\n', "\r\n");
    assert_seam_2_conformance(
        plugin,
        &ConformanceFixture {
            name,
            production_path: files.0,
            production_source: src,
            production_source_crlf: &crlf,
            test_path: files.2,
            test_source: files.3,
            empty_path: files.4,
            declaration_only: None,
            unparsable_path: files.5,
            unparsable_source: files.6,
            unsupported_path: "scripts/tool.unknown",
            repository_paths: workspace.3,
            expected_manifest_path: workspace.0,
            valid_manifests: workspace.4,
            expected_workspace_package: workspace.1,
            invalid_manifest_text: workspace.2,
            import_requests: workspace.5,
            expected_resolved_target: files.1,
        },
    );
}

#[test]
fn built_in_php_plugin_passes_seam_2_conformance() {
    const REPO: [&str; 4] = [
        "composer.json",
        "packages/app/composer.json",
        "packages/app/src/Main.php",
        "packages/app/src/Helper.php",
    ];
    const MANIFESTS: [(&str, &str); 2] = [
        (
            "composer.json",
            r#"{"repositories": [{"type": "path", "url": "packages/*"}]}"#,
        ),
        (
            "packages/app/composer.json",
            r#"{"name": "sample/app", "autoload": {"psr-4": {"Sample\\App\\": "src/"}}}"#,
        ),
    ];
    const REQUESTS: [(&str, &str); 3] = [
        ("packages/app/src/Main.php", "use:Sample\\App\\Helper"),
        ("packages/app/src/Main.php", "use:Illuminate\\Support\\Str"),
        ("packages/app/src/Main.php", "req:./Missing.php"),
    ];
    let prod_lf = "<?php\nnamespace Sample\\App;\nuse Sample\\App\\Helper;\n\n// comment\nfunction run(bool $flag): int {\n    if ($flag) {\n        return Helper::value();\n    } else {\n        return 0;\n    }\n}\n";
    let prod_crlf = prod_lf.replace('\n', "\r\n");
    let fixture = ConformanceFixture {
        name: "php",
        production_path: REPO[2],
        production_source: prod_lf,
        production_source_crlf: &prod_crlf,
        test_path: "packages/app/tests/MainTest.php",
        test_source: "<?php\nfunction check(): int { return 1; }\n",
        empty_path: "packages/app/src/Empty.php",
        declaration_only: None,
        unparsable_path: "packages/app/src/Broken.php",
        unparsable_source: "<?php\nfunction broken() { $x = 'unclosed; }\n",
        unsupported_path: "scripts/tool.unknown",
        repository_paths: &REPO,
        expected_manifest_path: REPO[0],
        valid_manifests: &MANIFESTS,
        expected_workspace_package: "packages/app",
        invalid_manifest_text: "{not json",
        import_requests: &REQUESTS,
        expected_resolved_target: REPO[3],
    };
    assert_seam_2_conformance(&Php, &fixture);
}

#[test]
fn built_in_dart_plugin_passes_seam_2_conformance() {
    let paths = [
        "pubspec.yaml",
        "packages/app/pubspec.yaml",
        "packages/app/lib/main.dart",
        "packages/app/lib/helper.dart",
    ];
    let manifests = [
        ("pubspec.yaml", "name: root\nworkspace:\n  - packages/app\n"),
        ("packages/app/pubspec.yaml", "name: app\n"),
    ];
    let requests = [
        ("packages/app/lib/main.dart", "package:app/helper.dart"),
        ("packages/app/lib/main.dart", "dart:core"),
        ("packages/app/lib/main.dart", "missing.dart"),
    ];
    let src = "import 'package:app/helper.dart';\n\n// comment\nint run(bool flag) {\n  if (flag) {\n    return value();\n  } else {\n    return 0;\n  }\n}\n";
    let src_crlf = src.replace('\n', "\r\n");
    assert_seam_2_conformance(
        &Dart,
        &ConformanceFixture {
            name: "dart",
            production_path: paths[2],
            production_source: src,
            production_source_crlf: &src_crlf,
            test_path: "packages/app/test/main_test.dart",
            test_source: "int check() => 1;\n",
            empty_path: "packages/app/lib/empty.dart",
            declaration_only: None,
            unparsable_path: "packages/app/lib/broken.dart",
            unparsable_source: "void broken() { final s = 'unclosed; }\n",
            unsupported_path: "scripts/tool.unknown",
            repository_paths: &paths,
            expected_manifest_path: paths[0],
            valid_manifests: &manifests,
            expected_workspace_package: "packages/app",
            invalid_manifest_text: "workspace: [\n",
            import_requests: &requests,
            expected_resolved_target: paths[3],
        },
    );
}

#[test]
fn built_in_python_plugin_passes_seam_2_conformance() {
    let prod = "from app.helper import value\n\n# comment\ndef run(flag):\n    if flag:\n        return value()\n    else:\n        return 0\n";
    let prod_crlf = prod.replace('\n', "\r\n");
    let fixture = ConformanceFixture {
        expected_resolved_target: "packages/app/src/app/helper.py",
        import_requests: &[
            ("packages/app/src/app/main.py", "app.helper"),
            ("packages/app/src/app/main.py", "os"),
            ("packages/app/src/app/main.py", ".missing"),
        ],
        invalid_manifest_text: "[tool.uv.workspace\n",
        expected_workspace_package: "packages/app",
        valid_manifests: &[
            (
                "pyproject.toml",
                "[tool.uv.workspace]\nmembers = [\"packages/*\"]\n",
            ),
            ("packages/app/pyproject.toml", "[project]\nname = \"app\"\n"),
        ],
        expected_manifest_path: "pyproject.toml",
        repository_paths: &[
            "pyproject.toml",
            "packages/app/pyproject.toml",
            "packages/app/src/app/main.py",
            "packages/app/src/app/helper.py",
        ],
        unsupported_path: "scripts/tool.unknown",
        unparsable_source: "def broken():\n    s = 'unclosed\n",
        unparsable_path: "packages/app/src/app/broken.py",
        declaration_only: None,
        empty_path: "packages/app/src/app/empty.py",
        test_source: "def check():\n    return 1\n",
        test_path: "packages/app/tests/test_main.py",
        production_source_crlf: &prod_crlf,
        production_source: prod,
        production_path: "packages/app/src/app/main.py",
        name: "python",
    };
    assert_seam_2_conformance(&Python, &fixture);
}
