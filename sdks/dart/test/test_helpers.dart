import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:archive/archive.dart';
import 'package:crypto/crypto.dart';

String resolveTestCodeintBinary() {
  final fromEnv = Platform.environment['CODEINT_BIN'];
  if (fromEnv != null && fromEnv.isNotEmpty && File(fromEnv).existsSync()) {
    return File(fromEnv).absolute.path;
  }
  final binaryName = Platform.isWindows ? 'codeint.exe' : 'codeint';
  final candidate = File('../../target/debug/$binaryName');
  if (candidate.existsSync()) {
    return candidate.absolute.path;
  }
  throw StateError(
    'codeint binary not found. Set CODEINT_BIN or build target/debug/codeint.',
  );
}

Future<Directory> createTempDir(String prefix) =>
    Directory.systemTemp.createTemp('codeint_dart_sdk_${prefix}_');

Future<void> runGit(Directory dir, List<String> args) async {
  final result = await Process.run(
    'git',
    args,
    workingDirectory: dir.path,
    environment: <String, String>{
      ...Platform.environment,
      'GIT_CONFIG_GLOBAL': Platform.isWindows ? 'NUL' : '/dev/null',
      'GIT_CONFIG_NOSYSTEM': '1',
      'GIT_AUTHOR_NAME': 'Test User',
      'GIT_AUTHOR_EMAIL': 'test@example.com',
      'GIT_COMMITTER_NAME': 'Test User',
      'GIT_COMMITTER_EMAIL': 'test@example.com',
    },
  );
  if (result.exitCode != 0) {
    throw StateError(
      'git ${args.join(' ')} failed (${result.exitCode}): ${result.stderr}',
    );
  }
}

Future<Directory> createSampleRepo({bool includeConfig = true}) async {
  final dir = await createTempDir('repo');
  await runGit(dir, const <String>['init', '-b', 'main']);
  await writeRepoFile(
    dir,
    'src/math.ts',
    'export function add(a: number, b: number): number {\n  return a + b;\n}\n',
  );
  await writeRepoFile(
    dir,
    'src/index.ts',
    'import { add } from "./math";\n'
    'export function compute(x: number): number {\n'
    '  return add(x, 1);\n'
    '}\n',
  );
  if (includeConfig) {
    await writeRepoFile(
      dir,
      'codeint.json',
      '{"format":"json"}\n',
    );
  }
  await runGit(dir, const <String>['add', '.']);
  await runGit(dir, const <String>['commit', '-m', 'initial']);
  return dir;
}

Future<File> writeRepoFile(Directory dir, String relative, String content) async {
  final file = File('${dir.path}${Platform.pathSeparator}$relative');
  await file.parent.create(recursive: true);
  return file.writeAsString(content);
}

Uint8List buildArchiveBytes({
  required String archiveExtension,
  required String binaryName,
  required Uint8List binaryBytes,
}) {
  final archive = Archive()
    ..addFile(ArchiveFile(binaryName, binaryBytes.length, binaryBytes));
  if (archiveExtension == 'zip') {
    return Uint8List.fromList(ZipEncoder().encode(archive));
  }
  final tarBytes = TarEncoder().encode(archive);
  return Uint8List.fromList(GZipEncoder().encode(tarBytes));
}

class MockReleaseServer {
  MockReleaseServer._(this._server, this._assets);

  final HttpServer _server;
  final Map<String, Uint8List> _assets;
  int requestCount = 0;

  String get baseUrl => 'http://${_server.address.host}:${_server.port}';

  static Future<MockReleaseServer> start(Map<String, Uint8List> assets) async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final mock = MockReleaseServer._(server, assets);
    server.listen(mock._handleRequest);
    return mock;
  }

  Future<void> _handleRequest(HttpRequest request) async {
    requestCount += 1;
    final name = request.uri.path.replaceFirst(RegExp(r'^/+'), '');
    final payload = _assets[name];
    if (payload == null) {
      request.response.statusCode = HttpStatus.notFound;
      await request.response.close();
      return;
    }
    request.response.statusCode = HttpStatus.ok;
    request.response.add(payload);
    await request.response.close();
  }

  Future<void> close() => _server.close(force: true);
}

Map<String, Uint8List> buildMockReleaseAssets({
  required String archiveFileName,
  required Uint8List archiveBytes,
  String? checksumOverride,
  bool omitChecksumEntry = false,
}) {
  final digest = checksumOverride ?? sha256.convert(archiveBytes).toString();
  final checksumText = omitChecksumEntry
      ? '0000000000000000000000000000000000000000000000000000000000000000  other.tar.gz\n'
      : '$digest  $archiveFileName\n';
  return <String, Uint8List>{
    'SHA256SUMS': Uint8List.fromList(utf8.encode(checksumText)),
    archiveFileName: archiveBytes,
  };
}
