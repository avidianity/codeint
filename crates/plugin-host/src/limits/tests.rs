use codeint_config::PluginLimits;

use super::enforce_lifted_budget;
use crate::PluginHostError;

#[test]
fn enforce_lifted_budget_rejects_excessive_bytes_and_elements() {
    let limits = PluginLimits {
        memory_mb: 1,
        timeout_ms: 1_000,
    };
    let max_bytes = limits.memory_bytes();
    let max_elements = max_bytes / 32;

    assert!(enforce_lifted_budget("probe", limits, max_elements, max_bytes).is_ok());
    assert!(matches!(
        enforce_lifted_budget("probe", limits, 1, max_bytes + 1),
        Err(PluginHostError::MemoryLimitExceeded { ref name, memory_mb: 1 }) if name == "probe"
    ));
    assert!(matches!(
        enforce_lifted_budget("probe", limits, max_elements + 1, 0),
        Err(PluginHostError::MemoryLimitExceeded { ref name, memory_mb: 1 }) if name == "probe"
    ));
}
