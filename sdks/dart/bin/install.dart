import 'dart:io';

import 'package:codeint/codeint.dart';

Future<void> main() async {
  try {
    final binary = await ensureBinary();
    stdout.writeln(binary);
  } on CodeintError catch (error) {
    stderr.writeln(error.message);
    exitCode = error.exitCode ?? 2;
  }
}
