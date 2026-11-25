#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

mod clock_probe {
    wit_bindgen::generate!({
        inline: "
            package wasi:clocks@0.2.0;
            interface wall-clock {
                record datetime {
                    seconds: u64,
                    nanoseconds: u32,
                }
                now: func() -> datetime;
            }
            world clock-reader {
                import wall-clock;
            }
        ",
    });
}

wit_bindgen::generate!({
    world: "language-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::language::{
    DeclaredDep, FileModel, FrameworkProfileResult, FrameworkProfileSelection, Guest,
    ImportRequest, LowerError, Manifest, ReferenceRange, Resolution, WorkspaceError,
};

struct ClockReaderPlugin;

export!(ClockReaderPlugin);

impl Guest for ClockReaderPlugin {
    fn extensions() -> Vec<String> {
        let _now = clock_probe::wasi::clocks::wall_clock::now();
        vec!["clk".to_string()]
    }

    probe_runtime::capability_probe_language_tail!();
}
