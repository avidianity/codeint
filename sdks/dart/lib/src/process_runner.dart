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
  return parse(_decodeJsonObject(raw, args));
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
  final report = parse(_decodeJsonObject(raw, args));
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
  String? outputPath,
}) async {
  final raw = await runCodeintProcess(args, options, extraEnv: extraEnv);
  if (raw.exitCode != 0) {
    throw _errorFromRawResult(args, raw);
  }
  return _readOutputPayload(raw, options, outputPath);
}

Future<String> _readOutputPayload(
  RawCommandResult raw,
  CommonOptions options,
  String? outputPath,
) async {
  if (outputPath == null || outputPath.isEmpty) {
    return raw.stdout;
  }
  final resolvedPath = _resolveOutputPath(options.cwd, outputPath);
  try {
    return await File(resolvedPath).readAsString();
  } on FileSystemException catch (error) {
    throw CodeintError(
      'Failed to read output file "$resolvedPath": ${error.message}',
      exitCode: raw.exitCode,
      stderr: raw.stderr,
      cause: error,
    );
  }
}

String _resolveOutputPath(String? cwd, String outputPath) {
  if (File(outputPath).isAbsolute) {
    return outputPath;
  }
  final baseDir = cwd == null || cwd.isEmpty ? Directory.current.path : cwd;
  return Directory(baseDir).uri.resolveUri(Uri.file(outputPath)).toFilePath();
}

Map<String, Object?> _decodeJsonObject(
  RawCommandResult raw,
  List<String> args,
) {
  try {
    final decoded = jsonDecode(raw.stdout);
    return requireJsonMap(decoded, 'codeint ${args.join(' ')}');
  } on CodeintError catch (error) {
    throw CodeintError(
      error.message,
      exitCode: raw.exitCode,
      stderr: raw.stderr,
      cause: error,
    );
  } on FormatException catch (error) {
    throw CodeintError(
      'Invalid JSON output from codeint ${args.join(' ')}: ${error.message}',
      exitCode: raw.exitCode,
      stderr: raw.stderr,
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
  return CodeintError(message, exitCode: raw.exitCode, stderr: raw.stderr);
}
