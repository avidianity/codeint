import 'dart:io';
import 'dart:typed_data';

import 'package:archive/archive.dart';
import 'package:codeint/codeint.dart';
import 'package:codeint/src/binary_resolver.dart';
import 'package:test/test.dart';

import 'test_helpers.dart';

void main() {
  final realBinary = resolveTestCodeintBinary();

  group('detectReleaseTarget', () {
    test('maps all 6 supported release triples and archive extensions', () {
      expect(
        detectReleaseTarget(
          operatingSystem: 'macos',
          dartVersion: '3.11.0 on "macos_arm64"',
        ).archiveFileName,
        equals('codeint-aarch64-apple-darwin.tar.gz'),
      );
      expect(
        detectReleaseTarget(
          operatingSystem: 'macos',
          dartVersion: '3.11.0 on "macos_x64"',
        ).archiveFileName,
        equals('codeint-x86_64-apple-darwin.tar.gz'),
      );
      expect(
        detectReleaseTarget(
          operatingSystem: 'linux',
          dartVersion: '3.11.0 on "linux_x64"',
        ).archiveFileName,
        equals('codeint-x86_64-unknown-linux-musl.tar.gz'),
      );
      expect(
        detectReleaseTarget(
          operatingSystem: 'linux',
          dartVersion: '3.11.0 on "linux_arm64"',
        ).archiveFileName,
        equals('codeint-aarch64-unknown-linux-musl.tar.gz'),
      );
      expect(
        detectReleaseTarget(
          operatingSystem: 'windows',
          dartVersion: '3.11.0 on "windows_x64"',
        ).archiveFileName,
        equals('codeint-x86_64-pc-windows-msvc.zip'),
      );
      expect(
        detectReleaseTarget(
          operatingSystem: 'windows',
          dartVersion: '3.11.0 on "windows_arm64"',
        ).archiveFileName,
        equals('codeint-aarch64-pc-windows-msvc.zip'),
      );
    });
  });

  group('4-step binary resolver', () {
    test('step 1 explicit binaryPath takes precedence over CODEINT_BIN', () async {
      final resolved = await ensureBinary(
        BinaryResolverOptions(
          binaryPath: realBinary,
          allowDownload: false,
          environment: const <String, String>{
            'CODEINT_BIN': '/nonexistent/codeint',
            'PATH': '',
          },
        ),
      );
      expect(resolved, equals(realBinary));
    });

    test('step 2 CODEINT_BIN takes precedence over PATH', () async {
      final resolved = await ensureBinary(
        BinaryResolverOptions(
          allowDownload: false,
          environment: <String, String>{
            'CODEINT_BIN': realBinary,
            'PATH': '/nonexistent',
          },
        ),
      );
      expect(resolved, equals(realBinary));
    });

    test('step 3 matches exact version on PATH and skips mismatched version',
        () async {
      if (Platform.isWindows) {
        return;
      }
      final wrongDir = await createTempDir('wrong_path');
      final rightDir = await createTempDir('right_path');
      addTearDown(() async {
        await wrongDir.delete(recursive: true);
        await rightDir.delete(recursive: true);
      });

      final wrongBin = File('${wrongDir.path}/codeint');
      await wrongBin.writeAsString('#!/bin/sh\necho "codeint 0.1.0"\n');
      await Process.run('chmod', <String>['0755', wrongBin.path]);

      final rightBin = File('${rightDir.path}/codeint');
      await rightBin.writeAsString('#!/bin/sh\necho "codeint $codeintVersion"\n');
      await Process.run('chmod', <String>['0755', rightBin.path]);

      final resolved = await ensureBinary(
        BinaryResolverOptions(
          allowDownload: false,
          environment: <String, String>{
            'PATH': '${wrongDir.path}:${rightDir.path}',
          },
        ),
      );
      expect(resolved, equals(rightBin.absolute.path));
    });

    test('step 4 downloads, verifies SHA-256, extracts, and reuses cache',
        () async {
      final target = detectReleaseTarget();
      final payload = await File(realBinary).readAsBytes();
      final archiveBytes = buildArchiveBytes(
        archiveExtension: target.archiveExtension,
        binaryName: target.binaryName,
        binaryBytes: payload,
      );
      final server = await MockReleaseServer.start(
        buildMockReleaseAssets(
          archiveFileName: target.archiveFileName,
          archiveBytes: archiveBytes,
        ),
      );
      final cacheDir = await createTempDir('cache');
      addTearDown(() async {
        await server.close();
        await cacheDir.delete(recursive: true);
      });

      final options = BinaryResolverOptions(
        releaseBaseUrl: server.baseUrl,
        cacheDir: cacheDir.path,
        environment: const <String, String>{'PATH': ''},
      );

      final firstPath = await ensureBinary(options);
      expect(File(firstPath).existsSync(), isTrue);
      expect(server.requestCount, equals(2));

      final versionCheck = await Process.run(firstPath, const <String>['--version']);
      expect(versionCheck.exitCode, equals(0));
      expect(versionCheck.stdout.toString().trim(), contains(codeintVersion));

      final secondPath = await ensureBinary(options);
      expect(secondPath, equals(firstPath));
      expect(server.requestCount, equals(2));
    });

    test('step 4 rejects tampered archive and missing checksum entry', () async {
      final target = detectReleaseTarget();
      final archiveBytes = buildArchiveBytes(
        archiveExtension: target.archiveExtension,
        binaryName: target.binaryName,
        binaryBytes: Uint8List.fromList(<int>[1, 2, 3, 4]),
      );

      final badHashServer = await MockReleaseServer.start(
        buildMockReleaseAssets(
          archiveFileName: target.archiveFileName,
          archiveBytes: archiveBytes,
          checksumOverride: 'a' * 64,
        ),
      );
      final missingEntryServer = await MockReleaseServer.start(
        buildMockReleaseAssets(
          archiveFileName: target.archiveFileName,
          archiveBytes: archiveBytes,
          omitChecksumEntry: true,
        ),
      );
      final cacheDir = await createTempDir('reject_cache');
      addTearDown(() async {
        await badHashServer.close();
        await missingEntryServer.close();
        await cacheDir.delete(recursive: true);
      });

      await _expectEnsureBinaryError(
        baseUrl: badHashServer.baseUrl,
        cacheDir: cacheDir.path,
        messageContains: 'checksum mismatch',
      );

      await _expectEnsureBinaryError(
        baseUrl: missingEntryServer.baseUrl,
        cacheDir: cacheDir.path,
        messageContains: 'No SHA-256 checksum entry',
      );
    });

    test('step 4 respects allowDownload: false with zero HTTP requests',
        () async {
      final server = await MockReleaseServer.start(const <String, Uint8List>{});
      final cacheDir = await createTempDir('offline_cache');
      addTearDown(() async {
        await server.close();
        await cacheDir.delete(recursive: true);
      });

      await _expectEnsureBinaryError(
        allowDownload: false,
        baseUrl: server.baseUrl,
        cacheDir: cacheDir.path,
        messageContains: 'allowDownload: false',
      );
      expect(server.requestCount, equals(0));
    });

    test('archive extraction rejects zip symlinks and traversal entries',
        () async {
      final symlinkZipArchive = Archive()
        ..addFile(ArchiveFile.symlink('codeint', '/etc/passwd'))
        ..addFile(ArchiveFile.symlink('codeint.exe', '/etc/passwd'));
      final symlinkZipBytes =
          Uint8List.fromList(ZipEncoder().encode(symlinkZipArchive));

      for (final name in const <String>['codeint', 'codeint.exe']) {
        expect(
          () => extractBinaryFromArchive(symlinkZipBytes, 'zip', name),
          throwsA(
            isA<CodeintError>().having(
              (e) => e.message,
              'message',
              contains('disallowed link entry'),
            ),
          ),
        );
      }

      final traversalBytes = buildArchiveBytes(
        archiveExtension: 'zip',
        binaryName: '../../evil/codeint',
        binaryBytes: Uint8List.fromList(<int>[1, 2, 3, 4]),
      );
      expect(
        () => extractBinaryFromArchive(traversalBytes, 'zip', 'codeint'),
        throwsA(
          isA<CodeintError>().having(
            (e) => e.message,
            'message',
            contains('unsafe path'),
          ),
        ),
      );

      final target = detectReleaseTarget();
      final traversalReleaseArchive = buildArchiveBytes(
        archiveExtension: target.archiveExtension,
        binaryName: '../../evil/${target.binaryName}',
        binaryBytes: Uint8List.fromList(<int>[1, 2, 3, 4]),
      );
      final server = await MockReleaseServer.start(
        buildMockReleaseAssets(
          archiveFileName: target.archiveFileName,
          archiveBytes: traversalReleaseArchive,
        ),
      );
      final cacheDir = await createTempDir('traversal_cache');
      addTearDown(() async {
        await server.close();
        await cacheDir.delete(recursive: true);
      });

      await _expectEnsureBinaryError(
        baseUrl: server.baseUrl,
        cacheDir: cacheDir.path,
        messageContains: 'unsafe path',
      );
    });
  });

  group('bin executables', () {
    test('bin/codeint.dart forwards args and exit code, bin/install.dart prints path',
        () async {
      final env = <String, String>{
        ...Platform.environment,
        'CODEINT_BIN': realBinary,
      };
      final versionResult = await Process.run(
        Platform.resolvedExecutable,
        const <String>['run', 'bin/codeint.dart', '--version'],
        environment: env,
      );
      expect(versionResult.exitCode, equals(0));
      expect(versionResult.stdout.toString().trim(), contains(codeintVersion));

      final installResult = await Process.run(
        Platform.resolvedExecutable,
        const <String>['run', 'bin/install.dart'],
        environment: env,
      );
      expect(installResult.exitCode, equals(0));
      expect(installResult.stdout.toString().trim(), equals(realBinary));
    });
  });
}

Future<void> _expectEnsureBinaryError({
  required String baseUrl,
  required String cacheDir,
  required String messageContains,
  bool allowDownload = true,
}) =>
    expectLater(
      ensureBinary(
        BinaryResolverOptions(
          allowDownload: allowDownload,
          releaseBaseUrl: baseUrl,
          cacheDir: cacheDir,
          environment: const <String, String>{'PATH': ''},
        ),
      ),
      throwsA(
        isA<CodeintError>().having(
          (e) => e.message,
          'message',
          contains(messageContains),
        ),
      ),
    );
