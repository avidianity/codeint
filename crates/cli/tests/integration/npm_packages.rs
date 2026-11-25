use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

struct PlatformSpec {
    folder: &'static str,
    package_name: &'static str,
    os: &'static str,
    cpu: &'static str,
    binary_relative: &'static str,
}

const PLATFORMS: [PlatformSpec; 6] = [
    PlatformSpec {
        folder: "darwin-arm64",
        package_name: "@avidian/codeint-darwin-arm64",
        os: "darwin",
        cpu: "arm64",
        binary_relative: "bin/codeint",
    },
    PlatformSpec {
        folder: "darwin-x64",
        package_name: "@avidian/codeint-darwin-x64",
        os: "darwin",
        cpu: "x64",
        binary_relative: "bin/codeint",
    },
    PlatformSpec {
        folder: "linux-arm64",
        package_name: "@avidian/codeint-linux-arm64",
        os: "linux",
        cpu: "arm64",
        binary_relative: "bin/codeint",
    },
    PlatformSpec {
        folder: "linux-x64",
        package_name: "@avidian/codeint-linux-x64",
        os: "linux",
        cpu: "x64",
        binary_relative: "bin/codeint",
    },
    PlatformSpec {
        folder: "win32-arm64",
        package_name: "@avidian/codeint-win32-arm64",
        os: "win32",
        cpu: "arm64",
        binary_relative: "bin/codeint.exe",
    },
    PlatformSpec {
        folder: "win32-x64",
        package_name: "@avidian/codeint-win32-x64",
        os: "win32",
        cpu: "x64",
        binary_relative: "bin/codeint.exe",
    },
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates folder")
        .parent()
        .expect("repository root")
        .to_path_buf()
}

fn npm_program() -> &'static str {
    if cfg!(windows) { "npm.cmd" } else { "npm" }
}

fn installed_codeint_bin(project_root: &Path) -> PathBuf {
    let bin_name = if cfg!(windows) {
        "codeint.cmd"
    } else {
        "codeint"
    };
    project_root
        .join("node_modules")
        .join(".bin")
        .join(bin_name)
}

fn host_platform() -> &'static PlatformSpec {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        "linux" => "linux",
        "windows" => "win32",
        other => panic!("unsupported test host OS: {other}"),
    };
    let cpu = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        other => panic!("unsupported test host architecture: {other}"),
    };
    PLATFORMS
        .iter()
        .find(|spec| spec.os == os && spec.cpu == cpu)
        .unwrap_or_else(|| panic!("no platform package for {os}-{cpu}"))
}

fn copy_directory_recursively(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create destination folder");
    for entry in fs::read_dir(source).expect("read source folder") {
        let entry = entry.expect("read folder entry");
        let source_path = entry.path();
        let target_path = destination.join(entry.file_name());
        if entry.file_type().expect("read file type").is_dir() {
            copy_directory_recursively(&source_path, &target_path);
        } else {
            fs::copy(&source_path, &target_path).expect("copy file");
        }
    }
}

fn run_npm(working_dir: &Path, cache_dir: &Path, args: &[&str]) -> Output {
    Command::new(npm_program())
        .args(args)
        .arg("--cache")
        .arg(cache_dir)
        .current_dir(working_dir)
        .env("npm_config_registry", "http://127.0.0.1:1")
        .env("npm_config_update_notifier", "false")
        .env("npm_config_fund", "false")
        .env("npm_config_audit", "false")
        .output()
        .expect("run npm")
}

fn assert_success(output: &Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} failed with {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn pack_package(package_dir: &Path, destination: &Path, cache_dir: &Path) -> PathBuf {
    let output = run_npm(
        package_dir,
        cache_dir,
        &[
            "pack",
            "--silent",
            "--pack-destination",
            destination.to_str().expect("utf-8 destination path"),
        ],
    );
    assert_success(&output, &format!("npm pack {}", package_dir.display()));
    let filename = String::from_utf8_lossy(&output.stdout)
        .lines()
        .last()
        .expect("npm pack printed a tarball name")
        .trim()
        .to_string();
    let tarball = destination.join(filename);
    assert!(
        tarball.is_file(),
        "expected tarball at {}",
        tarball.display()
    );
    tarball
}

fn stage_and_pack_all(workspace: &Path) -> (PathBuf, PathBuf, Vec<PathBuf>) {
    let npm_copy = workspace.join("npm");
    copy_directory_recursively(&repo_root().join("npm"), &npm_copy);

    let host = host_platform();
    let staged_binary = npm_copy
        .join("platforms")
        .join(host.folder)
        .join(host.binary_relative);
    fs::create_dir_all(staged_binary.parent().expect("binary parent folder"))
        .expect("create staged bin folder");
    crate::common::stage_codeint_for_packaging(&staged_binary);

    let tarballs_dir = workspace.join("tarballs");
    fs::create_dir_all(&tarballs_dir).expect("create tarballs folder");
    let cache_dir = workspace.join("npm-cache");
    fs::create_dir_all(&cache_dir).expect("create npm cache folder");

    let mut targets: Vec<(&str, PathBuf)> = Vec::with_capacity(1 + PLATFORMS.len());
    targets.push(("wrapper", npm_copy.join("codeint")));
    for platform in PLATFORMS {
        targets.push((
            platform.folder,
            npm_copy.join("platforms").join(platform.folder),
        ));
    }

    let packed: Vec<(&str, PathBuf)> = std::thread::scope(|scope| {
        let handles: Vec<_> = targets
            .into_iter()
            .map(|(label, package_dir)| {
                let pkg_cache = cache_dir.join(label);
                fs::create_dir_all(&pkg_cache).expect("create per-package npm cache folder");
                let tarballs_dir = &tarballs_dir;
                scope.spawn(move || (label, pack_package(&package_dir, tarballs_dir, &pkg_cache)))
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("npm pack thread"))
            .collect()
    });

    let mut wrapper_tarball = None;
    let mut host_tarball = None;
    let mut all_tarballs = Vec::with_capacity(packed.len());
    for (label, tarball) in packed {
        if label == "wrapper" {
            wrapper_tarball = Some(tarball.clone());
        }
        if label == host.folder {
            host_tarball = Some(tarball.clone());
        }
        all_tarballs.push(tarball);
    }

    (
        wrapper_tarball.expect("wrapper tarball"),
        host_tarball.expect("host platform tarball"),
        all_tarballs,
    )
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path).expect("read json file");
    serde_json::from_str(&text).expect("parse json file")
}

#[test]
fn npm_manifests_pin_workspace_version_and_enable_provenance_for_wrapper_and_six_platforms() {
    let version = env!("CARGO_PKG_VERSION");
    let npm_root = repo_root().join("npm");

    let wrapper = read_json(&npm_root.join("codeint/package.json"));
    assert_eq!(wrapper["name"], "codeint");
    assert_eq!(wrapper["version"], version);
    assert_eq!(wrapper["bin"]["codeint"], "bin/codeint.js");
    assert_eq!(wrapper["publishConfig"]["access"], "public");
    assert_eq!(wrapper["publishConfig"]["provenance"], true);

    let optional = wrapper["optionalDependencies"]
        .as_object()
        .expect("optionalDependencies object");
    assert_eq!(optional.len(), PLATFORMS.len());

    for platform in PLATFORMS {
        assert_eq!(
            optional.get(platform.package_name).and_then(Value::as_str),
            Some(version)
        );
        let manifest = read_json(
            &npm_root
                .join("platforms")
                .join(platform.folder)
                .join("package.json"),
        );
        assert_eq!(manifest["name"], platform.package_name);
        assert_eq!(manifest["version"], version);
        assert_eq!(manifest["os"], serde_json::json!([platform.os]));
        assert_eq!(manifest["cpu"], serde_json::json!([platform.cpu]));
        assert_eq!(manifest["files"], serde_json::json!(["bin/"]));
        assert_eq!(manifest["publishConfig"]["access"], "public");
        assert_eq!(manifest["publishConfig"]["provenance"], true);
    }
}

fn install_tarballs_offline(project_dir: &Path, cache_dir: &Path, tarballs: &[&Path]) {
    fs::create_dir_all(project_dir).expect("create consumer folder");
    fs::write(
        project_dir.join("package.json"),
        r#"{"name":"codeint-consumer-test","version":"0.0.0","private":true}"#,
    )
    .expect("write consumer package.json");
    let mut args = vec![
        "install",
        "--offline",
        "--omit=optional",
        "--no-package-lock",
        "--no-audit",
        "--no-fund",
    ];
    for tarball in tarballs {
        args.push(tarball.to_str().expect("tarball path"));
    }
    let install_output = run_npm(project_dir, cache_dir, &args);
    assert_success(&install_output, "offline npm install");
}

#[test]
fn npm_pack_produces_wrapper_and_six_platform_packages_and_local_install_runs_codeint_version() {
    let workspace = tempfile::tempdir().expect("create temp workspace");
    let (wrapper_tarball, host_tarball, all_tarballs) = stage_and_pack_all(workspace.path());

    let unique_names: BTreeSet<String> = all_tarballs
        .iter()
        .map(|path| {
            path.file_name()
                .expect("tarball filename")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(unique_names.len(), 7);

    let project_dir = workspace.path().join("consumer");
    let cache_dir = workspace.path().join("npm-cache");
    install_tarballs_offline(
        &project_dir,
        &cache_dir,
        &[host_tarball.as_path(), wrapper_tarball.as_path()],
    );

    let version_output = Command::new(installed_codeint_bin(&project_dir))
        .arg("--version")
        .current_dir(&project_dir)
        .output()
        .expect("run installed codeint --version");
    assert_success(&version_output, "installed codeint --version");
    assert_eq!(
        String::from_utf8_lossy(&version_output.stdout),
        format!("codeint {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn wrapper_exits_2_with_clear_error_when_platform_package_is_missing_or_platform_is_unsupported() {
    let workspace = tempfile::tempdir().expect("create temp workspace");
    let tarballs_dir = workspace.path().join("tarballs");
    fs::create_dir_all(&tarballs_dir).expect("create tarballs folder");
    let cache_dir = workspace.path().join("npm-cache");
    fs::create_dir_all(&cache_dir).expect("create npm cache folder");
    let wrapper_tarball = pack_package(&repo_root().join("npm/codeint"), &tarballs_dir, &cache_dir);

    let project_dir = workspace.path().join("missing-platform-consumer");
    install_tarballs_offline(&project_dir, &cache_dir, &[wrapper_tarball.as_path()]);

    let host = host_platform();
    let missing_output = Command::new(installed_codeint_bin(&project_dir))
        .arg("--version")
        .current_dir(&project_dir)
        .output()
        .expect("run codeint without platform package");
    assert_eq!(missing_output.status.code(), Some(2));
    let missing_stderr = String::from_utf8_lossy(&missing_output.stderr);
    assert!(
        missing_stderr.contains(host.package_name),
        "expected stderr to name {}, got: {missing_stderr}",
        host.package_name
    );

    let launcher = repo_root().join("npm/codeint/bin/codeint.js");
    let unsupported_output = Command::new("node")
        .args([
            "-e",
            r#"Object.defineProperty(process, "platform", { value: "freebsd" }); require(process.argv[1]);"#,
            launcher.to_str().expect("launcher path"),
        ])
        .output()
        .expect("run launcher on unsupported platform");
    assert_eq!(unsupported_output.status.code(), Some(2));
    let unsupported_stderr = String::from_utf8_lossy(&unsupported_output.stderr);
    assert!(
        unsupported_stderr.contains("unsupported platform")
            && unsupported_stderr.contains("freebsd-"),
        "unexpected unsupported platform stderr: {unsupported_stderr}"
    );
}
