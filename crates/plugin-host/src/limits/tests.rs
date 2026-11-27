use codeint_config::PluginLimits;
use wasmtime::{Config, Engine};

use super::{EpochTicker, LiftedBudget, enforce_lifted_budget};
use crate::PluginHostError;

#[test]
fn enforce_lifted_budget_rejects_excessive_bytes_and_elements() {
    let limits = PluginLimits {
        memory_mb: 1,
        timeout_ms: 1_000,
    };
    let max_bytes = limits.memory_bytes();
    let max_elements = max_bytes / 32;

    assert!(
        enforce_lifted_budget("probe", limits, LiftedBudget::new(max_elements, max_bytes)).is_ok()
    );
    assert!(matches!(
        enforce_lifted_budget("probe", limits, LiftedBudget::new(1, max_bytes + 1)),
        Err(PluginHostError::MemoryLimitExceeded { ref name, memory_mb: 1 }) if name == "probe"
    ));
    assert!(matches!(
        enforce_lifted_budget("probe", limits, LiftedBudget::new(max_elements + 1, 0)),
        Err(PluginHostError::MemoryLimitExceeded { ref name, memory_mb: 1 }) if name == "probe"
    ));
}

#[test]
fn shared_epoch_ticker_reference_counts_leases_and_stops_cleanly() {
    let mut config = Config::new();
    config.epoch_interruption(true);
    let engine = Engine::new(&config).expect("engine with epoch interruption");

    assert_eq!(EpochTicker::snapshot_for_test(), (0, false));
    let first = EpochTicker::acquire(&engine);
    assert_eq!(EpochTicker::snapshot_for_test(), (1, true));
    let second = EpochTicker::acquire(&engine);
    assert_eq!(EpochTicker::snapshot_for_test(), (2, true));
    drop(first);
    assert_eq!(EpochTicker::snapshot_for_test(), (1, true));
    drop(second);
    assert_eq!(EpochTicker::snapshot_for_test(), (0, false));
}
