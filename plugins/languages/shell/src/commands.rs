use crate::lexer::{ShellToken, ShellTokenKind};

#[derive(Clone, Debug)]
pub(crate) struct SimpleCommand {
    pub(crate) assignments: Vec<usize>,
    pub(crate) cmd_idx: Option<usize>,
    pub(crate) arg_indices: Vec<usize>,
    pub(crate) redir_targets: Vec<usize>,
    pub(crate) has_redirection: bool,
    pub(crate) in_condition: bool,
    pub(crate) preceded_by_pipe: bool,
    pub(crate) followed_by_op: Option<String>,
    pub(crate) pipeline_id: usize,
}

pub(crate) struct ParsedCommands {
    pub(crate) commands: Vec<SimpleCommand>,
    pub(crate) pipelines: Vec<Vec<usize>>,
    pub(crate) double_bracket_indices: Vec<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CasePhase {
    Header,
    Pattern,
    Body,
}

struct CmdParser<'a> {
    tokens: &'a [ShellToken],
    commands: Vec<SimpleCommand>,
    pipelines: Vec<Vec<usize>>,
    double_bracket_indices: Vec<usize>,
    in_condition: bool,
    in_double_bracket: bool,
    case_stack: Vec<CasePhase>,
    current: Option<SimpleCommand>,
    expect_redir_target: bool,
    preceded_by_pipe: bool,
    current_pipeline: usize,
    skip_for_header: bool,
}

pub(crate) fn parse_commands(tokens: &[ShellToken]) -> ParsedCommands {
    let mut parser = CmdParser::new(tokens);
    parser.run();
    ParsedCommands {
        commands: parser.commands,
        pipelines: parser.pipelines,
        double_bracket_indices: parser.double_bracket_indices,
    }
}

pub(crate) fn is_assignment_word(text: &str) -> bool {
    let Some((lhs, _)) = text.split_once('=') else {
        return false;
    };
    let lhs = lhs.strip_suffix('+').unwrap_or(lhs);
    let lhs = lhs.split_once('[').map_or(lhs, |(name, _)| name);
    is_shell_ident(lhs)
}

pub(crate) fn is_shell_ident(name: &str) -> bool {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return false;
    }
    bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

impl<'a> CmdParser<'a> {
    fn new(tokens: &'a [ShellToken]) -> Self {
        Self {
            tokens,
            commands: Vec::new(),
            pipelines: vec![Vec::new()],
            double_bracket_indices: Vec::new(),
            in_condition: false,
            in_double_bracket: false,
            case_stack: Vec::new(),
            current: None,
            expect_redir_target: false,
            preceded_by_pipe: false,
            current_pipeline: 0,
            skip_for_header: false,
        }
    }

    fn run(&mut self) {
        let mut idx = 0usize;
        while idx < self.tokens.len() {
            idx = self.step(idx);
        }
        self.finish_command(None);
    }

    fn step(&mut self, idx: usize) -> usize {
        let tok = &self.tokens[idx];
        if self.in_double_bracket {
            self.double_bracket_indices.push(idx);
            if tok.is_word("]]") {
                self.in_double_bracket = false;
            }
            return idx + 1;
        }
        if tok.is_newline() {
            self.finish_command(Some("\n".to_owned()));
            self.reset_pipeline();
            self.skip_for_header = false;
            return idx + 1;
        }
        if tok.kind == ShellTokenKind::Operator {
            return self.handle_operator(idx, &tok.text);
        }
        if tok.kind == ShellTokenKind::ArithCmd {
            self.finish_command(None);
            return idx + 1;
        }
        self.handle_word(idx)
    }

    fn handle_operator(&mut self, idx: usize, op: &str) -> usize {
        if is_redirection_op(op) {
            self.ensure_current().has_redirection = true;
            self.expect_redir_target = true;
            return idx + 1;
        }
        if self.case_stack.last() == Some(&CasePhase::Pattern) {
            if op == ")"
                && let Some(top) = self.case_stack.last_mut()
            {
                *top = CasePhase::Body;
            }
            return idx + 1;
        }
        if matches!(op, "|" | "|&") {
            self.finish_command(Some(op.to_owned()));
            self.preceded_by_pipe = true;
            return idx + 1;
        }
        self.finish_command(Some(op.to_owned()));
        self.reset_pipeline();
        self.skip_for_header = false;
        if matches!(op, ";;" | ";&" | ";;&")
            && let Some(top) = self.case_stack.last_mut()
        {
            *top = CasePhase::Pattern;
        }
        idx + 1
    }

    fn handle_word(&mut self, idx: usize) -> usize {
        let tok = &self.tokens[idx];
        if self.expect_redir_target {
            self.expect_redir_target = false;
            self.ensure_current().redir_targets.push(idx);
            return idx + 1;
        }
        if self.handle_case_phase(tok) {
            return idx + 1;
        }
        if self.skip_for_header {
            return self.handle_for_header_word(idx, tok);
        }
        if self.current.is_none() && self.handle_command_start_keyword(idx, tok) {
            return idx + 1;
        }
        self.push_word_to_current(idx, tok);
        idx + 1
    }

    fn handle_case_phase(&mut self, tok: &ShellToken) -> bool {
        match self.case_stack.last().copied() {
            Some(CasePhase::Header) => {
                if tok.is_word("in")
                    && let Some(top) = self.case_stack.last_mut()
                {
                    *top = CasePhase::Pattern;
                }
                true
            }
            Some(CasePhase::Pattern) => {
                if tok.is_word("esac") {
                    self.case_stack.pop();
                }
                true
            }
            _ => false,
        }
    }

    fn handle_for_header_word(&mut self, idx: usize, tok: &ShellToken) -> usize {
        if tok.is_word("do") {
            self.skip_for_header = false;
            return idx + 1;
        }
        if tok.is_word("in") {
            self.skip_for_header = false;
            let cmd = self.ensure_current();
            cmd.cmd_idx = Some(idx);
            return idx + 1;
        }
        idx + 1
    }

    fn handle_command_start_keyword(&mut self, idx: usize, tok: &ShellToken) -> bool {
        if tok.has_quotes {
            return false;
        }
        match tok.text.as_str() {
            "if" | "elif" | "while" | "until" => {
                self.in_condition = true;
                true
            }
            "then" | "do" => {
                self.in_condition = false;
                true
            }
            "else" | "fi" | "done" | "{" | "}" | "!" | "time" | "coproc" => true,
            "for" | "select" => {
                self.skip_for_header = true;
                true
            }
            "case" => {
                self.case_stack.push(CasePhase::Header);
                true
            }
            "esac" => {
                self.case_stack.pop();
                true
            }
            "[[" => {
                self.in_double_bracket = true;
                self.double_bracket_indices.push(idx);
                true
            }
            "function" => true,
            _ => false,
        }
    }

    fn push_word_to_current(&mut self, idx: usize, tok: &ShellToken) {
        if is_io_number_prefix(self.tokens, idx) {
            return;
        }
        let cmd = self.ensure_current();
        if cmd.cmd_idx.is_none() && is_assignment_word(&tok.text) {
            cmd.assignments.push(idx);
        } else if cmd.cmd_idx.is_none() {
            cmd.cmd_idx = Some(idx);
        } else {
            cmd.arg_indices.push(idx);
        }
    }

    fn ensure_current(&mut self) -> &mut SimpleCommand {
        let in_condition = self.in_condition;
        let preceded_by_pipe = self.preceded_by_pipe;
        let pipeline_id = self.current_pipeline;
        self.current.get_or_insert_with(|| SimpleCommand {
            assignments: Vec::new(),
            cmd_idx: None,
            arg_indices: Vec::new(),
            redir_targets: Vec::new(),
            has_redirection: false,
            in_condition,
            preceded_by_pipe,
            followed_by_op: None,
            pipeline_id,
        })
    }

    fn finish_command(&mut self, followed_by_op: Option<String>) {
        let Some(mut cmd) = self.current.take() else {
            return;
        };
        cmd.followed_by_op = followed_by_op;
        let cmd_id = self.commands.len();
        let pid = cmd.pipeline_id;
        self.commands.push(cmd);
        if let Some(pipe) = self.pipelines.get_mut(pid) {
            pipe.push(cmd_id);
        }
    }

    fn reset_pipeline(&mut self) {
        self.preceded_by_pipe = false;
        if self.pipelines.last().is_some_and(|pipe| !pipe.is_empty()) {
            self.current_pipeline = self.pipelines.len();
            self.pipelines.push(Vec::new());
        }
    }
}

fn is_redirection_op(op: &str) -> bool {
    matches!(
        op,
        "<" | ">" | ">>" | "<<" | "<<-" | "<<<" | "<&" | ">&" | "<>" | ">|" | "&>" | "&>>"
    )
}

fn is_io_number_prefix(tokens: &[ShellToken], idx: usize) -> bool {
    let tok = &tokens[idx];
    !tok.has_quotes
        && !tok.text.is_empty()
        && tok.text.bytes().all(|b| b.is_ascii_digit())
        && tokens.get(idx + 1).is_some_and(|next| {
            next.kind == ShellTokenKind::Operator && is_redirection_op(&next.text)
        })
}
