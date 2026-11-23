use codeint_lang_ts::TypeScriptAndJavaScript;
use codeint_model::{LanguagePlugin, Manifest, RepoPath};

#[test]
fn extracts_main_module_bin_and_exports_from_package_json() {
    let repo: Vec<RepoPath> = [
        "package.json",
        "tsconfig.json",
        "pnpm-workspace.yaml",
        "src/index.ts",
        "src/esm.mts",
        "src/cli.ts",
        "src/alt-cli.js",
        "src/button.tsx",
        "src/fallback.ts",
        "src/features/auth.ts",
        "src/features/billing.ts",
        "src/features/auth.d.ts",
        "dist/index.d.ts",
        "bin/tool",
        "packages/cli/package.json",
        "packages/cli/src/entry.ts",
        "packages/cli/src/esm.ts",
        "packages/cli/src/dir/index.ts",
        "packages/cli/bin/run.mjs",
    ]
    .map(|p| RepoPath::new(p).unwrap())
    .into();
    let root_pkg = r#"{
        "name": "demo",
        "main": "./src/index.ts",
        "module": "./src/esm.mts",
        "bin": {
            "demo-cli": "./src/cli.ts",
            "alt": "src/alt-cli.js",
            "raw": "./bin/tool",
            "missing": "./bin/absent.js"
        },
        "exports": {
            ".": {
                "types": "./dist/index.d.ts",
                "import": "./src/index.ts",
                "default": "./dist/missing.js"
            },
            "./button": {
                "bun": "./src/button.tsx"
            },
            "./fallback": ["./dist/nope.js", "./src/fallback.ts"],
            "./features/*": "./src/features/*.js",
            "./internal/*": null,
            "./package.json": "./package.json",
            "./escape": "../outside.ts",
            "./abs": "/etc/passwd"
        }
    }"#;
    let cli_pkg = r#"{
        "name": "@demo/cli",
        "main": "./src/entry.js",
        "module": "./src/esm",
        "bin": "./bin/run.mjs",
        "exports": "./src/dir"
    }"#;
    let manifests = vec![
        Manifest {
            path: "package.json".into(),
            text: root_pkg.into(),
        },
        Manifest {
            path: "tsconfig.json".into(),
            text: r#"{"compilerOptions": {}}"#.into(),
        },
        Manifest {
            path: "pnpm-workspace.yaml".into(),
            text: "packages:\n  - packages/*\n".into(),
        },
        Manifest {
            path: "packages/cli/package.json".into(),
            text: cli_pkg.into(),
        },
    ];

    let expected: Vec<RepoPath> = [
        "bin/tool",
        "packages/cli/bin/run.mjs",
        "packages/cli/src/dir/index.ts",
        "packages/cli/src/entry.ts",
        "packages/cli/src/esm.ts",
        "src/alt-cli.js",
        "src/button.tsx",
        "src/cli.ts",
        "src/esm.mts",
        "src/fallback.ts",
        "src/features/auth.ts",
        "src/features/billing.ts",
        "src/index.ts",
    ]
    .into_iter()
    .map(|entry| RepoPath::new(entry).unwrap())
    .collect();
    assert_eq!(
        TypeScriptAndJavaScript
            .entry_points(&repo, manifests)
            .expect("valid entry points"),
        expected
    );
}

#[test]
fn returns_empty_when_package_json_declares_no_entry_points_or_is_invalid() {
    let repo: Vec<RepoPath> = ["package.json", "src/index.ts", "src/helper.ts"]
        .map(|p| RepoPath::new(p).unwrap())
        .into();
    for manifests in [
        vec![Manifest {
            path: "package.json".into(),
            text: r#"{"name": "webapp", "dependencies": {"react": "19.0.0"}}"#.into(),
        }],
        vec![Manifest {
            path: "package.json".into(),
            text: "{not valid json".into(),
        }],
        Vec::new(),
    ] {
        assert!(
            TypeScriptAndJavaScript
                .entry_points(&repo, manifests)
                .expect("valid empty result")
                .is_empty()
        );
    }
}
