#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const FAKE_CODEINT: &str = "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 'codeint 0.0.0'; exit 0; fi\necho '{\"source_sets\":{\"production\":{\"source_lines\":2500}},\"skipped\":[]}'\n";

fn git(folder: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(folder)
        .output()
        .expect("run git");
    assert!(output.status.success(), "{output:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

struct Fixture {
    _source: TempDir,
    work: TempDir,
}

impl Fixture {
    fn new() -> Fixture {
        let source = TempDir::new().expect("source repository");
        git(source.path(), &["init", "--quiet"]);
        std::fs::write(source.path().join("calc.py"), "def f():\n    return 1\n")
            .expect("write calc.py");
        git(source.path(), &["add", "calc.py"]);
        git(source.path(), &["commit", "--quiet", "-m", "init"]);
        let commit = git(source.path(), &["rev-parse", "HEAD"]);
        let work = TempDir::new().expect("work folder");
        let corpus = serde_json::json!({
            "measured_on": "2026-11-25",
            "repositories": [{
                "name": "local/python",
                "language": "python",
                "bucket": "small",
                "url": format!("file://{}", source.path().display()),
                "commit": commit,
            }]
        });
        std::fs::write(work.path().join("corpus.json"), corpus.to_string()).expect("manifest");
        let codeint = work.path().join("codeint");
        std::fs::write(&codeint, FAKE_CODEINT).expect("fake codeint");
        std::fs::set_permissions(&codeint, std::fs::Permissions::from_mode(0o755))
            .expect("make fake codeint executable");
        Fixture {
            _source: source,
            work,
        }
    }

    fn run(&self, extra: &[&str]) -> (Output, Value) {
        let output = Command::new(env!("CARGO_BIN_EXE_codeint-eval"))
            .current_dir(self.work.path())
            .arg("--manifest")
            .arg(self.work.path().join("corpus.json"))
            .arg("--codeint")
            .arg(self.work.path().join("codeint"))
            .arg("--output-dir")
            .arg(self.work.path().join("out"))
            .args(["--runs", "1"])
            .args(extra)
            .output()
            .expect("run codeint-eval");
        let summary = std::fs::read_to_string(self.work.path().join("out/evaluation-summary.json"))
            .map(|text| serde_json::from_str(&text).expect("summary JSON"))
            .unwrap_or(Value::Null);
        (output, summary)
    }
}

fn oracle_script(work: &Path) -> String {
    let script = work.join("oracle.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\nprintf '{\"path\":\"calc.py\",\"functions\":[[1,2,0]],\"imports\":[]}\\n'\n",
    )
    .expect("oracle script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("make oracle executable");
    format!("python={}", script.display())
}

fn run_with_oracle(extra: &[&str], code: i32, message: &str) -> Value {
    let fixture = Fixture::new();
    let oracle = oracle_script(fixture.work.path());
    let mut arguments = vec!["--oracle", oracle.as_str()];
    arguments.extend_from_slice(extra);

    let (output, summary) = fixture.run(&arguments);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(code), "{stderr}");
    assert!(stderr.contains(message), "{stderr}");
    summary
}

#[test]
fn a_wall_time_ceiling_fails_a_repository_that_exceeds_it() {
    let summary = run_with_oracle(&["--max-wall-seconds", "0"], 1, "exceeds the ceiling of 0");

    assert_eq!(summary["repositories"][0]["passed"], false);
}

#[test]
fn a_wall_time_ceiling_above_the_measurement_passes() {
    let summary = run_with_oracle(&["--max-wall-seconds", "600"], 0, "PASS local/python");

    assert_eq!(summary["repositories"][0]["passed"], true);
}

#[test]
fn a_memory_ceiling_fails_a_repository_that_exceeds_it_or_has_no_measurement() {
    run_with_oracle(&["--max-rss-mib", "0"], 1, "peak memory");
}

#[test]
fn require_oracles_fails_a_language_whose_oracle_runtime_is_missing() {
    let fixture = Fixture::new();
    let empty_path = TempDir::new().expect("empty PATH folder");
    let located = Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .expect("locate git");
    assert!(located.status.success(), "{located:?}");
    let git_binary = String::from_utf8(located.stdout)
        .expect("git path")
        .trim()
        .to_owned();
    std::os::unix::fs::symlink(git_binary, empty_path.path().join("git")).expect("link git");

    let run = |extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_codeint-eval"))
            .env("PATH", empty_path.path())
            .current_dir(fixture.work.path())
            .arg("--manifest")
            .arg(fixture.work.path().join("corpus.json"))
            .arg("--codeint")
            .arg(fixture.work.path().join("codeint"))
            .arg("--output-dir")
            .arg(fixture.work.path().join("out"))
            .args(["--runs", "1"])
            .args(extra)
            .output()
            .expect("run codeint-eval")
    };

    let lenient = run(&[]);
    assert_eq!(
        lenient.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&lenient.stderr)
    );
    let strict = run(&["--require-oracles"]);
    let stderr = String::from_utf8_lossy(&strict.stderr);
    assert_eq!(strict.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("oracle skipped"), "{stderr}");
}
