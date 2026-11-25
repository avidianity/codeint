use std::collections::BTreeMap;
use std::fs;
use std::process::Command;

use serde_json::{Value, json};
use tempfile::TempDir;

struct TsFixture {
    dir: TempDir,
}

impl TsFixture {
    fn from_bundle(bundle: &str) -> Self {
        let fixture = Self {
            dir: TempDir::new().expect("tempdir"),
        };
        let mut current: Option<(&str, String)> = None;
        for line in bundle.lines() {
            if let Some(path) = line.strip_prefix("--- ") {
                if let Some((prev, body)) = current.replace((path.trim(), String::new())) {
                    fixture.write(prev, &body);
                }
            } else if let Some((_, body)) = current.as_mut() {
                body.push_str(line);
                body.push('\n');
            }
        }
        if let Some((prev, body)) = current {
            fixture.write(prev, &body);
        }
        let ok = Command::new("git")
            .current_dir(fixture.dir.path())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args(["init", "-q", "-b", "main"])
            .status()
            .expect("git init")
            .success();
        assert!(ok, "git init failed");
        fixture
    }

    fn write(&self, rel: &str, content: &str) {
        let target = self.dir.path().join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(target, content).expect("write fixture file");
    }

    fn run_check(&self) -> Value {
        let output = Command::new(env!("CARGO_BIN_EXE_codeint"))
            .current_dir(self.dir.path())
            .args(["check", "--format", "json"])
            .output()
            .expect("run codeint check");
        assert!(
            output.status.success(),
            "codeint check failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("parse check json")
    }
}

struct ExpectedCheck<'a> {
    profiles: &'a [&'a str],
    adr: Option<&'a str>,
    orphans: &'a [&'a str],
    generated_files: u64,
}

fn assert_framework_check(report: &Value, expected: &ExpectedCheck<'_>) {
    assert_eq!(report["framework_profiles"], json!(expected.profiles));
    assert!(report.get("reference_range_note").is_none());
    if let Some(adr) = expected.adr {
        let source = report
            .pointer("/source_sets/production/erosion/reference_range/source")
            .and_then(Value::as_str)
            .expect("erosion source");
        assert!(source.contains(adr), "expected {adr} in {source}");
    }
    assert_eq!(
        report
            .pointer("/source_sets/production/import_cycles/unresolved_imports/count")
            .and_then(Value::as_u64),
        Some(0),
        "unexpected unresolved imports: {report}"
    );
    let unlisted = report
        .pointer("/dependencies/unlisted_dependencies/findings")
        .and_then(Value::as_array)
        .expect("unlisted findings");
    assert!(
        unlisted.is_empty(),
        "unexpected unlisted deps: {unlisted:?}"
    );
    let orphans: Vec<&str> = report
        .pointer("/source_sets/production/orphan_files/findings")
        .and_then(Value::as_array)
        .expect("orphan findings")
        .iter()
        .filter_map(|item| item["path"].as_str())
        .collect();
    assert_eq!(orphans, expected.orphans);
    assert_eq!(
        report.pointer("/source_sets/generated/files"),
        Some(&json!(expected.generated_files))
    );
}

fn package_profiles_map(report: &Value) -> BTreeMap<String, Option<Vec<String>>> {
    report
        .pointer("/source_sets/production/packages")
        .and_then(Value::as_array)
        .expect("packages list")
        .iter()
        .map(|entry| {
            let path = entry["path"].as_str().expect("package path").to_owned();
            let profiles = entry.get("framework_profiles").and_then(|val| {
                val.as_array().map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_owned))
                        .collect()
                })
            });
            (path, profiles)
        })
        .collect()
}

#[test]
fn nextjs_profile_wires_conventions_generated_priority_aliases_and_react_ranges() {
    let fixture = TsFixture::from_bundle(
        r#"
--- codeint.json
{"source_sets":{"production":[".next/forced_prod.ts"]}}
--- package.json
{"name":"next-app","dependencies":{"next":"^15.0.0","react":"^19.0.0","react-dom":"^19.0.0"}}
--- next.config.ts
export default {};
--- src/middleware.ts
export function middleware() { return 1; }
--- src/app/layout.tsx
export default function Layout() { return <main />; }
--- src/app/page.tsx
import { fmt } from '@/lib/fmt';
import { Hero } from '~/components/Hero';
export default function Page() { return <Hero label={fmt()} />; }
--- src/pages/api/health.ts
export default function handler() { return 'ok'; }
--- src/lib/fmt.ts
export function fmt(): string { return 'hi'; }
--- components/Hero.tsx
export function Hero(props: { label: string }) { return <h1>{props.label}</h1>; }
--- src/app/_components/dead.ts
export function dead(): number { return 0; }
--- .next/types/generated.ts
export function gen(): number { return 1; }
--- .next/vendor/third.ts
export function vendored(): number { return 2; }
--- .next/forced_prod.ts
export function forced(): number { return 3; }
"#,
    );

    let report = fixture.run_check();
    assert_framework_check(
        &report,
        &ExpectedCheck {
            profiles: &["nextjs", "react"],
            adr: Some("ADR 0020"),
            orphans: &[".next/forced_prod.ts", "src/app/_components/dead.ts"],
            generated_files: 1,
        },
    );
    assert_eq!(
        report.pointer("/source_sets/vendored/files"),
        Some(&json!(1))
    );
}

#[test]
fn nuxt_profile_wires_conventions_generated_paths_virtual_modules_and_vue3_ranges() {
    let fixture = TsFixture::from_bundle(
        r#"
--- package.json
{"name":"nuxt-app","dependencies":{"nuxt":"^4.0.0","vue":"^3.5.0"}}
--- nuxt.config.ts
export default {};
--- app/app.vue
<script setup lang="ts">
const title = 'ok';
</script>
--- app/pages/index.vue
<script setup lang="ts">
import { ref } from '#imports';
import { Nav } from '#components';
import { useNuxtApp } from '#app';
import { routes } from '#build/routes';
import { fmt } from '~/utils/fmt';
import { sharedVal } from '@@/shared/util';
export const state = [ref, Nav, useNuxtApp, routes, fmt(), sharedVal()];
</script>
--- app/utils/fmt.ts
export function fmt(): string { return 'v'; }
--- shared/util.ts
export function sharedVal(): number { return 7; }
--- server/api/ping.ts
export default function ping() { return 'pong'; }
--- app/lib/dead.ts
export function dead(): number { return 0; }
--- .nuxt/auto.ts
export function autoGen(): number { return 1; }
--- .output/server/entry.js
export function built() { return 2; }
"#,
    );

    let report = fixture.run_check();
    assert_framework_check(
        &report,
        &ExpectedCheck {
            profiles: &["nuxt", "vue3"],
            adr: Some("ADR 0015"),
            orphans: &["app/lib/dead.ts"],
            generated_files: 2,
        },
    );

    fixture.write("codeint.json", r#"{"framework_profiles":false}"#);
    let disabled = fixture.run_check();
    assert!(disabled.get("framework_profiles").is_none());
    let unresolved = disabled
        .pointer("/source_sets/production/import_cycles/unresolved_imports/count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    assert!(unresolved > 0, "expected #imports unresolved when disabled");
}

#[test]
fn sveltekit_profile_wires_routes_hooks_generated_paths_virtual_modules_and_svelte_union() {
    let fixture = TsFixture::from_bundle(
        r#"
--- package.json
{"name":"kit-app","devDependencies":{"@sveltejs/kit":"^2.8.0","svelte":"^5.0.0"}}
--- svelte.config.js
export default {};
--- src/app.html
<html></html>
--- src/hooks.server.ts
export function handle() { return 1; }
--- src/service-worker.ts
import { build } from '$service-worker';
export const assets = build;
--- src/routes/+page.svelte
<script lang="ts">
import { goto } from '$app/navigation';
import { PUBLIC_API } from '$env/static/public';
import { fmt } from '$lib/format';
export const run = () => [goto, PUBLIC_API, fmt()];
</script>
--- src/routes/+page.server.ts
export function load() { return {}; }
--- src/lib/format.ts
export function fmt(): string { return 'ok'; }
--- src/routes/unused.ts
export function dead(): number { return 0; }
--- .svelte-kit/generated/client.ts
export function gen(): number { return 1; }
"#,
    );

    let report = fixture.run_check();
    assert_framework_check(
        &report,
        &ExpectedCheck {
            profiles: &["svelte", "sveltekit"],
            adr: None,
            orphans: &["src/routes/unused.ts"],
            generated_files: 1,
        },
    );
}

#[test]
fn angular_nestjs_monorepo_enforces_hoisting_isolation_and_peer_library_safety() {
    let fixture = TsFixture::from_bundle(
        r#"
--- package.json
{"name":"mono","private":true,"workspaces":["packages/*"],"dependencies":{"next":"^15.0.0","nuxt":"^4.0.0","@sveltejs/kit":"^2.8.0","@angular/core":"^19.0.0","@nestjs/core":"^11.0.0"}}
--- packages/web-ng/package.json
{"name":"@mono/web-ng","dependencies":{"@angular/core":"^19.0.0"}}
--- packages/web-ng/angular.json
{}
--- packages/web-ng/src/main.ts
export const boot = 1;
--- packages/web-ng/src/app/app.config.ts
export const appConfig = {};
--- packages/web-ng/src/app/app.routes.ts
export const routes = [];
--- packages/web-ng/src/app/dead.ts
export function dead(): number { return 0; }
--- packages/web-ng/.angular/cache/gen.ts
export const cached = 1;
--- packages/api-nest/package.json
{"name":"@mono/api-nest","dependencies":{"@nestjs/core":"^11.0.0"}}
--- packages/api-nest/nest-cli.json
{}
--- packages/api-nest/src/main.ts
export function bootstrap() { return 1; }
--- packages/api-nest/src/app.module.ts
export class AppModule {}
--- packages/api-nest/src/dead.service.ts
export class DeadService {}
--- packages/api-nest/dist/main.js
export const compiled = 1;
--- packages/ng-lib/package.json
{"name":"@mono/ng-lib","main":"src/index.ts","peerDependencies":{"@angular/core":"^19.0.0"}}
--- packages/ng-lib/src/index.ts
export const lib = 1;
--- packages/ng-lib/src/main.ts
export const stray = 2;
--- packages/nest-lib/package.json
{"name":"@mono/nest-lib","main":"src/index.ts","peerDependencies":{"@nestjs/core":"^11.0.0"}}
--- packages/nest-lib/src/index.ts
export const lib = 1;
--- packages/nest-lib/src/main.ts
export const stray = 2;
--- packages/plain/package.json
{"name":"@mono/plain","main":"src/index.ts"}
--- packages/plain/src/index.ts
export const plain = 1;
"#,
    );

    let report = fixture.run_check();
    assert_framework_check(
        &report,
        &ExpectedCheck {
            profiles: &["angular", "nestjs"],
            adr: None,
            orphans: &[
                "packages/api-nest/src/dead.service.ts",
                "packages/nest-lib/src/main.ts",
                "packages/ng-lib/src/main.ts",
                "packages/web-ng/src/app/dead.ts",
            ],
            generated_files: 2,
        },
    );
    let by_pkg = package_profiles_map(&report);
    assert_eq!(
        by_pkg.get("packages/web-ng"),
        Some(&Some(vec!["angular".to_owned()]))
    );
    assert_eq!(
        by_pkg.get("packages/api-nest"),
        Some(&Some(vec!["nestjs".to_owned()]))
    );
    for isolated in ["packages/ng-lib", "packages/nest-lib", "packages/plain"] {
        assert_eq!(by_pkg.get(isolated), Some(&None), "{isolated}");
    }
}
