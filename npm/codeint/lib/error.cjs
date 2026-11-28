"use strict";

class CodeintError extends Error {
  constructor(message, exitCode, stderr, options) {
    super(message, options);
    this.name = "CodeintError";
    this.exitCode = exitCode;
    this.stderr = stderr;
  }
}

module.exports = { CodeintError };
