enum InitFormat {
  jsonc('jsonc'),
  json('json'),
  toml('toml'),
  yaml('yaml');

  const InitFormat(this.cliValue);

  final String cliValue;
}

enum AgentClient {
  claudeCode('claude-code'),
  cursor('cursor'),
  codex('codex');

  const AgentClient(this.cliValue);

  final String cliValue;
}

enum PluginKind {
  language('language'),
  metric('metric');

  const PluginKind(this.cliValue);

  final String cliValue;
}

enum ReportRenderFormat {
  toon('toon'),
  text('text'),
  json('json'),
  sarif('sarif'),
  markdown('markdown');

  const ReportRenderFormat(this.cliValue);

  final String cliValue;
}
