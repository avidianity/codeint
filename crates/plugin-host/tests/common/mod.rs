mod wasm_fixtures;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use codeint_config::{PluginDeclaration, PluginKind, PluginSource, Sha256Digest};
use codeint_plugin_host::{sha256, sha256_hex};

static SAMPLE_WASM: OnceLock<PathBuf> = OnceLock::new();

pub(crate) fn sample_plugin_wasm() -> &'static Path {
    SAMPLE_WASM.get_or_init(|| wasm_fixtures::wasm_fixture_path("sample_mini_plugin.wasm"))
}

pub(crate) fn sandbox_probe_wasm(file_name: &str) -> PathBuf {
    wasm_fixtures::wasm_fixture_path(file_name)
}

pub(crate) fn declaration_for(name: &str, path: &Path) -> PluginDeclaration {
    declaration_with_kind(name, PluginKind::Language, path)
}

pub(crate) fn declaration_with_kind(
    name: &str,
    kind: PluginKind,
    path: &Path,
) -> PluginDeclaration {
    let bytes = std::fs::read(path).expect("read wasm file");
    let hex = sha256_hex(&sha256(&bytes));
    PluginDeclaration {
        name: name.to_owned(),
        kind,
        source: PluginSource::Path(path.to_path_buf()),
        sha256: Sha256Digest::parse_hex(&hex).expect("valid sha256 hex"),
    }
}
