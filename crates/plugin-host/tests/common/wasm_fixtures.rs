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

const STAMP_INPUT_FILES: [&str; 3] = ["rust-toolchain.toml", "Cargo.toml", ".cargo/config.toml"];

const STAMP_INPUT_DIRS: [&str; 3] = [
    "wit",
    "crates/plugin-sdk",
    "crates/plugin-host/tests/fixtures",
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
    for file in STAMP_INPUT_FILES {
        if root.join(file).is_file() {
            relative_paths.push(PathBuf::from(file));
        }
    }
    for dir in STAMP_INPUT_DIRS {
        collect_input_files(root, &root.join(dir), &mut relative_paths);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const BUILD_INPUT_FILES: [&str; 3] =
        ["rust-toolchain.toml", "Cargo.toml", ".cargo/config.toml"];

    fn scratch_workspace(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "codeint-fixtures-stamp-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        for file in BUILD_INPUT_FILES {
            let path = root.join(file);
            fs::create_dir_all(path.parent().expect("input file has a parent")).expect("mkdir");
            fs::write(path, file).expect("write input file");
        }
        for dir in STAMP_INPUT_DIRS {
            fs::create_dir_all(root.join(dir)).expect("mkdir input dir");
            fs::write(root.join(dir).join("input.txt"), dir).expect("write dir input");
        }
        root
    }

    #[test]
    fn stamp_changes_when_any_fixture_build_input_changes() {
        let root = scratch_workspace("inputs");
        let baseline = compute_fixtures_stamp(&root);
        let mut touched: Vec<PathBuf> = BUILD_INPUT_FILES.iter().map(PathBuf::from).collect();
        touched.extend(
            STAMP_INPUT_DIRS
                .iter()
                .map(|dir| PathBuf::from(dir).join("input.txt")),
        );
        for relative in touched {
            let path = root.join(&relative);
            let original = fs::read(&path).expect("read input");
            fs::write(&path, [original.as_slice(), b"changed"].concat()).expect("edit input");
            assert_ne!(
                compute_fixtures_stamp(&root),
                baseline,
                "editing {} must change the fixtures stamp",
                relative.display()
            );
            fs::write(&path, original).expect("restore input");
        }
        fs::remove_dir_all(&root).expect("remove scratch workspace");
    }

    #[test]
    fn ci_cache_keys_hash_exactly_the_stamp_inputs() {
        let workflow = fs::read_to_string(workspace_root().join(".github/workflows/ci.yml"))
            .expect("read ci workflow");
        let mut expected: Vec<String> = STAMP_INPUT_FILES
            .iter()
            .map(|file| (*file).to_owned())
            .chain(STAMP_INPUT_DIRS.iter().map(|dir| format!("{dir}/**")))
            .collect();
        expected.sort();
        let keys: Vec<&str> = workflow
            .lines()
            .filter(|line| line.contains("v0-wasm-fixtures-"))
            .collect();
        assert!(
            !keys.is_empty(),
            "ci workflow has no wasm fixtures cache key"
        );
        for key in keys {
            let list = key
                .split("hashFiles(")
                .nth(1)
                .and_then(|rest| rest.split(')').next())
                .expect("cache key calls hashFiles");
            let mut actual: Vec<String> = list
                .split(',')
                .map(|item| item.trim().trim_matches('\'').to_owned())
                .collect();
            actual.sort();
            assert_eq!(actual, expected, "ci cache key out of sync: {key}");
        }
    }
}
