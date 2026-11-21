mod common;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use codeint_config::{PluginDeclaration, PluginHostConfig, PluginKind, PluginSource, Sha256Digest};
use codeint_model::language::{Function, ImportRequest, Resolution};
use codeint_model::{
    CodeModel, FileModel, LanguagePlugin, LowerError, Manifest, MetricPlugin, RepoPath, SourceFile,
    SourceSet, Token, TokenKind, token_text_hash,
};
use codeint_plugin_host::{
    PluginHostError, load_language_plugin, load_language_plugins, load_metric_plugin,
    load_metric_plugins,
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
    assert_eq!(model.functions[0].decision_points, 1);
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
        ],
    );
    assert_eq!(
        resolutions,
        vec![
            Resolution::Resolved("src/util.mini".to_owned()),
            Resolution::External,
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
                    },
                    Function {
                        line: 5,
                        end_line: 18,
                        decision_points: 1,
                        source_lines: 14,
                    },
                ],
                tokens: vec![Token {
                    kind: TokenKind::Other,
                    text_hash: token_text_hash("return"),
                    line: 6,
                }],
                flagged_lines: Vec::new(),
                runtime_imports: Vec::new(),
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
