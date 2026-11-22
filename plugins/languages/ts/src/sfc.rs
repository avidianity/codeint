use codeint_model::language::{LowerError, Manifest, ReferenceRange};
use codeint_model::scan::build_reference_ranges;
use oxc_span::SourceType;

const VUE2_REFERENCE_SOURCE: &str =
    "codeint panel of 20 Vue 2 repositories, ADR 0014, codeint 0.4.0";
const VUE3_REFERENCE_SOURCE: &str =
    "codeint panel of 20 Vue 3 repositories, ADR 0015, codeint 0.4.0";

pub fn vue_reference_ranges(manifests: &[Manifest]) -> Option<Vec<ReferenceRange>> {
    let major = detect_vue_major(manifests)?;
    match major {
        2 => Some(build_reference_ranges(
            VUE2_REFERENCE_SOURCE,
            [
                ("erosion", 0.020, 0.452),
                ("verbosity", 0.036, 0.192),
                ("duplication", 0.036, 0.192),
                ("import_cycles", 0.000, 0.114),
            ],
        )),
        3 => Some(build_reference_ranges(
            VUE3_REFERENCE_SOURCE,
            [
                ("erosion", 0.019, 0.388),
                ("verbosity", 0.024, 0.346),
                ("duplication", 0.023, 0.345),
                ("import_cycles", 0.000, 0.077),
            ],
        )),
        _ => None,
    }
}

fn detect_vue_major(manifests: &[Manifest]) -> Option<u8> {
    if let Some(root) = manifests.iter().find(|m| m.path == "package.json")
        && let Some(major) = vue_major_in_manifest(&root.text)
    {
        return Some(major);
    }
    manifests
        .iter()
        .filter(|m| m.path.ends_with("/package.json"))
        .find_map(|m| vue_major_in_manifest(&m.text))
}

fn vue_major_in_manifest(text: &str) -> Option<u8> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    for section in ["dependencies", "devDependencies", "peerDependencies"] {
        if let Some(spec) = value
            .get(section)
            .and_then(|obj| obj.get("vue"))
            .and_then(serde_json::Value::as_str)
            && let Some(major) = parse_vue_major(spec)
        {
            return Some(major);
        }
    }
    None
}

fn parse_vue_major(spec: &str) -> Option<u8> {
    let bytes = spec.as_bytes();
    for (idx, &b) in bytes.iter().enumerate() {
        if !matches!(b, b'2' | b'3') {
            continue;
        }
        let prev_ok = idx == 0 || !bytes[idx - 1].is_ascii_digit();
        let next_ok = bytes.get(idx + 1).is_none_or(|&n| !n.is_ascii_digit());
        if prev_ok && next_ok {
            return Some(b - b'0');
        }
    }
    None
}

pub(crate) struct ScriptBlock<'a> {
    pub(crate) source_type: SourceType,
    pub(crate) start_line: u32,
    pub(crate) body: &'a str,
}

pub(crate) struct SfcExtracted<'a> {
    pub(crate) scripts: Vec<ScriptBlock<'a>>,
    pub(crate) header_comments: Vec<&'a str>,
}

pub(crate) fn is_sfc(path: &str) -> bool {
    path.ends_with(".vue") || path.ends_with(".svelte")
}

pub(crate) fn extract<'a>(path: &str, text: &'a str) -> Result<SfcExtracted<'a>, LowerError> {
    let skip_templates = path.ends_with(".vue");
    let mut parser = SfcScanner::new(text, skip_templates);
    parser.run()?;
    Ok(SfcExtracted {
        scripts: parser.scripts,
        header_comments: parser.header_comments,
    })
}

struct SfcScanner<'a> {
    text: &'a str,
    bytes: &'a [u8],
    pos: usize,
    line: u32,
    skip_templates: bool,
    scripts: Vec<ScriptBlock<'a>>,
    header_comments: Vec<&'a str>,
}

impl<'a> SfcScanner<'a> {
    fn new(text: &'a str, skip_templates: bool) -> Self {
        Self {
            text,
            bytes: text.as_bytes(),
            pos: 0,
            line: 1,
            skip_templates,
            scripts: Vec::new(),
            header_comments: Vec::new(),
        }
    }

    fn run(&mut self) -> Result<(), LowerError> {
        while self.pos < self.bytes.len() {
            let Some(lt_rel) = self.text[self.pos..].find('<') else {
                break;
            };
            self.advance(lt_rel);
            self.step_tag()?;
        }
        Ok(())
    }

    fn step_tag(&mut self) -> Result<(), LowerError> {
        let rest = &self.text[self.pos..];
        if rest.starts_with("<!--") {
            self.consume_html_comment();
            return Ok(());
        }
        if self.skip_templates && starts_with_tag(rest, "template") {
            self.skip_nested_template();
            return Ok(());
        }
        if starts_with_tag(rest, "style") {
            self.skip_raw_block("style");
            return Ok(());
        }
        if starts_with_tag(rest, "script") {
            return self.consume_script();
        }
        self.advance(1);
        Ok(())
    }

    fn consume_html_comment(&mut self) {
        let start = self.pos;
        if let Some(end_rel) = self.text[self.pos + 4..].find("-->") {
            let end = self.pos + 4 + end_rel + 3;
            if self.scripts.is_empty() {
                self.header_comments.push(&self.text[start..end]);
            }
            self.advance(end - start);
        } else {
            self.advance(self.bytes.len() - self.pos);
        }
    }

    fn skip_nested_template(&mut self) {
        let Some(open_end) = find_tag_end(&self.text[self.pos..]) else {
            self.advance(self.bytes.len() - self.pos);
            return;
        };
        let self_closing = self.text[self.pos..self.pos + open_end]
            .trim_end()
            .ends_with('/');
        self.advance(open_end + 1);
        if self_closing {
            return;
        }
        let mut depth = 1usize;
        while depth > 0 && self.pos < self.bytes.len() {
            let Some(lt_rel) = self.text[self.pos..].find('<') else {
                self.advance(self.bytes.len() - self.pos);
                return;
            };
            self.advance(lt_rel);
            let rest = &self.text[self.pos..];
            if rest.starts_with("<!--") {
                self.consume_html_comment();
            } else if starts_with_tag(rest, "template") {
                if let Some(end) = find_tag_end(rest) {
                    if !rest[..end].trim_end().ends_with('/') {
                        depth += 1;
                    }
                    self.advance(end + 1);
                } else {
                    self.advance(1);
                }
            } else if starts_with_close_tag(rest, "template") {
                depth -= 1;
                let end = rest.find('>').map_or(1, |i| i + 1);
                self.advance(end);
            } else {
                self.advance(1);
            }
        }
    }

    fn skip_raw_block(&mut self, tag: &str) {
        let Some(open_end) = find_tag_end(&self.text[self.pos..]) else {
            self.advance(self.bytes.len() - self.pos);
            return;
        };
        self.advance(open_end + 1);
        if let Some(close_pos) = find_close_tag(&self.text[self.pos..], tag) {
            let close_end = self.text[self.pos + close_pos..]
                .find('>')
                .map_or(close_pos, |i| close_pos + i + 1);
            self.advance(close_end);
        } else {
            self.advance(self.bytes.len() - self.pos);
        }
    }

    fn consume_script(&mut self) -> Result<(), LowerError> {
        let open_line = self.line;
        let rest = &self.text[self.pos..];
        let Some(open_end) = find_tag_end(rest) else {
            return Err(LowerError::Unparsable(format!(
                "unclosed <script> tag at line {open_line}"
            )));
        };
        let attrs = &rest[7..open_end];
        let self_closing = attrs.trim_end().ends_with('/');
        self.advance(open_end + 1);
        if self_closing {
            return Ok(());
        }
        let body_start = self.pos;
        let body_start_line = self.line;
        let Some(close_rel) = find_close_tag(&self.text[self.pos..], "script") else {
            return Err(LowerError::Unparsable(format!(
                "missing </script> closing tag for <script> at line {open_line}"
            )));
        };
        let body = &self.text[body_start..body_start + close_rel];
        let close_end = self.text[body_start + close_rel..]
            .find('>')
            .map_or(close_rel, |i| close_rel + i + 1);
        self.advance(close_end);
        if has_attr(attrs, "src") || body.trim().is_empty() {
            return Ok(());
        }
        let source_type = script_source_type(extract_attr(attrs, "lang"));
        self.scripts.push(ScriptBlock {
            source_type,
            start_line: body_start_line,
            body,
        });
        Ok(())
    }

    fn advance(&mut self, len: usize) {
        let end = (self.pos + len).min(self.bytes.len());
        self.line += self.bytes[self.pos..end]
            .iter()
            .filter(|&&b| b == b'\n')
            .count() as u32;
        self.pos = end;
    }
}

fn starts_with_tag(rest: &str, tag: &str) -> bool {
    let bytes = rest.as_bytes();
    let tag_bytes = tag.as_bytes();
    let tag_len = tag_bytes.len();
    if bytes.len() <= tag_len + 1 || bytes[0] != b'<' {
        return false;
    }
    if !bytes[1..1 + tag_len].eq_ignore_ascii_case(tag_bytes) {
        return false;
    }
    matches!(
        bytes[1 + tag_len],
        b'>' | b'/' | b' ' | b'\t' | b'\r' | b'\n'
    )
}

fn starts_with_close_tag(rest: &str, tag: &str) -> bool {
    let bytes = rest.as_bytes();
    let tag_bytes = tag.as_bytes();
    let tag_len = tag_bytes.len();
    if bytes.len() <= tag_len + 2 || bytes[0] != b'<' || bytes[1] != b'/' {
        return false;
    }
    if !bytes[2..2 + tag_len].eq_ignore_ascii_case(tag_bytes) {
        return false;
    }
    matches!(bytes[2 + tag_len], b'>' | b' ' | b'\t' | b'\r' | b'\n')
}

fn find_tag_end(rest: &str) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (idx, &b) in rest.as_bytes().iter().enumerate() {
        if let Some(q) = quote {
            if b == q {
                quote = None;
            }
        } else if matches!(b, b'"' | b'\'') {
            quote = Some(b);
        } else if b == b'>' {
            return Some(idx);
        }
    }
    None
}

fn find_close_tag(rest: &str, tag: &str) -> Option<usize> {
    let mut offset = 0usize;
    while let Some(rel) = rest[offset..].find("</") {
        let pos = offset + rel;
        if starts_with_close_tag(&rest[pos..], tag) {
            return Some(pos);
        }
        offset = pos + 2;
    }
    None
}

fn has_attr(attrs: &str, name: &str) -> bool {
    extract_attr(attrs, name).is_some()
}

fn extract_attr<'a>(attrs: &'a str, name: &str) -> Option<&'a str> {
    let bytes = attrs.as_bytes();
    let mut pos = 0usize;
    while pos < bytes.len() {
        skip_ws(bytes, &mut pos);
        let key = read_attr_key(attrs, &mut pos);
        skip_ws(bytes, &mut pos);
        let value = if bytes.get(pos) == Some(&b'=') {
            pos += 1;
            read_attr_value(attrs, &mut pos)
        } else {
            ""
        };
        if key.eq_ignore_ascii_case(name) {
            return Some(value);
        }
        if key.is_empty() && pos < bytes.len() {
            pos += 1;
        }
    }
    None
}

fn skip_ws(bytes: &[u8], pos: &mut usize) {
    while *pos < bytes.len() && bytes[*pos].is_ascii_whitespace() {
        *pos += 1;
    }
}

fn read_attr_key<'a>(attrs: &'a str, pos: &mut usize) -> &'a str {
    let bytes = attrs.as_bytes();
    let start = *pos;
    while *pos < bytes.len()
        && !bytes[*pos].is_ascii_whitespace()
        && !matches!(bytes[*pos], b'=' | b'/')
    {
        *pos += 1;
    }
    &attrs[start..*pos]
}

fn read_attr_value<'a>(attrs: &'a str, pos: &mut usize) -> &'a str {
    let bytes = attrs.as_bytes();
    while *pos < bytes.len() && bytes[*pos].is_ascii_whitespace() {
        *pos += 1;
    }
    if let Some(&q @ (b'"' | b'\'')) = bytes.get(*pos) {
        *pos += 1;
        let start = *pos;
        while *pos < bytes.len() && bytes[*pos] != q {
            *pos += 1;
        }
        let val = &attrs[start..*pos];
        if *pos < bytes.len() {
            *pos += 1;
        }
        val
    } else {
        let start = *pos;
        while *pos < bytes.len() && !bytes[*pos].is_ascii_whitespace() && bytes[*pos] != b'/' {
            *pos += 1;
        }
        &attrs[start..*pos]
    }
}

fn script_source_type(lang: Option<&str>) -> SourceType {
    match lang.map(str::to_ascii_lowercase).as_deref() {
        Some("ts") => SourceType::ts(),
        Some("tsx") => SourceType::tsx(),
        Some("jsx") => SourceType::jsx(),
        _ => SourceType::mjs().with_jsx(true),
    }
}
