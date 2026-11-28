use std::collections::BTreeMap;

use codeint_model::language::Function;
use codeint_model::{CodeModel, FunctionCoverage, LanguageFacts, RepoPath, SourceFile, SourceSet};

use super::{
    CoverageApplyError, CoverageFormat, CoverageLimits, CoverageParseError, NamedCoverageReport,
    PathMatcher, apply_coverage_reports, match_coverage_path, parse_cobertura,
    parse_cobertura_with_limits, parse_coverage_report, parse_coverage_report_with_limits,
    parse_lcov, parse_lcov_with_limits,
};
use crate::test_support::{empty_source_file, repo_path};

fn make_function(name: &str, line: u32, end_line: u32) -> Function {
    Function {
        line,
        end_line,
        decision_points: 0,
        source_lines: end_line.saturating_sub(line).saturating_add(1),
        name: Some(name.to_owned()),
        cognitive_complexity: 0,
        coverage: None,
    }
}

fn source_file_with_functions(
    path: &str,
    source_set: SourceSet,
    source_lines: u32,
    functions: Vec<Function>,
) -> SourceFile {
    let mut file = empty_source_file(path, source_set, source_lines);
    file.model.functions = functions;
    file
}

fn language_model(language: &str, files: Vec<SourceFile>) -> CodeModel {
    CodeModel {
        files,
        languages: vec![LanguageFacts {
            language: language.to_owned(),
            has_coverage: false,
            has_hazard_rules: false,
            has_imports: true,
        }],
        ..CodeModel::default()
    }
}

#[test]
fn lcov_reads_documented_records_ignores_branch_and_function_records_and_merges_duplicates() {
    let input = concat!(
        "\u{feff}",
        "TN:unit\r\n",
        "SF:src/app.ts\r\n",
        "FN:1,alpha\r\n",
        "FNDA:2,alpha\r\n",
        "FNF:1\r\n",
        "FNH:1\r\n",
        "BRDA:2,0,0,1\r\n",
        "BRF:1\r\n",
        "BRH:1\r\n",
        "DA:1,0\r\n",
        "DA:1,3,abcdef1234\r\n",
        "DA:2,0\r\n",
        "LF:2\r\n",
        "LH:1\r\n",
        "end_of_record\r\n",
        "\r\n",
        "SF:src/app.ts\r\n",
        "DA:2,1\r\n",
        "DA:3,0\r\n",
        "end_of_record\r\n",
        "SF:src/empty.ts\r\n",
        "end_of_record\r\n",
    );

    let parsed = parse_lcov(input.as_bytes()).expect("valid LCOV");
    assert_eq!(parsed.format, CoverageFormat::Lcov);
    assert!(parsed.source_roots.is_empty());
    assert_eq!(parsed.files.len(), 2);

    let app = &parsed.files[0];
    assert_eq!(app.path, "src/app.ts");
    assert_eq!(app.lines.get(&1), Some(&3));
    assert_eq!(app.lines.get(&2), Some(&1));
    assert_eq!(app.lines.get(&3), Some(&0));

    let empty = &parsed.files[1];
    assert_eq!(empty.path, "src/empty.ts");
    assert!(empty.lines.is_empty());
}

#[test]
fn lcov_rejects_undocumented_records_and_invalid_data_with_first_offending_line() {
    let unknown = "SF:src/app.ts\nDA:1,1\nUNKNOWN:value\nBOGUS:later\nend_of_record\n";
    let error = parse_lcov(unknown.as_bytes()).expect_err("unknown record must fail");
    assert_eq!(
        error,
        CoverageParseError::InvalidLcovRecord {
            line: 3,
            content: "UNKNOWN:value".to_owned(),
        }
    );

    let da_outside = "DA:1,1\n";
    assert!(matches!(
        parse_lcov(da_outside.as_bytes()),
        Err(CoverageParseError::InvalidLcovData { line: 1, .. })
    ));

    let zero_line = "SF:src/app.ts\nDA:0,1\nend_of_record\n";
    assert!(matches!(
        parse_lcov(zero_line.as_bytes()),
        Err(CoverageParseError::InvalidLcovData { line: 2, .. })
    ));

    let negative_hits = "SF:src/app.ts\nDA:2,-1\nend_of_record\n";
    assert!(matches!(
        parse_lcov(negative_hits.as_bytes()),
        Err(CoverageParseError::InvalidLcovData { line: 2, .. })
    ));

    let too_many_fields = "SF:src/app.ts\nDA:2,1,sum,extra\nend_of_record\n";
    assert!(matches!(
        parse_lcov(too_many_fields.as_bytes()),
        Err(CoverageParseError::InvalidLcovData { line: 2, .. })
    ));
}

#[test]
fn lcov_enforces_byte_size_limit() {
    let input = b"SF:src/app.ts\nDA:1,1\nend_of_record\n";
    let error = parse_lcov_with_limits(input, 10).expect_err("oversized LCOV must fail");
    assert_eq!(
        error,
        CoverageParseError::ReportTooLarge {
            size_bytes: input.len(),
            max_bytes: 10,
        }
    );
}

#[test]
fn cobertura_reads_sources_classes_and_lines_ignores_methods_and_merges_duplicate_classes() {
    let xml = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<!-- report generated for test -->\n",
        "<coverage line-rate=\"0.5\" branch-rate=\"0.25\">\n",
        "  <sources>\n",
        "    <source>/repo/packages/a &amp; b</source>\n",
        "    <source><![CDATA[/repo/packages/c]]></source>\n",
        "  </sources>\n",
        "  <packages>\n",
        "    <package name=\"app\">\n",
        "      <classes>\n",
        "        <class name=\"First\" filename=\"src/worker.php\" line-rate=\"0.5\">\n",
        "          <methods>\n",
        "            <method name=\"ignored\">\n",
        "              <lines>\n",
        "                <line number=\"99\" hits=\"1\"/>\n",
        "              </lines>\n",
        "            </method>\n",
        "          </methods>\n",
        "          <lines>\n",
        "            <line number=\"5\" hits=\"0\" branch=\"true\" condition-coverage=\"50% (1/2)\"/>\n",
        "            <line number=\"6\" hits=\"2\" branch=\"false\"/>\n",
        "          </lines>\n",
        "        </class>\n",
        "        <class name=\"Second\" filename='src/worker.php'>\n",
        "          <lines>\n",
        "            <line number=\"5\" hits=\"1\"/>\n",
        "            <line number=\"7\" hits=\"0\"/>\n",
        "          </lines>\n",
        "        </class>\n",
        "        <class name=\"Empty\" filename=\"src/empty.php\"/>\n",
        "      </classes>\n",
        "    </package>\n",
        "  </packages>\n",
        "</coverage>\n",
    );

    let parsed = parse_cobertura(xml.as_bytes()).expect("valid Cobertura XML");
    assert_eq!(parsed.format, CoverageFormat::Cobertura);
    assert_eq!(
        parsed.source_roots,
        vec![
            "/repo/packages/a & b".to_owned(),
            "/repo/packages/c".to_owned()
        ]
    );
    assert_eq!(parsed.files.len(), 2);
    assert_eq!(parsed.files[0].path, "src/empty.php");
    assert!(parsed.files[0].lines.is_empty());

    let worker = &parsed.files[1];
    assert_eq!(worker.path, "src/worker.php");
    assert_eq!(worker.lines.get(&5), Some(&1));
    assert_eq!(worker.lines.get(&6), Some(&2));
    assert_eq!(worker.lines.get(&7), Some(&0));
    assert!(!worker.lines.contains_key(&99));
}

#[test]
fn cobertura_decodes_predefined_and_numeric_entities_in_attributes_and_sources() {
    let xml = concat!(
        "<coverage>",
        "<sources><source>/dir/&lt;a&gt;&quot;&apos;&#65;&#x42;</source></sources>",
        "<packages><package><classes>",
        "<class filename=\"src/file&#x2d;1.py\"><lines><line number=\"1\" hits=\"1\"/></lines></class>",
        "</classes></package></packages>",
        "</coverage>",
    );
    let parsed = parse_cobertura(xml.as_bytes()).expect("valid entities");
    assert_eq!(parsed.source_roots, vec!["/dir/<a>\"'AB".to_owned()]);
    assert_eq!(parsed.files[0].path, "src/file-1.py");
}

#[test]
fn cobertura_rejects_broken_xml() {
    let cases = [
        "<coverage><packages></coverage>",
        "<coverage><packages>",
        "<coverage><packages><package><classes><class><lines/></class></classes></package></packages></coverage>",
        "<coverage><packages><package><classes><class filename=\"a.py\"><lines><line number=\"0\" hits=\"1\"/></lines></class></classes></package></packages></coverage>",
        "<coverage><packages><package><classes><class filename=\"a.py\"><lines><line number=\"1\" hits=\"-3\"/></lines></class></classes></package></packages></coverage>",
        "<coverage a=\"1\" a=\"2\"/>",
        "<coverage attr=\"a<b\"/>",
        "<coverage><!-- unclosed",
        "<coverage/><extra/>",
    ];
    for case in cases {
        let error = parse_cobertura(case.as_bytes()).expect_err("broken XML must fail");
        assert!(
            matches!(error, CoverageParseError::InvalidCoberturaXml { .. }),
            "expected InvalidCoberturaXml for {case:?}, got {error:?}"
        );
    }
}

#[test]
fn cobertura_rejects_hostile_dtd_external_entity_deep_nesting_and_oversized_input() {
    let dtd = concat!(
        "<?xml version=\"1.0\"?>",
        "<!DOCTYPE coverage SYSTEM \"http://127.0.0.1/coverage-04.dtd\">",
        "<coverage/>",
    );
    assert!(matches!(
        parse_coverage_report(dtd.as_bytes()),
        Err(CoverageParseError::XmlSecurityViolation { .. })
    ));

    let external_entity = concat!(
        "<?xml version=\"1.0\"?>",
        "<!DOCTYPE coverage [<!ENTITY xxe SYSTEM \"file:///etc/passwd\">]>",
        "<coverage><sources><source>&xxe;</source></sources></coverage>",
    );
    assert!(matches!(
        parse_coverage_report(external_entity.as_bytes()),
        Err(CoverageParseError::XmlSecurityViolation { .. })
    ));

    let custom_entity_without_dtd =
        "<coverage><sources><source>&xxe;</source></sources></coverage>";
    assert!(matches!(
        parse_coverage_report(custom_entity_without_dtd.as_bytes()),
        Err(CoverageParseError::XmlSecurityViolation { .. })
    ));

    let stylesheet_pi = "<?xml-stylesheet type=\"text/xsl\" href=\"evil.xsl\"?><coverage/>";
    assert!(matches!(
        parse_coverage_report(stylesheet_pi.as_bytes()),
        Err(CoverageParseError::XmlSecurityViolation { .. })
    ));

    let mut deeply_nested = String::from("<coverage>");
    for _ in 0..65 {
        deeply_nested.push_str("<a>");
    }
    for _ in 0..65 {
        deeply_nested.push_str("</a>");
    }
    deeply_nested.push_str("</coverage>");
    assert_eq!(
        parse_coverage_report(deeply_nested.as_bytes()),
        Err(CoverageParseError::XmlNestingTooDeep { max_depth: 64 })
    );

    let limits = CoverageLimits {
        max_bytes: 32,
        max_xml_depth: 4,
    };
    let oversized = b"<coverage><packages><package name=\"very-long-name\"/></packages></coverage>";
    assert_eq!(
        parse_cobertura_with_limits(oversized, limits),
        Err(CoverageParseError::ReportTooLarge {
            size_bytes: oversized.len(),
            max_bytes: 32,
        })
    );
    assert_eq!(
        parse_coverage_report_with_limits(oversized, limits),
        Err(CoverageParseError::ReportTooLarge {
            size_bytes: oversized.len(),
            max_bytes: 32,
        })
    );
}

#[test]
fn auto_detection_distinguishes_lcov_cobertura_and_unrecognized_formats() {
    let lcov =
        parse_coverage_report(b"SF:src/a.ts\nDA:1,1\nend_of_record\n").expect("auto-detected LCOV");
    assert_eq!(lcov.format, CoverageFormat::Lcov);

    let cobertura = parse_coverage_report(b"  \n<?xml version=\"1.0\"?><coverage/>")
        .expect("auto-detected Cobertura");
    assert_eq!(cobertura.format, CoverageFormat::Cobertura);

    for bad in [
        b"".as_slice(),
        b"   \n\t ",
        b"{\"coverage\": 1}",
        b"plain text file",
        b"<report name=\"jacoco\"/>",
        b"<coverage generated=\"1\"><project/></coverage>",
    ] {
        let error = parse_coverage_report(bad).expect_err("unrecognized format must fail");
        assert_eq!(error, CoverageParseError::UnrecognizedFormat);
        let message = error.to_string();
        assert!(message.contains("LCOV"), "missing LCOV in {message}");
        assert!(
            message.contains("Cobertura XML"),
            "missing Cobertura XML in {message}"
        );
    }

    assert_eq!(
        parse_coverage_report(&[0xff, 0xfe]),
        Err(CoverageParseError::InvalidUtf8)
    );
}

#[test]
fn path_matcher_resolves_relative_absolute_windows_package_relative_and_ties() {
    let repo_paths: Vec<RepoPath> = vec![
        repo_path("src/index.ts"),
        repo_path("packages/app/src/index.ts"),
        repo_path("packages/app/src/only_app.ts"),
        repo_path("packages/lib/src/shared.ts"),
        repo_path("packages/web/src/shared.ts"),
    ];
    let matcher = PathMatcher::new(&repo_paths);

    assert_eq!(
        matcher.match_path("./src/index.ts", &[]),
        Some(&repo_paths[0])
    );
    assert_eq!(
        matcher.match_path("/workspace/repo/packages/app/src/index.ts", &[]),
        Some(&repo_paths[1])
    );
    assert_eq!(
        matcher.match_path(r"C:\workspace\repo\packages\app\src\index.ts", &[]),
        Some(&repo_paths[1])
    );
    assert_eq!(
        matcher.match_path("src/only_app.ts", &[]),
        Some(&repo_paths[2])
    );
    assert_eq!(matcher.match_path("src/shared.ts", &[]), None);
    assert_eq!(matcher.match_path("other/index.ts", &[]), None);
    assert_eq!(matcher.match_path("/etc/passwd", &[]), None);
    assert_eq!(matcher.match_path("../../etc/passwd", &[]), None);
}

#[test]
fn path_matcher_uses_cobertura_source_roots_and_never_escapes_repository() {
    let repo_paths: Vec<RepoPath> = vec![
        repo_path("packages/alpha/src/service.php"),
        repo_path("packages/beta/src/service.php"),
        repo_path("packages/beta/src/helper.php"),
        repo_path("src/root.php"),
    ];

    let single_root = vec!["/ci/build/packages/alpha".to_owned()];
    assert_eq!(
        match_coverage_path("src/service.php", &single_root, &repo_paths),
        Some(&repo_paths[0])
    );

    let two_roots = vec![
        "/ci/build/packages/alpha".to_owned(),
        "/ci/build/packages/beta".to_owned(),
    ];
    assert_eq!(
        match_coverage_path("src/helper.php", &two_roots, &repo_paths),
        Some(&repo_paths[2])
    );
    assert_eq!(
        match_coverage_path("src/service.php", &two_roots, &repo_paths),
        None
    );

    let outside_root = vec!["/outside/unrelated".to_owned()];
    assert_eq!(
        match_coverage_path("src/root.php", &outside_root, &repo_paths),
        Some(&repo_paths[3])
    );
    assert_eq!(
        match_coverage_path("etc/passwd", &outside_root, &repo_paths),
        None
    );

    let parent_hop_root = vec!["/ci/build/packages/alpha/tests".to_owned()];
    assert_eq!(
        match_coverage_path("../src/service.php", &parent_hop_root, &repo_paths),
        Some(&repo_paths[0])
    );
}

#[test]
fn apply_merges_mixed_formats_marks_covered_languages_and_reports_unmatched_paths() {
    let ts_file = source_file_with_functions(
        "src/app.ts",
        SourceSet::Production,
        10,
        vec![make_function("run", 1, 5), make_function("notListed", 7, 9)],
    );
    let ts_uncovered = source_file_with_functions(
        "src/unloaded.ts",
        SourceSet::Production,
        5,
        vec![make_function("dead", 1, 4)],
    );
    let rs_file = source_file_with_functions(
        "src/lib.rs",
        SourceSet::Production,
        6,
        vec![make_function("calc", 1, 5)],
    );
    let mut models = vec![
        language_model("typescript", vec![ts_file, ts_uncovered]),
        language_model("rust", vec![rs_file]),
    ];

    let lcov = parse_coverage_report(
        b"SF:/ci/repo/src/app.ts\nDA:1,1\nDA:2,0\nDA:3,0\nend_of_record\nSF:src/missing.ts\nDA:1,1\nend_of_record\n",
    )
    .expect("valid LCOV");
    let cobertura = parse_coverage_report(
        br#"<coverage><packages><package><classes>
            <class filename="src/app.ts"><lines>
                <line number="2" hits="4"/>
                <line number="10" hits="1"/>
            </lines></class>
            <class filename="src/missing.ts"><lines><line number="1" hits="0"/></lines></class>
            <class filename="src/ghost.ts"><lines><line number="1" hits="1"/></lines></class>
        </classes></package></packages></coverage>"#,
    )
    .expect("valid Cobertura");

    let reports = vec![
        NamedCoverageReport {
            path: "lcov.info".to_owned(),
            report: lcov,
        },
        NamedCoverageReport {
            path: "cobertura.xml".to_owned(),
            report: cobertura,
        },
    ];
    let mut line_counts = BTreeMap::new();
    line_counts.insert(repo_path("src/app.ts"), 10);
    line_counts.insert(repo_path("src/unloaded.ts"), 5);
    line_counts.insert(repo_path("src/lib.rs"), 6);

    let outcome = apply_coverage_reports(&reports, &mut models, &line_counts)
        .expect("coverage application should succeed");

    assert_eq!(
        outcome.unmatched_paths,
        vec!["src/ghost.ts".to_owned(), "src/missing.ts".to_owned()]
    );
    assert!(models[0].languages[0].has_coverage);
    assert!(!models[1].languages[0].has_coverage);

    assert_eq!(
        models[0].files[0].model.functions[0].coverage,
        Some(FunctionCoverage {
            listed_lines: 3,
            executed_lines: 2,
        })
    );
    assert_eq!(
        models[0].files[0].model.functions[1].coverage,
        Some(FunctionCoverage {
            listed_lines: 0,
            executed_lines: 0,
        })
    );
    assert_eq!(models[0].files[1].model.functions[0].coverage, None);
    assert_eq!(models[1].files[0].model.functions[0].coverage, None);
}

#[test]
fn apply_rejects_line_beyond_file_end_and_accepts_last_line() {
    let file = source_file_with_functions(
        "src/app.ts",
        SourceSet::Production,
        4,
        vec![make_function("run", 1, 4)],
    );
    let mut line_counts = BTreeMap::new();
    line_counts.insert(repo_path("src/app.ts"), 4);

    let ok_report = NamedCoverageReport {
        path: "ok.info".to_owned(),
        report: parse_coverage_report(b"SF:src/app.ts\nDA:4,1\nend_of_record\n")
            .expect("valid LCOV"),
    };
    let mut models = vec![language_model("typescript", vec![file.clone()])];
    apply_coverage_reports(&[ok_report], &mut models, &line_counts)
        .expect("last line must be accepted");

    let stale_report = NamedCoverageReport {
        path: "stale.info".to_owned(),
        report: parse_coverage_report(b"SF:src/app.ts\nDA:5,1\nend_of_record\n")
            .expect("valid LCOV"),
    };
    let mut stale_models = vec![language_model("typescript", vec![file])];
    let error = apply_coverage_reports(&[stale_report], &mut stale_models, &line_counts)
        .expect_err("line beyond end of file must fail");
    assert_eq!(
        error,
        CoverageApplyError::LineOutOfRange {
            report: "stale.info".to_owned(),
            file: repo_path("src/app.ts"),
            line: 5,
            line_count: 4,
        }
    );
}

#[test]
fn apply_with_no_reports_leaves_all_coverage_facts_absent() {
    let file = source_file_with_functions(
        "src/app.ts",
        SourceSet::Production,
        4,
        vec![make_function("run", 1, 4)],
    );
    let mut models = vec![language_model("typescript", vec![file])];
    let outcome = apply_coverage_reports(&[], &mut models, &BTreeMap::new())
        .expect("empty reports should succeed");
    assert!(outcome.unmatched_paths.is_empty());
    assert!(!models[0].languages[0].has_coverage);
    assert_eq!(models[0].files[0].model.functions[0].coverage, None);
}

#[test]
fn path_matcher_rejects_relative_parent_traversal_paths() {
    let repo = vec![repo_path("packages/web/src/view.ts")];
    let matcher = PathMatcher::new(&repo);
    assert_eq!(
        matcher.match_path("../../packages/web/src/view.ts", &[]),
        None
    );
    assert_eq!(matcher.match_path("../outside/view.ts", &[]), None);
    assert_eq!(
        matcher.match_path("/workspace/packages/web/src/../src/view.ts", &[]),
        Some(&repo[0])
    );
}

fn named_report(path: &str, bytes: &[u8]) -> NamedCoverageReport {
    NamedCoverageReport {
        path: path.to_owned(),
        report: parse_coverage_report(bytes).expect("valid coverage report"),
    }
}

fn lcov_for_file(path: &str, hits: &[(u32, u64)]) -> Vec<u8> {
    let mut out = format!("SF:{path}\n");
    for (line, count) in hits {
        out.push_str(&format!("DA:{line},{count}\n"));
    }
    out.push_str("end_of_record\n");
    out.into_bytes()
}

fn apply_single_ts_lcov(
    path: &str,
    lines: u32,
    functions: Vec<Function>,
    hits: &[(u32, u64)],
) -> Vec<Function> {
    let file = source_file_with_functions(path, SourceSet::Production, lines, functions);
    let mut models = vec![language_model("typescript", vec![file])];
    let report = named_report("lcov.info", &lcov_for_file(path, hits));
    let line_counts = BTreeMap::from([(repo_path(path), lines as usize)]);
    apply_coverage_reports(&[report], &mut models, &line_counts)
        .expect("coverage application should succeed");
    models.remove(0).files.remove(0).model.functions
}

fn assert_function_coverages(functions: &[Function], expected: &[(u32, u32)]) {
    let actual: Vec<Option<(u32, u32)>> = functions
        .iter()
        .map(|function| {
            function
                .coverage
                .map(|coverage| (coverage.listed_lines, coverage.executed_lines))
        })
        .collect();
    let expected: Vec<Option<(u32, u32)>> = expected.iter().copied().map(Some).collect();
    assert_eq!(actual, expected);
}

#[test]
fn apply_attributes_lines_to_innermost_nested_function_across_deep_nesting_and_closures() {
    let mut closure = make_function("", 16, 19);
    closure.name = None;
    let functions = apply_single_ts_lcov(
        "src/nested.ts",
        24,
        vec![
            make_function("outer", 1, 10),
            make_function("inner", 4, 7),
            make_function("deepOuter", 12, 24),
            make_function("deepMiddle", 14, 22),
            closure,
        ],
        &[
            (2, 1),
            (3, 1),
            (5, 0),
            (6, 0),
            (9, 1),
            (13, 1),
            (15, 1),
            (17, 0),
            (18, 0),
            (21, 0),
            (23, 1),
        ],
    );

    assert_function_coverages(&functions, &[(3, 3), (2, 0), (2, 2), (2, 1), (2, 0)]);
}

#[test]
fn apply_shares_identical_spans_and_handles_zero_own_lines_invalid_spans_and_partial_overlaps() {
    let functions = apply_single_ts_lcov(
        "src/edges.ts",
        50,
        vec![
            make_function("enclosing", 1, 10),
            make_function("sameLineA", 5, 5),
            make_function("sameLineB", 5, 5),
            make_function("wrapperOnly", 20, 25),
            make_function("wrappedChild", 21, 24),
            make_function("withInvalidChild", 28, 38),
            make_function("invalidSpan", 35, 30),
            make_function("overlapLeft", 40, 45),
            make_function("overlapRight", 43, 48),
        ],
        &[
            (2, 1),
            (5, 0),
            (22, 1),
            (23, 0),
            (31, 1),
            (32, 0),
            (41, 1),
            (44, 0),
            (47, 1),
        ],
    );

    assert_function_coverages(
        &functions,
        &[
            (1, 1),
            (1, 0),
            (1, 0),
            (0, 0),
            (2, 1),
            (2, 1),
            (0, 0),
            (2, 1),
            (2, 1),
        ],
    );
}

#[test]
fn apply_separates_shell_script_lines_from_named_functions_across_merged_reports() {
    let shell_file = source_file_with_functions(
        "scripts/deploy.sh",
        SourceSet::Production,
        12,
        vec![
            make_function("<script>", 1, 12),
            make_function("helper", 3, 6),
            make_function("unlisted_fn", 8, 10),
        ],
    );
    let omitted_file = source_file_with_functions(
        "scripts/omitted.sh",
        SourceSet::Production,
        4,
        vec![make_function("<script>", 1, 4)],
    );
    let mut models = vec![language_model("shell", vec![shell_file, omitted_file])];
    let lcov = named_report(
        "unit.info",
        b"SF:scripts/deploy.sh\nDA:2,1\nDA:4,0\nDA:5,0\nDA:11,1\nend_of_record\n",
    );
    let cobertura = named_report(
        "e2e.xml",
        br#"<coverage><packages><package><classes>
            <class filename="scripts/deploy.sh"><lines>
                <line number="4" hits="2"/>
            </lines></class>
        </classes></package></packages></coverage>"#,
    );
    let line_counts = BTreeMap::from([
        (repo_path("scripts/deploy.sh"), 12),
        (repo_path("scripts/omitted.sh"), 4),
    ]);

    apply_coverage_reports(&[lcov, cobertura], &mut models, &line_counts)
        .expect("shell merged coverage should succeed");

    assert_function_coverages(
        &models[0].files[0].model.functions,
        &[(2, 2), (2, 1), (0, 0)],
    );
    assert_eq!(models[0].files[1].model.functions[0].coverage, None);
}
