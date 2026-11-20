#![forbid(unsafe_code)]

use codeint_model::{FileModel, LanguagePlugin, LowerError, RepoPath};

pub struct TypeScriptAndJavaScript;

impl LanguagePlugin for TypeScriptAndJavaScript {
    fn extensions(&self) -> Vec<String> {
        todo!()
    }

    fn lower(&self, _path: &RepoPath, _text: &str) -> Result<FileModel, LowerError> {
        todo!()
    }
}
