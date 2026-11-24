use codeint_model::language::WorkspaceError;

pub(crate) const GO_MOD_FILENAME: &str = "go.mod";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RequireEntry {
    pub(crate) module_path: String,
    pub(crate) indirect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReplaceEntry {
    pub(crate) old_path: String,
    pub(crate) new_local_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoMod {
    pub(crate) module_path: String,
    pub(crate) requires: Vec<RequireEntry>,
    pub(crate) replaces: Vec<ReplaceEntry>,
    pub(crate) tools: Vec<String>,
}

struct GoModLine {
    number: usize,
    tokens: Vec<String>,
    trailing_comment: Option<String>,
}

pub(crate) fn is_go_mod_path(path: &str) -> bool {
    let matches_name = path == GO_MOD_FILENAME || path.ends_with("/go.mod");
    let ignored_dir = path
        .split('/')
        .any(|segment| matches!(segment, "vendor" | "testdata"));
    matches_name && !ignored_dir
}

pub(crate) fn manifest_dir(path: &str) -> &str {
    path.strip_suffix("/go.mod").unwrap_or("")
}

pub(crate) fn parse(path: &str, text: &str) -> Result<GoMod, WorkspaceError> {
    let src = text.strip_prefix('\u{feff}').unwrap_or(text);
    let lines = tokenize_lines(path, src)?;
    let mut parser = GoModParser::new(path);
    parser.parse_lines(&lines)?;
    parser.finish()
}

struct GoModParser<'a> {
    path: &'a str,
    module_path: Option<String>,
    requires: Vec<RequireEntry>,
    replaces: Vec<ReplaceEntry>,
    tools: Vec<String>,
    active_group: Option<String>,
    group_open_line: usize,
}

impl<'a> GoModParser<'a> {
    fn new(path: &'a str) -> Self {
        Self {
            path,
            module_path: None,
            requires: Vec::new(),
            replaces: Vec::new(),
            tools: Vec::new(),
            active_group: None,
            group_open_line: 0,
        }
    }

    fn err(&self, message: impl Into<String>) -> WorkspaceError {
        WorkspaceError {
            path: self.path.to_owned(),
            message: message.into(),
        }
    }

    fn parse_lines(&mut self, lines: &[GoModLine]) -> Result<(), WorkspaceError> {
        for line in lines {
            if line.tokens.is_empty() {
                continue;
            }
            if self.active_group.is_some() {
                self.parse_group_line(line)?;
            } else {
                self.parse_top_level_line(line)?;
            }
        }
        if let Some(verb) = &self.active_group {
            return Err(self.err(format!(
                "unclosed {verb} block opened at line {}",
                self.group_open_line
            )));
        }
        Ok(())
    }

    fn parse_top_level_line(&mut self, line: &GoModLine) -> Result<(), WorkspaceError> {
        let verb = line.tokens[0].as_str();
        if !is_known_directive(verb) {
            return Err(self.err(format!(
                "unknown directive {verb:?} at line {}",
                line.number
            )));
        }
        let rest = &line.tokens[1..];
        if rest.first().is_some_and(|tok| tok == "(") {
            return self.open_group(verb, rest, line);
        }
        self.apply_directive(verb, rest, line.trailing_comment.as_deref(), line.number)
    }

    fn open_group(
        &mut self,
        verb: &str,
        rest: &[String],
        line: &GoModLine,
    ) -> Result<(), WorkspaceError> {
        if verb == "module" {
            return Err(self.err(format!(
                "module directive cannot be grouped at line {}",
                line.number
            )));
        }
        if rest.len() == 2 && rest[1] == ")" {
            return Ok(());
        }
        if rest.len() > 1 {
            if rest.last().is_some_and(|tok| tok == ")") {
                return self.apply_directive(
                    verb,
                    &rest[1..rest.len() - 1],
                    line.trailing_comment.as_deref(),
                    line.number,
                );
            }
            return Err(self.err(format!(
                "unexpected tokens after '(' at line {}",
                line.number
            )));
        }
        self.active_group = Some(verb.to_owned());
        self.group_open_line = line.number;
        Ok(())
    }

    fn parse_group_line(&mut self, line: &GoModLine) -> Result<(), WorkspaceError> {
        if line.tokens.len() == 1 && line.tokens[0] == ")" {
            self.active_group = None;
            return Ok(());
        }
        let closes_on_line = line.tokens.last().is_some_and(|tok| tok == ")");
        let body = if closes_on_line {
            &line.tokens[..line.tokens.len() - 1]
        } else {
            &line.tokens[..]
        };
        let verb = self.active_group.clone().unwrap_or_default();
        if !body.is_empty() {
            self.apply_directive(&verb, body, line.trailing_comment.as_deref(), line.number)?;
        }
        if closes_on_line {
            self.active_group = None;
        }
        Ok(())
    }

    fn apply_directive(
        &mut self,
        verb: &str,
        args: &[String],
        comment: Option<&str>,
        line_num: usize,
    ) -> Result<(), WorkspaceError> {
        match verb {
            "module" => self.apply_module(args, line_num),
            "require" => self.apply_require(args, comment, line_num),
            "replace" => self.apply_replace(args, line_num),
            "tool" => self.apply_tool(args, line_num),
            _ => Ok(()),
        }
    }

    fn apply_module(&mut self, args: &[String], line_num: usize) -> Result<(), WorkspaceError> {
        if self.module_path.is_some() {
            return Err(self.err(format!("duplicate module directive at line {line_num}")));
        }
        if args.len() != 1 || args[0].is_empty() {
            return Err(self.err(format!(
                "module directive expects one module path at line {line_num}"
            )));
        }
        self.module_path = Some(args[0].clone());
        Ok(())
    }

    fn apply_require(
        &mut self,
        args: &[String],
        comment: Option<&str>,
        line_num: usize,
    ) -> Result<(), WorkspaceError> {
        if args.len() != 2 || args[0].is_empty() || args[1].is_empty() {
            return Err(self.err(format!(
                "require directive expects module path and version at line {line_num}"
            )));
        }
        self.requires.push(RequireEntry {
            module_path: args[0].clone(),
            indirect: is_indirect_comment(comment),
        });
        Ok(())
    }

    fn apply_replace(&mut self, args: &[String], line_num: usize) -> Result<(), WorkspaceError> {
        let Some(arrow) = args.iter().position(|tok| tok == "=>") else {
            return Err(self.err(format!("replace directive missing '=>' at line {line_num}")));
        };
        let lhs = &args[..arrow];
        let rhs = &args[arrow + 1..];
        if !(1..=2).contains(&lhs.len()) || !(1..=2).contains(&rhs.len()) {
            return Err(self.err(format!("malformed replace directive at line {line_num}")));
        }
        let new_local_path = (rhs.len() == 1).then(|| rhs[0].clone());
        self.replaces.push(ReplaceEntry {
            old_path: lhs[0].clone(),
            new_local_path,
        });
        Ok(())
    }

    fn apply_tool(&mut self, args: &[String], line_num: usize) -> Result<(), WorkspaceError> {
        if args.len() != 1 || args[0].is_empty() {
            return Err(self.err(format!(
                "tool directive expects one package path at line {line_num}"
            )));
        }
        self.tools.push(args[0].clone());
        Ok(())
    }

    fn finish(self) -> Result<GoMod, WorkspaceError> {
        let Some(module_path) = self.module_path else {
            return Err(self.err("missing module directive"));
        };
        Ok(GoMod {
            module_path,
            requires: self.requires,
            replaces: self.replaces,
            tools: self.tools,
        })
    }
}

fn is_known_directive(verb: &str) -> bool {
    matches!(
        verb,
        "module"
            | "go"
            | "toolchain"
            | "require"
            | "exclude"
            | "replace"
            | "retract"
            | "godebug"
            | "tool"
            | "ignore"
    )
}

fn is_indirect_comment(comment: Option<&str>) -> bool {
    let Some(raw) = comment else {
        return false;
    };
    let trimmed = raw.trim();
    trimmed == "indirect"
        || trimmed.starts_with("indirect ")
        || trimmed.starts_with("indirect\t")
        || trimmed.starts_with("indirect;")
}

fn tokenize_lines(path: &str, src: &str) -> Result<Vec<GoModLine>, WorkspaceError> {
    src.lines()
        .enumerate()
        .map(|(idx, raw_line)| tokenize_line(path, raw_line, idx + 1))
        .collect()
}

fn tokenize_line(path: &str, raw: &str, line_num: usize) -> Result<GoModLine, WorkspaceError> {
    let bytes = raw.as_bytes();
    let mut tokens = Vec::new();
    let mut trailing_comment = None;
    let mut idx = 0usize;
    while idx < bytes.len() {
        idx = skip_ascii_spaces(bytes, idx);
        if idx >= bytes.len() {
            break;
        }
        if bytes[idx..].starts_with(b"//") {
            trailing_comment = Some(raw[idx + 2..].to_owned());
            break;
        }
        let (tok, next) = read_line_token(path, raw, idx, line_num)?;
        tokens.push(tok);
        idx = next;
    }
    Ok(GoModLine {
        number: line_num,
        tokens,
        trailing_comment,
    })
}

fn skip_ascii_spaces(bytes: &[u8], mut idx: usize) -> usize {
    while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }
    idx
}

fn read_line_token(
    path: &str,
    raw: &str,
    idx: usize,
    line_num: usize,
) -> Result<(String, usize), WorkspaceError> {
    let b = raw.as_bytes()[idx];
    if b == b'(' || b == b')' {
        return Ok(((b as char).to_string(), idx + 1));
    }
    if b == b'"' || b == b'`' {
        return read_go_mod_string(path, raw, idx, line_num);
    }
    Ok(read_bare_token(raw, idx))
}

fn read_bare_token(raw: &str, start: usize) -> (String, usize) {
    let bytes = raw.as_bytes();
    let mut idx = start;
    while idx < bytes.len() {
        let ch = bytes[idx];
        if ch.is_ascii_whitespace()
            || matches!(ch, b'(' | b')' | b'"' | b'`')
            || bytes[idx..].starts_with(b"//")
        {
            break;
        }
        idx += 1;
    }
    (raw[start..idx].to_owned(), idx)
}

fn read_go_mod_string(
    path: &str,
    raw: &str,
    start: usize,
    line_num: usize,
) -> Result<(String, usize), WorkspaceError> {
    let bytes = raw.as_bytes();
    let quote = bytes[start];
    let mut idx = start + 1;
    let mut out = String::new();
    while idx < bytes.len() {
        let b = bytes[idx];
        if quote == b'"' && b == b'\\' {
            let Some(escaped) = raw[idx + 1..].chars().next() else {
                break;
            };
            out.push(escaped);
            idx += 1 + escaped.len_utf8();
            continue;
        }
        if b == quote {
            return Ok((out, idx + 1));
        }
        let ch = raw[idx..].chars().next().unwrap_or('\0');
        out.push(ch);
        idx += ch.len_utf8();
    }
    Err(WorkspaceError {
        path: path.to_owned(),
        message: format!("unterminated string at line {line_num}"),
    })
}
