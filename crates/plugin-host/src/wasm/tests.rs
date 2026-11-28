use std::path::Path;

use codeint_config::PluginLimits;
use wasmtime::Engine;
use wasmtime::component::Component;

use super::{ComponentCache, guest, shared_engine};
use crate::PluginHostError;
use crate::limits::{LiftedSize, enforce_lifted_budget};

const EMPTY_COMPONENT_WASM: &[u8] = b"\0asm\x0d\x00\x01\x00";

fn tight_limits() -> PluginLimits {
    PluginLimits {
        memory_mb: 1,
        timeout_ms: 1_000,
    }
}

#[test]
fn shared_engine_reuses_one_process_wide_engine() {
    let first = shared_engine("first-plugin", Path::new("first.wasm"))
        .expect("initialize shared wasmtime engine");
    let second = shared_engine("second-plugin", Path::new("second.wasm"))
        .expect("reuse shared wasmtime engine");

    assert!(Engine::same(&first, &second));
}

#[test]
fn component_cache_evicts_oldest_entries_past_capacity() {
    let engine = shared_engine("cache-test", Path::new("cache.wasm")).expect("shared engine");
    let component = Component::from_binary(&engine, EMPTY_COMPONENT_WASM)
        .expect("compile minimal empty component");
    let first_digest = [1u8; 32];
    let second_digest = [2u8; 32];
    let third_digest = [3u8; 32];

    let mut cache = ComponentCache::new();
    cache.insert(first_digest, component.clone(), 2);
    cache.insert(second_digest, component.clone(), 2);
    assert!(cache.get(&first_digest).is_some());
    assert!(cache.get(&second_digest).is_some());

    cache.insert(third_digest, component, 2);
    assert!(cache.get(&first_digest).is_none());
    assert!(cache.get(&second_digest).is_some());
    assert!(cache.get(&third_digest).is_some());
}

#[test]
fn lifted_budget_rejects_oversized_manifest_analysis() {
    let limits = tight_limits();
    let analysis = guest::ManifestAnalysis {
        workspace_packages: Vec::new(),
        invalid_manifests: vec![guest::WorkspaceError {
            path: "package.json".to_owned(),
            message: "x".repeat(limits.memory_bytes() + 1),
        }],
        dependency_manifests: Vec::new(),
        declared_dependencies: Vec::new(),
        manifest_used_dependencies: Vec::new(),
        root_dependency_inheritance: Vec::new(),
    };
    let result = enforce_lifted_budget("probe", limits, analysis.lifted_budget());
    assert!(matches!(
        result,
        Err(PluginHostError::MemoryLimitExceeded { ref name, memory_mb: 1 }) if name == "probe"
    ));
}

#[test]
fn lifted_budget_rejects_oversized_scope_analysis() {
    let limits = tight_limits();
    let analysis = guest::ScopeAnalysis {
        extra_hazards: vec![guest::FileHazards {
            path: "a.sql".to_owned(),
            hazard_lines: vec![guest::HazardLine {
                line: 1,
                rule: "r".repeat(limits.memory_bytes() + 1),
            }],
        }],
        summaries: Vec::new(),
    };
    let result = enforce_lifted_budget("probe", limits, analysis.lifted_budget());
    assert!(matches!(
        result,
        Err(PluginHostError::MemoryLimitExceeded { ref name, memory_mb: 1 }) if name == "probe"
    ));
}

fn file_model_result(
    diagnostics: Vec<guest::FileDiagnostic>,
    scope_artifact: Option<Vec<u8>>,
) -> Result<guest::FileModel, guest::LowerError> {
    Ok(guest::FileModel {
        source_set: guest::SourceSet::Production,
        source_lines: 1,
        functions: Vec::new(),
        tokens: Vec::new(),
        flagged_lines: Vec::new(),
        hazard_lines: Vec::new(),
        runtime_imports: Vec::new(),
        program_start: false,
        is_tooling_file: false,
        diagnostics,
        scope_artifact,
    })
}

#[test]
fn lifted_budget_rejects_oversized_file_diagnostics_and_scope_artifact() {
    let limits = tight_limits();
    let with_diagnostics = file_model_result(
        vec![guest::FileDiagnostic {
            line: 1,
            kind: "rejected".to_owned(),
            message: "d".repeat(limits.memory_bytes() + 1),
        }],
        None,
    );
    assert!(matches!(
        enforce_lifted_budget("probe", limits, with_diagnostics.lifted_budget()),
        Err(PluginHostError::MemoryLimitExceeded { ref name, memory_mb: 1 }) if name == "probe"
    ));

    let with_artifact = file_model_result(Vec::new(), Some(vec![0u8; limits.memory_bytes() + 1]));
    assert!(matches!(
        enforce_lifted_budget("probe", limits, with_artifact.lifted_budget()),
        Err(PluginHostError::MemoryLimitExceeded { ref name, memory_mb: 1 }) if name == "probe"
    ));
}
