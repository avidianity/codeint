use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use codeint_plugin_host::{sha256, sha256_hex};

const STAMP_FILE: &str = ".fixtures-stamp";
const LOCK_FILE: &str = ".build-lock";

const FIXTURE_BUILDS: [(&str, bool); 3] = [
    (
        "crates/plugin-host/tests/fixtures/sample-plugin/Cargo.toml",
        false,
    ),
    (
        "crates/plugin-host/tests/fixtures/sample-metric-plugin/Cargo.toml",
        false,
    ),
    (
        "crates/plugin-host/tests/fixtures/sandbox-probes/Cargo.toml",
        true,
    ),
];

const STD_CORE_MODULE_BUILDS: [(&str, &str, &str); 2] = [
    (
        "crates/plugin-host/tests/fixtures/sample-plugin/Cargo.toml",
        "sample_mini_plugin.wasm",
        "sample_mini_plugin_std_core.wasm",
    ),
    (
        "crates/plugin-host/tests/fixtures/sample-metric-plugin/Cargo.toml",
        "sample_long_functions_plugin.wasm",
        "sample_long_functions_plugin_std_core.wasm",
    ),
];

static FIXTURES_DIR: OnceLock<PathBuf> = OnceLock::new();
static IN_PROCESS_LOCK: Mutex<()> = Mutex::new(());

pub(crate) fn wasm_fixture_path(file_name: &str) -> PathBuf {
    let dir = FIXTURES_DIR.get_or_init(|| ensure_fixtures_dir(file_name));
    let path = dir.join(file_name);
    if path.is_file() {
        path
    } else {
        ensure_fixtures_dir(file_name).join(file_name)
    }
}

fn ensure_fixtures_dir(required_file: &str) -> PathBuf {
    let root = workspace_root();
    let target_dir = root.join("target/wasm-fixtures");
    let output_dir = target_dir.join("wasm32-wasip2/debug");
    let expected_stamp = compute_fixtures_stamp(&root);
    if fixtures_are_fresh(&output_dir, &expected_stamp, required_file) {
        return output_dir;
    }

    let _guard = IN_PROCESS_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    fs::create_dir_all(&target_dir).expect("create wasm-fixtures target directory");
    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(target_dir.join(LOCK_FILE))
        .expect("open wasm-fixtures build lock");
    lock_file
        .lock()
        .expect("acquire exclusive wasm-fixtures build lock");

    if !fixtures_are_fresh(&output_dir, &expected_stamp, required_file) {
        let _ = fs::remove_file(output_dir.join(STAMP_FILE));
        build_all_fixtures(&root, &target_dir);
        write_stamp_atomically(&output_dir, &expected_stamp);
    }
    output_dir
}

fn fixtures_are_fresh(output_dir: &Path, expected_stamp: &str, required_file: &str) -> bool {
    output_dir.join(required_file).is_file()
        && fs::read_to_string(output_dir.join(STAMP_FILE))
            .is_ok_and(|actual| actual.trim() == expected_stamp)
}

fn build_all_fixtures(root: &Path, target_dir: &Path) {
    for (manifest_relative, workspace) in FIXTURE_BUILDS {
        let extra_args: &[&str] = if workspace { &["--workspace"] } else { &[] };
        run_fixture_cargo_build(
            &root.join(manifest_relative),
            target_dir,
            "wasm32-wasip2",
            extra_args,
        );
    }
    let output_dir = target_dir.join("wasm32-wasip2/debug");
    let std_dir = target_dir.join("wasm32-unknown-unknown/debug");
    for (manifest_relative, built_file, core_module_file) in STD_CORE_MODULE_BUILDS {
        run_fixture_cargo_build(
            &root.join(manifest_relative),
            target_dir,
            "wasm32-unknown-unknown",
            &["--features", "codeint-plugin-sdk/std"],
        );
        fs::copy(std_dir.join(built_file), output_dir.join(core_module_file))
            .expect("copy std core module next to wasm32-wasip2 fixtures");
    }
}

fn run_fixture_cargo_build(manifest: &Path, target_dir: &Path, target: &str, extra_args: &[&str]) {
    let output = Command::new("cargo")
        .arg("build")
        .args(extra_args)
        .args([
            "--manifest-path",
            manifest.to_str().expect("UTF-8 manifest path"),
            "--target",
            target,
            "--locked",
            "--offline",
            "--target-dir",
            target_dir.to_str().expect("UTF-8 target dir"),
        ])
        .output()
        .expect("invoke cargo to build wasm fixtures");
    assert!(
        output.status.success(),
        "building {} failed:\nstdout: {}\nstderr: {}",
        manifest.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_stamp_atomically(output_dir: &Path, stamp: &str) {
    let temp_path = output_dir.join(format!("{STAMP_FILE}.{}.tmp", std::process::id()));
    fs::write(&temp_path, stamp).expect("write temporary wasm-fixtures stamp");
    fs::rename(&temp_path, output_dir.join(STAMP_FILE)).expect("promote wasm-fixtures stamp");
}

fn compute_fixtures_stamp(root: &Path) -> String {
    let mut relative_paths = Vec::new();
    if root.join("rust-toolchain.toml").is_file() {
        relative_paths.push(PathBuf::from("rust-toolchain.toml"));
    }
    collect_input_files(root, &root.join("wit"), &mut relative_paths);
    collect_input_files(root, &root.join("crates/plugin-sdk"), &mut relative_paths);
    collect_input_files(
        root,
        &root.join("crates/plugin-host/tests/fixtures"),
        &mut relative_paths,
    );
    relative_paths.sort();

    let mut buffer = Vec::new();
    for relative in relative_paths {
        let normalized = relative.to_string_lossy().replace('\\', "/");
        buffer.extend_from_slice(normalized.as_bytes());
        buffer.push(0);
        let bytes = fs::read(root.join(&relative)).expect("read wasm fixture input file");
        buffer.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        buffer.extend_from_slice(&bytes);
    }
    sha256_hex(&sha256(&buffer))
}

fn collect_input_files(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).expect("read wasm fixture input directory");
    for entry in entries {
        let path = entry.expect("read directory entry").path();
        let name = path.file_name().and_then(|value| value.to_str());
        if matches!(name, Some("target")) || name.is_some_and(|value| value.starts_with('.')) {
            continue;
        }
        if path.is_dir() {
            collect_input_files(root, &path, files);
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .expect("input file is inside workspace root")
                .to_path_buf();
            files.push(relative);
        }
    }
}

fn workspace_root() -> PathBuf {
    let candidate = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    candidate.canonicalize().unwrap_or(candidate)
}
