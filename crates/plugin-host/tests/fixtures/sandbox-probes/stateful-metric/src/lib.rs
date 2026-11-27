#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};

wit_bindgen::generate!({
    world: "metric-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::metric::{CodeModel, Guest, Measurement};

static CALL_COUNT: AtomicU32 = AtomicU32::new(0);

struct Probe;

export!(Probe);

impl Guest for Probe {
    probe_runtime::metric_metadata!(
        "stateful_metric",
        "Stateful metric",
        "Increments a static counter on every measure call to test per-call store isolation",
        1
    );

    fn measure(_model: CodeModel) -> Measurement {
        let count = CALL_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
        let numerator = f64::from(count);
        Measurement {
            ratio: (numerator / 10.0).min(1.0),
            numerator,
            denominator: 10.0,
            findings: Vec::new(),
        }
    }
}
