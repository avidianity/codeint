use codeint_lang_ts::TypeScriptAndJavaScript;
use codeint_model::language::{ImportRequest, Resolution};
use codeint_model::{LanguagePlugin, LowerError, RepoPath, SourceSet};

#[test]
fn vue_2_options_api_extracts_script_and_preserves_line_numbers() {
    let src = "<template>\n  <div>{{ count }}</div>\n</template>\n\n<script>\nimport { helper } from './helper';\nexport default {\n  data() {\n    return { count: 0 };\n  },\n  methods: {\n    check(x) {\n      if (x > 0) {\n        return true;\n      } else {\n        return false;\n      }\n    }\n  }\n};\n</script>\n\n<style scoped>\n.box { color: red; }\n</style>\n";
    let model = TypeScriptAndJavaScript
        .lower(&RepoPath::new("src/Counter.vue").unwrap(), src, None)
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
        .lower(&RepoPath::new("src/App.vue").unwrap(), src, None)
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
        .lower(&RepoPath::new("src/Page.svelte").unwrap(), src, None)
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
    let res = TypeScriptAndJavaScript
        .resolve_imports(&files, vec![], reqs, &[])
        .expect("resolve imports");
    assert_eq!(res[0], Resolution::Resolved(vec!["src/Child.vue".into()]));
    assert_eq!(res[1], Resolution::Resolved(vec!["src/helper.ts".into()]));
    assert_eq!(
        res[2],
        Resolution::Resolved(vec!["src/Panel.svelte".into()])
    );
}

#[test]
fn sfc_without_script_is_empty_and_unclosed_script_is_unparsable() {
    let empty = TypeScriptAndJavaScript
        .lower(
            &RepoPath::new("src/Icon.vue").unwrap(),
            "<template><svg /></template>\n<style>.i {}</style>\n",
            None,
        )
        .unwrap();
    assert_eq!(empty.source_lines, 0);
    assert!(empty.functions.is_empty());

    let broken = TypeScriptAndJavaScript.lower(
        &RepoPath::new("src/Broken.vue").unwrap(),
        "<template><div /></template>\n<script>\nconst x = 1;\n",
        None,
    );
    assert!(
        matches!(broken, Err(LowerError::Unparsable { message }) if message.contains("line 2"))
    );

    let unicode = TypeScriptAndJavaScript
        .lower(
            &RepoPath::new("src/Unicode.vue").unwrap(),
            "<template><中文>你好</中文></template>\n<script>\nexport const ok = 1;\n</script>\n",
            None,
        )
        .unwrap();
    assert_eq!(unicode.source_lines, 1);
}
