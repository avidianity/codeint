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
    CodeModel, FileModel, LanguageFacts, LanguagePlugin, LanguagePluginError, LowerError, Manifest,
    MetricPlugin, MetricPluginError, RepoPath, SourceFile, SourceSet, Token, TokenKind,
    token_text_hash,
};
use codeint_plugin_host::{
    PLUGIN_INTERFACE_VERSION, PluginHostError, WasmLanguagePlugin, load_language_plugin,
    load_language_plugin_with_limits, load_language_plugins, load_metric_plugin,
    load_metric_plugin_with_limits, load_metric_plugins, verify_plugin_component,
};

use common::{build_fixture_workspace, declaration_for, declaration_with_kind, sample_plugin_wasm};

static SAMPLE_METRIC_WASM: OnceLock<PathBuf> = OnceLock::new();
static SANDBOX_PROBES_DIR: OnceLock<PathBuf> = OnceLock::new();

fn sample_metric_plugin_wasm() -> &'static Path {
    SAMPLE_METRIC_WASM.get_or_init(|| {
        build_fixture_workspace(
            "tests/fixtures/sample-metric-plugin/Cargo.toml",
            "wasm-fixtures",
            false,
        )
        .join("sample_long_functions_plugin.wasm")
    })
}

fn sandbox_probe_wasm(file_name: &str) -> PathBuf {
    SANDBOX_PROBES_DIR
        .get_or_init(|| {
            build_fixture_workspace(
                "tests/fixtures/sandbox-probes/Cargo.toml",
                "wasm-fixtures",
                true,
            )
        })
        .join(file_name)
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
        plugin.lower(&RepoPath::new("src/types.d.mini").unwrap(), "fn x() {}\n"),
        Err(LowerError::DeclarationOnly)
    ));

    let repo_paths = vec![
        RepoPath::new("mini.workspace").unwrap(),
        RepoPath::new("src/app.mini").unwrap(),
        RepoPath::new("src/util.mini").unwrap(),
    ];
    assert_eq!(plugin.manifest_paths(&repo_paths), vec!["mini.workspace"]);
    let packages = plugin
        .workspace_packages(vec![Manifest {
            path: "mini.workspace".to_owned(),
            text: "packages/core\n".to_owned(),
        }])
        .expect("workspace packages");
    assert_eq!(packages, vec![RepoPath::new("packages/core").unwrap()]);

    let resolutions = plugin.resolve_imports(
        &repo_paths,
        Vec::new(),
        vec![
            ImportRequest {
                importer: "src/app.mini".to_owned(),
                specifier: "./util".to_owned(),
            },
            ImportRequest {
                importer: "src/app.mini".to_owned(),
                specifier: "external-pkg".to_owned(),
            },
            ImportRequest {
                importer: "src/app.mini".to_owned(),
                specifier: "std:io".to_owned(),
            },
        ],
    );
    assert_eq!(
        resolutions,
        vec![
            Resolution::Resolved("src/util.mini".to_owned()),
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
                flagged_lines: Vec::new(),
                runtime_imports: Vec::new(),
                program_start: false,
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

#[test]
fn language_plugin_that_loops_forever_hits_time_limit_and_still_lowers_other_files() {
    let wasm_path = sandbox_probe_wasm("loop_language_plugin.wasm");
    let declaration = declaration_for("loop-lang", &wasm_path);
    let limits = PluginLimits {
        memory_mb: 4,
        timeout_ms: 50,
    };
    let plugin = load_language_plugin_with_limits(
        Path::new("."),
        &declaration,
        &PluginHostConfig::default(),
        limits,
    )
    .expect("loop language plugin loads");

    let error = plugin
        .lower(
            &RepoPath::new("src/hang.loop").expect("valid repo path"),
            "LOOP_FOREVER\n",
        )
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

    let ok = plugin
        .lower(
            &RepoPath::new("src/ok.loop").expect("valid repo path"),
            "line 1\nline 2\n",
        )
        .expect("healthy file lowers after time-limit trap");
    assert_eq!(ok.source_lines, 2);
}

#[test]
fn language_plugin_that_exceeds_memory_cap_fails_under_tight_cap_and_succeeds_when_raised() {
    let wasm_path = sandbox_probe_wasm("memory_hog_language_plugin.wasm");
    let declaration = declaration_for("hog-lang", &wasm_path);

    let tight = PluginLimits {
        memory_mb: 2,
        timeout_ms: 1_000,
    };
    let tight_plugin = load_language_plugin_with_limits(
        Path::new("."),
        &declaration,
        &PluginHostConfig::default(),
        tight,
    )
    .expect("memory hog plugin loads under 2 MiB cap");
    let error = tight_plugin
        .lower(
            &RepoPath::new("src/big.hog").expect("valid repo path"),
            "ALLOC_2MB\n",
        )
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
    let ok = tight_plugin
        .lower(
            &RepoPath::new("src/small.hog").expect("valid repo path"),
            "small\n",
        )
        .expect("small file lowers after memory-limit trap");
    assert_eq!(ok.source_lines, 1);

    let raised = PluginLimits {
        memory_mb: 8,
        timeout_ms: 1_000,
    };
    let raised_plugin = load_language_plugin_with_limits(
        Path::new("."),
        &declaration,
        &PluginHostConfig::default(),
        raised,
    )
    .expect("memory hog plugin loads under 8 MiB cap");
    let measured = raised_plugin
        .lower(
            &RepoPath::new("src/big.hog").expect("valid repo path"),
            "ALLOC_2MB\n",
        )
        .expect("2 MiB allocation succeeds when memory cap is raised to 8 MiB");
    assert_eq!(measured.source_lines, 1);
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
                source_set: SourceSet::Production,
                source_lines: 1,
                functions: Vec::new(),
                tokens: Vec::new(),
                flagged_lines: Vec::new(),
                runtime_imports: Vec::new(),
                program_start: false,
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
        .lower(&ts_path, &large_ts)
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
        .lower(&mini_path, &large_mini)
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
fn verifies_0_3_0_contract_fields_and_methods_across_wasm_boundary() {
    assert_eq!(PLUGIN_INTERFACE_VERSION, "0.3.0");
    let wasm_path = sample_plugin_wasm();
    let declaration = declaration_for("sample-mini", wasm_path);
    let plugin = load_language_plugin(Path::new("."), &declaration, &PluginHostConfig::default())
        .expect("0.3.0 sample plugin loads");

    let lowered = plugin
        .lower(
            &RepoPath::new("src/main.mini").expect("valid repo path"),
            "fn outer(x) {\n  if x {\n    while x {\n      return x\n    }\n  }\n}\nfn() {\n  return 1\n}\n",
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
    assert!(!lowered.program_start);

    let repo_paths = vec![
        RepoPath::new("mini.workspace").expect("valid repo path"),
        RepoPath::new("src/main.mini").expect("valid repo path"),
        RepoPath::new("src/util.mini").expect("valid repo path"),
    ];
    let manifests = vec![Manifest {
        path: "mini.workspace".to_owned(),
        text: "packages/core\ndep:serde\ndev-dep:testkit\nopt-dep:extras\nentry:src/main.mini\nentry:src/missing.mini\n".to_owned(),
    }];

    assert_eq!(
        plugin
            .declared_dependencies(manifests.clone())
            .expect("valid declared dependencies"),
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
        plugin.resolve_imports(
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
        ),
        vec![
            Resolution::Resolved("src/util.mini".to_owned()),
            Resolution::External("serde".to_owned()),
            Resolution::Builtin,
            Resolution::Unresolved("cannot find src/missing.mini".to_owned()),
        ]
    );
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
        .expect("0.1.0 Language plugin must be rejected by 0.3.0 host");

    assert_incompatible_contract_error(
        error,
        "legacy-lang",
        "codeint:plugin/language@0.3.0",
        "codeint:plugin/language@0.1.0",
    );
}

#[test]
fn rejects_v0_1_0_metric_plugin_with_actionable_version_mismatch_error() {
    let wasm_path = sandbox_probe_wasm("legacy_v0_1_0_metric_plugin.wasm");
    let declaration = declaration_with_kind("legacy-metric", PluginKind::Metric, &wasm_path);

    let error = load_metric_plugin(Path::new("."), &declaration, &PluginHostConfig::default())
        .err()
        .expect("0.1.0 Metric plugin must be rejected by 0.3.0 host");

    assert_incompatible_contract_error(
        error,
        "legacy-metric",
        "codeint:plugin/metric@0.3.0",
        "codeint:plugin/metric@0.1.0",
    );
}

#[test]
fn rejects_v0_2_0_language_and_metric_plugins_with_actionable_version_mismatch_error() {
    let lang_path = sandbox_probe_wasm("legacy_v0_2_0_language_plugin.wasm");
    let lang_decl = declaration_for("legacy-v02-lang", &lang_path);
    let lang_err = load_language_plugin(Path::new("."), &lang_decl, &PluginHostConfig::default())
        .err()
        .expect("0.2.0 Language plugin must be rejected by 0.3.0 host");
    assert_incompatible_contract_error(
        lang_err,
        "legacy-v02-lang",
        "codeint:plugin/language@0.3.0",
        "codeint:plugin/language@0.2.0",
    );

    let metric_path = sandbox_probe_wasm("legacy_v0_2_0_metric_plugin.wasm");
    let metric_decl = declaration_with_kind("legacy-v02-metric", PluginKind::Metric, &metric_path);
    let metric_err = load_metric_plugin(Path::new("."), &metric_decl, &PluginHostConfig::default())
        .err()
        .expect("0.2.0 Metric plugin must be rejected by 0.3.0 host");
    assert_incompatible_contract_error(
        metric_err,
        "legacy-v02-metric",
        "codeint:plugin/metric@0.3.0",
        "codeint:plugin/metric@0.2.0",
    );

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
    let legacy_lang_bytes = std::fs::read(&lang_path).expect("read legacy 0.2.0 language wasm");
    let verify_lang_err = verify_plugin_component(
        "legacy-v02-lang",
        &lang_path,
        PluginKind::Language,
        &legacy_lang_bytes,
    )
    .expect_err("0.2.0 language component must fail contract verification");
    assert_incompatible_contract_error(
        verify_lang_err,
        "legacy-v02-lang",
        "codeint:plugin/language@0.3.0",
        "codeint:plugin/language@0.2.0",
    );
    let legacy_metric_bytes = std::fs::read(&metric_path).expect("read legacy 0.2.0 metric wasm");
    let verify_metric_err = verify_plugin_component(
        "legacy-v02-metric",
        &metric_path,
        PluginKind::Metric,
        &legacy_metric_bytes,
    )
    .expect_err("0.2.0 metric component must fail contract verification");
    assert_incompatible_contract_error(
        verify_metric_err,
        "legacy-v02-metric",
        "codeint:plugin/metric@0.3.0",
        "codeint:plugin/metric@0.2.0",
    );
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
fn declared_dependencies_and_entry_points_propagate_timeout_trap_and_oom_and_recover() {
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
        loop_plugin.declared_dependencies(trigger_manifest("LOOP_FOREVER")),
        Err(LanguagePluginError::TimeLimitExceeded {
            ref plugin,
            timeout_ms: 150,
        }) if plugin == "loop-lang"
    ));
    assert!(matches!(
        loop_plugin.declared_dependencies(trigger_manifest("PANIC_TRAP")),
        Err(LanguagePluginError::Call {
            ref plugin,
            call: "declared-dependencies",
            ..
        }) if plugin == "loop-lang"
    ));
    assert_eq!(
        loop_plugin
            .declared_dependencies(Vec::new())
            .expect("healthy declared_dependencies succeeds after trap")
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
        hog_plugin.declared_dependencies(trigger_manifest("ALLOC_2MB")),
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
        plugin.declared_dependencies(trigger_manifest("ALIASED_BOMB")),
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
            plugin.declared_dependencies(trigger_manifest(directive)),
            Err(LanguagePluginError::EmptyDependencyName { .. })
        ));
    }
    for (directive, expected) in [
        ("INVALID_TRAVERSAL_ROOT", "../escape"),
        ("INVALID_ABSOLUTE_ROOT", "/etc/shadow"),
        ("INVALID_BACKSLASH_ROOT", "packages\\app"),
    ] {
        assert!(matches!(
            plugin.declared_dependencies(trigger_manifest(directive)),
            Err(LanguagePluginError::InvalidDependencyPackageRoot { ref package_root })
                if package_root == expected
        ));
    }
}

#[test]
fn loads_dual_version_language_plugin_exporting_0_2_0_and_0_3_0() {
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
            .declared_dependencies(Vec::new())
            .expect("dual plugin declared_dependencies"),
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
                source_set: SourceSet::Production,
                source_lines: 30,
                functions,
                tokens: Vec::new(),
                flagged_lines: Vec::new(),
                runtime_imports: Vec::new(),
                program_start: false,
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

#[test]
fn language_plugin_sets_program_start_mark_and_metric_plugin_reads_it() {
    let language_wasm = sandbox_probe_wasm("program_start_language_plugin.wasm");
    let language_decl = declaration_for("program-start-language", &language_wasm);
    let language =
        load_language_plugin(Path::new("."), &language_decl, &PluginHostConfig::default())
            .expect("program-start Language plugin loads");

    let metric_wasm = sandbox_probe_wasm("program_start_metric_plugin.wasm");
    let metric_decl =
        declaration_with_kind("program-start-metric", PluginKind::Metric, &metric_wasm);
    let metric = load_metric_plugin(Path::new("."), &metric_decl, &PluginHostConfig::default())
        .expect("program-start Metric plugin loads");

    let main_path = RepoPath::new("cmd/app/main.prog").expect("valid repo path");
    let lib_path = RepoPath::new("internal/util/helper.prog").expect("valid repo path");

    let main_model = language
        .lower(&main_path, "program main\nrun()\n")
        .expect("lowers program-start file");
    let lib_model = language
        .lower(&lib_path, "library util\nhelper()\n")
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
            .lower(&RepoPath::new(path).expect("valid repo path"), source)
            .unwrap_or_else(|error| panic!("{name}: lower failed: {error}"));
        assert!(
            !model.program_start,
            "{name} built-in Language plugin must leave program_start false"
        );
    }
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
        }],
        ..coverage_model_with(Vec::new())
    };
    let covered_language = CodeModel {
        languages: vec![LanguageFacts {
            language: "typescript-javascript".to_owned(),
            has_coverage: true,
        }],
        ..coverage_model_with(Vec::new())
    };

    let absent_measured = plugin
        .measure(&absent_language)
        .expect("measure language without coverage");
    let covered_measured = plugin
        .measure(&covered_language)
        .expect("measure language with coverage");

    fn summarize(measurement: &codeint_model::ExternalMetricMeasurement) -> Vec<(&str, &str)> {
        measurement
            .findings
            .iter()
            .map(|finding| (finding.id.as_str(), finding.message.as_str()))
            .collect()
    }

    assert_eq!(
        summarize(&absent_measured),
        vec![(
            "language:typescript-javascript:absent",
            "language=typescript-javascript has_coverage=false",
        )]
    );
    assert_eq!(
        summarize(&covered_measured),
        vec![(
            "language:typescript-javascript:covered",
            "language=typescript-javascript has_coverage=true",
        )]
    );

    let rust_covered = CodeModel {
        files: vec![SourceFile {
            path: RepoPath::new("src/lib.rs").expect("valid repo path"),
            model: FileModel {
                source_set: SourceSet::Production,
                source_lines: 10,
                functions: Vec::new(),
                tokens: Vec::new(),
                flagged_lines: Vec::new(),
                runtime_imports: Vec::new(),
                program_start: false,
            },
        }],
        languages: vec![LanguageFacts {
            language: "rust".to_owned(),
            has_coverage: true,
        }],
        ..CodeModel::default()
    };
    let merged = CodeModel::merge([absent_language, rust_covered]);
    let source_sets = merged.into_source_sets();

    let multi_measured = plugin
        .measure(source_sets.get(SourceSet::Production))
        .expect("measure merged multi-language model");

    assert_eq!(
        summarize(&multi_measured),
        vec![
            ("language:rust:covered", "language=rust has_coverage=true"),
            (
                "language:typescript-javascript:absent",
                "language=typescript-javascript has_coverage=false",
            ),
        ]
    );
}
