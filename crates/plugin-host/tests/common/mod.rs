use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use codeint_config::{PluginDeclaration, PluginKind, Sha256Digest};
use codeint_plugin_host::{sha256, sha256_hex};

static SAMPLE_WASM: OnceLock<PathBuf> = OnceLock::new();

pub(crate) fn build_fixture_workspace(
    manifest_relative: &str,
    target_name: &str,
    workspace: bool,
) -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = crate_dir.join(manifest_relative);
    let target_dir = crate_dir.join("../../target").join(target_name);
    let mut command = Command::new("cargo");
    command.arg("build");
    if workspace {
        command.arg("--workspace");
    }
    let output = command
        .args([
            "--manifest-path",
            manifest.to_str().expect("UTF-8 manifest path"),
            "--target",
            "wasm32-wasip2",
            "--locked",
            "--offline",
            "--target-dir",
            target_dir.to_str().expect("UTF-8 target dir"),
        ])
        .output()
        .expect("invoke cargo to build wasm fixture");
    assert!(
        output.status.success(),
        "building {target_name} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    target_dir.join("wasm32-wasip2/debug")
}

pub(crate) fn sample_plugin_wasm() -> &'static Path {
    SAMPLE_WASM.get_or_init(|| {
        build_fixture_workspace(
            "tests/fixtures/sample-plugin/Cargo.toml",
            "wasm-fixtures",
            false,
        )
        .join("sample_mini_plugin.wasm")
    })
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
        path: path.to_path_buf(),
        sha256: Sha256Digest::parse_hex(&hex).expect("valid sha256 hex"),
    }
}
