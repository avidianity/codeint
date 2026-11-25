#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::String;
use alloc::vec::Vec;

mod fs_probe {
    wit_bindgen::generate!({
        inline: "
            package wasi:filesystem@0.2.0;
            interface preopens {
                get-directories: func() -> list<string>;
            }
            world file-reader {
                import preopens;
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

struct FileReaderPlugin;

export!(FileReaderPlugin);

impl Guest for FileReaderPlugin {
    fn extensions() -> Vec<String> {
        fs_probe::wasi::filesystem::preopens::get_directories()
    }

    probe_runtime::capability_probe_language_tail!();
}
