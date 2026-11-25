use super::*;

fn temp_wasm(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path =
        std::env::temp_dir().join(format!("codeint-conformance-{}-{name}", std::process::id()));
    std::fs::write(&path, bytes).expect("write temporary file");
    path
}

#[test]
fn unreadable_file_is_a_read_error() {
    let outcome = run_conformance(
        PluginKind::Language,
        Path::new("/nonexistent/plugin.wasm"),
        PluginLimits::default(),
        &[],
    );
    assert!(matches!(outcome, Err(PluginHostError::Read { .. })));
}

#[test]
fn garbage_bytes_fail_the_component_check_and_skip_the_rest() {
    let path = temp_wasm("garbage.wasm", b"not wasm");
    let report =
        run_conformance(PluginKind::Metric, &path, PluginLimits::default(), &[]).expect("report");
    std::fs::remove_file(&path).expect("remove temporary file");
    assert!(!report.passed());
    assert_eq!(report.checks[0].id, "component");
    assert_eq!(report.checks[0].status, ConformanceStatus::Fail);
    assert!(report.checks[0].hint.is_some());
    assert!(
        report.checks[1..]
            .iter()
            .all(|check| check.status == ConformanceStatus::Skipped)
    );
}

#[test]
fn extension_check_rejects_empty_dotted_and_duplicate_extensions() {
    for extensions in [
        vec![],
        vec![".ts".to_owned()],
        vec!["ts".to_owned(), "ts".to_owned()],
    ] {
        let mut report = ConformanceReport {
            kind: PluginKind::Language,
            checks: Vec::new(),
        };
        check_extensions(&mut report, &extensions);
        assert_eq!(
            report.checks[0].status,
            ConformanceStatus::Fail,
            "{extensions:?}"
        );
    }
    let mut report = ConformanceReport {
        kind: PluginKind::Language,
        checks: Vec::new(),
    };
    check_extensions(&mut report, &["ts".to_owned(), "tsx".to_owned()]);
    assert_eq!(report.checks[0].status, ConformanceStatus::Pass);
}
