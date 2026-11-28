use super::*;

fn parse(text: &str) -> Result<Configuration, ConfigurationFileError> {
    parse_with_overrides(
        ConfigFileKind::Json,
        PathBuf::from(CONFIGURATION_FILE_NAME),
        true,
        Some(text),
        &ConfigOverrides::default(),
    )
    .map(|resolved| resolved.configuration)
}

#[test]
fn defaults_disable_near_clones_with_0_85_min_similarity() {
    let config = parse("{}").expect("empty config parses");

    assert!(!config.duplication.near_clones);
    assert_eq!(config.duplication.min_similarity.ratio(), 0.85);
}

#[test]
fn parses_valid_near_clones_and_min_similarity_bounds() {
    let enabled = parse(r#"{ "duplication": { "near_clones": true, "min_similarity": 1.0 } }"#)
        .expect("min_similarity 1.0 parses");
    let low = parse(r#"{ "duplication": { "near_clones": true, "min_similarity": 0.01 } }"#)
        .expect("min_similarity 0.01 parses");

    assert!(enabled.duplication.near_clones);
    assert_eq!(enabled.duplication.min_similarity.ratio(), 1.0);
    assert_eq!(low.duplication.min_similarity.ratio(), 0.01);
}

#[test]
fn rejects_min_similarity_outside_open_closed_unit_interval() {
    for invalid in ["0", "0.0", "-0.1", "1.01", "1.5"] {
        let text = format!(r#"{{ "duplication": {{ "min_similarity": {invalid} }} }}"#);
        let error = parse(&text).expect_err("invalid min_similarity must fail");
        assert!(
            matches!(
                error,
                ConfigurationFileError::OutOfExclusiveMinRange {
                    exclusive_minimum: 0.0,
                    maximum: 1.0,
                    ..
                }
            ),
            "unexpected error for {invalid}: {error}"
        );
    }
}

#[test]
fn effective_serialization_matches_default_when_set_to_defaults_and_differs_when_enabled() {
    let default_json = serde_json::to_string(&parse("{}").expect("default")).expect("json");
    let explicit_default_json = serde_json::to_string(
        &parse(r#"{ "duplication": { "near_clones": false, "min_similarity": 0.85 } }"#)
            .expect("explicit default"),
    )
    .expect("json");
    let enabled_json = serde_json::to_string(
        &parse(r#"{ "duplication": { "near_clones": true } }"#).expect("enabled"),
    )
    .expect("json");

    assert_eq!(default_json, explicit_default_json);
    assert_ne!(default_json, enabled_json);
}

#[test]
fn parses_jsonc_comments_trailing_commas_crlf_and_bom() {
    let input = concat!(
        "// leading comment\n",
        "{\n",
        "  /* block comment */ \"a\": 1, // inline comment\n",
        "  \"b\": [\n",
        "    1,\n",
        "    2, // trailing comma before comment\n",
        "    /* another comment */\n",
        "  ],\n",
        "}\n",
    );
    assert_eq!(
        parse_jsonc(input).expect("valid JSONC"),
        json!({ "a": 1, "b": [1, 2] })
    );
    assert_eq!(
        parse_jsonc("{\"a\": 1} // eof without newline").expect("eof comment"),
        json!({ "a": 1 })
    );
    assert_eq!(
        parse_jsonc("{\r\n  // crlf comment\r\n  \"a\": [1,],\r\n}\r\n").expect("crlf"),
        json!({ "a": [1] })
    );
    assert_eq!(
        parse_jsonc("\u{FEFF}{\n  // bom\n  \"a\": 1,\n}\n").expect("bom"),
        json!({ "a": 1 })
    );
}

#[test]
fn preserves_comment_tokens_and_commas_inside_jsonc_strings() {
    let input = concat!(
        "{\n",
        "  \"url\": \"https://example.com/a//b\",\n",
        "  \"block\": \"/* not a comment */\",\n",
        "  \"escaped_quote\": \"foo\\\" // still string /* nope */ ,]\",\n",
        "  \"escaped_backslash\": \"end\\\\\", // comment after string\n",
        "  \"trailing\": [\"a,\", \"b,}\",],\n",
        "}\n",
    );
    assert_eq!(
        parse_jsonc(input).expect("valid JSONC"),
        json!({
            "url": "https://example.com/a//b",
            "block": "/* not a comment */",
            "escaped_quote": "foo\" // still string /* nope */ ,]",
            "escaped_backslash": "end\\",
            "trailing": ["a,", "b,}"]
        })
    );
}

#[test]
fn reports_exact_line_and_column_for_jsonc_syntax_and_unterminated_block_comment() {
    let syntax_input = concat!(
        "{\n",
        "  /* line 2\n",
        "     line 3\n",
        "     line 4 */\n",
        "  \"a\": /* inline */ @\n",
        "}\n",
    );
    let syntax_error = parse_jsonc(syntax_input).expect_err("invalid token must fail");
    assert_eq!(syntax_error.line, 5);
    assert_eq!(syntax_error.column, 21);

    let unterminated_input = "{\n  \"a\": 1,\n    /* never closed\n  \"b\": 2\n}\n";
    let unterminated_error =
        parse_jsonc(unterminated_input).expect_err("unterminated block comment must fail");
    assert_eq!(unterminated_error.line, 3);
    assert_eq!(unterminated_error.column, 5);
    assert!(
        unterminated_error
            .message
            .to_ascii_lowercase()
            .contains("unterminated"),
        "unexpected error message: {unterminated_error}"
    );
}

#[test]
fn rejects_non_jsonc_extensions_invalid_commas_and_empty_input() {
    for invalid in [
        "",
        "   // only comment\n",
        "{ unquoted: 1 }",
        "{ \"a\": 'single' }",
        "{ \"a\": 0xFF }",
        "{ \"a\": +1 }",
        "{ \"a\": .5 }",
        "{ \"a\": 1 \"b\": 2 }",
        "# hash comment\n{}",
        "[1,,]",
        "[,1]",
        "[,]",
        "{,}",
        "{\"a\":,}",
    ] {
        assert!(
            parse_jsonc(invalid).is_err(),
            "expected invalid JSONC for {invalid:?}"
        );
    }
}

#[test]
fn schema_value_span_targets_active_top_level_schema_and_ignores_comments_and_nested_keys() {
    let commented = concat!(
        "\u{FEFF}{\n",
        "  // \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.0.1/schema/codeint.schema.json\",\n",
        "  /* \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.0.2/schema/codeint.schema.json\" */\n",
        "  \"nested\": { \"$schema\": \"https://example.invalid/nested.json\" },\n",
        "  \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json\",\n",
        "}\n",
    );
    let schema = schema_value_span(commented).expect("active top-level $schema span");
    assert_eq!(
        &commented[schema.span],
        "\"https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json\""
    );
    assert_eq!(
        schema.value,
        "https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json"
    );

    assert_eq!(
        schema_value_span(
            "{\n  // \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json\"\n}\n"
        ),
        None
    );
    assert_eq!(schema_value_span(r#"{ "$schema": 123 }"#), None);
    assert_eq!(schema_value_span(r#"{ "$schema": "#), None);
}

#[test]
fn schema_value_span_uses_last_duplicate_key_and_unescapes_string_value() {
    let duplicated = concat!(
        "{\n",
        "  \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json\",\n",
        "  \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.2.0/schema/codeint.schema.json\",\n",
        "}\n",
    );
    let schema = schema_value_span(duplicated).expect("last duplicate $schema span");
    assert_eq!(
        &duplicated[schema.span],
        "\"https://raw.githubusercontent.com/avidianity/codeint/v0.2.0/schema/codeint.schema.json\""
    );
    assert_eq!(
        schema.value,
        "https://raw.githubusercontent.com/avidianity/codeint/v0.2.0/schema/codeint.schema.json"
    );

    let shadowed_by_non_string = concat!(
        "{\n",
        "  \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json\",\n",
        "  \"$schema\": 123,\n",
        "}\n",
    );
    assert_eq!(schema_value_span(shadowed_by_non_string), None);

    let escaped = r#"{ "$schema": "https:\/\/raw.githubusercontent.com\/avidianity\/codeint\/v0.1.0\u002fextra\/schema\/codeint.schema.json" }"#;
    let escaped_schema = schema_value_span(escaped).expect("escaped $schema span");
    assert_eq!(
        &escaped[escaped_schema.span],
        r#""https:\/\/raw.githubusercontent.com\/avidianity\/codeint\/v0.1.0\u002fextra\/schema\/codeint.schema.json""#
    );
    assert_eq!(
        escaped_schema.value,
        "https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/extra/schema/codeint.schema.json"
    );
}

#[test]
fn new_findings_rule_accepts_canonical_numerator_rise_and_deprecated_alias() {
    use codeint_model::{Metric, NewFindingsRule, SourceSet};

    let canonical =
        parse(r#"{ "regression_limits": { "erosion": { "new_findings": "numerator_rise" } } }"#)
            .expect("parse canonical numerator_rise");
    let alias = parse(
        r#"{ "regression_limits": { "erosion": { "new_findings": "numerator_rise_with_new_findings" } } }"#,
    )
    .expect("parse deprecated alias");
    assert_eq!(
        canonical
            .regression_limits
            .limit(SourceSet::Production, Metric::Erosion)
            .new_findings,
        NewFindingsRule::NumeratorRise
    );
    assert_eq!(
        alias
            .regression_limits
            .limit(SourceSet::Production, Metric::Erosion)
            .new_findings,
        NewFindingsRule::NumeratorRise
    );
    assert_eq!(
        serde_json::to_string(&canonical).expect("serialize canonical"),
        serde_json::to_string(&alias).expect("serialize alias")
    );
    let default_config = parse("{}").expect("default config");
    assert_eq!(
        default_config
            .regression_limits
            .limit(SourceSet::Production, Metric::Hazards)
            .new_findings,
        NewFindingsRule::AnyNewFinding
    );
}
