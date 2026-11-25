import 'dart:convert';
import 'dart:io';
import 'dart:math';

import 'package:analyzer/dart/analysis/features.dart';
import 'package:analyzer/dart/analysis/utilities.dart';
import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/dart/ast/token.dart';
import 'package:analyzer/dart/ast/visitor.dart';
import 'package:analyzer/source/line_info.dart';

const decisionOperators = {'&&', '||', '??'};

class DecisionCounter extends RecursiveAstVisitor<void> {
  int total = 0;

  @override
  void visitFunctionExpression(FunctionExpression node) {}

  @override
  void visitIfStatement(IfStatement node) {
    total++;
    super.visitIfStatement(node);
  }

  @override
  void visitIfElement(IfElement node) {
    total++;
    super.visitIfElement(node);
  }

  @override
  void visitForStatement(ForStatement node) {
    total++;
    super.visitForStatement(node);
  }

  @override
  void visitForElement(ForElement node) {
    total++;
    super.visitForElement(node);
  }

  @override
  void visitWhileStatement(WhileStatement node) {
    total++;
    super.visitWhileStatement(node);
  }

  @override
  void visitDoStatement(DoStatement node) {
    total++;
    super.visitDoStatement(node);
  }

  @override
  void visitSwitchCase(SwitchCase node) {
    total++;
    super.visitSwitchCase(node);
  }

  @override
  void visitSwitchPatternCase(SwitchPatternCase node) {
    total++;
    super.visitSwitchPatternCase(node);
  }

  @override
  void visitSwitchExpression(SwitchExpression node) {
    total += max(0, node.cases.length - 1);
    super.visitSwitchExpression(node);
  }

  @override
  void visitCatchClause(CatchClause node) {
    total += node.catchKeyword == null ? 0 : 1;
    super.visitCatchClause(node);
  }

  @override
  void visitBinaryExpression(BinaryExpression node) {
    total += decisionOperators.contains(node.operator.lexeme) ? 1 : 0;
    super.visitBinaryExpression(node);
  }

  @override
  void visitConditionalExpression(ConditionalExpression node) {
    total++;
    super.visitConditionalExpression(node);
  }
}

class FunctionCollector extends RecursiveAstVisitor<void> {
  FunctionCollector(this.lines);

  final LineInfo lines;
  final List<List<int>> functions = [];

  void record(Token? start, FunctionBody body) {
    if (start == null || body is EmptyFunctionBody) {
      return;
    }
    final counter = DecisionCounter();
    body.visitChildren(counter);
    functions.add([
      lines.getLocation(start.offset).lineNumber,
      lines.getLocation(body.end - 1).lineNumber,
      counter.total,
    ]);
  }

  @override
  void visitFunctionDeclaration(FunctionDeclaration node) {
    final expression = node.functionExpression;
    record(expression.parameters?.leftParenthesis ?? node.propertyKeyword, expression.body);
    super.visitFunctionDeclaration(node);
  }

  @override
  void visitMethodDeclaration(MethodDeclaration node) {
    record(node.parameters?.leftParenthesis ?? node.propertyKeyword, node.body);
    super.visitMethodDeclaration(node);
  }

  @override
  void visitConstructorDeclaration(ConstructorDeclaration node) {
    record(node.parameters.leftParenthesis, node.body);
    super.visitConstructorDeclaration(node);
  }

  @override
  void visitFunctionExpression(FunctionExpression node) {
    if (node.parent is! FunctionDeclaration) {
      record(node.parameters?.leftParenthesis, node.body);
    }
    super.visitFunctionExpression(node);
  }
}

List<String> directiveUris(CompilationUnit unit) {
  final uris = <String>{};
  for (final directive in unit.directives) {
    if (directive is NamespaceDirective) {
      uris.add(directive.uri.stringValue ?? '');
      uris.addAll(directive.configurations.map((c) => c.uri.stringValue ?? ''));
    } else if (directive is PartDirective) {
      uris.add(directive.uri.stringValue ?? '');
    }
  }
  return (uris..remove('')).toList()..sort();
}

Map<String, Object> describe(String path) {
  final String content;
  try {
    content = File(path).readAsStringSync();
  } on FileSystemException catch (error) {
    return {'path': path, 'error': error.message};
  } on FormatException catch (error) {
    return {'path': path, 'error': error.message};
  }
  final result = parseString(
    content: content,
    featureSet: FeatureSet.latestLanguageVersion(),
    throwIfDiagnostics: false,
  );
  if (result.errors.isNotEmpty) {
    return {'path': path, 'error': result.errors.first.message};
  }
  final collector = FunctionCollector(result.lineInfo);
  result.unit.accept(collector);
  return {'path': path, 'functions': collector.functions, 'imports': directiveUris(result.unit)};
}

void main(List<String> arguments) {
  final paths = File(arguments.first).readAsStringSync().split('\u0000').where((path) => path.isNotEmpty);
  for (final path in paths) {
    stdout.writeln(jsonEncode(describe(path)));
  }
}
