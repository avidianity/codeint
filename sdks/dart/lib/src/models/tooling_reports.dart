import '../json_parsing.dart';

class InitResult {
  const InitResult({
    required this.configPath,
    this.adopted,
    required this.stdout,
  });

  final String configPath;
  final AdoptedFiles? adopted;
  final String stdout;
}

class AdoptedFiles {
  const AdoptedFiles({
    required this.configuration,
    required this.ratchet,
    required this.ratchetEntries,
    required this.baseline,
    required this.baselinedFindings,
  });

  final String configuration;
  final String ratchet;
  final int ratchetEntries;
  final String baseline;
  final int baselinedFindings;
}

typedef CliArgumentSchema = ({
  String name,
  String valueName,
  bool required,
  String? description,
});
typedef CliFlagSchema = ({
  String? short,
  String? long,
  String? valueName,
  bool takesValue,
  bool required,
  String? defaultValue,
  List<String> possibleValues,
  String? description,
});
typedef MetricSchema = ({
  String key,
  String name,
  int metricVersion,
  String shortDescription,
  String description,
  String unit,
  double? defaultRatioRiseLimit,
  String? defaultNewFindingsRule,
});
typedef LanguageSchemaEntry = ({
  String name,
  List<String> extensions,
  List<String> hazardRules,
  List<String> frameworkProfiles,
});
typedef OutputFormatSchema = ({String name, List<String> commands});
typedef McpToolSchema = ({
  String name,
  String description,
  Map<String, Object?> inputSchema,
});
typedef ExitCodeSchema = ({int code, String name, String description});
typedef ResolvedSetting = ({String key, Object? value, String source});
typedef ConfigReport = ({
  String configFile,
  bool configFilePresent,
  List<ResolvedSetting> settings,
});
typedef DoctorSummary = ({int passed, int warnings, int errors});
typedef DoctorCheck = ({
  String id,
  String status,
  String summary,
  String? detail,
  String? remedy,
});
typedef DoctorReport = ({
  String version,
  String path,
  String status,
  DoctorSummary summary,
  List<DoctorCheck> checks,
});
typedef AgentStatusEntry = ({
  String client,
  String name,
  String configPath,
  bool detected,
  bool configExists,
  bool registered,
});
typedef AgentStatusReport = ({
  String configRoot,
  List<AgentStatusEntry> agents,
});
typedef AgentInstallEntry = ({
  String client,
  String name,
  String configPath,
  String action,
  bool written,
  String? preview,
});
typedef AgentInstallReport = ({
  String configRoot,
  bool dryRun,
  List<AgentInstallEntry> results,
  String? note,
});
typedef RatchetUpdateReport = ({
  String path,
  bool created,
  bool reset,
  bool? replacedInvalid,
  int tightened,
  int loosened,
  int refreshed,
  int added,
  int unchanged,
  int removed,
  int totalEntries,
});
typedef BaselineUpdateReport = ({
  String path,
  bool created,
  bool reset,
  bool? replacedInvalid,
  int added,
  int removed,
  int refreshed,
  int skippedNew,
  int totalFindings,
});
typedef PluginTestSummary = ({int passed, int failed, int skipped});
typedef PluginTestCheck = ({
  String id,
  String status,
  String message,
  String? hint,
});
typedef PluginTestReport = ({
  String plugin,
  String kind,
  bool passed,
  PluginTestSummary summary,
  List<PluginTestCheck> checks,
});

class CliCommandSchema extends JsonModel {
  const CliCommandSchema.fromJson(super.json);

  String get name => str('name');
  String? get description => optStr('description');
  List<CliArgumentSchema> get arguments => list(
        'arguments',
        (m) => (
          name: readString(m, 'name'),
          valueName: readString(m, 'value_name'),
          required: readBool(m, 'required'),
          description: readOptionalString(m, 'description'),
        ),
      );
  List<CliFlagSchema> get flags => list(
        'flags',
        (m) => (
          short: readOptionalString(m, 'short'),
          long: readOptionalString(m, 'long'),
          valueName: readOptionalString(m, 'value_name'),
          takesValue: readBool(m, 'takes_value'),
          required: readBool(m, 'required'),
          defaultValue: readOptionalString(m, 'default_value'),
          possibleValues: readOptionalStringList(m, 'possible_values'),
          description: readOptionalString(m, 'description'),
        ),
      );
  List<CliCommandSchema> get subcommands =>
      optList('subcommands', CliCommandSchema.fromJson);
}

class SchemaReport extends JsonModel {
  const SchemaReport.fromJson(super.json);

  String get name => str('name');
  String get version => str('version');
  String get description => str('description');
  List<CliCommandSchema> get commands =>
      list('commands', CliCommandSchema.fromJson);
  List<MetricSchema> get metrics => list(
        'metrics',
        (m) => (
          key: readString(m, 'key'),
          name: readString(m, 'name'),
          metricVersion: readInt(m, 'metric_version'),
          shortDescription: readString(m, 'short_description'),
          description: readString(m, 'description'),
          unit: readString(m, 'unit'),
          defaultRatioRiseLimit:
              readOptionalDouble(m, 'default_ratio_rise_limit'),
          defaultNewFindingsRule:
              readOptionalString(m, 'default_new_findings_rule'),
        ),
      );
  List<String> get configurationKeys => strs('configuration_keys');
  List<String> get sourceSets => strs('source_sets');
  List<LanguageSchemaEntry> get languages => list(
        'languages',
        (m) => (
          name: readString(m, 'name'),
          extensions: readStringList(m, 'extensions'),
          hazardRules: readOptionalStringList(m, 'hazard_rules'),
          frameworkProfiles: readOptionalStringList(m, 'framework_profiles'),
        ),
      );
  List<OutputFormatSchema> get outputFormats => list(
        'output_formats',
        (m) => (
          name: readString(m, 'name'),
          commands: readStringList(m, 'commands'),
        ),
      );
  List<McpToolSchema> get mcpTools => list(
        'mcp_tools',
        (m) => (
          name: readString(m, 'name'),
          description: readString(m, 'description'),
          inputSchema: requireJsonMap(m['inputSchema'], 'inputSchema'),
        ),
      );
  List<ExitCodeSchema> get exitCodes => list(
        'exit_codes',
        (m) => (
          code: readInt(m, 'code'),
          name: readString(m, 'name'),
          description: readString(m, 'description'),
        ),
      );
}

ConfigReport parseConfigReport(Map<String, Object?> j) => (
      configFile: readString(j, 'config_file'),
      configFilePresent: readBool(j, 'config_file_present'),
      settings: readObjectList(
        j,
        'settings',
        (m) => (
          key: readString(m, 'key'),
          value: m['value'],
          source: readString(m, 'source'),
        ),
      ),
    );

DoctorReport parseDoctorReport(Map<String, Object?> j) => (
      version: readString(j, 'version'),
      path: readString(j, 'path'),
      status: readString(j, 'status'),
      summary: _parseDoctorSummary(requireJsonMap(j['summary'], 'summary')),
      checks: readObjectList(j, 'checks', _parseDoctorCheck),
    );

DoctorSummary _parseDoctorSummary(Map<String, Object?> m) => (
      passed: readInt(m, 'passed'),
      warnings: readInt(m, 'warnings'),
      errors: readInt(m, 'errors'),
    );

DoctorCheck _parseDoctorCheck(Map<String, Object?> m) => (
      id: readString(m, 'id'),
      status: readString(m, 'status'),
      summary: readString(m, 'summary'),
      detail: readOptionalString(m, 'detail'),
      remedy: readOptionalString(m, 'remedy'),
    );

AgentStatusReport parseAgentStatusReport(Map<String, Object?> j) => (
      configRoot: readString(j, 'config_root'),
      agents: readObjectList(
        j,
        'agents',
        (m) => (
          client: readString(m, 'client'),
          name: readString(m, 'name'),
          configPath: readString(m, 'config_path'),
          detected: readBool(m, 'detected'),
          configExists: readBool(m, 'config_exists'),
          registered: readBool(m, 'registered'),
        ),
      ),
    );

AgentInstallReport parseAgentInstallReport(Map<String, Object?> j) => (
      configRoot: readString(j, 'config_root'),
      dryRun: readBool(j, 'dry_run'),
      note: readOptionalString(j, 'note'),
      results: readObjectList(
        j,
        'results',
        (m) => (
          client: readString(m, 'client'),
          name: readString(m, 'name'),
          configPath: readString(m, 'config_path'),
          action: readString(m, 'action'),
          written: readBool(m, 'written'),
          preview: readOptionalString(m, 'preview'),
        ),
      ),
    );

RatchetUpdateReport parseRatchetUpdateReport(Map<String, Object?> j) => (
      path: readString(j, 'path'),
      created: readBool(j, 'created'),
      reset: readBool(j, 'reset'),
      replacedInvalid: readOptionalBool(j, 'replaced_invalid'),
      tightened: readInt(j, 'tightened'),
      loosened: readInt(j, 'loosened'),
      refreshed: readInt(j, 'refreshed'),
      added: readInt(j, 'added'),
      unchanged: readInt(j, 'unchanged'),
      removed: readInt(j, 'removed'),
      totalEntries: readInt(j, 'total_entries'),
    );

BaselineUpdateReport parseBaselineUpdateReport(Map<String, Object?> j) => (
      path: readString(j, 'path'),
      created: readBool(j, 'created'),
      reset: readBool(j, 'reset'),
      replacedInvalid: readOptionalBool(j, 'replaced_invalid'),
      added: readInt(j, 'added'),
      removed: readInt(j, 'removed'),
      refreshed: readInt(j, 'refreshed'),
      skippedNew: readInt(j, 'skipped_new'),
      totalFindings: readInt(j, 'total_findings'),
    );

PluginTestReport parsePluginTestReport(Map<String, Object?> j) {
  final s = requireJsonMap(j['summary'], 'summary');
  return (
    plugin: readString(j, 'plugin'),
    kind: readString(j, 'kind'),
    passed: readBool(j, 'passed'),
    summary: (
      passed: readInt(s, 'passed'),
      failed: readInt(s, 'failed'),
      skipped: readInt(s, 'skipped'),
    ),
    checks: readObjectList(j, 'checks', _parsePluginTestCheck),
  );
}

PluginTestCheck _parsePluginTestCheck(Map<String, Object?> m) => (
      id: readString(m, 'id'),
      status: readString(m, 'status'),
      message: readString(m, 'message'),
      hint: readOptionalString(m, 'hint'),
    );
