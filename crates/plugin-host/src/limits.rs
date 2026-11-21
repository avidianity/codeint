use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use codeint_config::PluginLimits;
use wasmtime::{Engine, ResourceLimiter, Store, StoreLimits, StoreLimitsBuilder, Trap};

const EPOCH_TICK_MS: u64 = 10;

pub(crate) struct EpochTicker {
    stop: Arc<(Mutex<bool>, Condvar)>,
    handle: Option<JoinHandle<()>>,
}

impl EpochTicker {
    pub(crate) fn start(engine: Engine) -> EpochTicker {
        let stop = Arc::new((Mutex::new(false), Condvar::new()));
        let thread_stop = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            let (lock, cvar) = &*thread_stop;
            let mut stopped = lock
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            while !*stopped {
                let (guard, _) = cvar
                    .wait_timeout(stopped, Duration::from_millis(EPOCH_TICK_MS))
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                stopped = guard;
                if !*stopped {
                    engine.increment_epoch();
                }
            }
        });
        EpochTicker {
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for EpochTicker {
    fn drop(&mut self) {
        let (lock, cvar) = &*self.stop;
        if let Ok(mut stopped) = lock.lock() {
            *stopped = true;
            cvar.notify_all();
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub(crate) struct PluginStoreState {
    limits: StoreLimits,
    max_memory_bytes: usize,
    memory_limit_hit: bool,
    peak_memory_bytes: usize,
}

impl PluginStoreState {
    pub(crate) fn new(limits: PluginLimits) -> PluginStoreState {
        let max_memory_bytes = limits.memory_bytes();
        PluginStoreState {
            limits: StoreLimitsBuilder::new()
                .memory_size(max_memory_bytes)
                .trap_on_grow_failure(true)
                .build(),
            max_memory_bytes,
            memory_limit_hit: false,
            peak_memory_bytes: 0,
        }
    }

    pub(crate) fn reset_call_state(&mut self) {
        self.memory_limit_hit = false;
    }

    pub(crate) fn memory_limit_hit(&self) -> bool {
        self.memory_limit_hit
    }

    pub(crate) fn peak_memory_bytes(&self) -> usize {
        self.peak_memory_bytes
    }
}

impl ResourceLimiter for PluginStoreState {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        self.peak_memory_bytes = self.peak_memory_bytes.max(current);
        if desired > self.max_memory_bytes {
            self.memory_limit_hit = true;
        } else {
            self.peak_memory_bytes = self.peak_memory_bytes.max(desired);
        }
        self.limits.memory_growing(current, desired, maximum)
    }

    fn memory_grow_failed(&mut self, error: wasmtime::Error) -> wasmtime::Result<()> {
        self.limits.memory_grow_failed(error)
    }

    fn table_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        self.limits.table_growing(current, desired, maximum)
    }

    fn table_grow_failed(&mut self, error: wasmtime::Error) -> wasmtime::Result<()> {
        self.limits.table_grow_failed(error)
    }

    fn instances(&self) -> usize {
        self.limits.instances()
    }

    fn tables(&self) -> usize {
        self.limits.tables()
    }

    fn memories(&self) -> usize {
        self.limits.memories()
    }
}

pub(crate) enum LimitTrap {
    Memory,
    Time,
    Other,
}

pub(crate) fn new_limited_store(engine: &Engine, limits: PluginLimits) -> Store<PluginStoreState> {
    let mut store = Store::new(engine, PluginStoreState::new(limits));
    store.limiter(|state| state);
    store.epoch_deadline_trap();
    arm_call_deadline(&mut store, limits);
    store
}

pub(crate) fn arm_call_deadline(store: &mut Store<PluginStoreState>, limits: PluginLimits) {
    store.data_mut().reset_call_state();
    let ticks = limits.timeout_ms.div_ceil(EPOCH_TICK_MS).saturating_add(1);
    store.set_epoch_deadline(ticks);
}

pub(crate) fn classify_trap(store: &Store<PluginStoreState>, error: &wasmtime::Error) -> LimitTrap {
    if store.data().memory_limit_hit() {
        LimitTrap::Memory
    } else if error.downcast_ref::<Trap>() == Some(&Trap::Interrupt) {
        LimitTrap::Time
    } else {
        LimitTrap::Other
    }
}
