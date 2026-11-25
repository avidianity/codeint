use codeint_model::SourceSet;

use super::{ListedFindings, SarifLog, relative_uri};
use crate::{
    CloneFamilyFinding, CloneGroupFinding, CloneLocation, CognitiveComplexityFinding, Decimal,
    ErodedFunctionFinding, FlaggedLineRow, HazardLineRow, ImportCycleFinding,
};

fn empty_listed_findings<'a>() -> ListedFindings<'a> {
    ListedFindings {
        eroded_functions: &[],
        cognitive_complexity: &[],
        clone_groups: &[],
        clone_families: &[],
        clustered_clone_group_ids: &[],
        flagged_lines: &[],
        import_cycles: &[],
        boundaries: &[],
        orphan_files: &[],
        test_gap: &[],
        hazards: &[],
        external_metrics: Vec::new(),
    }
}

#[test]
fn a_path_of_letters_digits_dots_dashes_and_slashes_is_its_own_uri() {
    assert_eq!(
        relative_uri("packages/web-app/src/v2/index.test.ts"),
        "packages/web-app/src/v2/index.test.ts"
    );
}

#[test]
fn a_space_a_percent_sign_and_a_hash_are_percent_encoded() {
    assert_eq!(relative_uri("src/my file%#1.ts"), "src/my%20file%25%231.ts");
}

#[test]
fn a_colon_is_percent_encoded_so_the_first_segment_never_reads_as_a_uri_scheme() {
    assert_eq!(relative_uri("c:/notes.ts"), "c%3A/notes.ts");
}

#[test]
fn each_utf_8_byte_of_a_non_ascii_character_is_percent_encoded() {
    assert_eq!(relative_uri("src/café.ts"), "src/caf%C3%A9.ts");
}

#[test]
fn every_sarif_result_carries_its_finding_id_in_partial_fingerprints() {
    let eroded = [ErodedFunctionFinding {
        id: "1111111111111111".to_owned(),
        path: "src/big.ts".to_owned(),
        line: 3,
        end_line: 23,
        name: Some("big".to_owned()),
        cyclomatic_complexity: 12,
        cognitive_complexity: 14,
        source_lines: 20,
        function_mass: Decimal(53.6656),
        owners: Vec::new(),
    }];
    let complex = [CognitiveComplexityFinding {
        id: "5555555555555555".to_owned(),
        path: "src/deep.ts".to_owned(),
        line: 5,
        end_line: 30,
        name: Some("deep".to_owned()),
        cognitive_complexity: 21,
        cyclomatic_complexity: 7,
        source_lines: 25,
        owners: Vec::new(),
    }];
    let clones = [CloneGroupFinding {
        id: "2222222222222222".to_owned(),
        tokens: 80,
        similarity: None,
        locations: vec![
            CloneLocation {
                path: "src/a.ts".to_owned(),
                start_line: 1,
                end_line: 10,
            },
            CloneLocation {
                path: "src/b.ts".to_owned(),
                start_line: 1,
                end_line: 10,
            },
        ],
        owners: Vec::new(),
    }];
    let flagged = [FlaggedLineRow {
        id: "3333333333333333".to_owned(),
        path: "src/sign.ts".to_owned(),
        line: 4,
        rule: "else-after-return".to_owned(),
        owners: Vec::new(),
    }];
    let cycles = [ImportCycleFinding {
        id: "4444444444444444".to_owned(),
        modules: vec!["src/x.ts".to_owned(), "src/y.ts".to_owned()],
        owners: Vec::new(),
    }];
    let listed = ListedFindings {
        eroded_functions: &eroded,
        cognitive_complexity: &complex,
        clone_groups: &clones,
        flagged_lines: &flagged,
        import_cycles: &cycles,
        ..empty_listed_findings()
    };

    let log = serde_json::to_value(SarifLog::listing(
        std::iter::once((SourceSet::Production, listed)),
        super::ListedDependencies::EMPTY,
    ))
    .expect("SARIF serializes");

    let fingerprints: Vec<(&str, &str)> = log["runs"][0]["results"]
        .as_array()
        .expect("results array")
        .iter()
        .map(|result| {
            (
                result["ruleId"].as_str().expect("ruleId"),
                result["partialFingerprints"]["codeintFindingId"]
                    .as_str()
                    .expect("codeintFindingId"),
            )
        })
        .collect();
    assert_eq!(
        fingerprints,
        [
            ("erosion", "1111111111111111"),
            ("cognitive_complexity", "5555555555555555"),
            ("duplication", "2222222222222222"),
            ("verbosity", "3333333333333333"),
            ("import_cycles", "4444444444444444"),
        ]
    );
}

#[test]
fn clone_family_sarif_result_replaces_its_member_clone_groups_and_keeps_standalone_groups() {
    let clones = [
        CloneGroupFinding {
            id: "aaaa000000000001".to_owned(),
            tokens: 80,
            similarity: None,
            locations: vec![
                CloneLocation {
                    path: "src/a.ts".to_owned(),
                    start_line: 1,
                    end_line: 10,
                },
                CloneLocation {
                    path: "src/b.ts".to_owned(),
                    start_line: 1,
                    end_line: 10,
                },
                CloneLocation {
                    path: "src/c.ts".to_owned(),
                    start_line: 1,
                    end_line: 10,
                },
            ],
            owners: Vec::new(),
        },
        CloneGroupFinding {
            id: "bbbb000000000002".to_owned(),
            tokens: 75,
            similarity: Some(Decimal(0.92)),
            locations: vec![
                CloneLocation {
                    path: "src/x.ts".to_owned(),
                    start_line: 1,
                    end_line: 8,
                },
                CloneLocation {
                    path: "src/y.ts".to_owned(),
                    start_line: 1,
                    end_line: 8,
                },
            ],
            owners: Vec::new(),
        },
    ];
    let families = [CloneFamilyFinding {
        id: "ffff000000000009".to_owned(),
        duplicated_lines: 30,
        clone_group_ids: vec!["aaaa000000000001".to_owned()],
        locations: vec![
            CloneLocation {
                path: "src/a.ts".to_owned(),
                start_line: 1,
                end_line: 10,
            },
            CloneLocation {
                path: "src/b.ts".to_owned(),
                start_line: 1,
                end_line: 10,
            },
            CloneLocation {
                path: "src/c.ts".to_owned(),
                start_line: 1,
                end_line: 10,
            },
        ],
        owners: Vec::new(),
    }];
    let listed = ListedFindings {
        clone_groups: &clones,
        clone_families: &families,
        ..empty_listed_findings()
    };

    let log = serde_json::to_value(SarifLog::listing(
        std::iter::once((SourceSet::Production, listed)),
        super::ListedDependencies::EMPTY,
    ))
    .expect("SARIF serializes");

    let results = log["runs"][0]["results"].as_array().expect("results");
    assert_eq!(results.len(), 2);
    assert_eq!(
        results[0]["partialFingerprints"]["codeintFindingId"],
        "ffff000000000009"
    );
    assert_eq!(
        results[0]["message"]["text"],
        "Clone family of 30 duplicated lines across 3 locations (1 Clone group); the other locations: src/b.ts lines 1-10, src/c.ts lines 1-10."
    );
    assert_eq!(
        results[0]["relatedLocations"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(
        results[1]["partialFingerprints"]["codeintFindingId"],
        "bbbb000000000002"
    );
    assert_eq!(
        results[1]["message"]["text"],
        "Near-clone group of 75 tokens (92% similar) in 2 fragments; the other fragments: src/y.ts lines 1-8."
    );
}

#[test]
fn sarif_does_not_leak_member_clone_groups_of_families_beyond_display_cap() {
    use codeint_config::{
        Budgets, FindingsShown, RegressionLimits, ReportFormat, ReportRenderFormat,
    };
    use codeint_model::{
        CodeModel, FileModel, ReferenceRanges, RepoPath, SourceFile, Token, TokenKind,
        token_text_hash,
    };
    use std::collections::HashSet;

    use crate::{
        BaseCommit, CommitMeasurement, DiffReport, Head, MeasuredLanguage, Report, SavedReport,
    };

    let make_file = |path: &str, family_id: usize, lines: u32, tokens_per_line: usize| {
        let mut tokens = Vec::new();
        for line in 1..=lines {
            for col in 0..tokens_per_line {
                let text = format!("f{family_id}_l{line}_c{col}");
                tokens.push(Token {
                    kind: TokenKind::Other,
                    text_hash: token_text_hash(&text),
                    line,
                });
            }
        }
        SourceFile {
            path: RepoPath::new(path).expect("valid path"),
            model: FileModel {
                source_set: SourceSet::Production,
                source_lines: lines,
                functions: Vec::new(),
                tokens,
                flagged_lines: Vec::new(),
                hazard_lines: Vec::new(),
                runtime_imports: Vec::new(),
                program_start: false,
            },
        }
    };

    let mut files = Vec::new();
    for fam in 0..20usize {
        for copy in 0..3usize {
            files.push(make_file(
                &format!("src/tall_{fam:02}_{copy}.ts"),
                fam,
                10,
                8,
            ));
        }
    }
    for fam in 20..26usize {
        for copy in 0..3usize {
            files.push(make_file(
                &format!("src/wide_{fam:02}_{copy}.ts"),
                fam,
                1,
                160,
            ));
        }
    }
    for copy in 0..2usize {
        files.push(make_file(&format!("src/standalone_{copy}.ts"), 99, 1, 140));
    }

    let model = CodeModel {
        files,
        ..CodeModel::default()
    };
    let measured_language = || {
        vec![MeasuredLanguage {
            plugin: "typescript-javascript".to_owned(),
            model: model.clone(),
            reference_ranges: ReferenceRanges::default(),
            reference_range_note: None,
            has_imports: true,
            framework_profiles: Vec::new(),
            framework_profiles_by_package: std::collections::BTreeMap::new(),
        }]
    };
    let report = Report::summarize(
        measured_language(),
        Vec::new(),
        FindingsShown::default(),
        &Budgets::default(),
    );
    let check_json_text = report.render(ReportFormat::Json).expect("json");
    let report_json: serde_json::Value = serde_json::from_str(&check_json_text).expect("parse");
    let dup_json = &report_json["source_sets"]["production"]["duplication"];
    assert_eq!(
        dup_json["clone_families"].as_array().map(Vec::len),
        Some(20)
    );
    assert_eq!(dup_json["clone_families_left_out"], 6);
    assert_eq!(
        dup_json["unlisted_clone_family_group_ids"]
            .as_array()
            .map(Vec::len),
        Some(6)
    );

    let full_report = Report::summarize(
        measured_language(),
        Vec::new(),
        FindingsShown::All,
        &Budgets::default(),
    );
    let full_json: serde_json::Value =
        serde_json::from_str(&full_report.render(ReportFormat::Json).expect("json"))
            .expect("parse");
    assert_eq!(
        full_json["source_sets"]["production"]["duplication"]
            .get("unlisted_clone_family_group_ids"),
        None
    );
    let all_clustered_ids: HashSet<String> = full_json["source_sets"]["production"]["duplication"]
        ["clone_families"]
        .as_array()
        .expect("all clone families")
        .iter()
        .flat_map(|f| {
            f["clone_group_ids"]
                .as_array()
                .expect("group ids")
                .iter()
                .map(|id| id.as_str().expect("id string").to_owned())
        })
        .collect();
    assert_eq!(all_clustered_ids.len(), 26);

    let assert_sarif_omits_clustered = |rendered: &str, label: &str| {
        let sarif: serde_json::Value = serde_json::from_str(rendered).expect("parse sarif");
        let ids: Vec<&str> = sarif["runs"][0]["results"]
            .as_array()
            .expect("sarif results")
            .iter()
            .map(|r| {
                r["partialFingerprints"]["codeintFindingId"]
                    .as_str()
                    .expect("finding id")
            })
            .collect();
        assert_eq!(ids.len(), 21, "{label}");
        for id in &ids {
            assert!(
                !all_clustered_ids.contains(*id),
                "{label} leaked member CloneGroup {id} from a truncated CloneFamily"
            );
        }
    };

    let direct_check_sarif = report.render(ReportFormat::Sarif).expect("sarif");
    assert_sarif_omits_clustered(&direct_check_sarif, "SARIF");
    let saved_check = SavedReport::from_json(&check_json_text).expect("parse saved check");
    let saved_check_sarif = saved_check
        .render(ReportRenderFormat::Sarif)
        .expect("saved check sarif");
    assert_eq!(saved_check_sarif, direct_check_sarif);
    assert_eq!(
        saved_check
            .render(ReportRenderFormat::Json)
            .expect("saved check json"),
        check_json_text
    );
    assert_eq!(
        saved_check
            .render(ReportRenderFormat::Toon)
            .expect("saved check toon"),
        report.render(ReportFormat::Toon).expect("direct toon")
    );

    let diff = DiffReport::compare(
        BaseCommit {
            git_ref: "main".to_owned(),
            merge_base: "0000000000000000000000000000000000000000".to_owned(),
        },
        Head::WorkingTree,
        CommitMeasurement::of(CodeModel::default()),
        CommitMeasurement::of(model),
        FindingsShown::default(),
        &Budgets::default(),
        &RegressionLimits::default(),
    );
    let direct_diff_sarif = diff.render(ReportFormat::Sarif).expect("diff sarif");
    assert_sarif_omits_clustered(&direct_diff_sarif, "Diff SARIF");
    let diff_json_text = diff.render(ReportFormat::Json).expect("diff json");
    let diff_json: serde_json::Value = serde_json::from_str(&diff_json_text).expect("parse diff");
    assert_eq!(
        diff_json["source_sets"]["production"]["duplication"]["clone_families"]
            ["unlisted_clone_family_group_ids"]
            .as_array()
            .map(Vec::len),
        Some(6)
    );
    let saved_diff = SavedReport::from_json(&diff_json_text).expect("parse saved diff");
    let saved_diff_sarif = saved_diff
        .render(ReportRenderFormat::Sarif)
        .expect("saved diff sarif");
    assert_eq!(saved_diff_sarif, direct_diff_sarif);
    assert_eq!(
        saved_diff
            .render(ReportRenderFormat::Json)
            .expect("saved diff json"),
        diff_json_text
    );
    assert_eq!(
        saved_diff
            .render(ReportRenderFormat::Toon)
            .expect("saved diff toon"),
        diff.render(ReportFormat::Toon).expect("direct diff toon")
    );
}

#[test]
fn sarif_emits_hazard_rules_by_rule_name_and_never_includes_source_text() {
    let hazards = [
        HazardLineRow {
            id: "aaaa000000000001".to_owned(),
            path: "db/query.sql".to_owned(),
            line: 7,
            rule: "select-star".to_owned(),
            owners: Vec::new(),
        },
        HazardLineRow {
            id: "bbbb000000000002".to_owned(),
            path: "db/query.sql".to_owned(),
            line: 12,
            rule: "plain-text-secret".to_owned(),
            owners: Vec::new(),
        },
    ];
    let listed = ListedFindings {
        hazards: &hazards,
        ..empty_listed_findings()
    };

    let log = serde_json::to_value(SarifLog::listing(
        std::iter::once((SourceSet::Production, listed)),
        super::ListedDependencies::EMPTY,
    ))
    .expect("SARIF serializes");

    let rules = log["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .expect("rules");
    let results = log["runs"][0]["results"].as_array().expect("results");
    assert_eq!(results.len(), 2);
    for (result, (expected_rule, expected_line, expected_id)) in results.iter().zip([
        ("select-star", 7u64, "aaaa000000000001"),
        ("plain-text-secret", 12u64, "bbbb000000000002"),
    ]) {
        assert_eq!(result["ruleId"], expected_rule);
        let idx = result["ruleIndex"].as_u64().expect("ruleIndex") as usize;
        assert_eq!(rules[idx]["id"], expected_rule);
        assert_eq!(
            rules[idx]["shortDescription"]["text"],
            format!("Hazard line matched rule {expected_rule}.")
        );
        assert_eq!(
            result["locations"][0]["physicalLocation"]["region"]["startLine"],
            expected_line
        );
        assert_eq!(
            result["partialFingerprints"]["codeintFindingId"],
            expected_id
        );
    }
}
