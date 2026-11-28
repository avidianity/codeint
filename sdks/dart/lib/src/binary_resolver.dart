import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:archive/archive.dart';
import 'package:crypto/crypto.dart';

import 'errors.dart';
import 'options.dart';

class ReleaseTarget {
  const ReleaseTarget({
    required this.triple,
    required this.archiveExtension,
    required this.binaryName,
  });

  final String triple;
  final String archiveExtension;
  final String binaryName;

  String get archiveFileName => 'codeint-$triple.$archiveExtension';
}

Future<String> ensureBinary([
  BinaryResolverOptions options = const BinaryResolverOptions(),
]) async {
  final env = options.environment ?? Platform.environment;

  final explicitPath = _trimmedNonEmpty(options.binaryPath);
  if (explicitPath != null) {
    return _requireExistingFile(explicitPath, 'explicit binaryPath');
  }

  final envBinaryPath = _trimmedNonEmpty(env['CODEINT_BIN']);
  if (envBinaryPath != null) {
    return _requireExistingFile(envBinaryPath, 'CODEINT_BIN');
  }

  final pathMatch = await _findMatchingBinaryOnPath(
    env,
    options.expectedVersion,
  );
  if (pathMatch != null) {
    return pathMatch;
  }

  return _resolveFromVerifiedCache(options, env);
}

ReleaseTarget detectReleaseTarget({
  String? operatingSystem,
  String? dartVersion,
  Map<String, String>? environment,
}) {
  final os = operatingSystem ?? Platform.operatingSystem;
  final arch = _detectArchitecture(
    dartVersion ?? Platform.version,
    environment ?? Platform.environment,
  );
  return _targetForPlatform(os, arch);
}

String? parseChecksumEntry(String checksumsText, String archiveFileName) {
  for (final rawLine in const LineSplitter().convert(checksumsText)) {
    final line = rawLine.trim();
    if (line.isEmpty) {
      continue;
    }
    final match = RegExp(r'^([0-9a-fA-F]{64})\s+\*?(.+)$').firstMatch(line);
    if (match != null && match.group(2)?.trim() == archiveFileName) {
      return match.group(1)!.toLowerCase();
    }
  }
  return null;
}

ReleaseTarget _targetForPlatform(String os, String arch) {
  if (os == 'macos') {
    final triple = arch == 'arm64'
        ? 'aarch64-apple-darwin'
        : 'x86_64-apple-darwin';
    return ReleaseTarget(
      triple: triple,
      archiveExtension: 'tar.gz',
      binaryName: 'codeint',
    );
  }
  if (os == 'linux') {
    final triple = arch == 'arm64'
        ? 'aarch64-unknown-linux-musl'
        : 'x86_64-unknown-linux-musl';
    return ReleaseTarget(
      triple: triple,
      archiveExtension: 'tar.gz',
      binaryName: 'codeint',
    );
  }
  if (os == 'windows') {
    final triple = arch == 'arm64'
        ? 'aarch64-pc-windows-msvc'
        : 'x86_64-pc-windows-msvc';
    return ReleaseTarget(
      triple: triple,
      archiveExtension: 'zip',
      binaryName: 'codeint.exe',
    );
  }
  throw CodeintError('Unsupported operating system for codeint binary: $os');
}

String _detectArchitecture(String versionText, Map<String, String> env) {
  final versionMatch = RegExp(r'on "[^"]+_([^"]+)"').firstMatch(versionText);
  final rawArch = versionMatch?.group(1) ??
      env['PROCESSOR_ARCHITEW6432'] ??
      env['PROCESSOR_ARCHITECTURE'] ??
      '';
  final normalized = rawArch.toLowerCase();
  if (normalized == 'arm64' || normalized == 'aarch64') {
    return 'arm64';
  }
  if (normalized == 'x64' || normalized == 'x86_64' || normalized == 'amd64') {
    return 'x64';
  }
  throw CodeintError(
    'Unsupported CPU architecture for codeint binary: "$rawArch"',
  );
}

String? _trimmedNonEmpty(String? value) {
  if (value == null) {
    return null;
  }
  final trimmed = value.trim();
  return trimmed.isEmpty ? null : trimmed;
}

int _stagingCounter = 0;

Future<String> _requireExistingFile(String path, String sourceLabel) async {
  final file = File(path);
  try {
    if (await file.exists()) {
      return file.absolute.uri.toFilePath();
    }
  } on FileSystemException catch (error) {
    throw CodeintError(
      'Failed to access codeint binary from $sourceLabel ($path): ${error.message}',
      cause: error,
    );
  }
  throw CodeintError('codeint binary from $sourceLabel does not exist: $path');
}

Future<String?> _findMatchingBinaryOnPath(
  Map<String, String> env,
  String expectedVersion,
) async {
  final rawPath = env['PATH'] ?? env['Path'] ?? '';
  if (rawPath.isEmpty) {
    return null;
  }
  final separator = Platform.isWindows ? ';' : ':';
  final binaryName = Platform.isWindows ? 'codeint.exe' : 'codeint';
  for (final dir in rawPath.split(separator)) {
    if (dir.isEmpty) {
      continue;
    }
    final candidate = File('$dir${Platform.pathSeparator}$binaryName');
    if (!await candidate.exists()) {
      continue;
    }
    if (await _matchesBinaryVersion(candidate.path, expectedVersion)) {
      return candidate.absolute.uri.toFilePath();
    }
  }
  return null;
}

Future<bool> _matchesBinaryVersion(
  String binaryPath,
  String expectedVersion,
) async {
  try {
    final result = await Process.run(binaryPath, const <String>['--version']);
    if (result.exitCode != 0) {
      return false;
    }
    final output = result.stdout.toString().trim();
    return output == 'codeint $expectedVersion' ||
        output.startsWith('codeint $expectedVersion ');
  } on ProcessException {
    return false;
  }
}

Future<String> _resolveFromVerifiedCache(
  BinaryResolverOptions options,
  Map<String, String> env,
) async {
  final target = detectReleaseTarget(environment: env);
  final cacheRoot = _resolveCacheRoot(options.cacheDir, env);
  final cachedFile = File(
    '$cacheRoot${Platform.pathSeparator}bin'
    '${Platform.pathSeparator}${options.expectedVersion}'
    '${Platform.pathSeparator}${target.triple}'
    '${Platform.pathSeparator}${target.binaryName}',
  );
  if (await cachedFile.exists() && await cachedFile.length() > 0) {
    return cachedFile.absolute.uri.toFilePath();
  }
  if (!options.allowDownload) {
    throw CodeintError(
      'codeint ${options.expectedVersion} binary not found locally and '
      'downloads are disabled (allowDownload: false). Provide binaryPath, '
      'set CODEINT_BIN, install codeint ${options.expectedVersion} on PATH, '
      'or run `dart run codeint:install`.',
    );
  }
  await _downloadAndInstallBinary(
    target: target,
    destination: cachedFile,
    baseUrl: _resolveReleaseBaseUrl(options, env),
  );
  return cachedFile.absolute.uri.toFilePath();
}

String _resolveCacheRoot(String? explicitCacheDir, Map<String, String> env) {
  final explicit = _trimmedNonEmpty(explicitCacheDir);
  if (explicit != null) {
    return explicit;
  }
  final envCache = _trimmedNonEmpty(env['CODEINT_CACHE_DIR']);
  if (envCache != null) {
    return envCache;
  }
  if (Platform.isWindows) {
    final localAppData = _trimmedNonEmpty(env['LOCALAPPDATA']);
    if (localAppData != null) {
      return '$localAppData\\codeint';
    }
  }
  final xdg = _trimmedNonEmpty(env['XDG_CACHE_HOME']);
  if (xdg != null) {
    return '$xdg/codeint';
  }
  final home = _trimmedNonEmpty(env['HOME']) ?? _trimmedNonEmpty(env['USERPROFILE']);
  if (home != null) {
    return '$home${Platform.pathSeparator}.cache${Platform.pathSeparator}codeint';
  }
  throw const CodeintError(
    'Unable to determine local cache directory; set cacheDir or CODEINT_CACHE_DIR.',
  );
}

String _resolveReleaseBaseUrl(
  BinaryResolverOptions options,
  Map<String, String> env,
) {
  final configured = _trimmedNonEmpty(options.releaseBaseUrl) ??
      _trimmedNonEmpty(env['CODEINT_RELEASE_BASE_URL']) ??
      'https://github.com/avidianity/codeint/releases/download/v${options.expectedVersion}';
  return configured.endsWith('/')
      ? configured.substring(0, configured.length - 1)
      : configured;
}

Future<void> _downloadAndInstallBinary({
  required ReleaseTarget target,
  required File destination,
  required String baseUrl,
}) async {
  final client = HttpClient()..connectionTimeout = const Duration(seconds: 30);
  try {
    final checksumsBytes = await _httpGetBytes(client, '$baseUrl/SHA256SUMS');
    final checksumsText = _decodeUtf8(checksumsBytes, 'SHA256SUMS');
    final expectedSha = parseChecksumEntry(checksumsText, target.archiveFileName);
    if (expectedSha == null) {
      throw CodeintError(
        'No SHA-256 checksum entry for ${target.archiveFileName} in SHA256SUMS',
      );
    }
    final archiveBytes = await _httpGetBytes(
      client,
      '$baseUrl/${target.archiveFileName}',
    );
    final actualSha = sha256.convert(archiveBytes).toString().toLowerCase();
    if (actualSha != expectedSha) {
      throw CodeintError(
        'SHA-256 checksum mismatch for ${target.archiveFileName}: '
        'expected $expectedSha, got $actualSha',
      );
    }
    final binaryBytes = extractBinaryFromArchive(
      archiveBytes,
      target.archiveExtension,
      target.binaryName,
    );
    await _writeExecutableAtomically(destination, binaryBytes);
  } finally {
    client.close(force: true);
  }
}

String _decodeUtf8(Uint8List bytes, String label) {
  try {
    return utf8.decode(bytes);
  } on FormatException catch (error) {
    throw CodeintError('Invalid UTF-8 in $label: ${error.message}', cause: error);
  }
}

Future<Uint8List> _httpGetBytes(HttpClient client, String url) async {
  try {
    final uri = Uri.parse(url);
    final request = await client.getUrl(uri);
    final response = await request.close();
    if (response.statusCode != HttpStatus.ok) {
      throw CodeintError(
        'Failed to download $url: HTTP ${response.statusCode}',
      );
    }
    final builder = BytesBuilder(copy: false);
    await for (final chunk in response) {
      builder.add(chunk);
    }
    return builder.takeBytes();
  } on CodeintError {
    rethrow;
  } on Object catch (error) {
    throw CodeintError('Failed to download $url: $error', cause: error);
  }
}

Uint8List extractBinaryFromArchive(
  Uint8List archiveBytes,
  String archiveExtension,
  String binaryName,
) {
  final archive = _decodeArchive(archiveBytes, archiveExtension);
  Uint8List? matched;
  for (final file in archive.files) {
    final normalized = _validateArchiveEntryPath(file.name);
    if (file.isSymbolicLink || file.isHardLink) {
      throw CodeintError(
        'Archive contains disallowed link entry "${file.name}"',
      );
    }
    if (!file.isFile) {
      continue;
    }
    if (matched == null && normalized.split('/').last == binaryName) {
      final extracted = file.readBytes();
      if (extracted != null && extracted.isNotEmpty) {
        matched = extracted;
      }
    }
  }
  if (matched != null) {
    return matched;
  }
  throw CodeintError('Archive does not contain "$binaryName"');
}

String _validateArchiveEntryPath(String rawName) {
  if (rawName.startsWith('/') ||
      rawName.startsWith('\\') ||
      RegExp(r'^[a-zA-Z]:').hasMatch(rawName)) {
    throw CodeintError('Archive contains unsafe path "$rawName"');
  }
  final normalized = rawName.replaceAll('\\', '/');
  if (normalized.split('/').contains('..')) {
    throw CodeintError('Archive contains unsafe path "$rawName"');
  }
  return normalized;
}

Archive _decodeArchive(Uint8List archiveBytes, String archiveExtension) {
  try {
    if (archiveExtension == 'zip') {
      return ZipDecoder().decodeBytes(archiveBytes);
    }
    final tarBytes = GZipDecoder().decodeBytes(archiveBytes);
    return TarDecoder().decodeBytes(tarBytes);
  } on Object catch (error) {
    throw CodeintError(
      'Failed to extract .$archiveExtension archive: $error',
      cause: error,
    );
  }
}

Future<void> _writeExecutableAtomically(
  File destination,
  Uint8List binaryBytes,
) async {
  final stamp = DateTime.now().microsecondsSinceEpoch;
  final sequence = ++_stagingCounter;
  final staged = File('${destination.path}.tmp.$pid.$stamp.$sequence');
  try {
    await destination.parent.create(recursive: true);
    await staged.writeAsBytes(binaryBytes, flush: true);
    if (!Platform.isWindows) {
      final chmod = await Process.run('chmod', <String>['0755', staged.path]);
      if (chmod.exitCode != 0) {
        throw CodeintError(
          'Failed to mark ${staged.path} executable: ${chmod.stderr}',
        );
      }
    }
    await staged.rename(destination.path);
  } on CodeintError {
    await _deleteIfExists(staged);
    rethrow;
  } on Object catch (error) {
    await _deleteIfExists(staged);
    throw CodeintError(
      'Failed to install codeint binary at ${destination.path}: $error',
      cause: error,
    );
  }
}

Future<void> _deleteIfExists(File file) async {
  try {
    if (await file.exists()) {
      await file.delete();
    }
  } on FileSystemException {
    return;
  }
}
