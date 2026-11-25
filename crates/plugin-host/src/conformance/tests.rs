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
fn extension_check_rejects_empty_dotted_slashed_and_whitespace_extensions() {
    for extensions in [
        vec![],
        vec![String::new()],
        vec![".ts".to_owned()],
        vec!["a/b".to_owned()],
        vec!["a b".to_owned()],
    ] {
        let mut report = ConformanceReport {
            kind: PluginKind::Language,
            checks: Vec::new(),
            end_to_end: None,
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
        end_to_end: None,
    };
    check_extensions(&mut report, &["ts".to_owned(), "tsx".to_owned()]);
    assert_eq!(report.checks[0].status, ConformanceStatus::Pass);
}

#[test]
fn duplicate_extensions_pass_and_are_probed_once() {
    let mut report = ConformanceReport {
        kind: PluginKind::Language,
        checks: Vec::new(),
        end_to_end: None,
    };
    let extensions = ["ts".to_owned(), "ts".to_owned(), "tsx".to_owned()];
    check_extensions(&mut report, &extensions);
    assert_eq!(report.checks[0].status, ConformanceStatus::Pass);
    assert_eq!(probe_extensions(&extensions), ["ts", "tsx"]);
}

#[test]
fn built_in_probes_cover_every_extension_and_sources_once() {
    let extensions = ["aaa".to_owned(), "bbb".to_owned()];
    let sources = [ConformanceSource {
        file_name: "one.aaa".to_owned(),
        text: "x\n".to_owned(),
    }];
    let probes = built_in_probes(&extensions, &sources);
    assert_eq!(probes.len(), 8 * 2 + 1);
    for extension in &extensions {
        assert!(
            probes
                .iter()
                .any(|probe| probe.file_name == format!("probe.{extension}"))
        );
    }
}

#[test]
fn probe_paths_never_panic_on_backslashes() {
    assert_eq!(
        probe_repo_path("a\\b.noimp", "noimp").as_str(),
        "src/a_b.noimp"
    );
    assert_eq!(probe_repo_path("..", "ts").as_str(), "src/probe.ts");
    assert!(!usable_extension("a\\b"));
    assert!(usable_extension("ts"));
}

#[test]
fn extension_check_accepts_a_backslash_extension_like_codeint_check() {
    let mut report = ConformanceReport {
        kind: PluginKind::Language,
        checks: Vec::new(),
        end_to_end: None,
    };
    check_extensions(&mut report, &["a\\b".to_owned()]);
    assert_eq!(report.checks[0].status, ConformanceStatus::Pass);
}

#[test]
fn finalize_orders_checks_and_adds_the_missing_ones_as_skipped() {
    let mut report = ConformanceReport {
        kind: PluginKind::Metric,
        checks: Vec::new(),
        end_to_end: None,
    };
    report.pass("limits", "ok");
    report.pass("component", "ok");
    report.finalize(&METRIC_CHECKS);
    let ids: Vec<&str> = report.checks.iter().map(|check| check.id).collect();
    assert_eq!(ids, METRIC_CHECKS);
    assert_eq!(report.checks[1].status, ConformanceStatus::Skipped);
}
