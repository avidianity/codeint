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
    std::fs::write(checkout.path().join("list.txt"), "calc.py\nbroken.py\n").expect("write list");

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
