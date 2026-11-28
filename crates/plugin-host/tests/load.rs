mod common;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use codeint_config::{
    PluginDeclaration, PluginHostConfig, PluginKind, PluginLimits, PluginSource, Sha256Digest,
};
use codeint_model::language::{
    DeclaredDep, DepKind, Function, FunctionCoverage, ImportRequest, Resolution,
};
use codeint_model::{
    CodeModel, FileModel, HazardLine, ImportResolution, LanguageFacts, LanguagePlugin,
    LanguagePluginError, LowerError, Manifest, MetricPlugin, MetricPluginError, RepoPath,
    ResolveError, SourceFile, SourceSet, Token, TokenKind, choose_manifests,
    resolve_runtime_imports, token_text_hash,
};
use codeint_plugin_host::{
    PLUGIN_INTERFACE_VERSION, PluginHostError, WasmLanguagePlugin, load_language_plugin,
    load_language_plugin_with_limits, load_language_plugins, load_metric_plugin,
    load_metric_plugin_with_limits, load_metric_plugins, verify_plugin_component,
};

use common::{declaration_for, declaration_with_kind, sample_plugin_wasm, sandbox_probe_wasm};

static SAMPLE_METRIC_WASM: OnceLock<PathBuf> = OnceLock::new();

fn sample_metric_plugin_wasm() -> &'static Path {
    SAMPLE_METRIC_WASM.get_or_init(|| sandbox_probe_wasm("sample_long_functions_plugin.wasm"))
}

fn assert_sandbox_blocks(plugin_name: &str, wasm_file: &str, denied_interface: &str) {
    let wasm_path = sandbox_probe_wasm(wasm_file);
    let declaration = declaration_for(plugin_name, &wasm_path);

    let error = load_language_plugin(Path::new("."), &declaration, &PluginHostConfig::default())
        .err()
        .expect("plugin importing host capability must fail to instantiate");

    match &error {
        PluginHostError::Instantiate { name, path, reason } => {
            assert_eq!(name, plugin_name);
            assert_eq!(path, &wasm_path);
            assert!(
                reason.contains(denied_interface),
                "expected instantiation error to name {denied_interface}, got: {reason}"
            );
        }
        other => panic!("expected PluginHostError::Instantiate, got: {other}"),
    }

    let rendered = error.to_string();
    assert!(
        rendered.contains(plugin_name) && rendered.contains(denied_interface),
        "expected rendered error to name {plugin_name} and {denied_interface}, got: {rendered}"
    );
}

#[test]
fn loads_sample_external_plugin_with_zero_capabilities_and_valid_hash() {
    let wasm_path = sample_plugin_wasm();
    let declaration = declaration_for("sample-mini", wasm_path);

    let plugin = load_language_plugin(Path::new("."), &declaration, &PluginHostConfig::default())
        .expect("sample external plugin loads with zero capabilities");

    assert_eq!(plugin.extensions(), vec!["mini"]);

    let model = plugin
        .lower(
            &RepoPath::new("src/app.mini").expect("valid repo path"),
            "import \"./util\"\nfn main() {\n  if x {\n    pass;\n  }\n}\n",
            None,
        )
        .expect("valid mini file lowers");
    assert_eq!(model.source_set, SourceSet::Production);
    assert_eq!(model.source_lines, 6);
    assert_eq!(model.functions.len(), 1);
    assert_eq!(model.functions[0].name.as_deref(), Some("main"));
    assert_eq!(model.functions[0].decision_points, 1);
    assert_eq!(model.functions[0].cognitive_complexity, 1);
    assert_eq!(model.runtime_imports, vec!["./util"]);
    assert_eq!(model.flagged_lines.len(), 1);
    assert_eq!(model.flagged_lines[0].rule, "redundant-pass");

    assert!(matches!(
        plugin.lower(
            &RepoPath::new("src/types.d.mini").unwrap(),
            "fn x() {}\n",
            None,
        ),
        Err(LowerError::DeclarationOnly)
    ));

    let repo_paths = vec![
        RepoPath::new("mini.workspace").unwrap(),
        RepoPath::new("src/app.mini").unwrap(),
        RepoPath::new("src/util.mini").unwrap(),
    ];
    assert_eq!(
        plugin
            .manifest_paths(&repo_paths)
            .expect("manifest paths succeed"),
        vec!["mini.workspace"]
    );
    let analysis = plugin
        .analyze_manifests(vec![Manifest {
            path: "mini.workspace".to_owned(),
            text: "packages/core\n".to_owned(),
        }])
        .expect("analyze manifests");
    assert_eq!(
        analysis.workspace_packages,
        vec!["packages/core".to_owned()]
    );

    let requests: Vec<ImportRequest> = ["./util", "external-pkg", "std:io"]
        .into_iter()
        .map(|specifier| ImportRequest {
            importer: "src/app.mini".to_owned(),
            specifier: specifier.to_owned(),
        })
        .collect();
    let resolutions = plugin
        .resolve_imports(&repo_paths, Vec::new(), requests, &[])
        .expect("resolve imports");
    assert_eq!(
        resolutions,
        vec![
            Resolution::Resolved(vec!["src/util.mini".to_owned()]),
            Resolution::External("external-pkg".to_owned()),
            Resolution::Builtin,
        ]
    );
}

#[test]
fn refuses_to_load_plugin_when_sha256_mismatches() {
    let wasm_path = sample_plugin_wasm();
    let declaration = PluginDeclaration {
        name: "sample-mini".to_owned(),
        kind: PluginKind::Language,
        source: PluginSource::Path(wasm_path.to_path_buf()),
        sha256: Sha256Digest::parse_hex(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("valid hex"),
    };

    let error = load_language_plugin(Path::new("."), &declaration, &PluginHostConfig::default())
        .err()
        .expect("hash mismatch must fail");
    assert!(
        matches!(error, PluginHostError::HashMismatch { ref name, .. } if name == "sample-mini"),
        "unexpected error: {error}"
    );
}

#[test]
fn refuses_duplicate_plugin_names_and_reserved_names() {
    let wasm_path = sample_plugin_wasm();
    let declaration = declaration_for("typescript-javascript", wasm_path);

    let error = load_language_plugins(
        Path::new("."),
        &[declaration],
        &["typescript-javascript"],
        &PluginHostConfig::default(),
    )
    .err()
    .expect("reserved name must fail");
    assert!(
        matches!(error, PluginHostError::DuplicatePluginName { ref name } if name == "typescript-javascript"),
        "unexpected error: {error}"
    );
}

#[test]
fn loads_sample_external_metric_plugin_with_zero_capabilities_and_measures_code_model() {
    let wasm_path = sample_metric_plugin_wasm();
    let declaration = declaration_with_kind("long-functions", PluginKind::Metric, wasm_path);

    let plugin = load_metric_plugin(Path::new("."), &declaration, &PluginHostConfig::default())
        .expect("sample external Metric plugin loads with zero capabilities");

    assert_eq!(plugin.key(), "long_functions");
    assert_eq!(plugin.name(), "Long functions");
    assert_eq!(
        plugin.description(),
        "Share of functions with more than 10 source lines"
    );

    let model = CodeModel {
        files: vec![SourceFile {
            path: RepoPath::new("src/work.ts").expect("valid repo path"),
            model: FileModel {
                source_set: SourceSet::Production,
                source_lines: 18,
                functions: vec![
                    Function {
                        line: 1,
                        end_line: 3,
                        decision_points: 0,
                        source_lines: 3,
                        name: Some("brief".to_owned()),
                        cognitive_complexity: 0,
                        coverage: None,
                    },
                    Function {
                        line: 5,
                        end_line: 18,
                        decision_points: 1,
                        source_lines: 14,
                        name: Some("verbose".to_owned()),
                        cognitive_complexity: 1,
                        coverage: None,
                    },
                ],
                tokens: vec![Token {
                    kind: TokenKind::Other,
                    text_hash: token_text_hash("return"),
                    line: 6,
                }],
                ..codeint_model::empty_file_model(SourceSet::Production)
            },
        }],
        ..CodeModel::default()
    };

    let measured = plugin
        .measure(&model)
        .expect("sample Metric plugin measures Code model");
    assert!((measured.ratio - 0.5).abs() < 1e-9);
    assert!((measured.numerator - 1.0).abs() < 1e-9);
    assert!((measured.denominator - 2.0).abs() < 1e-9);
    assert_eq!(measured.findings.len(), 1);
    assert_eq!(measured.findings[0].path.as_str(), "src/work.ts");
    assert_eq!(measured.findings[0].line, 5);
    assert_eq!(measured.findings[0].end_line, Some(18));
    assert_eq!(measured.findings[0].id.len(), 16);
}

#[test]
fn refuses_duplicate_metric_keys_across_external_metric_plugins() {
    let wasm_path = sample_metric_plugin_wasm();
    let first = declaration_with_kind("first-long-functions", PluginKind::Metric, wasm_path);
    let second = declaration_with_kind("second-long-functions", PluginKind::Metric, wasm_path);

    let error = load_metric_plugins(
        Path::new("."),
        &[first, second],
        &[],
        &PluginHostConfig::default(),
    )
    .err()
    .expect("duplicate metric key must fail");
    assert!(
        matches!(error, PluginHostError::DuplicateMetricKey { ref key, .. } if key == "long_functions"),
        "unexpected error: {error}"
    );
}

#[test]
fn refuses_external_metric_plugin_declaring_built_in_metric_key() {
    let wasm_path = sandbox_probe_wasm("reserved_key_metric_plugin.wasm");
    let declaration = declaration_with_kind("reserved-metric", PluginKind::Metric, &wasm_path);

    let error = load_metric_plugin(Path::new("."), &declaration, &PluginHostConfig::default())
        .err()
        .expect("external Metric plugin using built-in key must fail");
    assert!(
        matches!(
            error,
            PluginHostError::InvalidMetricKey {
                ref name,
                ref key,
                ..
            } if name == "reserved-metric" && key == "cognitive_complexity"
        ),
        "unexpected error: {error}"
    );
}

#[test]
fn external_plugin_cannot_read_files() {
    assert_sandbox_blocks(
        "rogue-file-reader",
        "file_reader_plugin.wasm",
        "wasi:filesystem/",
    );
}

#[test]
fn external_plugin_cannot_open_sockets() {
    assert_sandbox_blocks(
        "rogue-socket-opener",
        "socket_opener_plugin.wasm",
        "wasi:sockets/",
    );
}

#[test]
fn external_plugin_cannot_read_the_clock() {
    assert_sandbox_blocks(
        "rogue-clock-reader",
        "clock_reader_plugin.wasm",
        "wasi:clocks/",
    );
}

#[test]
fn refuses_non_https_url_and_missing_cache_dir_for_url_plugins() {
    let valid_sha =
        Sha256Digest::parse_hex("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
            .expect("valid hex");

    let plain_http = PluginDeclaration {
        name: "plain-http".to_owned(),
        kind: PluginKind::Language,
        source: PluginSource::HttpsUrl("http://127.0.0.1:8080/sample.wasm".to_owned()),
        sha256: valid_sha.clone(),
    };
    let http_error =
        load_language_plugin(Path::new("."), &plain_http, &PluginHostConfig::default())
            .err()
            .expect("plain http URL must be refused");
    assert!(
        matches!(http_error, PluginHostError::NonHttpsUrl { ref name, .. } if name == "plain-http"),
        "unexpected error: {http_error}"
    );

    let https_without_cache = PluginDeclaration {
        name: "no-cache".to_owned(),
        kind: PluginKind::Language,
        source: PluginSource::HttpsUrl("https://127.0.0.1:8443/sample.wasm".to_owned()),
        sha256: valid_sha,
    };
    let cache_error = load_language_plugin(
        Path::new("."),
        &https_without_cache,
        &PluginHostConfig::default(),
    )
    .err()
    .expect("missing cache dir must fail");
    assert!(
        matches!(cache_error, PluginHostError::MissingCacheDir { ref name, .. } if name == "no-cache"),
        "unexpected error: {cache_error}"
    );
}

fn load_sandbox_language_with_limits(
    name: &str,
    wasm_file: &str,
    memory_mb: u64,
    timeout_ms: u64,
) -> WasmLanguagePlugin {
    let wasm_path = sandbox_probe_wasm(wasm_file);
    let declaration = declaration_for(name, &wasm_path);
    let limits = PluginLimits {
        memory_mb,
        timeout_ms,
    };
    load_language_plugin_with_limits(
        Path::new("."),
        &declaration,
        &PluginHostConfig::default(),
        limits,
    )
    .expect("sandbox language plugin loads")
}

fn lower_file(
    plugin: &WasmLanguagePlugin,
    path: &str,
    text: &str,
) -> Result<FileModel, LowerError> {
    plugin.lower(&RepoPath::new(path).expect("valid repo path"), text, None)
}

fn make_repo_paths(paths: &[&str]) -> Vec<RepoPath> {
    paths
        .iter()
        .map(|p| RepoPath::new(*p).expect("valid repo path"))
        .collect()
}

fn assert_lower_source_lines(plugin: &WasmLanguagePlugin, path: &str, text: &str, expected: u32) {
    let model = lower_file(plugin, path, text).expect("healthy file lowers");
    assert_eq!(model.source_lines, expected);
}

#[test]
fn language_plugin_that_loops_forever_hits_time_limit_and_still_lowers_other_files() {
    let plugin = load_sandbox_language_with_limits("loop-lang", "loop_language_plugin.wasm", 4, 50);

    let error = lower_file(&plugin, "src/hang.loop", "LOOP_FOREVER\n")
        .expect_err("infinite loop must hit time limit");
    assert!(
        matches!(
            error,
            LowerError::TimeLimitExceeded {
                ref plugin,
                timeout_ms: 50
            } if plugin == "loop-lang"
        ),
        "unexpected error: {error}"
    );

    assert_lower_source_lines(&plugin, "src/ok.loop", "line 1\nline 2\n", 2);
}

#[test]
fn language_plugin_that_exceeds_memory_cap_fails_under_tight_cap_and_succeeds_when_raised() {
    let tight_plugin =
        load_sandbox_language_with_limits("hog-lang", "memory_hog_language_plugin.wasm", 2, 1_000);
    let error = lower_file(&tight_plugin, "src/big.hog", "ALLOC_2MB\n")
        .expect_err("2 MiB allocation must exceed 2 MiB cap");
    assert!(
        matches!(
            error,
            LowerError::MemoryLimitExceeded {
                ref plugin,
                memory_mb: 2
            } if plugin == "hog-lang"
        ),
        "unexpected error: {error}"
    );
    assert_lower_source_lines(&tight_plugin, "src/small.hog", "small\n", 1);

    let raised_plugin =
        load_sandbox_language_with_limits("hog-lang", "memory_hog_language_plugin.wasm", 8, 1_000);
    assert_lower_source_lines(&raised_plugin, "src/big.hog", "ALLOC_2MB\n", 1);
}

#[test]
fn metric_plugin_that_loops_forever_or_exceeds_memory_cap_returns_limit_error() {
    let wasm_path = sandbox_probe_wasm("loop_metric_plugin.wasm");
    let declaration = declaration_with_kind("loop-metric", PluginKind::Metric, &wasm_path);
    let limits = PluginLimits {
        memory_mb: 2,
        timeout_ms: 50,
    };
    let plugin = load_metric_plugin_with_limits(
        Path::new("."),
        &declaration,
        &PluginHostConfig::default(),
        limits,
    )
    .expect("loop metric plugin loads");

    let timeout_error = plugin
        .measure(&CodeModel::default())
        .expect_err("infinite-loop Metric plugin must time out");
    assert!(
        matches!(
            timeout_error,
            MetricPluginError::TimeLimitExceeded {
                ref plugin,
                timeout_ms: 50
            } if plugin == "loop-metric"
        ),
        "unexpected error: {timeout_error}"
    );

    let hog_model = CodeModel {
        files: vec![SourceFile {
            path: RepoPath::new("src/hog_metric.ts").expect("valid repo path"),
            model: FileModel {
                source_lines: 1,
                ..codeint_model::empty_file_model(SourceSet::Production)
            },
        }],
        ..CodeModel::default()
    };
    let memory_error = plugin
        .measure(&hog_model)
        .expect_err("memory-hog Metric plugin must hit memory cap");
    assert!(
        matches!(
            memory_error,
            MetricPluginError::MemoryLimitExceeded {
                ref plugin,
                memory_mb: 2
            } if plugin == "loop-metric"
        ),
        "unexpected error: {memory_error}"
    );
}

#[test]
fn built_in_and_sample_plugins_fit_comfortably_within_default_limits() {
    use std::time::Instant;

    use codeint_lang_ts::TypeScriptAndJavaScript;

    let defaults = PluginLimits::default();

    let mut large_ts = String::new();
    for index in 0..100 {
        large_ts.push_str(&format!(
            "export function fn_{index}(x: number): number {{\n  if (x > {index}) {{\n    return x + 1;\n  }}\n  let sum = 0;\n  for (let i = 0; i < 4; i++) {{\n    sum += i;\n  }}\n  return sum;\n}}\n"
        ));
    }
    let ts_path = RepoPath::new("src/large.ts").expect("valid repo path");
    let ts_start = Instant::now();
    let ts_model = TypeScriptAndJavaScript
        .lower(&ts_path, &large_ts, None)
        .expect("built-in TS/JS lowers 1000-line file");
    let ts_elapsed = ts_start.elapsed();
    assert_eq!(ts_model.functions.len(), 100);
    assert!(ts_elapsed.as_millis() < u128::from(defaults.timeout_ms));

    let mut large_mini = String::new();
    for index in 0..100 {
        large_mini.push_str(&format!(
            "fn fn_{index}(x) {{\n  if x {{\n    pass;\n  }}\n  return x\n}}\n"
        ));
    }
    let mini_path = RepoPath::new("src/large.mini").expect("valid repo path");
    let mini_wasm = sample_plugin_wasm();
    let mini_plugin = load_language_plugin(
        Path::new("."),
        &declaration_for("sample-mini", mini_wasm),
        &PluginHostConfig::default(),
    )
    .expect("sample language plugin loads under default limits");
    let mini_start = Instant::now();
    let mini_model = mini_plugin
        .lower(&mini_path, &large_mini, None)
        .expect("sample language plugin lowers 600-line file under default limits");
    let mini_elapsed = mini_start.elapsed();
    let mini_peak = mini_plugin.peak_memory_bytes();
    assert_eq!(mini_model.functions.len(), 100);
    assert!(mini_peak < defaults.memory_bytes());
    assert!(mini_elapsed.as_millis() < u128::from(defaults.timeout_ms));

    let metric_wasm = sample_metric_plugin_wasm();
    let metric_plugin = load_metric_plugin(
        Path::new("."),
        &declaration_with_kind("long-functions", PluginKind::Metric, metric_wasm),
        &PluginHostConfig::default(),
    )
    .expect("sample metric plugin loads under default limits");
    let large_code_model = CodeModel {
        files: (0..100)
            .map(|index| SourceFile {
                path: RepoPath::new(format!("src/file_{index}.ts")).expect("valid repo path"),
                model: ts_model.clone(),
            })
            .collect(),
        ..CodeModel::default()
    };
    let metric_start = Instant::now();
    let metric_result = metric_plugin
        .measure(&large_code_model)
        .expect("sample metric plugin measures 100-file Code model under default limits");
    let metric_elapsed = metric_start.elapsed();
    let metric_peak = metric_plugin.peak_memory_bytes();
    assert!((metric_result.denominator - 10_000.0).abs() < 1e-9);
    assert!(metric_peak < defaults.memory_bytes());
    assert!(metric_elapsed.as_millis() < u128::from(defaults.timeout_ms));
}

#[test]
fn verifies_0_5_0_contract_fields_and_methods_across_wasm_boundary() {
    assert_eq!(PLUGIN_INTERFACE_VERSION, "0.5.0");
    let wasm_path = sample_plugin_wasm();
    let declaration = declaration_for("sample-mini", wasm_path);
    let plugin = load_language_plugin(Path::new("."), &declaration, &PluginHostConfig::default())
        .expect("0.5.0 sample plugin loads");

    assert_eq!(
        plugin.flagged_line_rules(),
        vec!["redundant-pass".to_owned()]
    );
    assert!(plugin.known_dialects().is_empty());
    assert!(!plugin.has_scope_analysis());

    let lowered = plugin
        .lower(
            &RepoPath::new("src/main.mini").expect("valid repo path"),
            "fn outer(x) {\n  if x {\n    while x {\n      return x\n    }\n  }\n}\nfn() {\n  return 1\n}\n",
            None,
        )
        .expect("lowers named and anonymous functions");
    assert_eq!(lowered.functions.len(), 2);
    assert_eq!(lowered.functions[0].name.as_deref(), Some("outer"));
    assert_eq!(lowered.functions[0].decision_points, 2);
    assert_eq!(lowered.functions[0].cognitive_complexity, 3);
    assert_eq!(lowered.functions[0].coverage, None);
    assert_eq!(lowered.functions[1].name, None);
    assert_eq!(lowered.functions[1].cognitive_complexity, 0);
    assert_eq!(lowered.functions[1].coverage, None);
    assert!(lowered.hazard_lines.is_empty());
    assert!(!lowered.program_start);
    assert!(!lowered.is_tooling_file);
    assert!(lowered.diagnostics.is_empty());
    assert!(lowered.scope_artifact.is_none());

    let repo_paths = make_repo_paths(&["mini.workspace", "src/main.mini", "src/util.mini"]);
    let manifests = vec![Manifest {
        path: "mini.workspace".to_owned(),
        text: "packages/core\ndep:serde\ndev-dep:testkit\nopt-dep:extras\nentry:src/main.mini\nentry:src/missing.mini\n".to_owned(),
    }];

    let analysis = plugin
        .analyze_manifests(manifests.clone())
        .expect("valid manifest analysis");
    assert_eq!(
        analysis.workspace_packages,
        vec!["packages/core".to_owned()]
    );
    assert!(analysis.invalid_manifests.is_empty());
    assert_eq!(
        analysis.declared_dependencies,
        vec![
            DeclaredDep {
                package_root: String::new(),
                name: "serde".to_owned(),
                kind: DepKind::Runtime,
            },
            DeclaredDep {
                package_root: String::new(),
                name: "testkit".to_owned(),
                kind: DepKind::Dev,
            },
            DeclaredDep {
                package_root: String::new(),
                name: "extras".to_owned(),
                kind: DepKind::Optional,
            },
        ]
    );
    assert_eq!(
        plugin
            .entry_points(&repo_paths, manifests.clone())
            .expect("valid entry points"),
        vec![RepoPath::new("src/main.mini").expect("valid repo path")]
    );
    assert_eq!(
        plugin
            .resolve_imports(
                &repo_paths,
                manifests,
                vec![
                    ImportRequest {
                        importer: "src/main.mini".to_owned(),
                        specifier: "./util".to_owned(),
                    },
                    ImportRequest {
                        importer: "src/main.mini".to_owned(),
                        specifier: "serde".to_owned(),
                    },
                    ImportRequest {
                        importer: "src/main.mini".to_owned(),
                        specifier: "std:fs".to_owned(),
                    },
                    ImportRequest {
                        importer: "src/main.mini".to_owned(),
                        specifier: "./missing".to_owned(),
                    },
                ],
                &[],
            )
            .expect("resolve imports"),
        vec![
            Resolution::Resolved(vec!["src/util.mini".to_owned()]),
            Resolution::External("serde".to_owned()),
            Resolution::Builtin,
            Resolution::Unresolved("cannot find src/missing.mini".to_owned()),
        ]
    );

    assert_eq!(
        plugin.known_framework_profiles(),
        vec![
            "mini-web".to_owned(),
            "mini-cli".to_owned(),
            "mini-app".to_owned()
        ]
    );
    let fw_paths = make_repo_paths(&[
        "mini.workspace",
        "src/main.mini",
        "src/util.mini",
        "pages/home.mini",
        "gen/routes.mini",
    ]);
    let fw_manifests = vec![Manifest {
        path: "mini.workspace".to_owned(),
        text: "framework:mini-web\nframework:mini-cli\n".to_owned(),
    }];
    let auto_resolved = plugin
        .resolve_framework_profiles(
            "",
            &fw_paths,
            fw_manifests.clone(),
            codeint_model::FrameworkProfileSelection::Auto,
        )
        .expect("auto framework profiles");
    assert_eq!(
        auto_resolved.active_profiles,
        vec!["mini-web".to_owned(), "mini-cli".to_owned()]
    );
    assert_eq!(auto_resolved.entry_points, vec!["pages/home.mini"]);
    assert_eq!(auto_resolved.generated_paths, vec!["gen/routes.mini"]);
    assert!(auto_resolved.reference_ranges.is_empty());
    assert!(
        auto_resolved
            .reference_range_note
            .as_deref()
            .is_some_and(|note| note.contains("falling back"))
    );

    let excluded = plugin
        .resolve_framework_profiles(
            "",
            &fw_paths,
            fw_manifests.clone(),
            codeint_model::FrameworkProfileSelection::Exclude(vec!["mini-cli".to_owned()]),
        )
        .expect("exclude mini-cli");
    assert_eq!(excluded.active_profiles, vec!["mini-web".to_owned()]);
    assert_eq!(excluded.reference_ranges.len(), 1);
    assert_eq!(excluded.reference_ranges[0].metric, "erosion");
    assert_eq!(excluded.reference_ranges[0].metric_version, 1);

    let disabled = plugin
        .resolve_framework_profiles(
            "",
            &fw_paths,
            fw_manifests,
            codeint_model::FrameworkProfileSelection::Disabled,
        )
        .expect("disabled framework profiles");
    assert!(disabled.active_profiles.is_empty());
    assert!(disabled.entry_points.is_empty());
    assert!(disabled.generated_paths.is_empty());

    let unknown_only = plugin.resolve_framework_profiles(
        "",
        &fw_paths,
        Vec::new(),
        codeint_model::FrameworkProfileSelection::Only(vec!["nonexistent".to_owned()]),
    );
    assert!(matches!(
        unknown_only,
        Err(LanguagePluginError::FrameworkProfile { ref message }) if message.contains("nonexistent")
    ));

    let alias_request = vec![ImportRequest {
        importer: "pages/home.mini".to_owned(),
        specifier: "@/util".to_owned(),
    }];
    assert_eq!(
        plugin
            .resolve_imports(&fw_paths, Vec::new(), alias_request.clone(), &[])
            .expect("resolve without profile"),
        vec![Resolution::External("@/util".to_owned())]
    );
    assert_eq!(
        plugin
            .resolve_imports(
                &fw_paths,
                Vec::new(),
                alias_request,
                &[codeint_model::PackageFrameworkProfiles {
                    package_root: String::new(),
                    active_profiles: vec!["mini-web".to_owned()],
                }],
            )
            .expect("resolve with mini-web active profile"),
        vec![Resolution::Resolved(vec!["src/util.mini".to_owned()])]
    );

    let metric_wasm = sample_metric_plugin_wasm();
    let metric_plugin = load_metric_plugin(
        Path::new("."),
        &declaration_with_kind("sample-metric", PluginKind::Metric, metric_wasm),
        &PluginHostConfig::default(),
    )
    .expect("0.5.0 sample metric plugin loads");
    assert_eq!(metric_plugin.version(), 1);
}

fn assert_incompatible_contract_error(
    error: PluginHostError,
    plugin_name: &str,
    expected_interface: &str,
    found_interface: &str,
) {
    assert!(
        matches!(
            &error,
            PluginHostError::IncompatibleContractVersion {
                name,
                expected,
                found,
                ..
            } if name == plugin_name
                && *expected == expected_interface
                && found == found_interface
        ),
        "expected IncompatibleContractVersion, got: {error}"
    );
    let rendered = error.to_string();
    assert!(
        rendered.contains(plugin_name)
            && rendered.contains(found_interface)
            && rendered.contains(expected_interface)
            && rendered.contains("rebuild"),
        "expected actionable version-mismatch error, got: {rendered}"
    );
}

#[test]
fn rejects_v0_1_0_language_plugin_with_actionable_version_mismatch_error() {
    let wasm_path = sandbox_probe_wasm("legacy_v0_1_0_language_plugin.wasm");
    let declaration = declaration_for("legacy-lang", &wasm_path);

    let error = load_language_plugin(Path::new("."), &declaration, &PluginHostConfig::default())
        .err()
        .expect("0.1.0 Language plugin must be rejected by 0.5.0 host");

    assert_incompatible_contract_error(
        error,
        "legacy-lang",
        "codeint:plugin/language@0.5.0",
        "codeint:plugin/language@0.1.0",
    );
}

#[test]
fn rejects_v0_1_0_metric_plugin_with_actionable_version_mismatch_error() {
    let wasm_path = sandbox_probe_wasm("legacy_v0_1_0_metric_plugin.wasm");
    let declaration = declaration_with_kind("legacy-metric", PluginKind::Metric, &wasm_path);

    let error = load_metric_plugin(Path::new("."), &declaration, &PluginHostConfig::default())
        .err()
        .expect("0.1.0 Metric plugin must be rejected by 0.5.0 host");

    assert_incompatible_contract_error(
        error,
        "legacy-metric",
        "codeint:plugin/metric@0.5.0",
        "codeint:plugin/metric@0.1.0",
    );
}

#[test]
fn rejects_v0_2_0_and_v0_3_0_language_and_metric_plugins_with_actionable_version_mismatch_error() {
    for (label, wasm_file, expected_found) in [
        (
            "legacy-v02-lang",
            "legacy_v0_2_0_language_plugin.wasm",
            "codeint:plugin/language@0.2.0",
        ),
        (
            "legacy-v03-lang",
            "legacy_v0_3_0_language_plugin.wasm",
            "codeint:plugin/language@0.3.0",
        ),
    ] {
        let lang_path = sandbox_probe_wasm(wasm_file);
        let lang_decl = declaration_for(label, &lang_path);
        let lang_err =
            load_language_plugin(Path::new("."), &lang_decl, &PluginHostConfig::default())
                .err()
                .expect("legacy Language plugin must be rejected by 0.5.0 host");
        assert_incompatible_contract_error(
            lang_err,
            label,
            "codeint:plugin/language@0.5.0",
            expected_found,
        );
        let legacy_lang_bytes = std::fs::read(&lang_path).expect("read legacy language wasm");
        let verify_lang_err =
            verify_plugin_component(label, &lang_path, PluginKind::Language, &legacy_lang_bytes)
                .expect_err("legacy language component must fail contract verification");
        assert_incompatible_contract_error(
            verify_lang_err,
            label,
            "codeint:plugin/language@0.5.0",
            expected_found,
        );
    }

    for (label, wasm_file, expected_found) in [
        (
            "legacy-v02-metric",
            "legacy_v0_2_0_metric_plugin.wasm",
            "codeint:plugin/metric@0.2.0",
        ),
        (
            "legacy-v03-metric",
            "legacy_v0_3_0_metric_plugin.wasm",
            "codeint:plugin/metric@0.3.0",
        ),
    ] {
        let metric_path = sandbox_probe_wasm(wasm_file);
        let metric_decl = declaration_with_kind(label, PluginKind::Metric, &metric_path);
        let metric_err =
            load_metric_plugin(Path::new("."), &metric_decl, &PluginHostConfig::default())
                .err()
                .expect("legacy Metric plugin must be rejected by 0.5.0 host");
        assert_incompatible_contract_error(
            metric_err,
            label,
            "codeint:plugin/metric@0.5.0",
            expected_found,
        );
        let legacy_metric_bytes = std::fs::read(&metric_path).expect("read legacy metric wasm");
        let verify_metric_err = verify_plugin_component(
            label,
            &metric_path,
            PluginKind::Metric,
            &legacy_metric_bytes,
        )
        .expect_err("legacy metric component must fail contract verification");
        assert_incompatible_contract_error(
            verify_metric_err,
            label,
            "codeint:plugin/metric@0.5.0",
            expected_found,
        );
    }

    let sample_lang_bytes = std::fs::read(sample_plugin_wasm()).expect("read sample language wasm");
    assert!(
        verify_plugin_component(
            "sample-mini",
            sample_plugin_wasm(),
            PluginKind::Language,
            &sample_lang_bytes,
        )
        .is_ok()
    );
    let sample_metric_bytes =
        std::fs::read(sample_metric_plugin_wasm()).expect("read sample metric wasm");
    assert!(
        verify_plugin_component(
            "sample-metric",
            sample_metric_plugin_wasm(),
            PluginKind::Metric,
            &sample_metric_bytes,
        )
        .is_ok()
    );
}

#[test]
fn rejects_v0_4_0_language_and_metric_plugins_with_actionable_version_mismatch_error() {
    let lang_path = sandbox_probe_wasm("legacy_v0_4_0_language_plugin.wasm");
    let lang_decl = declaration_for("legacy-v04-lang", &lang_path);
    let lang_err = load_language_plugin(Path::new("."), &lang_decl, &PluginHostConfig::default())
        .err()
        .expect("0.4.0 Language plugin must be rejected by 0.5.0 host");
    let lang_rendered = lang_err.to_string();
    assert!(
        !lang_rendered.contains("codeint.json"),
        "expected format-neutral configuration wording, got: {lang_rendered}"
    );
    assert_incompatible_contract_error(
        lang_err,
        "legacy-v04-lang",
        "codeint:plugin/language@0.5.0",
        "codeint:plugin/language@0.4.0",
    );

    let metric_path = sandbox_probe_wasm("legacy_v0_4_0_metric_plugin.wasm");
    let metric_decl = declaration_with_kind("legacy-v04-metric", PluginKind::Metric, &metric_path);
    let metric_err = load_metric_plugin(Path::new("."), &metric_decl, &PluginHostConfig::default())
        .err()
        .expect("0.4.0 Metric plugin must be rejected by 0.5.0 host");
    let metric_rendered = metric_err.to_string();
    assert!(
        !metric_rendered.contains("codeint.json"),
        "expected format-neutral configuration wording, got: {metric_rendered}"
    );
    assert_incompatible_contract_error(
        metric_err,
        "legacy-v04-metric",
        "codeint:plugin/metric@0.5.0",
        "codeint:plugin/metric@0.4.0",
    );
}

#[test]
fn rejects_metric_plugin_returning_version_zero_at_load_time() {
    let wasm_path = sandbox_probe_wasm("zero_version_metric_plugin.wasm");
    let declaration = declaration_with_kind("zero-ver-metric", PluginKind::Metric, &wasm_path);
    match load_metric_plugin(Path::new("."), &declaration, &PluginHostConfig::default()) {
        Err(
            ref error @ PluginHostError::InvalidMetricVersion {
                ref name,
                version: 0,
            },
        ) if name == "zero-ver-metric" => {
            assert!(error.to_string().contains("positive integer"));
        }
        Err(other) => panic!("expected InvalidMetricVersion, got: {other}"),
        Ok(_) => panic!("Metric plugin with version 0 must be rejected at load time"),
    }
}

fn trigger_manifest(directive: &str) -> Vec<Manifest> {
    vec![Manifest {
        path: "trigger.manifest".to_owned(),
        text: directive.to_owned(),
    }]
}

fn load_probe(name: &str, wasm_file: &str, limits: PluginLimits) -> WasmLanguagePlugin {
    let wasm_path = sandbox_probe_wasm(wasm_file);
    load_language_plugin_with_limits(
        Path::new("."),
        &declaration_for(name, &wasm_path),
        &PluginHostConfig::default(),
        limits,
    )
    .expect("probe plugin loads")
}

fn assert_memory_err<T: std::fmt::Debug>(
    result: Result<T, LanguagePluginError>,
    expected_plugin: &str,
    expected_mb: u64,
) {
    assert!(
        matches!(
            result,
            Err(LanguagePluginError::MemoryLimitExceeded {
                ref plugin,
                memory_mb,
            }) if plugin == expected_plugin && memory_mb == expected_mb
        ),
        "expected MemoryLimitExceeded({expected_plugin}, {expected_mb}), got: {result:?}"
    );
}

#[test]
fn analyze_manifests_and_entry_points_propagate_timeout_trap_and_oom_and_recover() {
    let loop_plugin = load_probe(
        "loop-lang",
        "loop_language_plugin.wasm",
        PluginLimits {
            memory_mb: 4,
            timeout_ms: 150,
        },
    );
    let repo_paths = vec![RepoPath::new("src/main.loop").expect("valid repo path")];

    assert!(matches!(
        loop_plugin.analyze_manifests(trigger_manifest("LOOP_FOREVER")),
        Err(LanguagePluginError::TimeLimitExceeded {
            ref plugin,
            timeout_ms: 150,
        }) if plugin == "loop-lang"
    ));
    assert!(matches!(
        loop_plugin.analyze_manifests(trigger_manifest("PANIC_TRAP")),
        Err(LanguagePluginError::Call {
            ref plugin,
            call: "analyze-manifests",
            ..
        }) if plugin == "loop-lang"
    ));
    assert_eq!(
        loop_plugin
            .analyze_manifests(Vec::new())
            .expect("healthy analyze_manifests succeeds after trap")
            .declared_dependencies
            .len(),
        1
    );

    assert!(matches!(
        loop_plugin.entry_points(&repo_paths, trigger_manifest("LOOP_FOREVER")),
        Err(LanguagePluginError::TimeLimitExceeded {
            ref plugin,
            timeout_ms: 150,
        }) if plugin == "loop-lang"
    ));
    assert!(matches!(
        loop_plugin.entry_points(&repo_paths, trigger_manifest("PANIC_TRAP")),
        Err(LanguagePluginError::Call {
            ref plugin,
            call: "entry-points",
            ..
        }) if plugin == "loop-lang"
    ));
    assert_eq!(
        loop_plugin
            .entry_points(&repo_paths, Vec::new())
            .expect("healthy entry_points succeeds after trap"),
        repo_paths
    );

    let hog_plugin = load_probe(
        "hog-lang",
        "memory_hog_language_plugin.wasm",
        PluginLimits {
            memory_mb: 2,
            timeout_ms: 1_000,
        },
    );
    let hog_paths = vec![RepoPath::new("src/main.hog").expect("valid repo path")];

    assert_memory_err(
        hog_plugin.analyze_manifests(trigger_manifest("ALLOC_2MB")),
        "hog-lang",
        2,
    );
    assert_memory_err(
        hog_plugin.entry_points(&hog_paths, trigger_manifest("ALLOC_2MB")),
        "hog-lang",
        2,
    );
    assert_eq!(
        hog_plugin
            .entry_points(&hog_paths, Vec::new())
            .expect("healthy entry_points recovers after OOM trap"),
        hog_paths
    );
}

#[test]
fn rejects_canonical_abi_aliased_string_amplification_bomb() {
    let plugin = load_probe(
        "hog-lang",
        "memory_hog_language_plugin.wasm",
        PluginLimits {
            memory_mb: 2,
            timeout_ms: 1_000,
        },
    );
    let repo_paths = vec![RepoPath::new("src/main.hog").expect("valid repo path")];

    assert_memory_err(
        plugin.entry_points(&repo_paths, trigger_manifest("ALIASED_BOMB")),
        "hog-lang",
        2,
    );
    assert_memory_err(
        plugin.analyze_manifests(trigger_manifest("ALIASED_BOMB")),
        "hog-lang",
        2,
    );
    assert_eq!(
        plugin
            .entry_points(&repo_paths, Vec::new())
            .expect("plugin recovers after aliased-string bomb is rejected"),
        repo_paths
    );
}

#[test]
fn rejects_invalid_entry_points_and_declared_dependencies_from_wasm_plugin() {
    let plugin = load_probe(
        "hog-lang",
        "memory_hog_language_plugin.wasm",
        PluginLimits::default(),
    );
    let repo_paths = vec![RepoPath::new("src/main.hog").expect("valid repo path")];

    assert!(matches!(
        plugin.entry_points(&repo_paths, trigger_manifest("INVALID_TRAVERSAL")),
        Err(LanguagePluginError::InvalidEntryPointPath { ref path })
            if path == "../../../etc/passwd"
    ));
    assert!(matches!(
        plugin.entry_points(&repo_paths, trigger_manifest("INVALID_ABSOLUTE")),
        Err(LanguagePluginError::InvalidEntryPointPath { ref path }) if path == "/etc/shadow"
    ));
    assert!(matches!(
        plugin.entry_points(&repo_paths, trigger_manifest("INVALID_UNKNOWN")),
        Err(LanguagePluginError::UnknownEntryPoint { ref path }) if path == "src/missing.hog"
    ));

    for directive in ["INVALID_EMPTY_NAME", "INVALID_WHITESPACE_NAME"] {
        assert!(matches!(
            plugin.analyze_manifests(trigger_manifest(directive)),
            Err(LanguagePluginError::EmptyDependencyName { .. })
        ));
    }
    for (directive, expected) in [
        ("INVALID_TRAVERSAL_ROOT", "../escape"),
        ("INVALID_ABSOLUTE_ROOT", "/etc/shadow"),
        ("INVALID_BACKSLASH_ROOT", "packages\\app"),
    ] {
        assert!(matches!(
            plugin.analyze_manifests(trigger_manifest(directive)),
            Err(LanguagePluginError::InvalidDependencyPackageRoot { ref package_root })
                if package_root == expected
        ));
    }
}

#[test]
fn loads_dual_version_language_plugin_exporting_0_4_0_and_0_5_0() {
    let plugin = load_probe(
        "dual-lang",
        "dual_version_language_plugin.wasm",
        PluginLimits::default(),
    );
    assert_eq!(plugin.extensions(), vec!["dual"]);
    let repo_paths = vec![RepoPath::new("src/app.dual").expect("valid repo path")];
    assert_eq!(
        plugin
            .entry_points(&repo_paths, Vec::new())
            .expect("dual plugin entry_points"),
        repo_paths
    );
    assert_eq!(
        plugin
            .analyze_manifests(Vec::new())
            .expect("dual plugin analyze_manifests")
            .declared_dependencies,
        vec![DeclaredDep {
            package_root: String::new(),
            name: "dual-dep".to_owned(),
            kind: DepKind::Runtime,
        }]
    );
}

fn coverage_model_with(functions: Vec<Function>) -> CodeModel {
    CodeModel {
        files: vec![SourceFile {
            path: RepoPath::new("src/work.ts").expect("valid repo path"),
            model: FileModel {
                source_lines: 30,
                functions,
                ..codeint_model::empty_file_model(SourceSet::Production)
            },
        }],
        ..CodeModel::default()
    }
}

#[test]
fn metric_plugin_reads_per_function_coverage_and_distinguishes_absent_from_zero_executed() {
    let wasm_path = sandbox_probe_wasm("coverage_reader_metric_plugin.wasm");
    let declaration = declaration_with_kind("coverage-reader", PluginKind::Metric, &wasm_path);
    let plugin = load_metric_plugin(Path::new("."), &declaration, &PluginHostConfig::default())
        .expect("coverage reader Metric plugin loads");

    assert_eq!(plugin.key(), "coverage_reader");

    let absent_only = coverage_model_with(vec![Function {
        line: 1,
        end_line: 6,
        decision_points: 1,
        source_lines: 5,
        name: Some("unreported".to_owned()),
        cognitive_complexity: 1,
        coverage: None,
    }]);
    let zero_only = coverage_model_with(vec![Function {
        line: 1,
        end_line: 6,
        decision_points: 1,
        source_lines: 5,
        name: Some("unexecuted".to_owned()),
        cognitive_complexity: 1,
        coverage: Some(FunctionCoverage {
            listed_lines: 5,
            executed_lines: 0,
        }),
    }]);

    let absent_measured = plugin
        .measure(&absent_only)
        .expect("measure absent coverage");
    let zero_measured = plugin.measure(&zero_only).expect("measure zero executed");

    assert!((absent_measured.ratio - 0.0).abs() < 1e-9);
    assert!((absent_measured.numerator - 0.0).abs() < 1e-9);
    assert!((absent_measured.denominator - 0.0).abs() < 1e-9);
    assert_eq!(absent_measured.findings.len(), 1);
    assert_eq!(absent_measured.findings[0].id, "absent:unreported");
    assert_eq!(absent_measured.findings[0].message, "absent");

    assert!((zero_measured.ratio - 1.0).abs() < 1e-9);
    assert!((zero_measured.numerator - 5.0).abs() < 1e-9);
    assert!((zero_measured.denominator - 5.0).abs() < 1e-9);
    assert_eq!(zero_measured.findings.len(), 1);
    assert_eq!(zero_measured.findings[0].id, "zero:unexecuted");
    assert_eq!(zero_measured.findings[0].message, "listed=5 executed=0");

    let mixed = coverage_model_with(vec![
        Function {
            line: 1,
            end_line: 6,
            decision_points: 1,
            source_lines: 5,
            name: Some("unreported".to_owned()),
            cognitive_complexity: 1,
            coverage: None,
        },
        Function {
            line: 8,
            end_line: 14,
            decision_points: 2,
            source_lines: 5,
            name: Some("unexecuted".to_owned()),
            cognitive_complexity: 2,
            coverage: Some(FunctionCoverage {
                listed_lines: 5,
                executed_lines: 0,
            }),
        },
        Function {
            line: 16,
            end_line: 22,
            decision_points: 2,
            source_lines: 5,
            name: Some("partial".to_owned()),
            cognitive_complexity: 2,
            coverage: Some(FunctionCoverage {
                listed_lines: 5,
                executed_lines: 3,
            }),
        },
    ]);

    let mixed_measured = plugin.measure(&mixed).expect("measure mixed coverage");
    assert!((mixed_measured.ratio - 0.7).abs() < 1e-9);
    assert!((mixed_measured.numerator - 7.0).abs() < 1e-9);
    assert!((mixed_measured.denominator - 10.0).abs() < 1e-9);
    let summary: Vec<(&str, &str)> = mixed_measured
        .findings
        .iter()
        .map(|finding| (finding.id.as_str(), finding.message.as_str()))
        .collect();
    assert_eq!(
        summary,
        vec![
            ("absent:unreported", "absent"),
            ("zero:unexecuted", "listed=5 executed=0"),
            ("covered:partial", "listed=5 executed=3"),
        ]
    );
}

fn load_probe_language(name: &str, wasm_file: &str) -> codeint_plugin_host::WasmLanguagePlugin {
    let wasm_path = sandbox_probe_wasm(wasm_file);
    let declaration = declaration_for(name, &wasm_path);
    load_language_plugin(Path::new("."), &declaration, &PluginHostConfig::default())
        .unwrap_or_else(|error| panic!("{name} Language plugin loads: {error}"))
}

fn load_probe_metric(name: &str, wasm_file: &str) -> codeint_plugin_host::WasmMetricPlugin {
    let wasm_path = sandbox_probe_wasm(wasm_file);
    let declaration = declaration_with_kind(name, PluginKind::Metric, &wasm_path);
    load_metric_plugin(Path::new("."), &declaration, &PluginHostConfig::default())
        .unwrap_or_else(|error| panic!("{name} Metric plugin loads: {error}"))
}

fn language_facts(
    language: &str,
    has_coverage: bool,
    has_hazard_rules: bool,
    has_imports: bool,
) -> LanguageFacts {
    LanguageFacts {
        language: language.to_owned(),
        has_coverage,
        has_hazard_rules,
        has_imports,
    }
}

fn model_with_language_facts(facts: LanguageFacts, with_file: bool) -> CodeModel {
    let base = if with_file {
        coverage_model_with(Vec::new())
    } else {
        CodeModel::default()
    };
    CodeModel {
        languages: vec![facts],
        ..base
    }
}

fn assert_two_language_fact_probe(
    plugin: &codeint_plugin_host::WasmMetricPlugin,
    first: (LanguageFacts, bool, f64, (&str, &str)),
    second: (LanguageFacts, bool, f64, (&str, &str)),
    expected_merged_ratio: f64,
    expected_merged: Vec<(&str, &str)>,
) {
    let first_model = model_with_language_facts(first.0, first.1);
    let second_model = model_with_language_facts(second.0, second.1);
    let first_measured = plugin
        .measure(&first_model)
        .expect("measure first language");
    let second_measured = plugin
        .measure(&second_model)
        .expect("measure second language");
    assert!((first_measured.ratio - first.2).abs() < 1e-9);
    assert_eq!(summarize_findings(&first_measured), vec![first.3]);
    assert!((second_measured.ratio - second.2).abs() < 1e-9);
    assert_eq!(summarize_findings(&second_measured), vec![second.3]);

    let merged = CodeModel::merge([first_model, second_model]);
    let source_sets = merged.into_source_sets();
    let multi_measured = plugin
        .measure(source_sets.get(SourceSet::Production))
        .expect("measure merged multi-language model");
    assert!((multi_measured.ratio - expected_merged_ratio).abs() < 1e-9);
    assert_eq!(summarize_findings(&multi_measured), expected_merged);
}

#[test]
fn language_plugin_sets_program_start_mark_and_metric_plugin_reads_it() {
    let language = load_probe_language(
        "program-start-language",
        "program_start_language_plugin.wasm",
    );
    let metric = load_probe_metric("program-start-metric", "program_start_metric_plugin.wasm");

    let main_path = RepoPath::new("cmd/app/main.prog").expect("valid repo path");
    let lib_path = RepoPath::new("internal/util/helper.prog").expect("valid repo path");

    let main_model = language
        .lower(&main_path, "program main\nrun()\n", None)
        .expect("lowers program-start file");
    let lib_model = language
        .lower(&lib_path, "library util\nhelper()\n", None)
        .expect("lowers library file");

    assert!(main_model.program_start);
    assert!(!lib_model.program_start);

    let code_model = CodeModel {
        files: vec![
            SourceFile {
                path: main_path.clone(),
                model: main_model,
            },
            SourceFile {
                path: lib_path,
                model: lib_model,
            },
        ],
        ..CodeModel::default()
    };

    let measured = metric
        .measure(&code_model)
        .expect("Metric plugin reads program_start from Code model");
    assert_eq!(metric.key(), "program_starts");
    assert!((measured.ratio - 0.5).abs() < 1e-9);
    assert!((measured.numerator - 1.0).abs() < 1e-9);
    assert!((measured.denominator - 2.0).abs() < 1e-9);
    assert_eq!(measured.findings.len(), 1);
    assert_eq!(measured.findings[0].id, "program-start:cmd/app/main.prog");
    assert_eq!(measured.findings[0].path, main_path);
    assert_eq!(measured.findings[0].message, "program start");
}

#[test]
fn built_in_language_plugins_never_set_program_start() {
    use codeint_lang_dart::Dart;
    use codeint_lang_php::Php;
    use codeint_lang_python::Python;
    use codeint_lang_rust::Rust;
    use codeint_lang_ts::TypeScriptAndJavaScript;

    let cases: [(&str, &dyn LanguagePlugin, &str, &str); 5] = [
        (
            "typescript-javascript",
            &TypeScriptAndJavaScript,
            "src/main.ts",
            "export function main(): void {\n  console.log('hi');\n}\nmain();\n",
        ),
        (
            "rust",
            &Rust,
            "src/main.rs",
            "fn main() {\n    println!(\"hi\");\n}\n",
        ),
        (
            "php",
            &Php,
            "public/index.php",
            "<?php\nfunction main(): int {\n    return 0;\n}\nmain();\n",
        ),
        (
            "dart",
            &Dart,
            "bin/main.dart",
            "void main() {\n  print('hi');\n}\n",
        ),
        (
            "python",
            &Python,
            "src/__main__.py",
            "def main():\n    return 0\n\nif __name__ == '__main__':\n    main()\n",
        ),
    ];

    for (name, plugin, path, source) in cases {
        let model = plugin
            .lower(&RepoPath::new(path).expect("valid repo path"), source, None)
            .unwrap_or_else(|error| panic!("{name}: lower failed: {error}"));
        assert!(
            !model.program_start,
            "{name} built-in Language plugin must leave program_start false"
        );
        assert!(
            model.hazard_lines.is_empty(),
            "{name} built-in Language plugin must report no hazard lines"
        );
    }
}

fn summarize_findings(measurement: &codeint_model::ExternalMetricMeasurement) -> Vec<(&str, &str)> {
    measurement
        .findings
        .iter()
        .map(|finding| (finding.id.as_str(), finding.message.as_str()))
        .collect()
}

#[test]
fn metric_plugin_reads_per_language_coverage_statement() {
    let wasm_path = sandbox_probe_wasm("coverage_reader_metric_plugin.wasm");
    let declaration = declaration_with_kind("coverage-reader", PluginKind::Metric, &wasm_path);
    let plugin = load_metric_plugin(Path::new("."), &declaration, &PluginHostConfig::default())
        .expect("coverage reader Metric plugin loads");

    let absent_language = CodeModel {
        languages: vec![LanguageFacts {
            language: "typescript-javascript".to_owned(),
            has_coverage: false,
            has_hazard_rules: false,
            has_imports: true,
        }],
        ..coverage_model_with(Vec::new())
    };
    let covered_language = CodeModel {
        languages: vec![LanguageFacts {
            language: "typescript-javascript".to_owned(),
            has_coverage: true,
            has_hazard_rules: false,
            has_imports: true,
        }],
        ..coverage_model_with(Vec::new())
    };

    let absent_measured = plugin
        .measure(&absent_language)
        .expect("measure language without coverage");
    let covered_measured = plugin
        .measure(&covered_language)
        .expect("measure language with coverage");

    assert_eq!(
        summarize_findings(&absent_measured),
        vec![(
            "language:typescript-javascript:absent",
            "language=typescript-javascript has_coverage=false",
        )]
    );
    assert_eq!(
        summarize_findings(&covered_measured),
        vec![(
            "language:typescript-javascript:covered",
            "language=typescript-javascript has_coverage=true",
        )]
    );

    let rust_covered = CodeModel {
        files: vec![SourceFile {
            path: RepoPath::new("src/lib.rs").expect("valid repo path"),
            model: FileModel {
                source_lines: 10,
                ..codeint_model::empty_file_model(SourceSet::Production)
            },
        }],
        languages: vec![LanguageFacts {
            language: "rust".to_owned(),
            has_coverage: true,
            has_hazard_rules: false,
            has_imports: true,
        }],
        ..CodeModel::default()
    };
    let merged = CodeModel::merge([absent_language, rust_covered]);
    let source_sets = merged.into_source_sets();

    let multi_measured = plugin
        .measure(source_sets.get(SourceSet::Production))
        .expect("measure merged multi-language model");

    assert_eq!(
        summarize_findings(&multi_measured),
        vec![
            ("language:rust:covered", "language=rust has_coverage=true"),
            (
                "language:typescript-javascript:absent",
                "language=typescript-javascript has_coverage=false",
            ),
        ]
    );
}

#[test]
fn built_in_language_plugins_have_no_hazard_rules() {
    use codeint_lang_dart::Dart;
    use codeint_lang_php::Php;
    use codeint_lang_python::Python;
    use codeint_lang_rust::Rust;
    use codeint_lang_ts::TypeScriptAndJavaScript;

    let plugins: [(&str, &dyn LanguagePlugin); 5] = [
        ("typescript-javascript", &TypeScriptAndJavaScript),
        ("rust", &Rust),
        ("php", &Php),
        ("dart", &Dart),
        ("python", &Python),
    ];

    for (name, plugin) in plugins {
        assert!(
            !plugin.has_hazard_rules(),
            "{name} built-in Language plugin must report has_hazard_rules = false"
        );
        assert!(
            plugin.hazard_rules().is_empty(),
            "{name} built-in Language plugin must list no hazard rules"
        );
    }
}

#[test]
fn language_plugin_exports_hazard_rules_across_wasm_boundary() {
    let wasm_path = sandbox_probe_wasm("hazard_rules_language_plugin.wasm");
    let declaration = declaration_for("hazard-rules-language", &wasm_path);
    let plugin = load_language_plugin(Path::new("."), &declaration, &PluginHostConfig::default())
        .expect("hazard-rules Language plugin loads");

    assert!(plugin.has_hazard_rules());
    assert_eq!(
        plugin.hazard_rules(),
        vec![
            "delete-without-where".to_owned(),
            "leading-wildcard-like".to_owned(),
        ]
    );
}

#[test]
fn metric_plugin_reads_per_language_hazard_rules_statement() {
    let plugin = load_probe_metric("hazard-facts", "hazard_facts_metric_plugin.wasm");
    assert_eq!(plugin.key(), "hazard_facts");
    assert_two_language_fact_probe(
        &plugin,
        (
            LanguageFacts {
                language: "typescript-javascript".to_owned(),
                has_coverage: false,
                has_hazard_rules: false,
                has_imports: true,
            },
            true,
            0.0,
            (
                "hazard-rules:typescript-javascript:absent",
                "language=typescript-javascript has_hazard_rules=false",
            ),
        ),
        (
            LanguageFacts {
                language: "sql".to_owned(),
                has_coverage: false,
                has_hazard_rules: true,
                has_imports: false,
            },
            false,
            1.0,
            (
                "hazard-rules:sql:enabled",
                "language=sql has_hazard_rules=true",
            ),
        ),
        0.5,
        vec![
            (
                "hazard-rules:sql:enabled",
                "language=sql has_hazard_rules=true",
            ),
            (
                "hazard-rules:typescript-javascript:absent",
                "language=typescript-javascript has_hazard_rules=false",
            ),
        ],
    );
}

#[test]
fn language_plugin_resolves_multi_file_imports_and_metric_plugin_reads_them() {
    let language = load_probe_language("folder-import", "folder_import_language_plugin.wasm");
    let metric = load_probe_metric("imports-reader", "imports_reader_metric_plugin.wasm");

    let main_path = RepoPath::new("src/main.fld").expect("valid repo path");
    let alpha_path = RepoPath::new("pkg/util/alpha.fld").expect("valid repo path");
    let beta_path = RepoPath::new("pkg/util/beta.fld").expect("valid repo path");
    let repo_paths = vec![main_path.clone(), alpha_path.clone(), beta_path.clone()];

    let main_model = language
        .lower(&main_path, "use pkg/util\nfn main()\n", None)
        .expect("lower main.fld");
    let main_file = SourceFile {
        path: main_path.clone(),
        model: main_model,
    };

    let resolved = resolve_runtime_imports(&language, &repo_paths, Vec::new(), &[&main_file], &[])
        .expect("resolve multi-file folder import");
    assert_eq!(resolved.len(), 1);
    assert_eq!(
        resolved[0].resolution,
        ImportResolution::Resolved(vec![alpha_path.clone(), beta_path.clone()])
    );

    let code_model = CodeModel {
        files: vec![main_file],
        runtime_imports: resolved,
        ..CodeModel::default()
    };
    let measured = metric
        .measure(&code_model)
        .expect("Metric plugin reads multi-file resolved import");
    assert_eq!(metric.key(), "resolved_links");
    assert!((measured.ratio - 1.0).abs() < 1e-9);
    assert_eq!(measured.findings.len(), 1);
    assert_eq!(
        measured.findings[0].message,
        "pkg/util -> [pkg/util/alpha.fld, pkg/util/beta.fld]"
    );

    let resolve_error_for = |source: &str| {
        let model = language
            .lower(&main_path, source, None)
            .expect("lower probe file");
        let file = SourceFile {
            path: main_path.clone(),
            model,
        };
        resolve_runtime_imports(&language, &repo_paths, Vec::new(), &[&file], &[])
            .expect_err("invalid folder import must fail")
    };
    let empty_err = resolve_error_for("use empty\nfn main()\n");
    assert!(
        matches!(
            empty_err,
            ResolveError::EmptyResolution {
                ref importer,
                ref specifier,
            } if importer == &main_path && specifier == "empty"
        ),
        "unexpected error: {empty_err:?}"
    );
    let partial_err = resolve_error_for("use partial-missing\nfn main()\n");
    assert!(
        matches!(
            partial_err,
            ResolveError::UnknownTarget { ref target, .. }
                if target == "missing/does_not_exist.fld"
        ),
        "unexpected error: {partial_err:?}"
    );
}

#[test]
fn language_plugin_reports_hazard_lines_and_metric_plugin_reads_them() {
    let language =
        load_probe_language("hazard-rules-language", "hazard_rules_language_plugin.wasm");
    let metric = load_probe_metric("hazard-facts", "hazard_facts_metric_plugin.wasm");

    let unsafe_path = RepoPath::new("queries/unsafe.haz").expect("valid repo path");
    let clean_path = RepoPath::new("queries/clean.haz").expect("valid repo path");
    let unsafe_source = "SELECT id FROM users;\nDELETE FROM sessions;\nSELECT name FROM users WHERE name LIKE '%admin';\nDELETE FROM audit_logs WHERE id = 1;\n";
    let clean_source = "SELECT id FROM users WHERE id = 1;\n";

    let unsafe_model = language
        .lower(&unsafe_path, unsafe_source, None)
        .expect("lowers file with hazard lines");
    let clean_model = language
        .lower(&clean_path, clean_source, None)
        .expect("lowers clean file");

    assert_eq!(
        unsafe_model.hazard_lines,
        vec![
            HazardLine {
                line: 2,
                rule: "delete-without-where".to_owned(),
            },
            HazardLine {
                line: 3,
                rule: "leading-wildcard-like".to_owned(),
            },
        ]
    );
    let HazardLine { line, rule } = &unsafe_model.hazard_lines[0];
    assert_eq!(*line, 2);
    assert_eq!(rule, "delete-without-where");
    assert!(clean_model.hazard_lines.is_empty());

    let listed_rules = language.hazard_rules();
    let line_count = unsafe_source.lines().count();
    for hazard in &unsafe_model.hazard_lines {
        assert!(hazard.line >= 1 && (hazard.line as usize) <= line_count);
        assert!(listed_rules.contains(&hazard.rule));
    }

    let code_model = CodeModel {
        files: vec![
            SourceFile {
                path: clean_path,
                model: clean_model,
            },
            SourceFile {
                path: unsafe_path.clone(),
                model: unsafe_model,
            },
        ],
        languages: vec![LanguageFacts {
            language: "hazard-lang".to_owned(),
            has_coverage: false,
            has_hazard_rules: true,
            has_imports: true,
        }],
        ..CodeModel::default()
    };

    let measured = metric
        .measure(&code_model)
        .expect("Metric plugin reads hazard_lines from Code model");

    assert_eq!(
        summarize_findings(&measured),
        vec![
            (
                "hazard-rules:hazard-lang:enabled",
                "language=hazard-lang has_hazard_rules=true",
            ),
            (
                "hazard:queries/unsafe.haz:2:delete-without-where",
                "hazard rule=delete-without-where",
            ),
            (
                "hazard:queries/unsafe.haz:3:leading-wildcard-like",
                "hazard rule=leading-wildcard-like",
            ),
        ]
    );
    assert_eq!(measured.findings[1].path, unsafe_path);
    assert_eq!(measured.findings[1].line, 2);
    assert_eq!(measured.findings[2].path, unsafe_path);
    assert_eq!(measured.findings[2].line, 3);
}

#[test]
fn built_in_language_plugins_state_they_have_imports() {
    use codeint_lang_dart::Dart;
    use codeint_lang_php::Php;
    use codeint_lang_python::Python;
    use codeint_lang_rust::Rust;
    use codeint_lang_ts::TypeScriptAndJavaScript;

    let plugins: [(&str, &dyn LanguagePlugin); 5] = [
        ("typescript-javascript", &TypeScriptAndJavaScript),
        ("rust", &Rust),
        ("php", &Php),
        ("dart", &Dart),
        ("python", &Python),
    ];

    for (name, plugin) in plugins {
        assert!(
            plugin.has_imports(),
            "{name} built-in Language plugin must report has_imports = true"
        );
    }
}

#[test]
fn language_plugin_exports_has_imports_false_across_wasm_boundary() {
    let wasm_path = sandbox_probe_wasm("no_imports_language_plugin.wasm");
    let declaration = declaration_for("no-imports-language", &wasm_path);
    let plugin = load_language_plugin(Path::new("."), &declaration, &PluginHostConfig::default())
        .expect("no-imports Language plugin loads");

    assert_eq!(plugin.extensions(), vec!["noimp"]);
    assert!(!plugin.has_imports());
    let lowered = plugin
        .lower(
            &RepoPath::new("queries/select.noimp").expect("valid repo path"),
            "fn select_users()\n",
            None,
        )
        .expect("no-imports Language plugin lowers .noimp file");
    assert_eq!(lowered.source_lines, 1);
    assert_eq!(lowered.functions.len(), 1);
    assert!(lowered.runtime_imports.is_empty());
}

#[test]
fn metric_plugin_reads_per_language_has_imports_statement() {
    let plugin = load_probe_metric("import-capability", "import_capability_metric_plugin.wasm");
    assert_eq!(plugin.key(), "import_capability");
    assert_two_language_fact_probe(
        &plugin,
        (
            language_facts("typescript-javascript", false, false, true),
            true,
            1.0,
            (
                "imports:typescript-javascript:imports",
                "language=typescript-javascript has_imports=true",
            ),
        ),
        (
            language_facts("no-imports-lang", false, false, false),
            false,
            0.0,
            (
                "imports:no-imports-lang:no-imports",
                "language=no-imports-lang has_imports=false",
            ),
        ),
        0.5,
        vec![
            (
                "imports:no-imports-lang:no-imports",
                "language=no-imports-lang has_imports=false",
            ),
            (
                "imports:typescript-javascript:imports",
                "language=typescript-javascript has_imports=true",
            ),
        ],
    );
}

#[test]
fn concurrent_plugin_loads_keep_independent_memory_and_time_limits_on_shared_engine() {
    let hog_path = sandbox_probe_wasm("memory_hog_language_plugin.wasm");
    let hog_decl = declaration_for("hog-lang", &hog_path);
    let tight_hog = load_language_plugin_with_limits(
        Path::new("."),
        &hog_decl,
        &PluginHostConfig::default(),
        PluginLimits {
            memory_mb: 2,
            timeout_ms: 1_000,
        },
    )
    .expect("tight memory hog plugin loads");
    let generous_hog = load_language_plugin_with_limits(
        Path::new("."),
        &hog_decl,
        &PluginHostConfig::default(),
        PluginLimits {
            memory_mb: 8,
            timeout_ms: 1_000,
        },
    )
    .expect("generous memory hog plugin loads");

    let loop_path = sandbox_probe_wasm("loop_language_plugin.wasm");
    let loop_decl = declaration_for("loop-lang", &loop_path);
    let short_timeout_loop = load_language_plugin_with_limits(
        Path::new("."),
        &loop_decl,
        &PluginHostConfig::default(),
        PluginLimits {
            memory_mb: 4,
            timeout_ms: 50,
        },
    )
    .expect("loop language plugin loads");

    let sample_metric = load_metric_plugin(
        Path::new("."),
        &declaration_with_kind(
            "long-functions",
            PluginKind::Metric,
            sample_metric_plugin_wasm(),
        ),
        &PluginHostConfig::default(),
    )
    .expect("sample metric plugin loads alongside language plugins");

    let hog_file = RepoPath::new("src/big.hog").expect("valid repo path");
    assert!(matches!(
        tight_hog.lower(&hog_file, "ALLOC_2MB\n", None),
        Err(LowerError::MemoryLimitExceeded { memory_mb: 2, .. })
    ));
    assert_eq!(
        generous_hog
            .lower(&hog_file, "ALLOC_2MB\n", None)
            .expect("raised memory cap succeeds while tight-cap instance is loaded")
            .source_lines,
        1
    );

    assert!(matches!(
        short_timeout_loop.lower(
            &RepoPath::new("src/hang.loop").expect("valid repo path"),
            "LOOP_FOREVER\n",
            None,
        ),
        Err(LowerError::TimeLimitExceeded { timeout_ms: 50, .. })
    ));

    let measurement = sample_metric
        .measure(&CodeModel::default())
        .expect("metric plugin stays healthy after sibling language plugins hit limits");
    assert_eq!(measurement.denominator, 0.0);
}

#[test]
fn sdk_language_and_metric_plugins_reclaim_linear_memory_across_pooled_calls_and_import_nothing() {
    let lang_path = sample_plugin_wasm();
    let lang_bytes = std::fs::read(lang_path).expect("read sdk language wasm");
    assert!(
        verify_plugin_component("sample-mini", lang_path, PluginKind::Language, &lang_bytes)
            .is_ok()
    );
    assert!(
        verify_plugin_component("sample-mini", lang_path, PluginKind::Metric, &lang_bytes).is_err()
    );

    let metric_path = sample_metric_plugin_wasm();
    let metric_bytes = std::fs::read(metric_path).expect("read sdk metric wasm");
    assert!(
        verify_plugin_component(
            "long-functions",
            metric_path,
            PluginKind::Metric,
            &metric_bytes,
        )
        .is_ok()
    );
    assert!(
        verify_plugin_component(
            "long-functions",
            metric_path,
            PluginKind::Language,
            &metric_bytes,
        )
        .is_err()
    );

    let tight_limits = PluginLimits {
        memory_mb: 2,
        timeout_ms: 5_000,
    };
    let language = load_language_plugin_with_limits(
        Path::new("."),
        &declaration_for("sample-mini", lang_path),
        &PluginHostConfig::default(),
        tight_limits,
    )
    .expect("sdk language plugin loads under 2 MiB cap");

    let mut source = String::new();
    for index in 0..80 {
        source.push_str(&format!(
            "fn work_{index}(x) {{\n  if x {{\n    pass;\n  }}\n  return x\n}}\n"
        ));
    }
    let repo_path = RepoPath::new("src/pooled.mini").expect("valid repo path");
    for _ in 0..200 {
        let model = language
            .lower(&repo_path, &source, None)
            .expect("pooled calls reclaim linear memory under 2 MiB cap");
        assert_eq!(model.functions.len(), 80);
    }

    let metric = load_metric_plugin_with_limits(
        Path::new("."),
        &declaration_with_kind("long-functions", PluginKind::Metric, metric_path),
        &PluginHostConfig::default(),
        tight_limits,
    )
    .expect("sdk metric plugin loads under 2 MiB cap");

    let sample_model = language
        .lower(&repo_path, &source, None)
        .expect("lower sample model");
    let code_model = CodeModel {
        files: (0..20)
            .map(|index| SourceFile {
                path: RepoPath::new(format!("src/file_{index}.mini")).expect("valid repo path"),
                model: sample_model.clone(),
            })
            .collect(),
        ..CodeModel::default()
    };
    for _ in 0..100 {
        let measured = metric
            .measure(&code_model)
            .expect("pooled metric calls reclaim linear memory under 2 MiB cap");
        assert!((measured.denominator - 1_600.0).abs() < 1e-9);
    }
}

#[test]
fn manifest_paths_and_reference_ranges_propagate_wasm_traps_instead_of_returning_empty() {
    let manifest_wasm = sandbox_probe_wasm("manifest_paths_trap_language_plugin.wasm");
    let manifest_plugin = load_language_plugin(
        Path::new("."),
        &declaration_for("manifest-trap", &manifest_wasm),
        &PluginHostConfig::default(),
    )
    .expect("manifest trap language plugin loads");
    let repo_paths = vec![RepoPath::new("src/app.probe").expect("valid repo path")];
    let manifest_error = choose_manifests(&manifest_plugin, &repo_paths)
        .expect_err("trapping manifest-paths must propagate an error instead of returning empty");
    assert!(
        matches!(
            manifest_error,
            ResolveError::Plugin(LanguagePluginError::Call {
                ref plugin,
                call: "manifest-paths",
                ..
            }) if plugin == "manifest-trap"
        ),
        "unexpected manifest-paths error: {manifest_error}"
    );

    let ranges_wasm = sandbox_probe_wasm("reference_ranges_trap_language_plugin.wasm");
    let ranges_plugin = load_language_plugin(
        Path::new("."),
        &declaration_for("ranges-trap", &ranges_wasm),
        &PluginHostConfig::default(),
    )
    .expect("reference ranges trap language plugin loads");
    let ranges_error = ranges_plugin
        .reference_ranges()
        .expect_err("trapping reference-ranges must propagate an error instead of returning empty");
    let rendered = ranges_error.to_string();
    assert!(
        rendered.contains("ranges-trap") && rendered.contains("reference-ranges"),
        "unexpected reference-ranges error: {rendered}"
    );
}

#[test]
fn manifest_paths_epoch_timeout_returns_time_limit_error_instead_of_empty() {
    let loop_wasm = sandbox_probe_wasm("loop_language_plugin.wasm");
    let plugin = load_language_plugin_with_limits(
        Path::new("."),
        &declaration_for("loop-manifests", &loop_wasm),
        &PluginHostConfig::default(),
        PluginLimits {
            memory_mb: 4,
            timeout_ms: 50,
        },
    )
    .expect("loop language plugin loads");
    let repo_paths = vec![RepoPath::new("loop-manifest-paths").expect("valid repo path")];
    let error = choose_manifests(&plugin, &repo_paths)
        .expect_err("timing out manifest-paths must return TimeLimitExceeded instead of empty");
    assert!(
        matches!(
            error,
            ResolveError::Plugin(LanguagePluginError::TimeLimitExceeded {
                ref plugin,
                timeout_ms: 50,
            }) if plugin == "loop-manifests"
        ),
        "unexpected manifest-paths timeout error: {error}"
    );
}

#[test]
fn sequential_calls_to_stateful_wasm_plugins_execute_in_fresh_stores() {
    let lang_wasm = sandbox_probe_wasm("stateful_language_plugin.wasm");
    let lang_plugin = load_language_plugin(
        Path::new("."),
        &declaration_for("stateful-lang", &lang_wasm),
        &PluginHostConfig::default(),
    )
    .expect("stateful language plugin loads");
    let path = RepoPath::new("src/a.stateful").expect("valid repo path");
    let first_lang = lang_plugin
        .lower(&path, "first\n", None)
        .expect("first lower call succeeds");
    let second_lang = lang_plugin
        .lower(&path, "second\n", None)
        .expect("second lower call succeeds");
    assert_eq!(first_lang.source_lines, 1);
    assert_eq!(
        second_lang.source_lines, 1,
        "second lower call must execute in a fresh store without seeing prior call state"
    );

    let metric_wasm = sandbox_probe_wasm("stateful_metric_plugin.wasm");
    let metric_plugin = load_metric_plugin(
        Path::new("."),
        &declaration_with_kind("stateful-metric", PluginKind::Metric, &metric_wasm),
        &PluginHostConfig::default(),
    )
    .expect("stateful metric plugin loads");
    let first_metric = metric_plugin
        .measure(&CodeModel::default())
        .expect("first measure call succeeds");
    let second_metric = metric_plugin
        .measure(&CodeModel::default())
        .expect("second measure call succeeds");
    assert!((first_metric.numerator - 1.0).abs() < 1e-9);
    assert!(
        (second_metric.numerator - 1.0).abs() < 1e-9,
        "second measure call must execute in a fresh store without seeing prior call state, got {}",
        second_metric.numerator
    );
}

#[test]
fn metric_plugin_returning_oversized_lifted_payload_is_rejected_by_lifted_budget() {
    let wasm = sandbox_probe_wasm("oversized_lifted_metric_plugin.wasm");
    let limits = PluginLimits {
        memory_mb: 2,
        timeout_ms: 5_000,
    };
    let plugin = load_metric_plugin_with_limits(
        Path::new("."),
        &declaration_with_kind("oversized-lifted", PluginKind::Metric, &wasm),
        &PluginHostConfig::default(),
        limits,
    )
    .expect("oversized lifted metric plugin loads");

    let error = plugin
        .measure(&CodeModel::default())
        .expect_err("oversized lifted Metric measurement must be rejected by lifted budget");
    assert!(
        matches!(
            error,
            MetricPluginError::MemoryLimitExceeded {
                ref plugin,
                memory_mb: 2,
            } if plugin == "oversized-lifted"
        ),
        "unexpected error: {error}"
    );
}

struct LocalTestHttpsServer {
    dir: PathBuf,
    cert_path: PathBuf,
    port: u16,
    child: Option<std::process::Child>,
}

impl LocalTestHttpsServer {
    fn start(valid_wasm: &Path) -> LocalTestHttpsServer {
        use std::io::{BufRead, BufReader};
        use std::process::{Command, Stdio};
        use std::sync::atomic::{AtomicU64, Ordering};

        static SERVER_ID: AtomicU64 = AtomicU64::new(0);
        let id = SERVER_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "codeint-plugin-host-https-{}-{id}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir for https server");

        let cert_path = dir.join("cert.pem");
        let key_path = dir.join("key.pem");
        let script = concat!(
            "import http.server, os, ssl, subprocess, sys\n",
            "valid_path, cert_path, key_path, work_dir = sys.argv[1:5]\n",
            "cfg_path = os.path.join(work_dir, 'tls.cnf')\n",
            "with open(cfg_path, 'w') as fh:\n",
            "    fh.write('[req]\\ndistinguished_name=dn\\nx509_extensions=v3\\nprompt=no\\n[dn]\\nCN=127.0.0.1\\n[v3]\\nsubjectAltName=IP:127.0.0.1\\n')\n",
            "subprocess.run(['openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1', '-keyout', key_path, '-out', cert_path, '-config', cfg_path], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)\n",
            "with open(valid_path, 'rb') as fh:\n",
            "    VALID_BYTES = fh.read()\n",
            "class Handler(http.server.BaseHTTPRequestHandler):\n",
            "    protocol_version = 'HTTP/1.1'\n",
            "    def log_message(self, fmt, *args):\n",
            "        pass\n",
            "    def do_GET(self):\n",
            "        if self.path == '/valid.wasm':\n",
            "            self.send_response(200)\n",
            "            self.send_header('Content-Length', str(len(VALID_BYTES)))\n",
            "            self.send_header('Connection', 'close')\n",
            "            self.end_headers()\n",
            "            self.wfile.write(VALID_BYTES)\n",
            "        elif self.path == '/over-50mib.wasm':\n",
            "            self.send_response(200)\n",
            "            self.send_header('Content-Length', str(50 * 1024 * 1024 + 1))\n",
            "            self.send_header('Connection', 'close')\n",
            "            self.end_headers()\n",
            "            self.wfile.write(b'0' * 1024)\n",
            "        elif self.path == '/sized-64k.wasm':\n",
            "            body = b'A' * (64 * 1024)\n",
            "            self.send_response(200)\n",
            "            self.send_header('Content-Length', str(len(body)))\n",
            "            self.send_header('Connection', 'close')\n",
            "            self.end_headers()\n",
            "            self.wfile.write(body)\n",
            "        elif self.path == '/chunked-64k.wasm':\n",
            "            self.send_response(200)\n",
            "            self.send_header('Transfer-Encoding', 'chunked')\n",
            "            self.send_header('Connection', 'close')\n",
            "            self.end_headers()\n",
            "            chunk = b'B' * 4096\n",
            "            header = f'{len(chunk):x}\\r\\n'.encode('ascii')\n",
            "            try:\n",
            "                for _ in range(16):\n",
            "                    self.wfile.write(header + chunk + b'\\r\\n')\n",
            "                    self.wfile.flush()\n",
            "                self.wfile.write(b'0\\r\\n\\r\\n')\n",
            "                self.wfile.flush()\n",
            "            except OSError:\n",
            "                pass\n",
            "        else:\n",
            "            self.send_response(404)\n",
            "            self.send_header('Content-Length', '0')\n",
            "            self.send_header('Connection', 'close')\n",
            "            self.end_headers()\n",
            "server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)\n",
            "ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)\n",
            "ctx.load_cert_chain(certfile=cert_path, keyfile=key_path)\n",
            "server.socket = ctx.wrap_socket(server.socket, server_side=True)\n",
            "print(server.server_address[1], flush=True)\n",
            "server.serve_forever()\n",
        );

        let mut child = Command::new("python3")
            .args(["-c", script])
            .arg(valid_wasm)
            .arg(&cert_path)
            .arg(&key_path)
            .arg(&dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn local python3 HTTPS server");
        let stdout = child.stdout.take().expect("server stdout");
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line).expect("read server port");
        let port: u16 = line
            .trim()
            .parse()
            .unwrap_or_else(|error| panic!("invalid server port {line:?}: {error}"));

        LocalTestHttpsServer {
            dir,
            cert_path,
            port,
            child: Some(child),
        }
    }

    fn url(&self, route: &str) -> String {
        format!("https://127.0.0.1:{}/{route}", self.port)
    }

    fn cache_dir(&self) -> PathBuf {
        self.dir.join("cache")
    }
}

impl Drop for LocalTestHttpsServer {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn https_download_rejects_response_exceeding_default_50_mib_cap() {
    let default_config = PluginHostConfig::default();
    assert_eq!(default_config.max_download_bytes, 50 * 1024 * 1024);

    let server = LocalTestHttpsServer::start(sample_plugin_wasm());
    let dummy_sha =
        Sha256Digest::parse_hex("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
            .expect("valid hex");
    let declaration = PluginDeclaration {
        name: "oversized-remote".to_owned(),
        kind: PluginKind::Language,
        source: PluginSource::HttpsUrl(server.url("over-50mib.wasm")),
        sha256: dummy_sha.clone(),
    };
    let host_config = PluginHostConfig {
        cache_dir: Some(server.cache_dir()),
        ca_cert: Some(server.cert_path.clone()),
        ..PluginHostConfig::default()
    };
    let error = load_language_plugin(Path::new("."), &declaration, &host_config)
        .err()
        .expect("response exceeding 50 MiB default cap must be rejected");
    assert!(
        matches!(
            error,
            PluginHostError::DownloadTooLarge {
                ref name,
                max_bytes: 52_428_800,
                ..
            } if name == "oversized-remote"
        ),
        "expected DownloadTooLarge with 50 MiB cap, got: {error}"
    );

    let tight_cap = 16 * 1024u64;
    let tight_config = PluginHostConfig {
        cache_dir: Some(server.cache_dir()),
        ca_cert: Some(server.cert_path.clone()),
        max_download_bytes: tight_cap,
        ..PluginHostConfig::default()
    };
    for (plugin_name, route) in [
        ("oversized-sized", "sized-64k.wasm"),
        ("oversized-chunked", "chunked-64k.wasm"),
    ] {
        let oversized = PluginDeclaration {
            name: plugin_name.to_owned(),
            kind: PluginKind::Language,
            source: PluginSource::HttpsUrl(server.url(route)),
            sha256: dummy_sha.clone(),
        };
        let cap_error = load_language_plugin(Path::new("."), &oversized, &tight_config)
            .err()
            .unwrap_or_else(|| panic!("{route} must fail under {tight_cap}-byte cap"));
        assert!(
            matches!(
                cap_error,
                PluginHostError::DownloadTooLarge {
                    ref name,
                    max_bytes,
                    ..
                } if name == plugin_name && max_bytes == tight_cap
            ),
            "expected DownloadTooLarge for {route}, got: {cap_error}"
        );
    }

    let remaining: Vec<_> = std::fs::read_dir(server.cache_dir())
        .map(|entries| entries.filter_map(Result::ok).collect())
        .unwrap_or_default();
    assert!(
        remaining.is_empty(),
        "expected no temp or cached files after oversized downloads, found: {remaining:?}"
    );

    let valid_bytes = std::fs::read(sample_plugin_wasm()).expect("read sample wasm");
    let valid_sha = Sha256Digest::parse_hex(&codeint_plugin_host::sha256_hex(
        &codeint_plugin_host::sha256(&valid_bytes),
    ))
    .expect("valid sha256");
    let valid_decl = PluginDeclaration {
        name: "valid-remote".to_owned(),
        kind: PluginKind::Language,
        source: PluginSource::HttpsUrl(server.url("valid.wasm")),
        sha256: valid_sha,
    };
    let generous_config = PluginHostConfig {
        cache_dir: Some(server.cache_dir()),
        ca_cert: Some(server.cert_path.clone()),
        max_download_bytes: u64::try_from(valid_bytes.len()).expect("len fits u64"),
        ..PluginHostConfig::default()
    };
    let loaded = load_language_plugin(Path::new("."), &valid_decl, &generous_config)
        .expect("plugin within configured download cap loads cleanly");
    assert_eq!(loaded.extensions(), vec!["mini"]);
}
