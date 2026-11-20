use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Language {
    Rust,
    Wit,
    Script,
    Toml,
    Yaml,
    Shell,
    PowerShell,
}

impl Language {
    pub(crate) fn of_path(path: &str) -> Option<Language> {
        let name = path.rsplit('/').next().unwrap_or(path);
        let extension = name.rsplit_once('.').map(|(_, extension)| extension)?;
        match extension {
            "rs" => Some(Language::Rust),
            "wit" => Some(Language::Wit),
            "js" | "mjs" | "cjs" | "jsx" | "ts" | "mts" | "cts" | "tsx" => Some(Language::Script),
            "toml" => Some(Language::Toml),
            "yml" | "yaml" => Some(Language::Yaml),
            "sh" | "bash" => Some(Language::Shell),
            "ps1" | "psm1" => Some(Language::PowerShell),
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
        Language::Toml | Language::Yaml | Language::Shell | Language::PowerShell => {
            scan_hash(language, text)
        }
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
        *entry = *entry && doc;
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
            } else if character == '\n' && self.string_ends_at_line_end(quote) {
                return;
            } else {
                self.out.hidden(character);
                self.index += 1;
            }
        }
    }

    fn string_ends_at_line_end(&self, quote: char) -> bool {
        self.language == Language::Script && quote != '`'
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
    let mut in_block_comment = false;
    let mut block_scalar_parent: Option<usize> = None;
    for line in text.split_inclusive('\n') {
        let chars: Vec<char> = line.chars().collect();
        if language == Language::Yaml && open_quote.is_none() {
            if let Some(parent) = block_scalar_parent {
                if inside_block_scalar(line, parent) {
                    block_scalar_line(&mut out, &chars);
                    continue;
                }
                block_scalar_parent = None;
            }
            if opens_block_scalar(line) {
                block_scalar_parent = Some(indentation(line));
            }
        }
        let mut position = 0;
        while position < chars.len() {
            let character = chars[position];
            let rest: String = chars[position..].iter().take(3).collect();
            if in_block_comment {
                if rest.starts_with("#>") {
                    out.comment('#', false);
                    out.comment('>', false);
                    position += 2;
                    in_block_comment = false;
                } else {
                    out.comment(character, false);
                    position += 1;
                }
                continue;
            }
            if let Some(quote) = open_quote {
                if let Some(escape) = escape_character(language, quote)
                    && character == escape
                {
                    out.hidden(character);
                    if let Some(next) = chars.get(position + 1) {
                        out.hidden(*next);
                    }
                    position += 2;
                    continue;
                }
                if rest.starts_with(quote) {
                    let length = closing_length(language, quote, &chars[position..]);
                    for closing in &chars[position..position + length] {
                        out.code(*closing);
                    }
                    position += length;
                    open_quote = None;
                } else {
                    out.hidden(character);
                    position += 1;
                }
                continue;
            }
            if language == Language::PowerShell && rest.starts_with("<#") {
                out.comment('<', false);
                out.comment('#', false);
                position += 2;
                in_block_comment = true;
                continue;
            }
            if character == '#' && starts_comment(language, &chars, position) {
                for comment_char in &chars[position..] {
                    out.comment(*comment_char, false);
                }
                break;
            }
            if let Some(escape) = escape_character(language, "")
                && character == escape
            {
                out.code(character);
                if let Some(next) = chars.get(position + 1).filter(|next| **next != '\n') {
                    out.code(*next);
                    position += 1;
                }
                position += 1;
                continue;
            }
            if let Some(quote) = opening_quote(language, &chars, position, &rest) {
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
        if matches!((language, open_quote), (Language::Toml, Some("\"" | "'"))) {
            open_quote = None;
        }
    }
    out.finish()
}

fn indentation(line: &str) -> usize {
    line.chars()
        .take_while(|character| *character == ' ')
        .count()
}

fn opens_block_scalar(line: &str) -> bool {
    let code = match line.find(" #") {
        Some(position) => &line[..position],
        None => line,
    };
    let trimmed = code.trim_end();
    let indicator = trimmed.trim_end_matches(|character: char| {
        character.is_ascii_digit() || character == '+' || character == '-'
    });
    (indicator.ends_with(": |") || indicator.ends_with(": >") || indicator.ends_with("- |"))
        || indicator == "|"
        || indicator == ">"
}

fn inside_block_scalar(line: &str, parent: usize) -> bool {
    line.trim().is_empty() || indentation(line) > parent
}

fn block_scalar_line(out: &mut Builder, chars: &[char]) {
    let is_comment = chars
        .iter()
        .find(|character| !character.is_whitespace())
        .is_some_and(|first| *first == '#');
    for character in chars {
        if is_comment {
            out.comment(*character, false);
        } else {
            out.code(*character);
        }
    }
}

fn escape_character(language: Language, quote: &str) -> Option<char> {
    match (language, quote) {
        (Language::Toml, "\"" | "\"\"\"") => Some('\\'),
        (Language::Yaml, "\"") => Some('\\'),
        (Language::Shell, "" | "\"") => Some('\\'),
        (Language::PowerShell, "" | "\"") => Some('`'),
        _ => None,
    }
}

fn closing_length(language: Language, quote: &str, rest: &[char]) -> usize {
    let quote_length = quote.chars().count();
    if language != Language::Toml || quote_length != 3 {
        return quote_length;
    }
    let delimiter = quote.chars().next().unwrap_or('"');
    let run = rest
        .iter()
        .take_while(|character| **character == delimiter)
        .count();
    run.min(5)
}

fn opening_quote(
    language: Language,
    chars: &[char],
    position: usize,
    rest: &str,
) -> Option<&'static str> {
    let character = chars[position];
    match language {
        Language::Toml => match character {
            '"' if rest == "\"\"\"" => Some("\"\"\""),
            '\'' if rest == "'''" => Some("'''"),
            '"' => Some("\""),
            '\'' => Some("'"),
            _ => None,
        },
        Language::Yaml => match character {
            '"' if yaml_scalar_starts(chars, position) => Some("\""),
            '\'' if yaml_scalar_starts(chars, position) => Some("'"),
            _ => None,
        },
        _ => match character {
            '"' => Some("\""),
            '\'' if quote_can_open(chars, position) => Some("'"),
            _ => None,
        },
    }
}

fn yaml_scalar_starts(chars: &[char], position: usize) -> bool {
    let before = &chars[..position];
    let previous = before
        .iter()
        .rev()
        .find(|character| !character.is_whitespace());
    match previous {
        None => true,
        Some('[' | '{' | ',') => true,
        Some(':' | '-' | '?') => before.last().is_some_and(|last| last.is_whitespace()),
        Some(_) => false,
    }
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
