use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Language {
    Rust,
    Wit,
    Script,
    Toml,
    Yaml,
    Shell,
}

impl Language {
    pub(crate) fn of_path(path: &str) -> Option<Language> {
        let name = path.rsplit('/').next().unwrap_or(path);
        let extension = name.rsplit_once('.').map(|(_, extension)| extension)?;
        match extension {
            "rs" => Some(Language::Rust),
            "wit" => Some(Language::Wit),
            "js" | "mjs" | "cjs" | "ts" | "mts" | "cts" => Some(Language::Script),
            "toml" => Some(Language::Toml),
            "yml" | "yaml" => Some(Language::Yaml),
            "sh" | "bash" | "ps1" => Some(Language::Shell),
            _ => None,
        }
    }
}

#[derive(Default)]
pub(crate) struct Scan {
    pub(crate) comment_lines: BTreeMap<usize, bool>,
    pub(crate) code_lines: Vec<String>,
}

impl Scan {
    pub(crate) fn is_comment(&self, line: usize) -> bool {
        self.comment_lines.contains_key(&line)
    }

    pub(crate) fn is_doc_comment(&self, line: usize) -> bool {
        self.comment_lines.get(&line).copied().unwrap_or(false)
    }
}

pub(crate) fn scan(language: Language, text: &str) -> Scan {
    match language {
        Language::Rust | Language::Wit | Language::Script => {
            CLikeScanner::new(language, text).run()
        }
        Language::Toml | Language::Yaml | Language::Shell => scan_hash(language, text),
    }
}

struct Builder {
    scan: Scan,
    current: String,
}

impl Builder {
    fn new() -> Builder {
        Builder {
            scan: Scan::default(),
            current: String::new(),
        }
    }

    fn line_number(&self) -> usize {
        self.scan.code_lines.len() + 1
    }

    fn code(&mut self, character: char) {
        if character == '\n' {
            self.scan.code_lines.push(std::mem::take(&mut self.current));
        } else {
            self.current.push(character);
        }
    }

    fn hidden(&mut self, character: char) {
        if character == '\n' {
            self.code('\n');
        } else {
            self.current.push(' ');
        }
    }

    fn comment(&mut self, character: char, doc: bool) {
        let line = self.line_number();
        let entry = self.scan.comment_lines.entry(line).or_insert(doc);
        *entry = *entry || doc;
        self.hidden(character);
    }

    fn finish(mut self) -> Scan {
        if !self.current.is_empty() {
            self.scan.code_lines.push(std::mem::take(&mut self.current));
        }
        self.scan
    }
}

struct CLikeScanner {
    language: Language,
    chars: Vec<char>,
    index: usize,
    out: Builder,
}

impl CLikeScanner {
    fn new(language: Language, text: &str) -> CLikeScanner {
        CLikeScanner {
            language,
            chars: text.chars().collect(),
            index: 0,
            out: Builder::new(),
        }
    }

    fn at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.index + offset).copied()
    }

    fn run(mut self) -> Scan {
        if self.language == Language::Script && self.at(0) == Some('#') && self.at(1) == Some('!') {
            self.line_comment(false);
        }
        while let Some(character) = self.at(0) {
            match (character, self.at(1)) {
                ('/', Some('/')) => {
                    let doc = self.line_doc();
                    self.line_comment(doc);
                }
                ('/', Some('*')) => self.block_comment(),
                ('\'', _) if self.language == Language::Rust => self.rust_quote(),
                ('"', _) | ('\'', _) | ('`', _) if self.is_quote(character) => {
                    self.string(character)
                }
                _ if self.language == Language::Rust && self.raw_string_hashes().is_some() => {
                    self.raw_string();
                }
                _ => {
                    self.out.code(character);
                    self.index += 1;
                }
            }
        }
        self.out.finish()
    }

    fn is_quote(&self, character: char) -> bool {
        match self.language {
            Language::Script => matches!(character, '"' | '\'' | '`'),
            _ => character == '"',
        }
    }

    fn line_doc(&self) -> bool {
        let third = self.at(2);
        match self.language {
            Language::Rust => (third == Some('/') && self.at(3) != Some('/')) || third == Some('!'),
            Language::Wit => third == Some('/') && self.at(3) != Some('/'),
            _ => false,
        }
    }

    fn line_comment(&mut self, doc: bool) {
        while let Some(character) = self.at(0) {
            if character == '\n' {
                break;
            }
            self.out.comment(character, doc);
            self.index += 1;
        }
    }

    fn block_comment(&mut self) {
        let third = self.at(2);
        let doc = match self.language {
            Language::Rust => (third == Some('*') && self.at(3) != Some('/')) || third == Some('!'),
            Language::Wit => third == Some('*') && self.at(3) != Some('/'),
            _ => false,
        };
        let nested = self.language == Language::Rust;
        let mut depth = 0usize;
        while let Some(character) = self.at(0) {
            if character == '/' && self.at(1) == Some('*') && (nested || depth == 0) {
                depth += 1;
                self.out.comment('/', doc);
                self.out.comment('*', doc);
                self.index += 2;
            } else if character == '*' && self.at(1) == Some('/') {
                depth -= 1;
                self.out.comment('*', doc);
                self.out.comment('/', doc);
                self.index += 2;
                if depth == 0 {
                    return;
                }
            } else {
                self.out.comment(character, doc);
                self.index += 1;
            }
        }
    }

    fn string(&mut self, quote: char) {
        self.out.code(quote);
        self.index += 1;
        while let Some(character) = self.at(0) {
            if character == '\\' {
                self.out.hidden(character);
                if let Some(escaped) = self.at(1) {
                    self.out.hidden(escaped);
                }
                self.index += 2;
            } else if character == quote {
                self.out.code(quote);
                self.index += 1;
                return;
            } else {
                self.out.hidden(character);
                self.index += 1;
            }
        }
    }

    fn rust_quote(&mut self) {
        let is_char_literal = self.at(1) == Some('\\') || self.at(2) == Some('\'');
        if !is_char_literal {
            self.out.code('\'');
            self.index += 1;
            return;
        }
        self.out.code('\'');
        self.index += 1;
        while let Some(character) = self.at(0) {
            if character == '\\' {
                self.out.hidden(character);
                if let Some(escaped) = self.at(1) {
                    self.out.hidden(escaped);
                }
                self.index += 2;
            } else if character == '\'' {
                self.out.code('\'');
                self.index += 1;
                return;
            } else {
                self.out.hidden(character);
                self.index += 1;
            }
        }
    }

    fn raw_string_hashes(&self) -> Option<(usize, usize)> {
        let previous = self
            .index
            .checked_sub(1)
            .and_then(|index| self.chars.get(index));
        if previous.is_some_and(|character| character.is_alphanumeric() || *character == '_') {
            return None;
        }
        let mut offset = 0;
        if matches!(self.at(0), Some('b') | Some('c')) {
            offset += 1;
        }
        if self.at(offset) != Some('r') {
            return None;
        }
        offset += 1;
        let mut hashes = 0;
        while self.at(offset) == Some('#') {
            hashes += 1;
            offset += 1;
        }
        (self.at(offset) == Some('"')).then_some((offset + 1, hashes))
    }

    fn raw_string(&mut self) {
        let Some((prefix_length, hashes)) = self.raw_string_hashes() else {
            return;
        };
        for _ in 0..prefix_length {
            if let Some(character) = self.at(0) {
                self.out.code(character);
            }
            self.index += 1;
        }
        while let Some(character) = self.at(0) {
            let closes =
                character == '"' && (1..=hashes).all(|offset| self.at(offset) == Some('#'));
            if closes {
                for _ in 0..=hashes {
                    if let Some(closing) = self.at(0) {
                        self.out.code(closing);
                    }
                    self.index += 1;
                }
                return;
            }
            self.out.hidden(character);
            self.index += 1;
        }
    }
}

fn scan_hash(language: Language, text: &str) -> Scan {
    let mut out = Builder::new();
    let mut open_quote: Option<&str> = None;
    for line in text.split_inclusive('\n') {
        let chars: Vec<char> = line.chars().collect();
        let mut position = 0;
        while position < chars.len() {
            let character = chars[position];
            let rest: String = chars[position..].iter().take(3).collect();
            if let Some(quote) = open_quote {
                if language == Language::Toml && character == '\\' && quote.starts_with('"') {
                    out.hidden(character);
                    if let Some(next) = chars.get(position + 1) {
                        out.hidden(*next);
                    }
                    position += 2;
                    continue;
                }
                if rest.starts_with(quote) {
                    for quote_char in quote.chars() {
                        out.code(quote_char);
                    }
                    position += quote.chars().count();
                    open_quote = None;
                } else {
                    out.hidden(character);
                    position += 1;
                }
                continue;
            }
            if character == '#' && starts_comment(language, &chars, position) {
                for comment_char in &chars[position..] {
                    out.comment(*comment_char, false);
                }
                break;
            }
            let quote = match (language, character) {
                (Language::Toml, '"') if rest == "\"\"\"" => Some("\"\"\""),
                (Language::Toml, '\'') if rest == "'''" => Some("'''"),
                (_, '"') => Some("\""),
                (_, '\'') if language == Language::Toml || quote_can_open(&chars, position) => {
                    Some("'")
                }
                _ => None,
            };
            if let Some(quote) = quote {
                for quote_char in quote.chars() {
                    out.code(quote_char);
                }
                position += quote.chars().count();
                open_quote = Some(quote);
                continue;
            }
            out.code(character);
            position += 1;
        }
        if language == Language::Yaml && open_quote.is_some() {
            open_quote = None;
        }
    }
    out.finish()
}

fn starts_comment(language: Language, chars: &[char], position: usize) -> bool {
    match language {
        Language::Toml => true,
        _ => {
            position == 0
                || chars.get(position - 1).is_some_and(|previous| {
                    previous.is_whitespace() || matches!(previous, ';' | '|' | '&' | '(')
                })
        }
    }
}

fn quote_can_open(chars: &[char], position: usize) -> bool {
    position == 0
        || chars
            .get(position - 1)
            .is_some_and(|previous| !previous.is_alphanumeric())
}

#[cfg(test)]
mod tests;
