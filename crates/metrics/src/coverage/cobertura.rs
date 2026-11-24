use std::collections::BTreeMap;

use super::{
    CoverageFormat, CoverageLimits, CoverageParseError, ParsedCoverageReport, collect_file_records,
    decode_report_utf8, merge_line_hit,
};

pub fn parse_cobertura(bytes: &[u8]) -> Result<ParsedCoverageReport, CoverageParseError> {
    parse_cobertura_with_limits(bytes, CoverageLimits::default())
}

pub fn parse_cobertura_with_limits(
    bytes: &[u8],
    limits: CoverageLimits,
) -> Result<ParsedCoverageReport, CoverageParseError> {
    let text = decode_report_utf8(bytes, limits.max_bytes)?;
    parse_cobertura_str(text, limits.max_xml_depth)
}

pub(crate) fn parse_cobertura_str(
    text: &str,
    max_depth: usize,
) -> Result<ParsedCoverageReport, CoverageParseError> {
    let mut scanner = XmlScanner::new(text);
    let mut builder = CoberturaBuilder::new(max_depth);
    while !scanner.is_eof() {
        if scanner.peek() == Some(b'<') {
            parse_markup(&mut scanner, &mut builder)?;
        } else {
            let text_chunk = scanner.read_text()?;
            builder.handle_text(&text_chunk)?;
        }
    }
    builder.finish()
}

fn parse_markup(
    scanner: &mut XmlScanner<'_>,
    builder: &mut CoberturaBuilder,
) -> Result<(), CoverageParseError> {
    if scanner.starts_with("<!--") {
        return scanner.skip_comment();
    }
    if scanner.starts_with("<![CDATA[") {
        let cdata = scanner.read_cdata()?;
        return builder.handle_cdata(cdata);
    }
    if scanner.starts_with("<?") {
        return scanner.skip_processing_instruction(&mut builder.saw_xml_decl, builder.saw_root);
    }
    if scanner.starts_with("<!") {
        return reject_markup_declaration(scanner);
    }
    if scanner.starts_with("</") {
        let name = scanner.read_end_tag()?;
        return builder.handle_end_tag(name);
    }
    let tag = scanner.read_start_tag()?;
    builder.handle_start_tag(tag.name, tag.attrs, tag.self_closing)
}

struct StartTag<'a> {
    name: &'a str,
    attrs: Vec<(String, String)>,
    self_closing: bool,
}

fn reject_markup_declaration(scanner: &XmlScanner<'_>) -> Result<(), CoverageParseError> {
    let reason = if scanner.starts_with_ignore_ascii_case("<!DOCTYPE") {
        "DOCTYPE declarations are not allowed"
    } else if scanner.starts_with_ignore_ascii_case("<!ENTITY") {
        "ENTITY declarations are not allowed"
    } else {
        "XML markup declarations are not allowed"
    };
    Err(CoverageParseError::XmlSecurityViolation {
        reason: reason.to_owned(),
    })
}

struct CoberturaBuilder {
    max_depth: usize,
    saw_xml_decl: bool,
    saw_root: bool,
    root_closed: bool,
    stack: Vec<String>,
    current_source: Option<String>,
    current_class: Option<String>,
    source_roots: Vec<String>,
    files: BTreeMap<String, BTreeMap<u32, u64>>,
}

impl CoberturaBuilder {
    fn new(max_depth: usize) -> CoberturaBuilder {
        CoberturaBuilder {
            max_depth,
            saw_xml_decl: false,
            saw_root: false,
            root_closed: false,
            stack: Vec::new(),
            current_source: None,
            current_class: None,
            source_roots: Vec::new(),
            files: BTreeMap::new(),
        }
    }

    fn handle_start_tag(
        &mut self,
        name: &str,
        attrs: Vec<(String, String)>,
        self_closing: bool,
    ) -> Result<(), CoverageParseError> {
        if self.root_closed {
            return Err(invalid_xml("unexpected element after root element"));
        }
        if !self.saw_root {
            self.saw_root = true;
            if name != "coverage" {
                return Err(CoverageParseError::UnrecognizedFormat);
            }
        }
        if self.stack.len() + 1 > self.max_depth {
            return Err(CoverageParseError::XmlNestingTooDeep {
                max_depth: self.max_depth,
            });
        }
        if self.stack.as_slice() == ["coverage"] && name == "project" {
            return Err(CoverageParseError::UnrecognizedFormat);
        }
        self.apply_start_tag(name, &attrs, self_closing)?;
        if self_closing {
            if self.stack.is_empty() {
                self.root_closed = true;
            }
        } else {
            self.stack.push(name.to_owned());
        }
        Ok(())
    }

    fn apply_start_tag(
        &mut self,
        name: &str,
        attrs: &[(String, String)],
        self_closing: bool,
    ) -> Result<(), CoverageParseError> {
        match name {
            "source" if self.stack.as_slice() == ["coverage", "sources"] => {
                if !self_closing {
                    self.current_source = Some(String::new());
                }
            }
            "class" if !self.stack.iter().any(|tag| tag == "methods") => {
                self.record_class(attrs, self_closing)?;
            }
            "line" if self.is_inside_class_lines() => {
                self.record_line(attrs)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn is_inside_class_lines(&self) -> bool {
        let len = self.stack.len();
        len >= 2 && self.stack[len - 2] == "class" && self.stack[len - 1] == "lines"
    }

    fn record_class(
        &mut self,
        attrs: &[(String, String)],
        self_closing: bool,
    ) -> Result<(), CoverageParseError> {
        let filename = find_attr(attrs, "filename")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                invalid_xml("class element is missing a non-empty filename attribute")
            })?;
        self.files.entry(filename.to_owned()).or_default();
        if !self_closing {
            self.current_class = Some(filename.to_owned());
        }
        Ok(())
    }

    fn record_line(&mut self, attrs: &[(String, String)]) -> Result<(), CoverageParseError> {
        let number_raw = find_attr(attrs, "number")
            .ok_or_else(|| invalid_xml("line element is missing a number attribute"))?;
        let hits_raw = find_attr(attrs, "hits")
            .ok_or_else(|| invalid_xml("line element is missing a hits attribute"))?;
        let line_number: u32 = number_raw
            .trim()
            .parse()
            .map_err(|_| invalid_xml(&format!("invalid line number {number_raw:?}")))?;
        if line_number == 0 {
            return Err(invalid_xml("line number must be positive"));
        }
        let hits: u64 = hits_raw
            .trim()
            .parse()
            .map_err(|_| invalid_xml(&format!("invalid line hits {hits_raw:?}")))?;
        if let Some(filename) = self.current_class.as_deref()
            && let Some(lines) = self.files.get_mut(filename)
        {
            merge_line_hit(lines, line_number, hits);
        }
        Ok(())
    }

    fn handle_end_tag(&mut self, name: &str) -> Result<(), CoverageParseError> {
        let Some(open_name) = self.stack.pop() else {
            return Err(invalid_xml(&format!("unexpected closing tag </{name}>")));
        };
        if open_name != name {
            return Err(invalid_xml(&format!(
                "mismatched closing tag </{name}> for <{open_name}>"
            )));
        }
        if name == "source"
            && let Some(source) = self.current_source.take()
        {
            let trimmed = source.trim();
            if !trimmed.is_empty() {
                self.source_roots.push(trimmed.to_owned());
            }
        }
        if name == "class" {
            self.current_class = None;
        }
        if self.stack.is_empty() {
            self.root_closed = true;
        }
        Ok(())
    }

    fn handle_text(&mut self, text: &str) -> Result<(), CoverageParseError> {
        if self.stack.is_empty() {
            if text.bytes().any(|byte| !is_xml_whitespace(byte)) {
                return Err(invalid_xml("unexpected text outside root element"));
            }
            return Ok(());
        }
        if let Some(source) = &mut self.current_source {
            source.push_str(text);
        }
        Ok(())
    }

    fn handle_cdata(&mut self, cdata: &str) -> Result<(), CoverageParseError> {
        if self.stack.is_empty() {
            return Err(invalid_xml(
                "CDATA section is not allowed outside root element",
            ));
        }
        if let Some(source) = &mut self.current_source {
            source.push_str(cdata);
        }
        Ok(())
    }

    fn finish(self) -> Result<ParsedCoverageReport, CoverageParseError> {
        if let Some(unclosed) = self.stack.last() {
            return Err(invalid_xml(&format!("unclosed XML element <{unclosed}>")));
        }
        if !self.saw_root {
            return Err(invalid_xml("XML document has no root element"));
        }
        Ok(ParsedCoverageReport {
            format: CoverageFormat::Cobertura,
            source_roots: self.source_roots,
            files: collect_file_records(self.files),
        })
    }
}

struct XmlScanner<'a> {
    text: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> XmlScanner<'a> {
    fn new(text: &'a str) -> XmlScanner<'a> {
        XmlScanner {
            text,
            bytes: text.as_bytes(),
            pos: 0,
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn remaining(&self) -> &'a str {
        &self.text[self.pos..]
    }

    fn starts_with(&self, prefix: &str) -> bool {
        self.remaining().starts_with(prefix)
    }

    fn starts_with_ignore_ascii_case(&self, prefix: &str) -> bool {
        self.remaining()
            .as_bytes()
            .get(..prefix.len())
            .is_some_and(|slice| slice.eq_ignore_ascii_case(prefix.as_bytes()))
    }

    fn skip_whitespace(&mut self) -> bool {
        let start = self.pos;
        while let Some(byte) = self.peek() {
            if is_xml_whitespace(byte) {
                self.pos += 1;
            } else {
                break;
            }
        }
        self.pos > start
    }

    fn skip_comment(&mut self) -> Result<(), CoverageParseError> {
        self.pos += 4;
        let Some(end_offset) = self.remaining().find("-->") else {
            return Err(invalid_xml("unclosed XML comment"));
        };
        if self.remaining()[..end_offset].contains("--") {
            return Err(invalid_xml("XML comment must not contain '--'"));
        }
        self.pos += end_offset + 3;
        Ok(())
    }

    fn read_cdata(&mut self) -> Result<&'a str, CoverageParseError> {
        self.pos += 9;
        let Some(end_offset) = self.remaining().find("]]>") else {
            return Err(invalid_xml("unclosed CDATA section"));
        };
        let content = &self.remaining()[..end_offset];
        self.pos += end_offset + 3;
        Ok(content)
    }

    fn skip_processing_instruction(
        &mut self,
        saw_xml_decl: &mut bool,
        saw_root: bool,
    ) -> Result<(), CoverageParseError> {
        self.pos += 2;
        let target = self.read_name()?;
        if target != "xml" {
            return Err(CoverageParseError::XmlSecurityViolation {
                reason: format!("processing instruction <?{target}?> is not allowed"),
            });
        }
        if *saw_xml_decl || saw_root {
            return Err(invalid_xml(
                "XML declaration is only allowed once before the root element",
            ));
        }
        let Some(end_offset) = self.remaining().find("?>") else {
            return Err(invalid_xml("unclosed XML declaration"));
        };
        let body = &self.remaining()[..end_offset];
        if !body.is_empty()
            && !body
                .as_bytes()
                .first()
                .copied()
                .is_some_and(is_xml_whitespace)
        {
            return Err(CoverageParseError::XmlSecurityViolation {
                reason: "processing instructions other than <?xml ...?> are not allowed".to_owned(),
            });
        }
        if body.contains('<') || body.contains('&') {
            return Err(invalid_xml("invalid XML declaration"));
        }
        self.pos += end_offset + 2;
        *saw_xml_decl = true;
        Ok(())
    }

    fn read_start_tag(&mut self) -> Result<StartTag<'a>, CoverageParseError> {
        self.pos += 1;
        let name = self.read_name()?;
        let (attrs, self_closing) = self.read_attributes()?;
        Ok(StartTag {
            name,
            attrs,
            self_closing,
        })
    }

    fn read_end_tag(&mut self) -> Result<&'a str, CoverageParseError> {
        self.pos += 2;
        let name = self.read_name()?;
        self.skip_whitespace();
        if self.peek() != Some(b'>') {
            return Err(invalid_xml("expected '>' at end of closing tag"));
        }
        self.pos += 1;
        Ok(name)
    }

    fn read_name(&mut self) -> Result<&'a str, CoverageParseError> {
        let start = self.pos;
        let mut chars = self.remaining().char_indices();
        let Some((_, first)) = chars.next() else {
            return Err(invalid_xml("unexpected end of XML while reading name"));
        };
        if !is_xml_name_start(first) {
            return Err(invalid_xml("invalid XML name"));
        }
        let mut end = first.len_utf8();
        for (offset, ch) in chars {
            if is_xml_name_char(ch) {
                end = offset + ch.len_utf8();
            } else {
                break;
            }
        }
        self.pos = start + end;
        Ok(&self.text[start..self.pos])
    }

    fn read_attributes(&mut self) -> Result<(Vec<(String, String)>, bool), CoverageParseError> {
        let mut attrs: Vec<(String, String)> = Vec::new();
        loop {
            let had_ws = self.skip_whitespace();
            if self.starts_with("/>") {
                self.pos += 2;
                return Ok((attrs, true));
            }
            if self.peek() == Some(b'>') {
                self.pos += 1;
                return Ok((attrs, false));
            }
            if self.is_eof() {
                return Err(invalid_xml("unclosed XML start tag"));
            }
            if !had_ws {
                return Err(invalid_xml("expected whitespace before XML attribute"));
            }
            let attr_name = self.read_name()?;
            if attrs.iter().any(|(existing, _)| existing == attr_name) {
                return Err(invalid_xml(&format!(
                    "duplicate XML attribute {attr_name:?}"
                )));
            }
            self.skip_whitespace();
            if self.peek() != Some(b'=') {
                return Err(invalid_xml("expected '=' after XML attribute name"));
            }
            self.pos += 1;
            self.skip_whitespace();
            let value = self.read_quoted_value()?;
            attrs.push((attr_name.to_owned(), value));
        }
    }

    fn read_quoted_value(&mut self) -> Result<String, CoverageParseError> {
        let quote = match self.peek() {
            Some(b'"') => b'"',
            Some(b'\'') => b'\'',
            _ => return Err(invalid_xml("expected quoted XML attribute value")),
        };
        self.pos += 1;
        let start = self.pos;
        while let Some(byte) = self.peek() {
            if byte == quote {
                let raw = &self.text[start..self.pos];
                self.pos += 1;
                return decode_xml_text(raw);
            }
            if byte == b'<' {
                return Err(invalid_xml("'<' is not allowed inside XML attribute value"));
            }
            self.pos += 1;
        }
        Err(invalid_xml("unclosed XML attribute value"))
    }

    fn read_text(&mut self) -> Result<String, CoverageParseError> {
        let start = self.pos;
        while let Some(byte) = self.peek() {
            if byte == b'<' {
                break;
            }
            self.pos += 1;
        }
        let raw = &self.text[start..self.pos];
        if raw.contains("]]>") {
            return Err(invalid_xml("']]>' is not allowed in XML text content"));
        }
        decode_xml_text(raw)
    }
}

fn find_attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

fn decode_xml_text(raw: &str) -> Result<String, CoverageParseError> {
    if !raw.contains('&') {
        return Ok(raw.to_owned());
    }
    let mut decoded = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(amp_index) = rest.find('&') {
        decoded.push_str(&rest[..amp_index]);
        let after_amp = &rest[amp_index + 1..];
        let Some(semi_index) = after_amp.find(';') else {
            return Err(invalid_xml("unterminated XML entity reference"));
        };
        let entity = &after_amp[..semi_index];
        if entity.is_empty() || entity.contains('&') {
            return Err(invalid_xml("malformed XML entity reference"));
        }
        decoded.push(decode_xml_entity(entity)?);
        rest = &after_amp[semi_index + 1..];
    }
    decoded.push_str(rest);
    Ok(decoded)
}

fn decode_xml_entity(entity: &str) -> Result<char, CoverageParseError> {
    match entity {
        "lt" => Ok('<'),
        "gt" => Ok('>'),
        "amp" => Ok('&'),
        "quot" => Ok('"'),
        "apos" => Ok('\''),
        _ if entity.starts_with('#') => parse_numeric_entity(&entity[1..]),
        _ => Err(CoverageParseError::XmlSecurityViolation {
            reason: format!("disallowed XML entity reference &{entity};"),
        }),
    }
}

fn parse_numeric_entity(spec: &str) -> Result<char, CoverageParseError> {
    let code = if let Some(hex) = spec.strip_prefix('x').or_else(|| spec.strip_prefix('X')) {
        u32::from_str_radix(hex, 16)
    } else {
        spec.parse::<u32>()
    }
    .map_err(|_| invalid_xml(&format!("invalid numeric character reference &#{spec};")))?;
    if !is_valid_xml_char_code(code) {
        return Err(invalid_xml(&format!(
            "character reference &#{spec}; is not a valid XML character"
        )));
    }
    char::from_u32(code).ok_or_else(|| {
        invalid_xml(&format!(
            "character reference &#{spec}; is not a valid Unicode scalar"
        ))
    })
}

fn is_valid_xml_char_code(code: u32) -> bool {
    matches!(
        code,
        0x09 | 0x0A | 0x0D | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF
    )
}

fn is_xml_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
}

fn is_xml_name_start(ch: char) -> bool {
    ch == ':' || ch == '_' || ch.is_alphabetic()
}

fn is_xml_name_char(ch: char) -> bool {
    is_xml_name_start(ch) || ch == '-' || ch == '.' || ch.is_numeric()
}

fn invalid_xml(reason: &str) -> CoverageParseError {
    CoverageParseError::InvalidCoberturaXml {
        reason: reason.to_owned(),
    }
}
