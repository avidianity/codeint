use codeint_model::language::Function;
use oxc_ast::ast::Program;

pub(crate) struct CodeLines {
    line_starts: Vec<usize>,
    is_code: Vec<bool>,
}

impl CodeLines {
    pub(crate) fn new(code: &str) -> CodeLines {
        todo!("{code}")
    }

    pub(crate) fn count(&self) -> usize {
        todo!()
    }
}

pub(crate) fn functions(program: &Program<'_>, code_lines: &CodeLines) -> Vec<Function> {
    todo!(
        "{:?} {:?}",
        program.span,
        code_lines.line_starts.len() + code_lines.is_code.len()
    )
}
