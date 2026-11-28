# codeint

Official pure Dart client SDK and CLI runner for [codeint](https://github.com/avidianity/codeint).

`codeint` measures the Software entropy of a codebase and reports Hazards next to Software entropy across TypeScript, JavaScript, Rust, PHP, Dart, Python, Go, SQL, and Shell.

## Installation

Add `codeint` to your `pubspec.yaml`:

```yaml
dev_dependencies:
  codeint: ^0.9.0
```

Pre-warm the verified native binary cache if desired:

```sh
dart run codeint:install
```

Run the CLI directly through Dart:

```sh
dart run codeint check
```

## Programmatic usage

```dart
import 'package:codeint/codeint.dart';

Future<void> main() async {
  final client = CodeintClient(cwd: '.');
  final result = await client.check();
  if (!result.passed) {
    throw StateError('codeint failed: ${result.failures.join(', ')}');
  }
  final production = result.report.sourceSets['production'];
  print('Erosion ratio: ${production?.erosion.ratio}');
}
```

## Binary resolution

`ensureBinary()` resolves the native `codeint` binary in four deterministic steps:

1. Explicit `binaryPath` option.
2. `CODEINT_BIN` environment variable.
3. Version-matching `codeint` executable on `PATH`.
4. Version-pinned GitHub Release archive verified against `SHA256SUMS` before extraction and stored in the local user cache.

Set `allowDownload: false` in offline CI environments to fail fast without network access when no local binary is available.
