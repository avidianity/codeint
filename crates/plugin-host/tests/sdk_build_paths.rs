mod common;

use std::path::{Path, PathBuf};

use codeint_config::{PluginHostConfig, PluginKind};
use codeint_model::{CodeModel, LanguagePlugin, MetricPlugin, RepoPath, SourceFile};
use codeint_plugin_host::{load_language_plugin, load_metric_plugin, verify_plugin_component};

use common::{declaration_for, declaration_with_kind, sample_plugin_wasm, sandbox_probe_wasm};

const PLUGIN_INTERFACE_PREFIX: &str = "codeint:plugin/";

fn componentize_std_core_module(core_module_file: &str, component_file: &str) -> PathBuf {
    let core_path = sandbox_probe_wasm(core_module_file);
    let core_bytes = std::fs::read(&core_path).expect("read std core module");
    let component_bytes = wit_component::ComponentEncoder::default()
        .validate(true)
        .module(&core_bytes)
        .expect("std core module carries codeint:plugin@0.4.0 component metadata")
        .encode()
        .expect("encode std core module as a component without a WASI adapter");
    let component_path = core_path.with_file_name(component_file);
    let staging_path =
        core_path.with_file_name(format!("{component_file}.{}.tmp", std::process::id()));
    std::fs::write(&staging_path, component_bytes).expect("write std component");
    std::fs::rename(&staging_path, &component_path).expect("promote std component");
    component_path
}

fn component_import_names(path: &Path) -> Vec<String> {
    let engine = wasmtime::Engine::default();
    let component =
        wasmtime::component::Component::from_file(&engine, path).expect("compile component");
    component
        .component_type()
        .imports(&engine)
        .map(|(name, _)| name.to_owned())
        .collect()
}

fn assert_only_plugin_interface_imports(path: &Path) {
    let imports = component_import_names(path);
    for import in &imports {
        assert!(
            import.starts_with(PLUGIN_INTERFACE_PREFIX),
            "{} imports {import}; codeint-plugin-sdk components must import only {PLUGIN_INTERFACE_PREFIX}* types",
            path.display()
        );
    }
}

fn sample_mini_source() -> String {
    let mut source = String::new();
    for index in 0..12 {
        source.push_str(&format!(
            "fn work_{index}(x) {{\n  if x {{\n    pass;\n  }}\n  return x\n}}\n"
        ));
    }
    source.push_str("fn long_one(x) {\n");
    for index in 0..14 {
        source.push_str(&format!("  let v{index} = x\n"));
    }
    source.push_str("  return x\n}\n");
    source
}

#[test]
fn sdk_no_std_wasip2_and_std_unknown_unknown_components_import_no_wasi_interfaces() {
    let std_language = componentize_std_core_module(
        "sample_mini_plugin_std_core.wasm",
        "sample_mini_plugin_std_component.wasm",
    );
    let std_metric = componentize_std_core_module(
        "sample_long_functions_plugin_std_core.wasm",
        "sample_long_functions_plugin_std_component.wasm",
    );
    let no_std_metric = sandbox_probe_wasm("sample_long_functions_plugin.wasm");

    for component in [
        sample_plugin_wasm(),
        no_std_metric.as_path(),
        std_language.as_path(),
        std_metric.as_path(),
    ] {
        assert_only_plugin_interface_imports(component);
    }
    assert!(component_import_names(sample_plugin_wasm()).is_empty());
    assert!(component_import_names(&std_language).is_empty());
}

#[test]
fn sdk_std_unknown_unknown_plugins_load_and_match_no_std_wasip2_results() {
    let std_language_path = componentize_std_core_module(
        "sample_mini_plugin_std_core.wasm",
        "sample_mini_plugin_std_language.wasm",
    );
    let std_metric_path = componentize_std_core_module(
        "sample_long_functions_plugin_std_core.wasm",
        "sample_long_functions_plugin_std_metric.wasm",
    );
    let no_std_metric_path = sandbox_probe_wasm("sample_long_functions_plugin.wasm");

    let std_language_bytes = std::fs::read(&std_language_path).expect("read std language");
    verify_plugin_component(
        "sample-mini-std",
        &std_language_path,
        PluginKind::Language,
        &std_language_bytes,
    )
    .expect("std language component exports codeint:plugin/language@0.4.0");
    let std_metric_bytes = std::fs::read(&std_metric_path).expect("read std metric");
    verify_plugin_component(
        "long-functions-std",
        &std_metric_path,
        PluginKind::Metric,
        &std_metric_bytes,
    )
    .expect("std metric component exports codeint:plugin/metric@0.4.0");

    let host_config = PluginHostConfig::default();
    let no_std_language = load_language_plugin(
        Path::new("."),
        &declaration_for("sample-mini", sample_plugin_wasm()),
        &host_config,
    )
    .expect("no_std language plugin loads");
    let std_language = load_language_plugin(
        Path::new("."),
        &declaration_for("sample-mini-std", &std_language_path),
        &host_config,
    )
    .expect("std language plugin loads under the empty Linker");

    assert_eq!(std_language.extensions(), no_std_language.extensions());
    let repo_path = RepoPath::new("src/paths.mini").expect("valid repo path");
    let source = sample_mini_source();
    let no_std_model = no_std_language
        .lower(&repo_path, &source)
        .expect("no_std lower succeeds");
    let std_model = std_language
        .lower(&repo_path, &source)
        .expect("std lower succeeds");
    assert_eq!(std_model, no_std_model);
    assert_eq!(std_model.functions.len(), 13);

    let no_std_metric = load_metric_plugin(
        Path::new("."),
        &declaration_with_kind("long-functions", PluginKind::Metric, &no_std_metric_path),
        &host_config,
    )
    .expect("no_std metric plugin loads");
    let std_metric = load_metric_plugin(
        Path::new("."),
        &declaration_with_kind("long-functions-std", PluginKind::Metric, &std_metric_path),
        &host_config,
    )
    .expect("std metric plugin loads under the empty Linker");

    let code_model = CodeModel {
        files: vec![SourceFile {
            path: repo_path,
            model: std_model,
        }],
        ..CodeModel::default()
    };
    let no_std_measurement = no_std_metric
        .measure(&code_model)
        .expect("no_std measure succeeds");
    let std_measurement = std_metric
        .measure(&code_model)
        .expect("std measure succeeds");
    assert_eq!(std_measurement, no_std_measurement);
    assert_eq!(std_measurement.numerator, 1.0);
    assert_eq!(std_measurement.denominator, 13.0);
}
