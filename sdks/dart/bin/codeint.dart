import 'dart:io';

import 'package:codeint/codeint.dart';

Future<void> main(List<String> arguments) async {
  try {
    final binary = await ensureBinary();
    final process = await Process.start(
      binary,
      arguments,
      mode: ProcessStartMode.inheritStdio,
    );
    exitCode = await process.exitCode;
  } on CodeintError catch (error) {
    stderr.writeln(error.message);
    exitCode = error.exitCode ?? 2;
  }
}
