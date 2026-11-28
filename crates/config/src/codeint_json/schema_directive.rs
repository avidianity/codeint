use std::ops::Range;

use jsonc_parser::{CollectOptions, ParseOptions, parse_to_ast};
use serde_json::Value;

use super::formats::{parse_toml_value, parse_yaml_value};
use crate::config_error::{ConfigFileKind, ConfigSyntax, ConfigurationFileError};

pub(super) const OFFICIAL_SCHEMA_PREFIX: &str =
    "https://raw.githubusercontent.com/avidianity/codeint/v";
pub(super) const OFFICIAL_SCHEMA_SUFFIX: &str = "/schema/codeint.schema.json";
const OFFICIAL_TAG_URL_PREFIX: &str = "https://raw.githubusercontent.com/avidianity/codeint/";

pub(super) const JSONC_PARSE_OPTIONS: ParseOptions = ParseOptions {
    allow_comments: true,
    allow_trailing_commas: true,
    allow_loose_object_property_names: false,
    allow_missing_commas: false,
    allow_single_quoted_strings: false,
    allow_hexadecimal_numbers: false,
    allow_unary_plus_numbers: false,
    allow_bare_decimal_point_numbers: false,
    allow_non_finite_numbers: false,
    allow_extended_string_escapes: false,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaValueSpan {
    pub span: Range<usize>,
    pub value: String,
}

pub fn schema_value_span(text: &str) -> Option<SchemaValueSpan> {
    let without_bom = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let bom_len = text.len() - without_bom.len();
    let parsed = parse_to_ast(
        without_bom,
        &CollectOptions::default(),
        &JSONC_PARSE_OPTIONS,
    )
    .ok()?;
    let value = parsed.value?;
    let object = value.as_object()?;
    let property = object
        .properties
        .iter()
        .rev()
        .find(|property| property.name.as_str() == "$schema")?;
    let jsonc_parser::ast::Value::StringLit(schema) = &property.value else {
        return None;
    };
    Some(SchemaValueSpan {
        span: (schema.range.start + bom_len)..(schema.range.end + bom_len),
        value: schema.value.to_string(),
    })
}

pub(super) fn resolve_schema_url(
    kind: ConfigFileKind,
    text: &str,
    value: &Value,
) -> Result<Option<String>, ConfigurationFileError> {
    let directives = extract_schema_directives(kind, text)?;
    let directive_url = directives.first().map(|span| span.value.as_str());
    let key_url = value.get("$schema").and_then(Value::as_str);
    match (directive_url, key_url) {
        (Some(directive), Some(key)) if directive != key => {
            Err(ConfigurationFileError::ConflictingSchemaDirectives {
                file: kind,
                first_label: "its leading schema comment",
                first_url: directive.to_owned(),
                second_label: "$schema key",
                second_url: key.to_owned(),
            })
        }
        (Some(directive), _) => Ok(Some(directive.to_owned())),
        (None, Some(key)) => Ok(Some(key.to_owned())),
        (None, None) => Ok(None),
    }
}

fn extract_schema_directives(
    kind: ConfigFileKind,
    text: &str,
) -> Result<Vec<SchemaValueSpan>, ConfigurationFileError> {
    let syntax = kind.syntax();
    if syntax == ConfigSyntax::Json {
        return Ok(Vec::new());
    }
    let without_bom = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let mut offset = text.len() - without_bom.len();
    let mut found: Vec<SchemaValueSpan> = Vec::new();
    for raw_line in without_bom.split_inclusive('\n') {
        let line = raw_line.trim_end_matches(['\r', '\n']);
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            offset += raw_line.len();
            continue;
        }
        if !trimmed.starts_with('#') {
            break;
        }
        let line_start = offset + (line.len() - trimmed.len());
        if let Some(span) = parse_directive_line(syntax, trimmed, line_start) {
            record_directive(kind, &mut found, span)?;
        }
        offset += raw_line.len();
    }
    Ok(found)
}

fn record_directive(
    file: ConfigFileKind,
    found: &mut Vec<SchemaValueSpan>,
    next: SchemaValueSpan,
) -> Result<(), ConfigurationFileError> {
    if let Some(existing) = found.first()
        && existing.value != next.value
    {
        return Err(ConfigurationFileError::ConflictingSchemaDirectives {
            file,
            first_label: "a leading schema comment",
            first_url: existing.value.clone(),
            second_label: "another leading schema comment",
            second_url: next.value,
        });
    }
    found.push(next);
    Ok(())
}

fn parse_directive_line(
    syntax: ConfigSyntax,
    trimmed: &str,
    line_start: usize,
) -> Option<SchemaValueSpan> {
    match syntax {
        ConfigSyntax::Toml => parse_toml_schema_directive(trimmed, line_start),
        ConfigSyntax::Yaml => parse_yaml_schema_directive(trimmed, line_start),
        ConfigSyntax::Json => None,
    }
}

fn parse_toml_schema_directive(trimmed: &str, line_start: usize) -> Option<SchemaValueSpan> {
    let after_prefix = trimmed.strip_prefix("#:schema")?;
    if !after_prefix.chars().next()?.is_whitespace() {
        return None;
    }
    extract_url_token(trimmed, after_prefix, line_start)
}

fn parse_yaml_schema_directive(trimmed: &str, line_start: usize) -> Option<SchemaValueSpan> {
    let after_hash = trimmed.strip_prefix('#')?.trim_start();
    let after_ls = after_hash
        .strip_prefix("yaml-language-server:")?
        .trim_start();
    let after_key = after_ls.strip_prefix("$schema=")?;
    extract_url_token(trimmed, after_key, line_start)
}

fn strip_inline_hash_comment(text: &str) -> &str {
    let bytes = text.as_bytes();
    for (index, &byte) in bytes.iter().enumerate() {
        if byte == b'#' && index > 0 && matches!(bytes[index - 1], b' ' | b'\t') {
            return text[..index].trim_end();
        }
    }
    text.trim_end()
}

fn extract_url_token(
    full_trimmed: &str,
    after_marker: &str,
    line_start: usize,
) -> Option<SchemaValueSpan> {
    let value_slice = strip_inline_hash_comment(after_marker.trim_start());
    if value_slice.is_empty() || value_slice.chars().any(char::is_whitespace) {
        return None;
    }
    let rel_start = value_slice.as_ptr() as usize - full_trimmed.as_ptr() as usize;
    let start = line_start + rel_start;
    Some(SchemaValueSpan {
        span: start..(start + value_slice.len()),
        value: value_slice.to_owned(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewrittenSchemaForm {
    JsonKey,
    TomlDirective,
    TomlKey,
    TomlDirectiveAndKey,
    YamlDirective,
    YamlKey,
    YamlDirectiveAndKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewrittenSchema {
    pub text: String,
    pub url: String,
    pub form: RewrittenSchemaForm,
}

struct Replacement {
    span: Range<usize>,
    text: String,
}

pub fn rewrite_schema_tag(
    kind: ConfigFileKind,
    text: &str,
    target_tag: &str,
) -> Option<RewrittenSchema> {
    match kind.syntax() {
        ConfigSyntax::Json => rewrite_json_schema_tag(text, target_tag),
        ConfigSyntax::Toml | ConfigSyntax::Yaml => {
            rewrite_toml_or_yaml_schema_tag(kind, text, target_tag)
        }
    }
}

fn updated_official_url(current_url: &str, target_tag: &str) -> Option<String> {
    let rest = current_url.strip_prefix(OFFICIAL_TAG_URL_PREFIX)?;
    let current_ref = rest.strip_suffix(OFFICIAL_SCHEMA_SUFFIX)?;
    if current_ref.is_empty() || current_ref.contains('/') || current_ref == target_tag {
        return None;
    }
    Some(format!(
        "{OFFICIAL_TAG_URL_PREFIX}{target_tag}{OFFICIAL_SCHEMA_SUFFIX}"
    ))
}

fn rewrite_json_schema_tag(text: &str, target_tag: &str) -> Option<RewrittenSchema> {
    let schema = schema_value_span(text)?;
    let new_url = updated_official_url(&schema.value, target_tag)?;
    let updated = apply_replacements(
        text,
        vec![Replacement {
            span: schema.span,
            text: format!("\"{new_url}\""),
        }],
    )?;
    Some(RewrittenSchema {
        text: updated,
        url: new_url,
        form: RewrittenSchemaForm::JsonKey,
    })
}

fn parse_structured_document(kind: ConfigFileKind, text: &str) -> Option<Value> {
    match kind.syntax() {
        ConfigSyntax::Toml => parse_toml_value(text, kind).ok(),
        ConfigSyntax::Yaml => parse_yaml_value(text, kind).ok(),
        ConfigSyntax::Json => None,
    }
}

fn rewrite_toml_or_yaml_schema_tag(
    kind: ConfigFileKind,
    text: &str,
    target_tag: &str,
) -> Option<RewrittenSchema> {
    let value = parse_structured_document(kind, text)?;
    let active_url = resolve_schema_url(kind, text, &value).ok()??;
    let new_url = updated_official_url(&active_url, target_tag)?;
    let (replacements, form) = collect_structured_replacements(kind, text, &value, &new_url)?;
    let updated = apply_replacements(text, replacements)?;
    let reparsed = parse_structured_document(kind, &updated)?;
    let verified = resolve_schema_url(kind, &updated, &reparsed).ok()??;
    (verified == new_url).then_some(RewrittenSchema {
        text: updated,
        url: new_url,
        form,
    })
}

fn collect_structured_replacements(
    kind: ConfigFileKind,
    text: &str,
    value: &Value,
    new_url: &str,
) -> Option<(Vec<Replacement>, RewrittenSchemaForm)> {
    let directives = extract_schema_directives(kind, text).ok()?;
    let has_directive = !directives.is_empty();
    let mut replacements: Vec<Replacement> = directives
        .into_iter()
        .map(|span| Replacement {
            span: span.span,
            text: new_url.to_owned(),
        })
        .collect();
    let mut has_key = false;
    if let Some(expected_key_url) = value.get("$schema").and_then(Value::as_str) {
        let key_span = unique_top_level_key_schema_span(kind.syntax(), text, expected_key_url)?;
        let original = text.get(key_span.span.clone())?;
        replacements.push(Replacement {
            span: key_span.span,
            text: format_scalar_replacement(original, new_url),
        });
        has_key = true;
    }
    let form = structured_schema_form(kind.syntax(), has_directive, has_key)?;
    Some((replacements, form))
}

fn structured_schema_form(
    syntax: ConfigSyntax,
    has_directive: bool,
    has_key: bool,
) -> Option<RewrittenSchemaForm> {
    match (syntax, has_directive, has_key) {
        (ConfigSyntax::Toml, true, true) => Some(RewrittenSchemaForm::TomlDirectiveAndKey),
        (ConfigSyntax::Toml, true, false) => Some(RewrittenSchemaForm::TomlDirective),
        (ConfigSyntax::Toml, false, true) => Some(RewrittenSchemaForm::TomlKey),
        (ConfigSyntax::Yaml, true, true) => Some(RewrittenSchemaForm::YamlDirectiveAndKey),
        (ConfigSyntax::Yaml, true, false) => Some(RewrittenSchemaForm::YamlDirective),
        (ConfigSyntax::Yaml, false, true) => Some(RewrittenSchemaForm::YamlKey),
        _ => None,
    }
}

fn format_scalar_replacement(original: &str, new_url: &str) -> String {
    if original.starts_with('"') && original.ends_with('"') {
        format!("\"{new_url}\"")
    } else if original.starts_with('\'') && original.ends_with('\'') {
        format!("'{new_url}'")
    } else {
        new_url.to_owned()
    }
}

fn unique_top_level_key_schema_span(
    syntax: ConfigSyntax,
    text: &str,
    expected_value: &str,
) -> Option<SchemaValueSpan> {
    let without_bom = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let mut offset = text.len() - without_bom.len();
    let mut matched: Option<SchemaValueSpan> = None;
    for raw_line in without_bom.split_inclusive('\n') {
        let line = raw_line.trim_end_matches(['\r', '\n']);
        if syntax == ConfigSyntax::Toml && line.trim_start().starts_with('[') {
            break;
        }
        if let Some(span) = parse_top_level_schema_key_line(syntax, line, offset)
            && span.value == expected_value
        {
            if matched.is_some() {
                return None;
            }
            matched = Some(span);
        }
        offset += raw_line.len();
    }
    matched
}

fn parse_top_level_schema_key_line(
    syntax: ConfigSyntax,
    line: &str,
    line_offset: usize,
) -> Option<SchemaValueSpan> {
    let rest = match syntax {
        ConfigSyntax::Toml => strip_toml_schema_assignment(line)?,
        ConfigSyntax::Yaml => strip_yaml_schema_assignment(line)?,
        ConfigSyntax::Json => return None,
    };
    let trimmed = rest.trim_start();
    let value_start = line_offset + (trimmed.as_ptr() as usize - line.as_ptr() as usize);
    extract_scalar_span(trimmed, value_start)
}

fn strip_toml_schema_assignment(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let after_key = trimmed
        .strip_prefix("\"$schema\"")
        .or_else(|| trimmed.strip_prefix("'$schema'"))?;
    after_key.trim_start().strip_prefix('=')
}

fn strip_yaml_schema_assignment(line: &str) -> Option<&str> {
    if line.starts_with(|c: char| c.is_whitespace()) {
        return None;
    }
    let after_key = line
        .strip_prefix("\"$schema\"")
        .or_else(|| line.strip_prefix("'$schema'"))
        .or_else(|| line.strip_prefix("$schema"))?;
    after_key.trim_start().strip_prefix(':')
}

fn extract_scalar_span(trimmed: &str, value_start: usize) -> Option<SchemaValueSpan> {
    if trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''") {
        return None;
    }
    if let Some(quote) = trimmed.chars().next().filter(|c| *c == '"' || *c == '\'') {
        let end_quote = trimmed[1..].find(quote)? + 1;
        let inner = &trimmed[1..end_quote];
        let trailing = trimmed[end_quote + 1..].trim_start();
        if !trailing.is_empty() && !trailing.starts_with('#') {
            return None;
        }
        return Some(SchemaValueSpan {
            span: value_start..(value_start + end_quote + 1),
            value: inner.to_owned(),
        });
    }
    let without_comment = strip_inline_hash_comment(trimmed);
    if without_comment.is_empty() || without_comment.chars().any(char::is_whitespace) {
        return None;
    }
    Some(SchemaValueSpan {
        span: value_start..(value_start + without_comment.len()),
        value: without_comment.to_owned(),
    })
}

fn apply_replacements(text: &str, mut replacements: Vec<Replacement>) -> Option<String> {
    if replacements.is_empty() {
        return None;
    }
    replacements.sort_by_key(|left| std::cmp::Reverse(left.span.start));
    let mut updated = text.to_owned();
    for replacement in replacements {
        updated.get(replacement.span.clone())?;
        updated.replace_range(replacement.span, &replacement.text);
    }
    Some(updated)
}
