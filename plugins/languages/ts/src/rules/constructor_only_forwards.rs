use oxc_ast::ast::{
    Argument, BindingPattern, CallExpression, Class, ClassElement, Expression, FormalParameterRest,
    FormalParameters, Function, MethodDefinition, MethodDefinitionKind, Program, Statement,
};
use oxc_ast_visit::{Visit, walk};
use oxc_span::Span;

use super::Rule;

pub(super) const RULE: Rule = Rule {
    id: "constructor-only-forwards",
    find: find_forwarding_constructors,
};

fn find_forwarding_constructors(program: &Program<'_>, _code: &str) -> Vec<Span> {
    let mut finder = ForwardingConstructorFinder { spans: Vec::new() };
    finder.visit_program(program);
    finder.spans
}

struct ForwardingConstructorFinder {
    spans: Vec<Span>,
}

impl<'a> Visit<'a> for ForwardingConstructorFinder {
    fn visit_class(&mut self, class: &Class<'a>) {
        if class.heritage.is_some()
            && class.decorators.is_empty()
            && let [constructor] = constructors(class).as_slice()
            && only_forwards_to_super(constructor)
        {
            self.spans.push(constructor.span);
        }
        walk::walk_class(self, class);
    }
}

fn constructors<'c, 'a>(class: &'c Class<'a>) -> Vec<&'c MethodDefinition<'a>> {
    class
        .body
        .body
        .iter()
        .filter_map(|element| match element {
            ClassElement::MethodDefinition(method)
                if method.kind == MethodDefinitionKind::Constructor =>
            {
                Some(&**method)
            }
            _ => None,
        })
        .collect()
}

fn only_forwards_to_super(constructor: &MethodDefinition<'_>) -> bool {
    constructor.accessibility.is_none()
        && constructor.decorators.is_empty()
        && constructor.value.this_param.is_none()
        && super_call(&constructor.value)
            .is_some_and(|call| forwards_untyped_single_rest(&constructor.value.params, call))
}

fn super_call<'f, 'a>(function: &'f Function<'a>) -> Option<&'f CallExpression<'a>> {
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let [Statement::ExpressionStatement(statement)] = body.statements.as_slice() else {
        return None;
    };
    let Expression::CallExpression(call) = statement.expression.without_parentheses() else {
        return None;
    };
    (matches!(call.callee, Expression::Super(_)) && call.type_arguments.is_none() && !call.optional)
        .then_some(&**call)
}

fn forwards_untyped_single_rest(
    parameters: &FormalParameters<'_>,
    call: &CallExpression<'_>,
) -> bool {
    if !parameters.items.is_empty() {
        return false;
    }
    let Some(rest) = parameters.rest.as_deref() else {
        return false;
    };
    let [argument] = call.arguments.as_slice() else {
        return false;
    };
    forwards_untyped_rest(rest, argument)
}

fn forwards_untyped_rest(rest: &FormalParameterRest<'_>, argument: &Argument<'_>) -> bool {
    rest.decorators.is_empty()
        && rest.type_annotation.is_none()
        && plain_name(&rest.rest.argument).is_some_and(|name| {
            matches!(
                argument,
                Argument::SpreadElement(spread)
                    if matches!(&spread.argument, Expression::Identifier(identifier) if identifier.name == name)
            )
        })
}

fn plain_name<'a>(pattern: &BindingPattern<'a>) -> Option<&'a str> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.as_str()),
        _ => None,
    }
}
