class CodeintError implements Exception {
  const CodeintError(
    this.message, {
    this.exitCode,
    this.stderr = '',
    this.cause,
  });

  final String message;
  final int? exitCode;
  final String stderr;
  final Object? cause;

  @override
  String toString() {
    final codePart = exitCode == null ? '' : ' (exit $exitCode)';
    return 'CodeintError$codePart: $message';
  }
}

class GatedOutcome<T> {
  const GatedOutcome({
    required this.passed,
    required this.exitCode,
    required this.failures,
    required this.report,
  });

  final bool passed;
  final int exitCode;
  final List<String> failures;
  final T report;
}
