use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use codeint_config::PluginLimits;
use codeint_model::{
    CodeModel, ExternalFinding, ExternalMetricMeasurement, FileModel, ImportResolution, Metric,
    MetricPluginError, RepoPath, SourceSet, TokenKind,
};
use wasmtime::{Engine, Store};

use crate::PluginHostError;
use crate::limits::{
    EpochTicker, LimitTrap, PluginStoreState, arm_call_deadline, classify_trap, new_limited_store,
};
use crate::wasm::{compile_sandboxed_component, map_host_trap};

mod bindings {
    wasmtime::component::bindgen!({
        world: "metric-plugin",
        path: "../../wit",
    });
}

use bindings::codeint::plugin::language as guest_language;
use bindings::exports::codeint::plugin::metric as guest_metric;

const RESERVED_KEYS: [&str; 3] = ["files", "source_lines", "packages"];

struct MetricPluginInstance {
    store: Store<PluginStoreState>,
    bindings: bindings::MetricPlugin,
}

pub struct WasmMetricPlugin {
    plugin_name: String,
    limits: PluginLimits,
    key: String,
    display_name: String,
    description: String,
    engine: Engine,
    _ticker: EpochTicker,
    pre: bindings::MetricPluginPre<PluginStoreState>,
    pool: Mutex<Vec<MetricPluginInstance>>,
    peak_memory_bytes: AtomicUsize,
}

impl WasmMetricPlugin {
    pub(crate) fn from_verified_bytes(
        plugin_name: String,
        path: &Path,
        bytes: &[u8],
        limits: PluginLimits,
    ) -> Result<WasmMetricPlugin, PluginHostError> {
        let (engine, instance_pre) = compile_sandboxed_component(&plugin_name, path, bytes)?;
        let ticker = EpochTicker::start(engine.clone());
        let pre = bindings::MetricPluginPre::new(instance_pre).map_err(|error| {
            PluginHostError::Instantiate {
                name: plugin_name.clone(),
                path: path.to_path_buf(),
                reason: error.to_string(),
            }
        })?;
        let mut store = new_limited_store(&engine, limits);
        let bindings = pre
            .instantiate(&mut store)
            .map_err(|error| map_host_trap(&plugin_name, path, None, limits, &store, error))?;
        let api = bindings.codeint_plugin_metric();
        arm_call_deadline(&mut store, limits);
        let key = api.call_key(&mut store).map_err(|error| {
            map_host_trap(&plugin_name, path, Some("key"), limits, &store, error)
        })?;
        validate_metric_key(&plugin_name, &key)?;
        arm_call_deadline(&mut store, limits);
        let display_name = api.call_name(&mut store).map_err(|error| {
            map_host_trap(&plugin_name, path, Some("name"), limits, &store, error)
        })?;
        arm_call_deadline(&mut store, limits);
        let description = api.call_description(&mut store).map_err(|error| {
            map_host_trap(
                &plugin_name,
                path,
                Some("description"),
                limits,
                &store,
                error,
            )
        })?;
        let display_name = if display_name.trim().is_empty() {
            key.clone()
        } else {
            display_name
        };
        let peak_memory_bytes = AtomicUsize::new(store.data().peak_memory_bytes());
        Ok(WasmMetricPlugin {
            plugin_name,
            limits,
            key,
            display_name,
            description,
            engine,
            _ticker: ticker,
            pre,
            pool: Mutex::new(vec![MetricPluginInstance { store, bindings }]),
            peak_memory_bytes,
        })
    }

    pub fn peak_memory_bytes(&self) -> usize {
        self.peak_memory_bytes.load(Ordering::Relaxed)
    }

    fn record_peak_memory(&self, store: &Store<PluginStoreState>) {
        self.peak_memory_bytes
            .fetch_max(store.data().peak_memory_bytes(), Ordering::Relaxed);
    }

    fn checkout(&self) -> Result<MetricPluginInstance, MetricPluginError> {
        if let Ok(mut guard) = self.pool.lock()
            && let Some(instance) = guard.pop()
        {
            return Ok(instance);
        }
        let mut store = new_limited_store(&self.engine, self.limits);
        let bindings = self
            .pre
            .instantiate(&mut store)
            .map_err(|error| map_metric_trap(&self.plugin_name, self.limits, &store, error))?;
        self.record_peak_memory(&store);
        Ok(MetricPluginInstance { store, bindings })
    }

    fn checkin(&self, instance: MetricPluginInstance) {
        self.record_peak_memory(&instance.store);
        if let Ok(mut guard) = self.pool.lock() {
            guard.push(instance);
        }
    }

    pub(crate) fn key_impl(&self) -> &str {
        &self.key
    }

    pub(crate) fn name_impl(&self) -> &str {
        &self.display_name
    }

    pub(crate) fn description_impl(&self) -> &str {
        &self.description
    }

    pub(crate) fn measure_impl(
        &self,
        model: &CodeModel,
    ) -> Result<ExternalMetricMeasurement, MetricPluginError> {
        let guest_model = to_guest_code_model(model);
        let mut instance = self.checkout()?;
        arm_call_deadline(&mut instance.store, self.limits);
        let result = instance
            .bindings
            .codeint_plugin_metric()
            .call_measure(&mut instance.store, &guest_model);
        self.record_peak_memory(&instance.store);
        match result {
            Ok(measurement) => {
                self.checkin(instance);
                convert_measurement(&self.plugin_name, measurement)
            }
            Err(error) => Err(map_metric_trap(
                &self.plugin_name,
                self.limits,
                &instance.store,
                error,
            )),
        }
    }
}

fn map_metric_trap(
    plugin: &str,
    limits: PluginLimits,
    store: &Store<PluginStoreState>,
    error: wasmtime::Error,
) -> MetricPluginError {
    match classify_trap(store, &error) {
        LimitTrap::Memory => MetricPluginError::MemoryLimitExceeded {
            plugin: plugin.to_owned(),
            memory_mb: limits.memory_mb,
        },
        LimitTrap::Time => MetricPluginError::TimeLimitExceeded {
            plugin: plugin.to_owned(),
            timeout_ms: limits.timeout_ms,
        },
        LimitTrap::Other => MetricPluginError::Call {
            plugin: plugin.to_owned(),
            reason: error.to_string(),
        },
    }
}

fn validate_metric_key(plugin_name: &str, key: &str) -> Result<(), PluginHostError> {
    let invalid = |reason: &'static str| PluginHostError::InvalidMetricKey {
        name: plugin_name.to_owned(),
        key: key.to_owned(),
        reason,
    };
    if key.is_empty() {
        return Err(invalid("the Metric key must not be empty"));
    }
    let valid_chars = key.as_bytes()[0].is_ascii_lowercase()
        && key.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        });
    if !valid_chars {
        return Err(invalid(
            "the Metric key must start with a lowercase ASCII letter and contain only lowercase ASCII letters, digits, underscores, or hyphens",
        ));
    }
    if Metric::BUILT_IN_KEYS.contains(&key) || RESERVED_KEYS.contains(&key) {
        return Err(invalid(
            "the Metric key collides with a built-in report key",
        ));
    }
    Ok(())
}

fn to_guest_code_model(model: &CodeModel) -> guest_metric::CodeModel {
    guest_metric::CodeModel {
        files: model
            .files
            .iter()
            .map(|file| guest_metric::SourceFile {
                path: file.path.as_str().to_owned(),
                model: to_guest_file_model(&file.model),
            })
            .collect(),
        runtime_imports: model
            .runtime_imports
            .iter()
            .map(|import| guest_metric::RuntimeImport {
                importer: import.importer.as_str().to_owned(),
                specifier: import.specifier.clone(),
                resolution: match &import.resolution {
                    ImportResolution::Resolved(path) => {
                        guest_language::Resolution::Resolved(path.as_str().to_owned())
                    }
                    ImportResolution::External => guest_language::Resolution::External,
                    ImportResolution::Unresolved { reason } => {
                        guest_language::Resolution::Unresolved(reason.clone())
                    }
                },
            })
            .collect(),
        workspace_packages: model
            .workspace
            .member_roots()
            .map(|root| root.as_str().to_owned())
            .collect(),
    }
}

fn to_guest_file_model(model: &FileModel) -> guest_language::FileModel {
    guest_language::FileModel {
        source_set: match model.source_set {
            SourceSet::Production => guest_language::SourceSet::Production,
            SourceSet::Test => guest_language::SourceSet::Test,
            SourceSet::Generated => guest_language::SourceSet::Generated,
            SourceSet::Vendored => guest_language::SourceSet::Vendored,
        },
        source_lines: model.source_lines,
        functions: model
            .functions
            .iter()
            .map(|function| guest_language::Function {
                line: function.line,
                end_line: function.end_line,
                decision_points: function.decision_points,
                source_lines: function.source_lines,
            })
            .collect(),
        tokens: model
            .tokens
            .iter()
            .map(|token| guest_language::Token {
                kind: match token.kind {
                    TokenKind::Identifier => guest_language::TokenKind::Identifier,
                    TokenKind::Literal => guest_language::TokenKind::Literal,
                    TokenKind::Other => guest_language::TokenKind::Other,
                    TokenKind::ImportOrExport => guest_language::TokenKind::ImportOrExport,
                },
                text_hash: token.text_hash,
                line: token.line,
            })
            .collect(),
        flagged_lines: model
            .flagged_lines
            .iter()
            .map(|flagged| guest_language::FlaggedLine {
                line: flagged.line,
                rule: flagged.rule.clone(),
            })
            .collect(),
        runtime_imports: model.runtime_imports.clone(),
    }
}

fn convert_measurement(
    plugin: &str,
    measurement: guest_metric::Measurement,
) -> Result<ExternalMetricMeasurement, MetricPluginError> {
    if !measurement.ratio.is_finite() || !(0.0..=1.0).contains(&measurement.ratio) {
        return Err(MetricPluginError::InvalidRatio {
            plugin: plugin.to_owned(),
            ratio: measurement.ratio,
        });
    }
    if !measurement.numerator.is_finite() || measurement.numerator < 0.0 {
        return Err(MetricPluginError::InvalidNumber {
            plugin: plugin.to_owned(),
            field: "numerator",
            value: measurement.numerator,
        });
    }
    if !measurement.denominator.is_finite() || measurement.denominator < 0.0 {
        return Err(MetricPluginError::InvalidNumber {
            plugin: plugin.to_owned(),
            field: "denominator",
            value: measurement.denominator,
        });
    }
    let findings = measurement
        .findings
        .into_iter()
        .map(|finding| {
            if finding.id.trim().is_empty() {
                return Err(MetricPluginError::EmptyFindingId {
                    plugin: plugin.to_owned(),
                });
            }
            if finding.line == 0 {
                return Err(MetricPluginError::InvalidFindingLine {
                    plugin: plugin.to_owned(),
                });
            }
            let path = RepoPath::new(finding.path.clone()).map_err(|_| {
                MetricPluginError::InvalidFindingPath {
                    plugin: plugin.to_owned(),
                    path: finding.path,
                }
            })?;
            Ok(ExternalFinding {
                id: finding.id,
                path,
                line: finding.line,
                end_line: finding.end_line,
                message: finding.message,
            })
        })
        .collect::<Result<Vec<_>, MetricPluginError>>()?;
    Ok(ExternalMetricMeasurement {
        ratio: measurement.ratio,
        numerator: measurement.numerator,
        denominator: measurement.denominator,
        findings,
    })
}
