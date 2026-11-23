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

fn discover_ts(paths: &[&str], manifests: &[(&str, &str)]) -> Vec<String> {
    let repo: Vec<RepoPath> = paths.iter().map(|p| RepoPath::new(*p).unwrap()).collect();
    let specs = manifests
        .iter()
        .map(|(path, text)| Manifest {
            path: (*path).into(),
            text: (*text).into(),
        })
        .collect();
    TypeScriptAndJavaScript
        .entry_points(&repo, specs)
        .unwrap()
        .into_iter()
        .map(|entry| entry.as_str().to_owned())
        .collect()
}

#[test]
fn rejects_parent_traversal_in_nested_package_json_entries() {
    let app_pkg = r#"{
        "name": "@demo/app",
        "main": "../other/secret.ts",
        "module": "./../other/secret.ts",
        "bin": { "bad": "../other/bin.js" },
        "exports": {
            ".": "./src/index.ts",
            "./escape": "../other/secret.ts",
            "./wild/*": "../other/wild/*.ts",
            "./dotdot/*": "./../*.ts"
        }
    }"#;
    let found = discover_ts(
        &[
            "packages/app/package.json",
            "packages/app/src/index.ts",
            "packages/other/secret.ts",
            "packages/other/bin.js",
            "packages/other/wild/a.ts",
        ],
        &[("packages/app/package.json", app_pkg)],
    );
    assert_eq!(found, ["packages/app/src/index.ts"]);
}

#[test]
fn subtracts_null_excluded_subpaths_from_wildcard_exports() {
    let root_pkg = r#"{
        "name": "demo",
        "exports": {
            "./*": { "import": "./src/*.js" },
            "./secret": null,
            "./internal/*": null,
            "./internal/public/*": "./src/internal/public/*.ts"
        }
    }"#;
    let lib_pkg = r#"{
        "name": "@demo/lib",
        "exports": {
            "./*.js": "./src/*.ts",
            "./private/*.js": null
        }
    }"#;
    let found = discover_ts(
        &[
            "package.json",
            "src/public.ts",
            "src/secret.ts",
            "src/internal/helper.ts",
            "src/internal/deep/util.ts",
            "src/internal/public/api.ts",
            "packages/lib/package.json",
            "packages/lib/src/open.ts",
            "packages/lib/src/private/hidden.ts",
        ],
        &[
            ("package.json", root_pkg),
            ("packages/lib/package.json", lib_pkg),
        ],
    );
    assert_eq!(
        found,
        [
            "packages/lib/src/open.ts",
            "src/internal/public/api.ts",
            "src/public.ts",
        ]
    );
}
