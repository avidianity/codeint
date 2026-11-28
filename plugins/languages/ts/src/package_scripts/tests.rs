use std::collections::HashSet;

use codeint_model::language::{DeclaredDep, DepKind, PackageDependency};
use serde_json::Value;

use super::{
    extract_commands_from_script, extract_manifest_used_dependencies, normalize_command_token,
};

fn parsed_manifest<'a>(path: &'a str, text: &str) -> (&'a str, &'a str, Value) {
    let root = crate::workspaces::package_manifest_root(path).expect("valid package manifest");
    let doc: Value = serde_json::from_str(text).expect("valid json");
    (root, path, doc)
}

fn dep(package_root: &str, name: &str, kind: DepKind) -> DeclaredDep {
    DeclaredDep {
        package_root: package_root.to_owned(),
        name: name.to_owned(),
        kind,
    }
}

fn used(package_root: &str, name: &str) -> PackageDependency {
    PackageDependency {
        package_root: package_root.to_owned(),
        name: name.to_owned(),
    }
}

fn commands_of(script: &str) -> Vec<String> {
    let mut out = HashSet::new();
    extract_commands_from_script(script, &mut out, 0);
    let mut sorted: Vec<String> = out.into_iter().collect();
    sorted.sort();
    sorted
}

#[test]
fn parses_command_position_only_and_ignores_arguments_quotes_comments_and_redirections() {
    assert_eq!(commands_of(r#"echo "skip vitest""#), vec!["echo"]);
    assert_eq!(commands_of("echo 'skip jest'"), vec!["echo"]);
    assert_eq!(commands_of("rm -rf storybook"), vec!["rm"]);
    assert_eq!(commands_of("vitest # old jest"), vec!["vitest"]);
    assert_eq!(
        commands_of(r#"GREETING="hello jest" vitest run"#),
        vec!["vitest"]
    );
    assert_eq!(commands_of("vitest>out.log"), vec!["vitest"]);
    assert_eq!(
        commands_of("vitest 2>&1 | tee log.txt"),
        vec!["tee", "vitest"]
    );
    assert_eq!(
        commands_of("vitest > out.log && tsc --noEmit"),
        vec!["tsc", "vitest"]
    );
    assert_eq!(
        commands_of("(cd packages/web && vite build); eslint ."),
        vec!["cd", "eslint", "vite"]
    );
    assert_eq!(commands_of("node scripts/unused-runner.mjs"), vec!["node"]);
}

#[test]
fn unwraps_runners_and_records_runner_and_wrapped_commands() {
    assert_eq!(
        commands_of("cross-env CI=1 vitest run"),
        vec!["cross-env", "vitest"]
    );
    assert_eq!(
        commands_of("env -u FOO BAR=1 eslint ."),
        vec!["env", "eslint"]
    );
    assert_eq!(
        commands_of("npx --yes --no-install prettier --check ."),
        vec!["npx", "prettier"]
    );
    assert_eq!(commands_of("pnpx biome check ."), vec!["biome", "pnpx"]);
    assert_eq!(commands_of("bunx --bun vitest run"), vec!["bunx", "vitest"]);
    assert_eq!(
        commands_of("pnpm --filter @app/web exec vitest run"),
        vec!["pnpm", "vitest"]
    );
    assert_eq!(commands_of("yarn exec tsc --noEmit"), vec!["tsc", "yarn"]);
    assert_eq!(
        commands_of("npm exec --yes -- eslint ."),
        vec!["eslint", "npm"]
    );
    assert_eq!(
        commands_of("pnpm dlx prettier --write ."),
        vec!["pnpm", "prettier"]
    );
    assert_eq!(commands_of("yarn dlx oxlint ."), vec!["oxlint", "yarn"]);
    assert_eq!(
        commands_of(r#"concurrently -n a,b -k "svelte-package -i src" "tsup""#),
        vec!["concurrently", "svelte-package", "tsup"]
    );
    assert_eq!(
        commands_of(r#"sh -c "vitest run && tsc --noEmit""#),
        vec!["sh", "tsc", "vitest"]
    );
    assert_eq!(
        commands_of(r#"bash -ec "eslint .""#),
        vec!["bash", "eslint"]
    );
    assert_eq!(commands_of("run-p lint typecheck"), vec!["run-p"]);
    assert_eq!(
        commands_of("time nice -n 10 nohup vite &"),
        vec!["nice", "nohup", "time", "vite"]
    );
}

#[test]
fn normalizes_node_modules_bin_prefixes_and_valid_package_versions_only() {
    assert_eq!(
        normalize_command_token("./node_modules/.bin/jscpd@5.4.3"),
        "jscpd"
    );
    assert_eq!(normalize_command_token("node_modules/.bin/tsc"), "tsc");
    assert_eq!(
        normalize_command_token("../../node_modules/.bin/vitest"),
        "vitest"
    );
    assert_eq!(
        normalize_command_token("@biomejs/biome@1.9.0"),
        "@biomejs/biome"
    );
    assert_eq!(normalize_command_token("vitest@latest"), "vitest");
    assert_eq!(normalize_command_token("foo@bar"), "foo@bar");
    assert_eq!(
        normalize_command_token("user@example.com"),
        "user@example.com"
    );
}

#[test]
fn matches_dev_dependencies_across_root_and_workspace_members_only() {
    let manifests = [
        parsed_manifest(
            "package.json",
            r#"{
              "name": "mono-root",
              "scripts": {
                "lint": "eslint . && biome check .",
                "typecheck": "NODE_ENV=test tsc --noEmit",
                "echo": "echo \"skip jest\" && rm -rf storybook",
                "run": "node scripts/unused-runner.mjs"
              }
            }"#,
        ),
        parsed_manifest(
            "packages/core/package.json",
            r#"{
              "name": "@mono/core",
              "scripts": {
                "test": "cross-env CI=1 vitest run # old mocha",
                "build": "concurrently \"svelte-package -i src\" \"tsup\""
              }
            }"#,
        ),
        parsed_manifest(
            "examples/standalone/package.json",
            r#"{
              "name": "standalone",
              "scripts": {
                "docs": "typedoc"
              }
            }"#,
        ),
    ];
    let members = ["packages/core".to_owned()];
    let declared = [
        dep("", "zod", DepKind::Runtime),
        dep("", "eslint", DepKind::Runtime),
        dep("", "eslint", DepKind::Dev),
        dep("", "@biomejs/biome", DepKind::Dev),
        dep("", "typescript", DepKind::Dev),
        dep("", "cross-env", DepKind::Dev),
        dep("", "vitest", DepKind::Dev),
        dep("", "concurrently", DepKind::Dev),
        dep("", "@sveltejs/package", DepKind::Dev),
        dep("", "tsup", DepKind::Dev),
        dep("", "jest", DepKind::Dev),
        dep("", "@storybook/cli", DepKind::Dev),
        dep("", "mocha", DepKind::Dev),
        dep("", "typedoc", DepKind::Dev),
        dep("", "@org/node", DepKind::Dev),
        dep("", "@org/rm", DepKind::Dev),
        dep("", "@org/echo", DepKind::Dev),
        dep("", "unused-runner", DepKind::Dev),
        dep("examples/standalone", "typedoc", DepKind::Dev),
    ];

    assert_eq!(
        extract_manifest_used_dependencies(&manifests, &members, &declared),
        vec![
            used("", "@biomejs/biome"),
            used("", "@sveltejs/package"),
            used("", "concurrently"),
            used("", "cross-env"),
            used("", "eslint"),
            used("", "tsup"),
            used("", "typescript"),
            used("", "vitest"),
            used("examples/standalone", "typedoc"),
        ]
    );
}
