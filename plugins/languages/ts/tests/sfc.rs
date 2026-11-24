use codeint_lang_ts::{TypeScriptAndJavaScript, react_reference_ranges, vue_reference_ranges};
use codeint_model::language::{ImportRequest, Manifest, Resolution};
use codeint_model::{LanguagePlugin, LowerError, RepoPath, SourceSet};

#[test]
fn vue_2_options_api_extracts_script_and_preserves_line_numbers() {
    let src = "<template>\n  <div>{{ count }}</div>\n</template>\n\n<script>\nimport { helper } from './helper';\nexport default {\n  data() {\n    return { count: 0 };\n  },\n  methods: {\n    check(x) {\n      if (x > 0) {\n        return true;\n      } else {\n        return false;\n      }\n    }\n  }\n};\n</script>\n\n<style scoped>\n.box { color: red; }\n</style>\n";
    let model = TypeScriptAndJavaScript
        .lower(&RepoPath::new("src/Counter.vue").unwrap(), src)
        .unwrap();
    assert_eq!(model.source_set, SourceSet::Production);
    assert_eq!(model.runtime_imports, ["./helper"]);
    assert_eq!(model.functions.len(), 2);
    assert_eq!(model.functions[0].line, 8);
    assert_eq!(model.functions[0].name.as_deref(), Some("data"));
    assert_eq!(model.functions[0].cognitive_complexity, 0);
    assert_eq!(model.functions[1].line, 12);
    assert_eq!(model.functions[1].name.as_deref(), Some("check"));
    assert_eq!(model.functions[1].cognitive_complexity, 2);
    assert!(
        model
            .flagged_lines
            .iter()
            .any(|f| f.rule == "if-else-returns-boolean" && f.line == 13)
    );
}

#[test]
fn vue_3_dual_script_setup_ts_merges_both_blocks() {
    let src = "<script lang=\"ts\">\nimport { shared } from './shared';\nexport const version: number = shared();\n</script>\n\n<script setup lang=\"ts\">\nimport Child from './Child.vue';\nconst props = defineProps<{ count: number }>();\nfunction doubled(n: number): number {\n  return n > 0 ? n * 2 : 0;\n}\n</script>\n\n<template><Child :n=\"doubled(props.count)\" /></template>\n";
    let model = TypeScriptAndJavaScript
        .lower(&RepoPath::new("src/App.vue").unwrap(), src)
        .unwrap();
    assert_eq!(model.runtime_imports, ["./shared", "./Child.vue"]);
    assert_eq!(model.functions.len(), 1);
    assert_eq!(model.functions[0].line, 9);
    assert_eq!(model.functions[0].name.as_deref(), Some("doubled"));
    assert_eq!(model.functions[0].decision_points, 1);
    assert_eq!(model.functions[0].cognitive_complexity, 1);
}

#[test]
fn svelte_module_and_instance_scripts_and_cross_file_resolution() {
    let src = "<script context=\"module\" lang=\"ts\">\nexport const prerender = true;\n</script>\n<script lang=\"ts\">\nimport Panel from './Panel.svelte';\nlet count = 0;\n$: doubled = count > 0 ? count * 2 : 0;\nfunction inc() {\n  count += 1;\n}\n</script>\n<Panel {doubled} on:click={inc} />\n";
    let model = TypeScriptAndJavaScript
        .lower(&RepoPath::new("src/Page.svelte").unwrap(), src)
        .unwrap();
    assert_eq!(model.runtime_imports, ["./Panel.svelte"]);
    assert_eq!(model.functions.len(), 1);
    assert_eq!(model.functions[0].line, 8);

    let files = [
        "src/App.vue",
        "src/Child.vue",
        "src/Page.svelte",
        "src/Panel.svelte",
        "src/helper.ts",
    ]
    .map(|p| RepoPath::new(p).unwrap());
    let reqs = vec![
        ImportRequest {
            importer: "src/App.vue".into(),
            specifier: "./Child.vue".into(),
        },
        ImportRequest {
            importer: "src/App.vue".into(),
            specifier: "./helper".into(),
        },
        ImportRequest {
            importer: "src/Page.svelte".into(),
            specifier: "./Panel.svelte".into(),
        },
    ];
    let res = TypeScriptAndJavaScript.resolve_imports(&files, vec![], reqs);
    assert_eq!(res[0], Resolution::Resolved("src/Child.vue".into()));
    assert_eq!(res[1], Resolution::Resolved("src/helper.ts".into()));
    assert_eq!(res[2], Resolution::Resolved("src/Panel.svelte".into()));
}

#[test]
fn sfc_without_script_is_empty_and_unclosed_script_is_unparsable() {
    let empty = TypeScriptAndJavaScript
        .lower(
            &RepoPath::new("src/Icon.vue").unwrap(),
            "<template><svg /></template>\n<style>.i {}</style>\n",
        )
        .unwrap();
    assert_eq!(empty.source_lines, 0);
    assert!(empty.functions.is_empty());

    let broken = TypeScriptAndJavaScript.lower(
        &RepoPath::new("src/Broken.vue").unwrap(),
        "<template><div /></template>\n<script>\nconst x = 1;\n",
    );
    assert!(
        matches!(broken, Err(LowerError::Unparsable { message }) if message.contains("line 2"))
    );

    let unicode = TypeScriptAndJavaScript
        .lower(
            &RepoPath::new("src/Unicode.vue").unwrap(),
            "<template><中文>你好</中文></template>\n<script>\nexport const ok = 1;\n</script>\n",
        )
        .unwrap();
    assert_eq!(unicode.source_lines, 1);
}

fn manifest(path: &str, text: &str) -> Manifest {
    Manifest {
        path: path.into(),
        text: text.into(),
    }
}

#[test]
fn vue_reference_ranges_detects_vue_2_and_vue_3_from_package_json() {
    let v2 = vue_reference_ranges(&[manifest(
        "package.json",
        r#"{"dependencies":{"vue":"^2.7.16"}}"#,
    )])
    .unwrap();
    assert!(v2[0].source.contains("ADR 0014"));

    let v3 = vue_reference_ranges(&[manifest(
        "package.json",
        r#"{"dependencies":{"vue":"^3.4.0"}}"#,
    )])
    .unwrap();
    assert!(v3[0].source.contains("ADR 0015"));

    assert!(
        vue_reference_ranges(&[manifest(
            "package.json",
            r#"{"dependencies":{"react":"^18.0.0"}}"#
        )])
        .is_none()
    );
    assert!(
        vue_reference_ranges(&[manifest(
            "node_modules/foo/package.json",
            r#"{"dependencies":{"vue":"^3.4.0"}}"#,
        )])
        .is_none()
    );
    assert!(
        vue_reference_ranges(&[manifest(
            "packages/app/node_modules/foo/package.json",
            r#"{"dependencies":{"vue":"^2.7.16"}}"#,
        )])
        .is_none()
    );
}

#[test]
fn react_reference_ranges_detects_react_manifests_and_skips_node_modules() {
    let root = react_reference_ranges(&[manifest(
        "package.json",
        r#"{"dependencies":{"react":"^18.3.1"}}"#,
    )])
    .unwrap();
    let by_metric: Vec<(&str, f64, f64)> = root
        .iter()
        .map(|r| (r.metric.as_str(), r.low, r.high))
        .collect();
    assert_eq!(
        by_metric,
        [
            ("erosion", 0.106, 0.544),
            ("verbosity", 0.047, 0.368),
            ("duplication", 0.046, 0.368),
            ("import_cycles", 0.000, 0.144),
        ]
    );
    assert!(root.iter().all(|r| r.source.contains("ADR 0020")));

    for section in [
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ] {
        let found = react_reference_ranges(&[manifest(
            "packages/web/package.json",
            &format!(r#"{{"{section}":{{"react":">=17"}}}}"#),
        )]);
        assert!(found.is_some(), "expected React ranges for {section}");
    }

    assert!(
        react_reference_ranges(&[manifest(
            "node_modules/foo/package.json",
            r#"{"dependencies":{"react":"^18.0.0"}}"#,
        )])
        .is_none()
    );
    assert!(react_reference_ranges(&[manifest("package.json", "{not valid json")]).is_none());
    assert!(
        react_reference_ranges(&[manifest(
            "package.json",
            r#"{"dependencies":{"vue":"^3.4.0"}}"#
        )])
        .is_none()
    );
}
