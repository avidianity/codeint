use super::{SavedReport, SavedReportError};

#[test]
fn rejects_non_object_json_values() {
    let err = SavedReport::from_json("[1, 2]").expect_err("array is not a saved report");
    assert!(matches!(err, SavedReportError::NotAnObject));
}

#[test]
fn accepts_explicit_schema_version_1() {
    let json = r#"{"schema_version": 1, "files": 0, "source_lines": 0, "source_sets": {}}"#;
    let parsed = SavedReport::from_json(json).expect("schema_version 1 is supported");
    assert!(matches!(parsed, SavedReport::Check(_)));
}

#[test]
fn rejects_unsupported_schema_version_values() {
    let err = SavedReport::from_json(
        r#"{"schema_version": "2.0", "files": 0, "source_lines": 0, "source_sets": {}}"#,
    )
    .expect_err("schema_version 2.0 is unsupported");
    assert!(matches!(
        err,
        SavedReportError::UnsupportedSchemaVersion { .. }
    ));
}
