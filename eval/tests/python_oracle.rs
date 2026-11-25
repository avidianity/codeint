use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};
use tempfile::TempDir;

const CALC: &str = "import os\nfrom a import b\n\n\ndef classify(v):\n    if v > 0 and v < 10:\n        return 1\n    return os.sep\n";

fn python_is_new_enough() -> bool {
    Command::new("python3")
        .args([
            "-c",
            "import sys; sys.exit(0 if sys.version_info >= (3, 12) else 1)",
        ])
        .status()
        .is_ok_and(|status| status.success())
}

fn require_python() -> bool {
    if python_is_new_enough() {
        return true;
    }
    eprintln!("SKIPPED: python3 3.12 or newer is not available, so the Python oracle is untested");
    false
}

#[test]
fn the_python_oracle_reports_functions_decision_points_imports_and_syntax_errors() {
    if !require_python() {
        return;
    }
    let checkout = TempDir::new().expect("checkout folder");
    std::fs::write(checkout.path().join("calc.py"), CALC).expect("write calc.py");
    std::fs::write(checkout.path().join("broken.py"), "def broken(:\n").expect("write broken.py");
    std::fs::write(checkout.path().join("list.txt"), "calc.py\0broken.py\0").expect("write list");

    let output = Command::new("python3")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("oracles/python/oracle.py"))
        .arg("list.txt")
        .current_dir(checkout.path())
        .output()
        .expect("run the oracle");

    assert!(output.status.success(), "{output:?}");
    let records: Vec<Value> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).expect("one JSON record per line"))
        .collect();
    assert_eq!(records.len(), 2, "{records:?}");
    assert_eq!(
        records[0],
        json!({"path": "calc.py", "functions": [[5, 8, 2]], "imports": ["a", "os"]})
    );
    assert_eq!(records[1]["path"], "broken.py");
    assert!(
        records[1]["error"]
            .as_str()
            .is_some_and(|error| error.contains("invalid syntax")),
        "{records:?}"
    );
}

fn run_oracle_on(checkout: &Path, paths: &[&str]) -> Vec<Value> {
    let mut listing = String::new();
    for path in paths {
        listing.push_str(path);
        listing.push('\0');
    }
    std::fs::write(checkout.join("list.bin"), listing).expect("write list");
    let output = Command::new("python3")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("oracles/python/oracle.py"))
        .arg("list.bin")
        .current_dir(checkout)
        .output()
        .expect("run the oracle");
    assert!(output.status.success(), "{output:?}");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).expect("one JSON record per line"))
        .collect()
}

#[test]
fn the_python_oracle_honors_a_coding_declaration_on_a_non_utf8_file() {
    if !require_python() {
        return;
    }
    let checkout = TempDir::new().expect("checkout folder");
    std::fs::write(
        checkout.path().join("latin.py"),
        b"# -*- coding: latin-1 -*-\nimport os\n\n\ndef f():\n    return \"caf\xe9\"\n",
    )
    .expect("write latin.py");

    let records = run_oracle_on(checkout.path(), &["latin.py"]);

    assert_eq!(
        records,
        vec![json!({"path": "latin.py", "functions": [[5, 6, 0]], "imports": ["os"]})]
    );
}

#[test]
fn the_python_oracle_reads_paths_that_contain_a_newline() {
    if !require_python() {
        return;
    }
    let checkout = TempDir::new().expect("checkout folder");
    std::fs::write(
        checkout.path().join("odd\nname.py"),
        "def f():\n    return 1\n",
    )
    .expect("write odd file");

    let records = run_oracle_on(checkout.path(), &["odd\nname.py"]);

    assert_eq!(
        records,
        vec![json!({"path": "odd\nname.py", "functions": [[1, 2, 0]], "imports": []})]
    );
}
