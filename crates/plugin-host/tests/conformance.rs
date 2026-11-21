mod common;

use std::path::Path;

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
    declaration_only_path: &'a str,
    declaration_only_source: &'a str,
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

    let decl = plugin.lower(
        &RepoPath::new(fixture.declaration_only_path).unwrap(),
        fixture.declaration_only_source,
    );
    assert!(
        matches!(decl, Err(LowerError::DeclarationOnly)),
        "{label}: expected DeclarationOnly, got {decl:?}"
    );

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
    let resolutions = plugin.resolve_imports(&repo_paths, valid_manifest, requests);
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
    assert_eq!(
        resolutions[1],
        Resolution::External,
        "{label}: second import is external"
    );
    assert!(
        matches!(resolutions[2], Resolution::Unresolved(_)),
        "{label}: third import is unresolved, got {:?}",
        resolutions[2]
    );

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
        declaration_only_path: "packages/app/src/types.d.ts",
        declaration_only_source: "export declare const version: string;\n",
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
    let plugin = load_language_plugin(Path::new("."), &declaration)
        .expect("sample external wasm plugin loads");

    let fixture = ConformanceFixture {
        name: "sample-mini",
        production_path: "packages/app/src/main.mini",
        production_source: "import \"./helper\"\n\n# comment\nfn run(flag) {\n  if flag {\n    pass;\n  }\n}\n",
        production_source_crlf: "import \"./helper\"\r\n\r\n# comment\r\nfn run(flag) {\r\n  if flag {\r\n    pass;\r\n  }\r\n}\r\n",
        test_path: "packages/app/src/main.test.mini",
        test_source: "fn check() {\n  return 1\n}\n",
        empty_path: "packages/app/src/empty.mini",
        declaration_only_path: "packages/app/src/types.d.mini",
        declaration_only_source: "fn extern_only() {}\n",
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
