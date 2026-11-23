use codeint_model::language::Function;
use oxc_ast::AstKind;
use oxc_ast::ast::{
    AssignmentExpression, BindingPattern, BreakStatement, CallExpression, CatchClause,
    ComputedMemberExpression, ConditionalExpression, ContinueStatement, DoWhileStatement,
    Expression, ForInStatement, ForOfStatement, ForStatement, FormalParameter, IfStatement,
    LogicalExpression, LogicalOperator, MethodDefinition, ObjectProperty, PrivateFieldExpression,
    Program, PropertyDefinition, PropertyKey, Statement, StaticMemberExpression, SwitchCase,
    SwitchStatement, VariableDeclarator, WhileStatement,
};
use oxc_ast_visit::{Visit, walk};
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
        inferred_names: Vec::new(),
    };
    collector.visit_program(program);
    collector.functions
}

struct FunctionCollector<'lines> {
    code_lines: &'lines CodeLines,
    functions: Vec<Function>,
    open: Vec<OpenFunction>,
    inferred_names: Vec<(Span, String)>,
}

struct OpenFunction {
    index: usize,
    last_line_index_given_to_nested: Option<usize>,
    nesting: u32,
    name: Option<String>,
    counted_recursion: bool,
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

    fn open_function(&mut self, span: Span, name: Option<String>) {
        let first_line_index = self.code_lines.line_index(span.start as usize);
        let last_line_index = self
            .code_lines
            .line_index((span.end as usize).saturating_sub(1));
        self.give_lines_to_nested(first_line_index, last_line_index);
        self.open.push(OpenFunction {
            index: self.functions.len(),
            last_line_index_given_to_nested: None,
            nesting: 0,
            name: name.clone(),
            counted_recursion: false,
        });
        self.functions.push(Function {
            line: saturating_u32(first_line_index + 1),
            end_line: saturating_u32(last_line_index + 1),
            decision_points: 0,
            source_lines: saturating_u32(
                self.code_lines
                    .count_between(first_line_index, last_line_index),
            ),
            name,
            cognitive_complexity: 0,
        });
    }

    fn add_decision_point(&mut self) {
        if let Some(innermost) = self.open.last() {
            let function = &mut self.functions[innermost.index];
            function.decision_points = function.decision_points.saturating_add(1);
        }
    }

    fn add_cognitive_flat(&mut self, amount: u32) {
        if let Some(innermost) = self.open.last() {
            let function = &mut self.functions[innermost.index];
            function.cognitive_complexity = function.cognitive_complexity.saturating_add(amount);
        }
    }

    fn add_cognitive_nested(&mut self) {
        if let Some(innermost) = self.open.last() {
            let increment = innermost.nesting.saturating_add(1);
            let function = &mut self.functions[innermost.index];
            function.cognitive_complexity = function.cognitive_complexity.saturating_add(increment);
        }
    }

    fn with_nesting(&mut self, visit: impl FnOnce(&mut Self)) {
        if let Some(innermost) = self.open.last_mut() {
            innermost.nesting = innermost.nesting.saturating_add(1);
        }
        visit(self);
        if let Some(innermost) = self.open.last_mut() {
            innermost.nesting = innermost.nesting.saturating_sub(1);
        }
    }

    fn record_direct_call(&mut self, callee: &str) {
        let Some(innermost) = self.open.last_mut() else {
            return;
        };
        if !innermost.counted_recursion && innermost.name.as_deref() == Some(callee) {
            innermost.counted_recursion = true;
            let index = innermost.index;
            let function = &mut self.functions[index];
            function.cognitive_complexity = function.cognitive_complexity.saturating_add(1);
        }
    }

    fn record_inferred_name(&mut self, expression: &Expression<'_>, name: &str) {
        if let Some(span) = unnamed_function_span(expression) {
            self.inferred_names.push((span, name.to_owned()));
        }
    }

    fn take_inferred_name(&mut self, span: Span) -> Option<String> {
        let position = self
            .inferred_names
            .iter()
            .rposition(|(candidate, _)| *candidate == span)?;
        Some(self.inferred_names.remove(position).1)
    }

    fn record_loop_structure(&mut self) {
        self.add_decision_point();
        self.add_cognitive_nested();
    }

    fn visit_for_each_loop<'a>(
        &mut self,
        left: &oxc_ast::ast::ForStatementLeft<'a>,
        right: &Expression<'a>,
        body: &Statement<'a>,
    ) {
        self.record_loop_structure();
        self.visit_for_statement_left(left);
        self.visit_expression(right);
        self.with_nesting(|collector| collector.visit_statement(body));
    }

    fn visit_while_loop<'a>(
        &mut self,
        test: &Expression<'a>,
        body: &Statement<'a>,
        test_first: bool,
    ) {
        self.record_loop_structure();
        if test_first {
            self.visit_expression(test);
            self.with_nesting(|collector| collector.visit_statement(body));
        } else {
            self.with_nesting(|collector| collector.visit_statement(body));
            self.visit_expression(test);
        }
    }

    fn visit_if_chain<'a>(&mut self, statement: &IfStatement<'a>, is_else_if: bool) {
        self.add_decision_point();
        if is_else_if {
            self.add_cognitive_flat(1);
        } else {
            self.add_cognitive_nested();
        }
        self.visit_expression(&statement.test);
        self.with_nesting(|collector| collector.visit_statement(&statement.consequent));
        if let Some(alternate) = &statement.alternate {
            if let Statement::IfStatement(else_if) = alternate {
                self.visit_if_chain(else_if, true);
            } else {
                self.add_cognitive_flat(1);
                self.with_nesting(|collector| collector.visit_statement(alternate));
            }
        }
    }
}

impl<'a> Visit<'a> for FunctionCollector<'_> {
    fn enter_node(&mut self, kind: AstKind<'a>) {
        match kind {
            AstKind::Function(function) if function.body.is_some() => {
                let inferred = self.take_inferred_name(function.span);
                let name = function
                    .id
                    .as_ref()
                    .map(|ident| ident.name.to_string())
                    .or(inferred);
                self.open_function(function.span, name);
            }
            AstKind::ArrowFunctionExpression(arrow) => {
                let name = self.take_inferred_name(arrow.span);
                self.open_function(arrow.span, name);
            }
            _ => {}
        }
    }

    fn leave_node(&mut self, kind: AstKind<'a>) {
        if matches!(
            kind,
            AstKind::Function(function) if function.body.is_some()
        ) || matches!(kind, AstKind::ArrowFunctionExpression(_))
        {
            self.open.pop();
        }
    }

    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        if let BindingPattern::BindingIdentifier(ident) = &it.id
            && let Some(init) = &it.init
        {
            self.record_inferred_name(init, ident.name.as_str());
        }
        walk::walk_variable_declarator(self, it);
    }

    fn visit_method_definition(&mut self, it: &MethodDefinition<'a>) {
        if it.value.body.is_some()
            && let Some(name) = property_key_name(&it.key)
        {
            self.inferred_names.push((it.value.span, name));
        }
        walk::walk_method_definition(self, it);
    }

    fn visit_property_definition(&mut self, it: &PropertyDefinition<'a>) {
        if let Some(value) = &it.value
            && let Some(name) = property_key_name(&it.key)
        {
            self.record_inferred_name(value, &name);
        }
        walk::walk_property_definition(self, it);
    }

    fn visit_object_property(&mut self, it: &ObjectProperty<'a>) {
        if let Some(name) = property_key_name(&it.key) {
            self.record_inferred_name(&it.value, &name);
        }
        walk::walk_object_property(self, it);
    }

    fn visit_if_statement(&mut self, it: &IfStatement<'a>) {
        self.visit_if_chain(it, false);
    }

    fn visit_for_statement(&mut self, it: &ForStatement<'a>) {
        self.record_loop_structure();
        if let Some(init) = &it.init {
            self.visit_for_statement_init(init);
        }
        if let Some(test) = &it.test {
            self.visit_expression(test);
        }
        if let Some(update) = &it.update {
            self.visit_expression(update);
        }
        self.with_nesting(|collector| collector.visit_statement(&it.body));
    }

    fn visit_for_in_statement(&mut self, it: &ForInStatement<'a>) {
        self.visit_for_each_loop(&it.left, &it.right, &it.body);
    }

    fn visit_for_of_statement(&mut self, it: &ForOfStatement<'a>) {
        self.visit_for_each_loop(&it.left, &it.right, &it.body);
    }

    fn visit_while_statement(&mut self, it: &WhileStatement<'a>) {
        self.visit_while_loop(&it.test, &it.body, true);
    }

    fn visit_do_while_statement(&mut self, it: &DoWhileStatement<'a>) {
        self.visit_while_loop(&it.test, &it.body, false);
    }

    fn visit_switch_statement(&mut self, it: &SwitchStatement<'a>) {
        self.add_cognitive_nested();
        self.visit_expression(&it.discriminant);
        self.with_nesting(|collector| collector.visit_switch_cases(&it.cases));
    }

    fn visit_switch_case(&mut self, it: &SwitchCase<'a>) {
        if it.test.is_some() {
            self.add_decision_point();
        }
        walk::walk_switch_case(self, it);
    }

    fn visit_catch_clause(&mut self, it: &CatchClause<'a>) {
        self.add_decision_point();
        self.add_cognitive_nested();
        if let Some(param) = &it.param {
            self.visit_catch_parameter(param);
        }
        self.with_nesting(|collector| collector.visit_block_statement(&it.body));
    }

    fn visit_conditional_expression(&mut self, it: &ConditionalExpression<'a>) {
        self.add_decision_point();
        self.add_cognitive_nested();
        self.visit_expression(&it.test);
        self.with_nesting(|collector| {
            collector.visit_expression(&it.consequent);
            collector.visit_expression(&it.alternate);
        });
    }

    fn visit_logical_expression(&mut self, it: &LogicalExpression<'a>) {
        let mut operators = Vec::new();
        let mut leaves = Vec::new();
        collect_logical_chain(it, &mut operators, &mut leaves);
        for _ in &operators {
            self.add_decision_point();
        }
        self.add_cognitive_flat(count_operator_runs(&operators));
        for leaf in leaves {
            self.visit_expression(leaf);
        }
    }

    fn visit_break_statement(&mut self, it: &BreakStatement<'a>) {
        if it.label.is_some() {
            self.add_cognitive_flat(1);
        }
        walk::walk_break_statement(self, it);
    }

    fn visit_continue_statement(&mut self, it: &ContinueStatement<'a>) {
        if it.label.is_some() {
            self.add_cognitive_flat(1);
        }
        walk::walk_continue_statement(self, it);
    }

    fn visit_assignment_expression(&mut self, it: &AssignmentExpression<'a>) {
        if it.operator.is_logical() {
            self.add_decision_point();
        }
        walk::walk_assignment_expression(self, it);
    }

    fn visit_static_member_expression(&mut self, it: &StaticMemberExpression<'a>) {
        if it.optional {
            self.add_decision_point();
        }
        walk::walk_static_member_expression(self, it);
    }

    fn visit_computed_member_expression(&mut self, it: &ComputedMemberExpression<'a>) {
        if it.optional {
            self.add_decision_point();
        }
        walk::walk_computed_member_expression(self, it);
    }

    fn visit_private_field_expression(&mut self, it: &PrivateFieldExpression<'a>) {
        if it.optional {
            self.add_decision_point();
        }
        walk::walk_private_field_expression(self, it);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        if it.optional {
            self.add_decision_point();
        }
        if let Expression::Identifier(ident) = &it.callee {
            self.record_direct_call(ident.name.as_str());
        }
        walk::walk_call_expression(self, it);
    }

    fn visit_formal_parameter(&mut self, it: &FormalParameter<'a>) {
        if it.initializer.is_some() {
            self.add_decision_point();
        }
        walk::walk_formal_parameter(self, it);
    }
}

fn collect_logical_chain<'a>(
    logical: &'a LogicalExpression<'a>,
    operators: &mut Vec<LogicalOperator>,
    leaves: &mut Vec<&'a Expression<'a>>,
) {
    collect_logical_operand(&logical.left, operators, leaves);
    operators.push(logical.operator);
    collect_logical_operand(&logical.right, operators, leaves);
}

fn collect_logical_operand<'a>(
    expression: &'a Expression<'a>,
    operators: &mut Vec<LogicalOperator>,
    leaves: &mut Vec<&'a Expression<'a>>,
) {
    match expression {
        Expression::ParenthesizedExpression(parenthesized) => {
            collect_logical_operand(&parenthesized.expression, operators, leaves);
        }
        Expression::LogicalExpression(logical) => {
            collect_logical_chain(logical, operators, leaves);
        }
        other => leaves.push(other),
    }
}

fn count_operator_runs<T: PartialEq>(operators: &[T]) -> u32 {
    if operators.is_empty() {
        return 0;
    }
    let switches = operators
        .windows(2)
        .filter(|pair| pair[0] != pair[1])
        .count();
    saturating_u32(switches + 1)
}

fn unnamed_function_span(expression: &Expression<'_>) -> Option<Span> {
    match unwrap_expression(expression) {
        Expression::ArrowFunctionExpression(arrow) => Some(arrow.span),
        Expression::FunctionExpression(function)
            if function.body.is_some() && function.id.is_none() =>
        {
            Some(function.span)
        }
        _ => None,
    }
}

fn unwrap_expression<'a>(mut expression: &'a Expression<'a>) -> &'a Expression<'a> {
    loop {
        match expression {
            Expression::ParenthesizedExpression(inner) => expression = &inner.expression,
            Expression::TSAsExpression(inner) => expression = &inner.expression,
            Expression::TSSatisfiesExpression(inner) => expression = &inner.expression,
            Expression::TSTypeAssertion(inner) => expression = &inner.expression,
            Expression::TSNonNullExpression(inner) => expression = &inner.expression,
            _ => return expression,
        }
    }
}

fn property_key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(ident) => Some(ident.name.to_string()),
        PropertyKey::PrivateIdentifier(ident) => Some(format!("#{}", ident.name)),
        PropertyKey::StringLiteral(literal) => literal.value.as_str().map(str::to_owned),
        PropertyKey::NumericLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
