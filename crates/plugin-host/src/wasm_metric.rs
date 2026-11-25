use std::path::Path;

use codeint_config::PluginLimits;
use codeint_model::{
    CodeModel, ExternalFinding, ExternalMetricMeasurement, FileModel, ImportResolution,
    MetricPluginError, RepoPath, SourceSet, TokenKind, is_reserved_report_key,
};
use wasmtime::Store;

use crate::PluginHostError;
use crate::limits::{LimitTrap, PluginStoreState, arm_call_deadline, classify_trap};
use crate::wasm::{
    ContractSpec, InstancePool, instantiate_sandboxed_component, map_host_trap,
    prepare_sandboxed_component,
};

const METRIC_CONTRACT: ContractSpec = ContractSpec {
    interface_prefix: "codeint:plugin/metric@",
    expected_interface: "codeint:plugin/metric@0.3.0",
};

pub(crate) fn verify_metric_component(
    name: &str,
    path: &Path,
    bytes: &[u8],
) -> Result<(), PluginHostError> {
    prepare_sandboxed_component(
        name,
        path,
        bytes,
        METRIC_CONTRACT,
        bindings::MetricPluginPre::new,
    )?;
    Ok(())
}

mod bindings {
    wasmtime::component::bindgen!({
        world: "metric-plugin",
        path: "../../wit",
    });
}

use bindings::codeint::plugin::language as guest_language;
use bindings::exports::codeint::plugin::metric as guest_metric;

const RESERVED_TREND_AND_CONFIG_KEYS: [&str; 3] = ["commit", "committed_on", "dependencies"];

#[cfg(test)]
mod tests;

pub struct WasmMetricPlugin {
    plugin_name: String,
    key: String,
    display_name: String,
    description: String,
    instances: InstancePool<bindings::MetricPluginPre<PluginStoreState>, bindings::MetricPlugin>,
}

impl WasmMetricPlugin {
    pub(crate) fn from_verified_bytes(
        plugin_name: String,
        path: &Path,
        bytes: &[u8],
        limits: PluginLimits,
    ) -> Result<WasmMetricPlugin, PluginHostError> {
        let mut component = instantiate_sandboxed_component(
            &plugin_name,
            path,
            bytes,
            limits,
            METRIC_CONTRACT,
            bindings::MetricPluginPre::new,
            |pre, store| pre.instantiate(store),
        )?;
        let api = component.bindings.codeint_plugin_metric();
        arm_call_deadline(&mut component.store, limits);
        let key = api.call_key(&mut component.store).map_err(|error| {
            map_host_trap(
                &plugin_name,
                path,
                Some("key"),
                limits,
                &component.store,
                error,
            )
        })?;
        validate_metric_key(&plugin_name, &key)?;
        arm_call_deadline(&mut component.store, limits);
        let display_name = api.call_name(&mut component.store).map_err(|error| {
            map_host_trap(
                &plugin_name,
                path,
                Some("name"),
                limits,
                &component.store,
                error,
            )
        })?;
        arm_call_deadline(&mut component.store, limits);
        let description = api
            .call_description(&mut component.store)
            .map_err(|error| {
                map_host_trap(
                    &plugin_name,
                    path,
                    Some("description"),
                    limits,
                    &component.store,
                    error,
                )
            })?;
        let display_name = if display_name.trim().is_empty() {
            key.clone()
        } else {
            display_name
        };
        Ok(WasmMetricPlugin {
            plugin_name,
            key,
            display_name,
            description,
            instances: InstancePool::from_component(component, limits),
        })
    }

    pub fn peak_memory_bytes(&self) -> usize {
        self.instances.peak_memory_bytes()
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
        let limits = self.instances.limits();
        let mut instance = self.instances.checkout(
            |pre, store| pre.instantiate(store),
            |store, error| map_metric_trap(&self.plugin_name, limits, store, error),
        )?;
        arm_call_deadline(&mut instance.store, limits);
        let result = instance
            .bindings
            .codeint_plugin_metric()
            .call_measure(&mut instance.store, &guest_model);
        self.instances.record_peak_memory(&instance.store);
        match result {
            Ok(measurement) => {
                self.instances.checkin(instance);
                convert_measurement(&self.plugin_name, measurement)
            }
            Err(error) => Err(map_metric_trap(
                &self.plugin_name,
                limits,
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

pub fn validate_metric_key(plugin_name: &str, key: &str) -> Result<(), PluginHostError> {
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
    if is_reserved_metric_key(key) {
        return Err(invalid(
            "the Metric key collides with a built-in report key",
        ));
    }
    Ok(())
}

fn is_reserved_metric_key(key: &str) -> bool {
    is_reserved_report_key(key)
        || SourceSet::ALL
            .iter()
            .any(|source_set| source_set.key() == key)
        || RESERVED_TREND_AND_CONFIG_KEYS.contains(&key)
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
                    ImportResolution::Resolved(paths) => guest_language::Resolution::Resolved(
                        paths.iter().map(|path| path.as_str().to_owned()).collect(),
                    ),
                    ImportResolution::External(package) => {
                        guest_language::Resolution::External(package.clone())
                    }
                    ImportResolution::Builtin => guest_language::Resolution::Builtin,
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
        languages: model
            .languages
            .iter()
            .map(|facts| guest_metric::LanguageFacts {
                language: facts.language.clone(),
                has_coverage: facts.has_coverage,
                has_hazard_rules: facts.has_hazard_rules,
                has_imports: facts.has_imports,
            })
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
                name: function.name.clone(),
                cognitive_complexity: function.cognitive_complexity,
                coverage: function.coverage.as_ref().map(|coverage| {
                    guest_language::FunctionCoverage {
                        listed_lines: coverage.listed_lines,
                        executed_lines: coverage.executed_lines,
                    }
                }),
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
        hazard_lines: model
            .hazard_lines
            .iter()
            .map(|hazard| guest_language::HazardLine {
                line: hazard.line,
                rule: hazard.rule.clone(),
            })
            .collect(),
        runtime_imports: model.runtime_imports.clone(),
        program_start: model.program_start,
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
