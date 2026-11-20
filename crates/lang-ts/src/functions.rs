use codeint_model::language::Function;
use oxc_ast::AstKind;
use oxc_ast::ast::Program;
use oxc_ast_visit::Visit;
use oxc_span::Span;

pub(crate) struct CodeLines {
    line_starts: Vec<usize>,
    code_lines_before: Vec<usize>,
}

impl CodeLines {
    pub(crate) fn new(code: &str) -> CodeLines {
        let line_starts = std::iter::once(0)
            .chain(code.match_indices('\n').map(|(index, _)| index + 1))
            .collect();
        let code_lines_before = std::iter::once(0)
            .chain(code.lines().scan(0, |count, line| {
                *count += usize::from(line.chars().any(is_code_character));
                Some(*count)
            }))
            .collect();
        CodeLines {
            line_starts,
            code_lines_before,
        }
    }

    pub(crate) fn count(&self) -> usize {
        self.code_lines_before.last().copied().unwrap_or(0)
    }

    pub(crate) fn source_line_indexes_in(&self, span: Span) -> impl Iterator<Item = usize> + '_ {
        let first = self.line_index(span.start as usize);
        let last = self.line_index(
            (span.end as usize)
                .saturating_sub(1)
                .max(span.start as usize),
        );
        (first..=last).filter(|&index| self.count_between(index, index) == 1)
    }

    fn line_index(&self, offset: usize) -> usize {
        self.line_starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1)
    }

    fn count_between(&self, first_line_index: usize, last_line_index: usize) -> usize {
        let after_last = (last_line_index + 1).min(self.code_lines_before.len() - 1);
        let first = first_line_index.min(after_last);
        self.code_lines_before[after_last] - self.code_lines_before[first]
    }
}

fn is_code_character(character: char) -> bool {
    !character.is_whitespace() && character != '\u{feff}'
}

pub(crate) fn functions(program: &Program<'_>, code_lines: &CodeLines) -> Vec<Function> {
    let mut collector = FunctionCollector {
        code_lines,
        functions: Vec::new(),
        open: Vec::new(),
    };
    collector.visit_program(program);
    collector.functions
}

struct FunctionCollector<'lines> {
    code_lines: &'lines CodeLines,
    functions: Vec<Function>,
    open: Vec<OpenFunction>,
}

struct OpenFunction {
    index: usize,
    last_line_index_given_to_nested: Option<usize>,
}

impl FunctionCollector<'_> {
    fn give_lines_to_nested(&mut self, first_line_index: usize, last_line_index: usize) {
        let Some(parent) = self.open.last_mut() else {
            return;
        };
        let first_line_index_not_given = match parent.last_line_index_given_to_nested {
            Some(given) => first_line_index.max(given + 1),
            None => first_line_index,
        };
        if first_line_index_not_given > last_line_index {
            return;
        }
        parent.last_line_index_given_to_nested = Some(last_line_index);
        let given_lines = saturating_u32(
            self.code_lines
                .count_between(first_line_index_not_given, last_line_index),
        );
        let function = &mut self.functions[parent.index];
        function.source_lines = function.source_lines.saturating_sub(given_lines);
    }

    fn open_function(&mut self, span: Span) {
        let first_line_index = self.code_lines.line_index(span.start as usize);
        let last_line_index = self
            .code_lines
            .line_index((span.end as usize).saturating_sub(1));
        self.give_lines_to_nested(first_line_index, last_line_index);
        self.open.push(OpenFunction {
            index: self.functions.len(),
            last_line_index_given_to_nested: None,
        });
        self.functions.push(Function {
            line: saturating_u32(first_line_index + 1),
            decision_points: 0,
            source_lines: saturating_u32(
                self.code_lines
                    .count_between(first_line_index, last_line_index),
            ),
        });
    }

    fn add_decision_point(&mut self) {
        if let Some(innermost) = self.open.last() {
            let function = &mut self.functions[innermost.index];
            function.decision_points = function.decision_points.saturating_add(1);
        }
    }
}

impl<'a> Visit<'a> for FunctionCollector<'_> {
    fn enter_node(&mut self, kind: AstKind<'a>) {
        if let Some(span) = function_with_body(kind) {
            self.open_function(span);
        } else if is_decision_point(kind) {
            self.add_decision_point();
        }
    }

    fn leave_node(&mut self, kind: AstKind<'a>) {
        if function_with_body(kind).is_some() {
            self.open.pop();
        }
    }
}

fn function_with_body(kind: AstKind<'_>) -> Option<Span> {
    match kind {
        AstKind::Function(function) if function.body.is_some() => Some(function.span),
        AstKind::ArrowFunctionExpression(arrow) => Some(arrow.span),
        _ => None,
    }
}

fn is_decision_point(kind: AstKind<'_>) -> bool {
    match kind {
        AstKind::IfStatement(_)
        | AstKind::ForStatement(_)
        | AstKind::ForInStatement(_)
        | AstKind::ForOfStatement(_)
        | AstKind::WhileStatement(_)
        | AstKind::DoWhileStatement(_)
        | AstKind::CatchClause(_)
        | AstKind::ConditionalExpression(_)
        | AstKind::LogicalExpression(_) => true,
        AstKind::SwitchCase(case) => case.test.is_some(),
        AstKind::AssignmentExpression(assignment) => assignment.operator.is_logical(),
        AstKind::StaticMemberExpression(member) => member.optional,
        AstKind::ComputedMemberExpression(member) => member.optional,
        AstKind::PrivateFieldExpression(member) => member.optional,
        AstKind::CallExpression(call) => call.optional,
        AstKind::FormalParameter(parameter) => parameter.initializer.is_some(),
        _ => false,
    }
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
