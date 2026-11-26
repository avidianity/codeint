#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "language-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::language::{
    DeclaredDep, FileModel, FrameworkProfileResult, FrameworkProfileSelection, Function, Guest,
    ImportRequest, LowerError, Manifest, ReferenceRange, Resolution, SourceSet, WorkspaceError,
};

struct CrlfMiscountLanguagePlugin;

export!(CrlfMiscountLanguagePlugin);

impl Guest for CrlfMiscountLanguagePlugin {
    fn extensions() -> Vec<String> {
        vec!["crl".to_string()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        let Some((_, "crl")) = path.rsplit_once('.') else {
            return Err(LowerError::UnsupportedFile(path));
        };
        let breaks = text.matches(['\n', '\r']).count() as u32;
        let last = Function {
            line: 1,
            end_line: breaks,
            decision_points: 0,
            source_lines: 0,
            name: None,
            cognitive_complexity: 0,
            coverage: None,
        };
        Ok(probe_runtime::blank_file_model!(
            SourceSet::Production,
            probe_runtime::count_non_empty_lines(&text),
            functions: (breaks > 0).then_some(last).into_iter().collect(),
        ))
    }

    probe_runtime::plain_language_tail!();
}
