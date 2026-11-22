use oxc_ast::ast::{
    CallExpression, ExportAllDeclaration, ExportFromDeclaration, Expression, ImportDeclaration,
    ImportDeclarationSpecifier, ImportExpression, Program, TSImportEqualsDeclaration,
    TSModuleReference,
};
use oxc_ast_visit::{Visit, walk};

pub(crate) fn runtime_import_specifiers(program: &Program<'_>) -> Vec<String> {
    let mut collector = RuntimeImportCollector::default();
    collector.visit_program(program);
    collector.specifiers
}

#[derive(Default)]
struct RuntimeImportCollector {
    specifiers: Vec<String>,
}

impl<'a> Visit<'a> for RuntimeImportCollector {
    fn visit_import_declaration(&mut self, declaration: &ImportDeclaration<'a>) {
        let imports_only_types = declaration.import_kind.is_type()
            || declaration.specifiers.as_ref().is_some_and(|specifiers| {
                !specifiers.is_empty()
                    && specifiers.iter().all(|specifier| {
                        matches!(
                            specifier,
                            ImportDeclarationSpecifier::ImportSpecifier(named)
                                if named.import_kind.is_type()
                        )
                    })
            });
        if !imports_only_types {
            self.push(declaration.source.value.as_str());
        }
    }

    fn visit_export_from_declaration(&mut self, declaration: &ExportFromDeclaration<'a>) {
        let exports_only_types = declaration.export_kind.is_type()
            || (!declaration.specifiers.is_empty()
                && declaration
                    .specifiers
                    .iter()
                    .all(|specifier| specifier.export_kind.is_type()));
        if !exports_only_types {
            self.push(declaration.source.value.as_str());
        }
    }

    fn visit_export_all_declaration(&mut self, declaration: &ExportAllDeclaration<'a>) {
        if !declaration.export_kind.is_type() {
            self.push(declaration.source.value.as_str());
        }
    }

    fn visit_ts_import_equals_declaration(&mut self, declaration: &TSImportEqualsDeclaration<'a>) {
        if let TSModuleReference::ExternalModuleReference(reference) = &declaration.module_reference
            && !declaration.import_kind.is_type()
        {
            self.push(reference.expression.value.as_str());
        }
    }

    fn visit_import_expression(&mut self, expression: &ImportExpression<'a>) {
        self.push(literal_text(&expression.source));
        walk::walk_import_expression(self, expression);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Some(specifier) = call.common_js_require() {
            self.push(specifier.value.as_str());
        }
        walk::walk_call_expression(self, call);
    }
}

impl RuntimeImportCollector {
    fn push(&mut self, specifier: Option<&str>) {
        if let Some(specifier) = specifier {
            self.specifiers.push(specifier.to_owned());
        }
    }
}

fn literal_text<'a>(expression: &Expression<'a>) -> Option<&'a str> {
    match expression {
        Expression::StringLiteral(literal) => literal.value.as_str(),
        Expression::TemplateLiteral(template) if template.expressions.is_empty() => template
            .quasis
            .first()
            .and_then(|quasi| quasi.value.cooked)
            .and_then(|cooked| cooked.as_str()),
        _ => None,
    }
}
