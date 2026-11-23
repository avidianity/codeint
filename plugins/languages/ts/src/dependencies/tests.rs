use codeint_model::language::{DeclaredDep, DepKind, Manifest};

use super::extract_declared;

fn manifest(path: &str, text: &str) -> Manifest {
    Manifest {
        path: path.to_owned(),
        text: text.to_owned(),
    }
}

fn dep(package_root: &str, name: &str, kind: DepKind) -> DeclaredDep {
    DeclaredDep {
        package_root: package_root.to_owned(),
        name: name.to_owned(),
        kind,
    }
}

#[test]
fn ambient_types_packages_are_excluded_from_declared_dependencies() {
    let manifests = [manifest(
        "package.json",
        r#"{
          "name": "typed-app",
          "dependencies": {
            "zod": "^3.0.0",
            "@types/lodash": "^4.17.0"
          },
          "devDependencies": {
            "@types/node": "^22.0.0",
            "@types/react": "^18.3.0",
            "vitest": "^2.0.0"
          }
        }"#,
    )];

    assert_eq!(
        extract_declared(&manifests),
        vec![
            dep("", "zod", DepKind::Runtime),
            dep("", "vitest", DepKind::Dev),
        ]
    );
}

#[test]
fn script_invoked_dev_dependencies_classify_as_optional_while_unused_stay_dev() {
    let manifests = [manifest(
        "package.json",
        r#"{
          "name": "scripted-app",
          "scripts": {
            "lint": "eslint . && biome check .",
            "format": "prettier --write .",
            "typecheck": "NODE_ENV=test tsc --noEmit",
            "test": "cross-env CI=1 vitest run",
            "e2e": "playwright test",
            "release": "changeset publish",
            "duplicates": "./node_modules/.bin/jscpd@5.4.3 src",
            "docs": "markdownlint-cli2 '**/*.md'",
            "build": "concurrently \"svelte-package -i src\" \"tsup\"",
            "custom": "node scripts/unused-runner.mjs"
          },
          "dependencies": {
            "zod": "^3.0.0"
          },
          "devDependencies": {
            "eslint": "^9.0.0",
            "@biomejs/biome": "^1.9.0",
            "prettier": "^3.4.0",
            "typescript": "^5.7.0",
            "cross-env": "^7.0.3",
            "vitest": "^2.1.0",
            "@playwright/test": "^1.49.0",
            "@changesets/cli": "^2.27.0",
            "jscpd": "^5.4.3",
            "markdownlint-cli2": "^0.24.0",
            "concurrently": "^9.0.0",
            "@sveltejs/package": "^2.3.0",
            "tsup": "^8.3.0",
            "@unused/test": "^1.0.0",
            "unused-runner": "^1.0.0",
            "faker": "^8.0.0"
          }
        }"#,
    )];

    assert_eq!(
        extract_declared(&manifests),
        vec![
            dep("", "zod", DepKind::Runtime),
            dep("", "@unused/test", DepKind::Dev),
            dep("", "faker", DepKind::Dev),
            dep("", "unused-runner", DepKind::Dev),
            dep("", "@biomejs/biome", DepKind::Optional),
            dep("", "@changesets/cli", DepKind::Optional),
            dep("", "@playwright/test", DepKind::Optional),
            dep("", "@sveltejs/package", DepKind::Optional),
            dep("", "concurrently", DepKind::Optional),
            dep("", "cross-env", DepKind::Optional),
            dep("", "eslint", DepKind::Optional),
            dep("", "jscpd", DepKind::Optional),
            dep("", "markdownlint-cli2", DepKind::Optional),
            dep("", "prettier", DepKind::Optional),
            dep("", "tsup", DepKind::Optional),
            dep("", "typescript", DepKind::Optional),
            dep("", "vitest", DepKind::Optional),
        ]
    );
}

#[test]
fn root_dev_dependencies_match_workspace_member_scripts_but_not_non_member_scripts() {
    let manifests = [
        manifest(
            "package.json",
            r#"{
              "name": "mono-root",
              "private": true,
              "workspaces": ["packages/*"],
              "devDependencies": {
                "vitest": "^2.0.0",
                "typedoc": "^0.26.0",
                "unused-root": "^1.0.0"
              }
            }"#,
        ),
        manifest(
            "packages/core/package.json",
            r#"{
              "name": "@mono/core",
              "scripts": {
                "test": "vitest run"
              }
            }"#,
        ),
        manifest(
            "examples/standalone/package.json",
            r#"{
              "name": "standalone-example",
              "scripts": {
                "docs": "typedoc"
              }
            }"#,
        ),
    ];

    assert_eq!(
        extract_declared(&manifests),
        vec![
            dep("", "typedoc", DepKind::Dev),
            dep("", "unused-root", DepKind::Dev),
            dep("", "vitest", DepKind::Optional),
        ]
    );
}
