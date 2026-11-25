use std::path::Path;

use wasmtime::Engine;
use wasmtime::component::Component;

use super::{ComponentCache, shared_engine};

const EMPTY_COMPONENT_WASM: &[u8] = b"\0asm\x0d\x00\x01\x00";

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
