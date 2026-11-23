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
    DeclaredDep, FileModel, Guest, ImportRequest, LowerError, Manifest, ReferenceRange, Resolution,
    WorkspaceError,
};

struct FileReaderPlugin;

export!(FileReaderPlugin);

impl Guest for FileReaderPlugin {
    fn extensions() -> Vec<String> {
        fs_probe::wasi::filesystem::preopens::get_directories()
    }

    fn lower(_path: String, _text: String) -> Result<FileModel, LowerError> {
        Err(LowerError::DeclarationOnly)
    }

    fn manifest_paths(_repository_paths: Vec<String>) -> Vec<String> {
        Vec::new()
    }

    fn workspace_packages(_manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        Ok(Vec::new())
    }

    fn resolve_imports(
        _repository_paths: Vec<String>,
        _manifests: Vec<Manifest>,
        _requests: Vec<ImportRequest>,
    ) -> Vec<Resolution> {
        Vec::new()
    }

    fn declared_dependencies(_manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        Vec::new()
    }

    fn entry_points(_repository_paths: Vec<String>, _manifests: Vec<Manifest>) -> Vec<String> {
        Vec::new()
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        Vec::new()
    }
}
