import 'dart:convert';
import 'dart:io';

import 'binary_resolver.dart';
import 'errors.dart';
import 'json_parsing.dart';
import 'options.dart';

class RawCommandResult {
  const RawCommandResult({
    required this.exitCode,
    required this.stdout,
    required this.stderr,
  });

  final int exitCode;
  final String stdout;
  final String stderr;
}

Future<RawCommandResult> runCodeintProcess(
  List<String> args,
  CommonOptions options, {
  Map<String, String>? extraEnv,
}) async {
  final binary = await ensureBinary(options.toResolverOptions());
  final env = <String, String>{
    ...(options.environment ?? Platform.environment),
    if (extraEnv != null) ...extraEnv,
  };
  try {
    final result = await Process.run(
      binary,
      args,
      workingDirectory: options.cwd,
      environment: env,
      stdoutEncoding: utf8,
      stderrEncoding: utf8,
    );
    return RawCommandResult(
      exitCode: result.exitCode,
      stdout: result.stdout.toString(),
      stderr: result.stderr.toString(),
    );
  } on ProcessException catch (error) {
    throw CodeintError(
      'Failed to execute codeint binary "$binary": ${error.message}',
      cause: error,
    );
  }
}

Future<T> runJsonCommand<T>(
  List<String> args,
  CommonOptions options,
  T Function(Map<String, Object?>) parse, {
  Map<String, String>? extraEnv,
}) async {
  final raw = await runCodeintProcess(args, options, extraEnv: extraEnv);
  if (raw.exitCode != 0) {
    throw _errorFromRawResult(args, raw);
  }
  return parse(_decodeJsonObject(raw.stdout, args));
}

Future<GatedOutcome<T>> runGatedJsonCommand<T>(
  List<String> args,
  CommonOptions options,
  T Function(Map<String, Object?>) parse, {
  Map<String, String>? extraEnv,
}) async {
  final raw = await runCodeintProcess(args, options, extraEnv: extraEnv);
  if (raw.exitCode != 0 && raw.exitCode != 1) {
    throw _errorFromRawResult(args, raw);
  }
  final report = parse(_decodeJsonObject(raw.stdout, args));
  final failures = raw.exitCode == 1
      ? _parseFailureLines(raw.stderr)
      : const <String>[];
  return GatedOutcome<T>(
    passed: raw.exitCode == 0,
    exitCode: raw.exitCode,
    failures: failures,
    report: report,
  );
}

Future<String> runTextCommand(
  List<String> args,
  CommonOptions options, {
  Map<String, String>? extraEnv,
}) async {
  final raw = await runCodeintProcess(args, options, extraEnv: extraEnv);
  if (raw.exitCode != 0) {
    throw _errorFromRawResult(args, raw);
  }
  return raw.stdout;
}

Map<String, Object?> _decodeJsonObject(String stdout, List<String> args) {
  try {
    final decoded = jsonDecode(stdout);
    return requireJsonMap(decoded, 'codeint ${args.join(' ')}');
  } on CodeintError {
    rethrow;
  } on FormatException catch (error) {
    throw CodeintError(
      'Invalid JSON output from codeint ${args.join(' ')}: ${error.message}',
      cause: error,
    );
  }
}

List<String> _parseFailureLines(String stderr) {
  final lines = <String>[];
  for (final rawLine in const LineSplitter().convert(stderr)) {
    final trimmed = rawLine.trim();
    if (trimmed.isEmpty) {
      continue;
    }
    const prefix = 'codeint: ';
    lines.add(
      trimmed.startsWith(prefix) ? trimmed.substring(prefix.length) : trimmed,
    );
  }
  return lines;
}

CodeintError _errorFromRawResult(List<String> args, RawCommandResult raw) {
  final trimmedStderr = raw.stderr.trim();
  final message = trimmedStderr.isNotEmpty
      ? trimmedStderr
      : 'codeint ${args.join(' ')} failed with exit code ${raw.exitCode}';
  return CodeintError(
    message,
    exitCode: raw.exitCode,
    stderr: raw.stderr,
  );
}
