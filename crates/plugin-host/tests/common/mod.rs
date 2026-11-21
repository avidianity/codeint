use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use codeint_config::{PluginDeclaration, Sha256Digest};
use codeint_plugin_host::{sha256, sha256_hex};

static SAMPLE_WASM: OnceLock<PathBuf> = OnceLock::new();

pub(crate) fn sample_plugin_wasm() -> &'static Path {
    SAMPLE_WASM.get_or_init(|| {
        let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let manifest = crate_dir.join("tests/fixtures/sample-plugin/Cargo.toml");
        let target_dir = crate_dir.join("../../target/sample-mini-plugin");
        let output = Command::new("cargo")
            .args([
                "build",
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
            .expect("invoke cargo to build sample-mini-plugin");
        assert!(
            output.status.success(),
            "building sample-mini-plugin failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        target_dir.join("wasm32-wasip2/debug/sample_mini_plugin.wasm")
    })
}

pub(crate) fn declaration_for(name: &str, path: &Path) -> PluginDeclaration {
    let bytes = std::fs::read(path).expect("read wasm file");
    let hex = sha256_hex(&sha256(&bytes));
    PluginDeclaration {
        name: name.to_owned(),
        path: path.to_path_buf(),
        sha256: Sha256Digest::parse_hex(&hex).expect("valid sha256 hex"),
    }
}
