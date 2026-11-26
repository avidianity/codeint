use std::ops::Range;

use jsonc_parser::{CollectOptions, ParseOptions, parse_to_ast};
use serde_json::Value;

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
    let directive = extract_schema_directive(kind, text)?;
    let key_url = value.get("$schema").and_then(Value::as_str);
    match (directive, key_url) {
        (Some(directive), Some(key)) if directive.value != key => {
            Err(ConfigurationFileError::ConflictingSchemaDirectives {
                file: kind,
                directive_url: directive.value,
                key_url: key.to_owned(),
            })
        }
        (Some(directive), _) => Ok(Some(directive.value)),
        (None, Some(key)) => Ok(Some(key.to_owned())),
        (None, None) => Ok(None),
    }
}

pub(super) fn extract_schema_directive(
    kind: ConfigFileKind,
    text: &str,
) -> Result<Option<SchemaValueSpan>, ConfigurationFileError> {
    let syntax = kind.syntax();
    if syntax == ConfigSyntax::Json {
        return Ok(None);
    }
    let without_bom = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let mut offset = text.len() - without_bom.len();
    let mut found: Option<SchemaValueSpan> = None;
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
    slot: &mut Option<SchemaValueSpan>,
    next: SchemaValueSpan,
) -> Result<(), ConfigurationFileError> {
    if let Some(existing) = slot.as_ref()
        && existing.value != next.value
    {
        return Err(ConfigurationFileError::ConflictingSchemaDirectives {
            file,
            directive_url: existing.value.clone(),
            key_url: next.value,
        });
    }
    *slot = Some(next);
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

fn extract_url_token(
    full_trimmed: &str,
    after_marker: &str,
    line_start: usize,
) -> Option<SchemaValueSpan> {
    let value_slice = after_marker.trim();
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

struct Replacement {
    span: Range<usize>,
    text: String,
}

pub fn rewrite_schema_tag(
    kind: ConfigFileKind,
    text: &str,
    target_tag: &str,
) -> Option<(String, String)> {
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

fn rewrite_json_schema_tag(text: &str, target_tag: &str) -> Option<(String, String)> {
    let schema = schema_value_span(text)?;
    let new_url = updated_official_url(&schema.value, target_tag)?;
    let updated = apply_replacements(
        text,
        vec![Replacement {
            span: schema.span,
            text: format!("\"{new_url}\""),
        }],
    )?;
    Some((updated, new_url))
}

fn rewrite_toml_or_yaml_schema_tag(
    kind: ConfigFileKind,
    text: &str,
    target_tag: &str,
) -> Option<(String, String)> {
    let directive = extract_schema_directive(kind, text).ok()?;
    let key_span = top_level_key_schema_span(kind.syntax(), text);
    if has_conflicting_spans(directive.as_ref(), key_span.as_ref()) {
        return None;
    }
    let mut replacements = Vec::new();
    let mut new_url = None;
    push_replacement(directive, target_tag, None, &mut replacements, &mut new_url)?;
    push_replacement(
        key_span,
        target_tag,
        Some(text),
        &mut replacements,
        &mut new_url,
    )?;
    let updated = apply_replacements(text, replacements)?;
    Some((updated, new_url?))
}

fn has_conflicting_spans(
    directive: Option<&SchemaValueSpan>,
    key_span: Option<&SchemaValueSpan>,
) -> bool {
    matches!((directive, key_span), (Some(d), Some(k)) if d.value != k.value)
}

fn push_replacement(
    span: Option<SchemaValueSpan>,
    target_tag: &str,
    preserve_quotes_from: Option<&str>,
    replacements: &mut Vec<Replacement>,
    new_url: &mut Option<String>,
) -> Option<()> {
    let Some(span) = span else {
        return Some(());
    };
    let Some(url) = updated_official_url(&span.value, target_tag) else {
        return Some(());
    };
    let replacement = match preserve_quotes_from {
        Some(source) => format_scalar_replacement(source.get(span.span.clone())?, &url),
        None => url.clone(),
    };
    replacements.push(Replacement {
        span: span.span,
        text: replacement,
    });
    *new_url = Some(url);
    Some(())
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

fn top_level_key_schema_span(syntax: ConfigSyntax, text: &str) -> Option<SchemaValueSpan> {
    let without_bom = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let mut offset = text.len() - without_bom.len();
    for raw_line in without_bom.split_inclusive('\n') {
        let line = raw_line.trim_end_matches(['\r', '\n']);
        if syntax == ConfigSyntax::Toml && line.trim_start().starts_with('[') {
            break;
        }
        if let Some(span) = parse_top_level_schema_key_line(syntax, line, offset) {
            return Some(span);
        }
        offset += raw_line.len();
    }
    None
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
    let without_comment = trimmed
        .split_once(" #")
        .map_or(trimmed, |(before, _)| before)
        .trim_end();
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
