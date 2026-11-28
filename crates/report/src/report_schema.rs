use serde_json::{Value, json};

mod validate;

pub use validate::{validate_json_against_schema, validate_report_json};

const REPORT_SCHEMA_DEFINITIONS: &str = include_str!("report_schema/definitions.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReportSchemaKind {
    Check,
    Diff,
    Trend,
    Impact,
    Trace,
    Guard,
    Schema,
    Config,
    Doctor,
    AgentStatus,
    AgentInstall,
    Churn,
    Coupling,
    RatchetUpdate,
    BaselineUpdate,
    PluginTest,
}

struct ReportSchemaSpec {
    name: &'static str,
    relative_path: &'static str,
}

const REPORT_SCHEMA_SPECS: [ReportSchemaSpec; 16] = [
    ReportSchemaSpec {
        name: "check",
        relative_path: "schema/codeint-report.schema.json",
    },
    ReportSchemaSpec {
        name: "diff",
        relative_path: "schema/reports/diff.schema.json",
    },
    ReportSchemaSpec {
        name: "trend",
        relative_path: "schema/reports/trend.schema.json",
    },
    ReportSchemaSpec {
        name: "impact",
        relative_path: "schema/reports/impact.schema.json",
    },
    ReportSchemaSpec {
        name: "trace",
        relative_path: "schema/reports/trace.schema.json",
    },
    ReportSchemaSpec {
        name: "guard",
        relative_path: "schema/reports/guard.schema.json",
    },
    ReportSchemaSpec {
        name: "schema",
        relative_path: "schema/reports/schema.schema.json",
    },
    ReportSchemaSpec {
        name: "config",
        relative_path: "schema/reports/config.schema.json",
    },
    ReportSchemaSpec {
        name: "doctor",
        relative_path: "schema/reports/doctor.schema.json",
    },
    ReportSchemaSpec {
        name: "agent-status",
        relative_path: "schema/reports/agent-status.schema.json",
    },
    ReportSchemaSpec {
        name: "agent-install",
        relative_path: "schema/reports/agent-install.schema.json",
    },
    ReportSchemaSpec {
        name: "churn",
        relative_path: "schema/reports/churn.schema.json",
    },
    ReportSchemaSpec {
        name: "coupling",
        relative_path: "schema/reports/coupling.schema.json",
    },
    ReportSchemaSpec {
        name: "ratchet-update",
        relative_path: "schema/reports/ratchet-update.schema.json",
    },
    ReportSchemaSpec {
        name: "baseline-update",
        relative_path: "schema/reports/baseline-update.schema.json",
    },
    ReportSchemaSpec {
        name: "plugin-test",
        relative_path: "schema/reports/plugin-test.schema.json",
    },
];

impl ReportSchemaKind {
    pub const ALL: [ReportSchemaKind; 16] = [
        ReportSchemaKind::Check,
        ReportSchemaKind::Diff,
        ReportSchemaKind::Trend,
        ReportSchemaKind::Impact,
        ReportSchemaKind::Trace,
        ReportSchemaKind::Guard,
        ReportSchemaKind::Schema,
        ReportSchemaKind::Config,
        ReportSchemaKind::Doctor,
        ReportSchemaKind::AgentStatus,
        ReportSchemaKind::AgentInstall,
        ReportSchemaKind::Churn,
        ReportSchemaKind::Coupling,
        ReportSchemaKind::RatchetUpdate,
        ReportSchemaKind::BaselineUpdate,
        ReportSchemaKind::PluginTest,
    ];

    pub const fn name(self) -> &'static str {
        REPORT_SCHEMA_SPECS[self as usize].name
    }

    pub const fn relative_path(self) -> &'static str {
        REPORT_SCHEMA_SPECS[self as usize].relative_path
    }
}

pub fn report_schema() -> String {
    schema_for(ReportSchemaKind::Check)
}

pub fn schema_for(kind: ReportSchemaKind) -> String {
    let schema = schema_value_for(kind);
    format!("{schema:#}\n")
}

pub(crate) fn schema_value_for(kind: ReportSchemaKind) -> Value {
    if kind == ReportSchemaKind::Check {
        return report_schema_value();
    }
    let definitions: Value = serde_json::from_str(REPORT_SCHEMA_DEFINITIONS).unwrap_or(Value::Null);
    definitions.get(kind.name()).cloned().unwrap_or(Value::Null)
}

pub fn all_report_schemas() -> Vec<(ReportSchemaKind, String)> {
    ReportSchemaKind::ALL
        .into_iter()
        .map(|kind| (kind, schema_for(kind)))
        .collect()
}

fn report_schema_value() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "codeint check JSON report",
        "description": "The JSON output of `codeint check --format json`.",
        "type": "object",
        "required": ["files", "source_lines", "source_sets"],
        "properties": {
            "files": { "type": "integer", "minimum": 0 },
            "source_lines": { "type": "integer", "minimum": 0 },
            "framework_profiles": {
                "type": "array",
                "description": "Sorted names of active Framework profiles for this measured scope.",
                "items": { "type": "string" }
            },
            "reference_range_note": {
                "type": "string",
                "description": "Note explaining Framework profile Reference range selection or fallback to base language Reference ranges."
            },
            "import_metrics_note": { "type": "string" },
            "source_sets": { "$ref": "#/$defs/SourceSetSections" },
            "test_gap_note": {
                "type": "string",
                "description": "One-line explanation shown when Test gap is absent for the Production Source set."
            },
            "coverage_unmatched_note": {
                "type": "string",
                "description": "One-line note listing every Coverage report path that matched no repository file unambiguously."
            },
            "ratchet_note": {
                "type": "string",
                "description": "One-line note shown when Ratchet enforcement is skipped, such as on --package runs."
            },
            "manifest_warnings": {
                "type": "array",
                "description": "One warning for each invalid manifest that codeint skipped; files under a skipped manifest count toward the surrounding Workspace package.",
                "items": { "type": "string" }
            },
            "baseline": {
                "type": "object",
                "description": "The Baseline in effect; present when .codeint/baseline.json exists and codeint check runs without --ignore-baseline.",
                "required": ["path", "hidden_findings"],
                "properties": {
                    "path": { "type": "string" },
                    "hidden_findings": { "type": "integer", "minimum": 0 }
                },
                "additionalProperties": false
            },
            "dependencies": { "$ref": "#/$defs/DependenciesSection" },
            "languages": {
                "type": "object",
                "additionalProperties": { "$ref": "#/$defs/MeasuredSection" }
            },
            "skipped": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["path", "reason"],
                    "properties": {
                        "path": { "type": "string" },
                        "reason": { "type": "string" }
                    }
                }
            },
            "file_diagnostics": {
                "type": "array",
                "description": "Non-fatal per-file diagnostics emitted while lowering source files.",
                "items": { "$ref": "#/$defs/FileDiagnosticEntry" }
            },
            "scope_summaries": {
                "type": "array",
                "description": "Language-neutral summaries emitted by cross-file scope analysis per Workspace package and Dialect.",
                "items": { "$ref": "#/$defs/ScopeSummaryEntry" }
            },
            "declaration_only": { "type": "array", "items": { "type": "string" } },
            "submodules": { "type": "array", "items": { "type": "string" } },
            "over_budget": {
                "type": "array",
                "items": { "$ref": "#/$defs/BudgetFailure" }
            },
            "over_ratchet": {
                "type": "array",
                "items": { "$ref": "#/$defs/RatchetFailure" }
            }
        },
        "$defs": {
            "MeasuredSection": {
                "type": "object",
                "required": ["files", "source_lines", "source_sets"],
                "properties": {
                    "files": { "type": "integer", "minimum": 0 },
                    "source_lines": { "type": "integer", "minimum": 0 },
                    "framework_profiles": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "reference_range_note": { "type": "string" },
                    "import_metrics_note": { "type": "string" },
                    "source_sets": { "$ref": "#/$defs/SourceSetSections" },
                    "test_gap_note": { "type": "string" },
                    "dependencies": { "$ref": "#/$defs/DependenciesSection" }
                }
            },
            "VersionedMetricSection": {
                "type": "object",
                "required": ["ratio", "metric_version"],
                "properties": {
                    "ratio": { "type": "number", "minimum": 0, "maximum": 1 },
                    "metric_version": { "type": "integer", "minimum": 1 },
                    "baselined_findings": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Findings of this Metric hidden by the Baseline."
                    }
                }
            },
            "DependenciesSection": {
                "type": "object",
                "required": [
                    "unused_dependencies",
                    "unlisted_dependencies",
                    "dev_in_production_dependencies"
                ],
                "properties": {
                    "unused_dependencies": { "$ref": "#/$defs/VersionedMetricSection" },
                    "unlisted_dependencies": { "$ref": "#/$defs/VersionedMetricSection" },
                    "dev_in_production_dependencies": { "$ref": "#/$defs/VersionedMetricSection" }
                },
                "additionalProperties": false
            },
            "SourceSetSections": {
                "type": "object",
                "properties": {
                    "production": { "$ref": "#/$defs/SourceSetSection" },
                    "test": { "$ref": "#/$defs/NonProductionSourceSetSection" },
                    "generated": { "$ref": "#/$defs/NonProductionSourceSetSection" },
                    "vendored": { "$ref": "#/$defs/NonProductionSourceSetSection" }
                },
                "additionalProperties": false
            },
            "SourceSetSection": {
                "type": "object",
                "required": ["files", "source_lines", "erosion", "cognitive_complexity", "duplication", "verbosity"],
                "properties": {
                    "files": { "type": "integer", "minimum": 0 },
                    "source_lines": { "type": "integer", "minimum": 0 },
                    "erosion": { "$ref": "#/$defs/VersionedMetricSection" },
                    "cognitive_complexity": { "$ref": "#/$defs/VersionedMetricSection" },
                    "duplication": { "$ref": "#/$defs/VersionedMetricSection" },
                    "verbosity": { "$ref": "#/$defs/VersionedMetricSection" },
                    "import_cycles": { "$ref": "#/$defs/VersionedMetricSection" },
                    "boundaries": { "$ref": "#/$defs/BoundariesSection" },
                    "orphan_files": { "$ref": "#/$defs/VersionedMetricSection" },
                    "test_gap": { "$ref": "#/$defs/TestGapSection" },
                    "hazards": { "$ref": "#/$defs/HazardsSection" },
                    "packages": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/PackageSection" }
                    }
                },
                "additionalProperties": { "$ref": "#/$defs/VersionedMetricSection" }
            },
            "NonProductionSourceSetSection": {
                "type": "object",
                "required": ["files", "source_lines", "erosion", "cognitive_complexity", "duplication", "verbosity"],
                "properties": {
                    "files": { "type": "integer", "minimum": 0 },
                    "source_lines": { "type": "integer", "minimum": 0 },
                    "erosion": { "$ref": "#/$defs/VersionedMetricSection" },
                    "cognitive_complexity": { "$ref": "#/$defs/VersionedMetricSection" },
                    "duplication": { "$ref": "#/$defs/VersionedMetricSection" },
                    "verbosity": { "$ref": "#/$defs/VersionedMetricSection" },
                    "import_cycles": { "$ref": "#/$defs/VersionedMetricSection" },
                    "boundaries": { "$ref": "#/$defs/BoundariesSection" },
                    "test_gap": false,
                    "hazards": { "$ref": "#/$defs/HazardsSection" },
                    "packages": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/PackageSection" }
                    }
                },
                "additionalProperties": { "$ref": "#/$defs/VersionedMetricSection" }
            },
            "BoundariesSection": {
                "type": "object",
                "description": "Import boundaries for this Source set; omitted when boundaries are not configured or do not apply to this Source set.",
                "required": ["ratio", "metric_version", "resolved_imports", "findings"],
                "properties": {
                    "ratio": { "type": "number", "minimum": 0, "maximum": 1 },
                    "metric_version": { "type": "integer", "minimum": 1 },
                    "resolved_imports": { "type": "integer", "minimum": 0 },
                    "findings": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/BoundaryFinding" }
                    },
                    "findings_left_out": { "type": "integer", "minimum": 1 },
                    "baselined_findings": { "type": "integer", "minimum": 1 }
                },
                "additionalProperties": false
            },
            "BoundaryFinding": {
                "type": "object",
                "required": ["id", "kind", "importer", "message"],
                "properties": {
                    "id": { "type": "string" },
                    "kind": { "type": "string", "enum": ["zone_rule", "external_package", "public_files", "unzoned_file"] },
                    "importer": { "type": "string" },
                    "target": { "type": "string" },
                    "external_package": { "type": "string" },
                    "specifier": { "type": "string" },
                    "source_zone": { "type": "string" },
                    "target_zone": { "type": "string" },
                    "reason": { "type": "string" },
                    "message": { "type": "string" },
                    "owners": { "type": "array", "items": { "type": "string" } }
                },
                "additionalProperties": false
            },
            "TestGapSection": {
                "type": "object",
                "description": "Test gap for the Production Source set; omitted when no Coverage report applies.",
                "required": ["ratio", "metric_version", "untested_function_mass", "total_function_mass", "measured_functions", "findings"],
                "properties": {
                    "ratio": { "type": "number", "minimum": 0, "maximum": 1 },
                    "metric_version": { "type": "integer", "minimum": 1 },
                    "untested_function_mass": { "type": "number", "minimum": 0 },
                    "total_function_mass": { "type": "number", "minimum": 0 },
                    "measured_functions": { "type": "integer", "minimum": 0 },
                    "findings": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/TestGapFinding" }
                    },
                    "findings_left_out": { "type": "integer", "minimum": 1 },
                    "baselined_findings": { "type": "integer", "minimum": 1 }
                },
                "additionalProperties": false
            },
            "TestGapFinding": {
                "type": "object",
                "required": ["id", "path", "line", "end_line", "function_mass", "untested_share"],
                "properties": {
                    "id": { "type": "string" },
                    "path": { "type": "string" },
                    "line": { "type": "integer", "minimum": 1 },
                    "end_line": { "type": "integer", "minimum": 1 },
                    "name": { "type": "string" },
                    "function_mass": { "type": "number", "minimum": 0 },
                    "untested_share": { "type": "number", "minimum": 0, "maximum": 1 },
                    "owners": { "type": "array", "items": { "type": "string" } }
                },
                "additionalProperties": false
            },
            "PackageSection": {
                "type": "object",
                "required": ["path", "files", "source_lines", "erosion", "cognitive_complexity", "duplication", "verbosity"],
                "properties": {
                    "path": { "type": "string" },
                    "files": { "type": "integer", "minimum": 0 },
                    "source_lines": { "type": "integer", "minimum": 0 },
                    "framework_profiles": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "erosion": { "type": "object" },
                    "cognitive_complexity": { "type": "object" },
                    "duplication": { "type": "object" },
                    "verbosity": { "type": "object" },
                    "import_cycles": { "type": "object" },
                    "test_gap": { "$ref": "#/$defs/PackageTestGap" },
                    "hazards": { "$ref": "#/$defs/PackageHazards" }
                },
                "additionalProperties": false
            },
            "PackageTestGap": {
                "type": "object",
                "required": ["ratio", "untested_function_mass", "total_function_mass"],
                "properties": {
                    "ratio": { "type": "number", "minimum": 0, "maximum": 1 },
                    "untested_function_mass": { "type": "number", "minimum": 0 },
                    "total_function_mass": { "type": "number", "minimum": 0 }
                },
                "additionalProperties": false
            },
            "HazardsSection": {
                "type": "object",
                "description": "Hazards for this Source set; omitted when no measured language has Hazard line rules.",
                "required": ["ratio", "metric_version", "hazard_lines", "source_lines", "findings"],
                "properties": {
                    "ratio": { "type": "number", "minimum": 0, "maximum": 1 },
                    "metric_version": { "type": "integer", "minimum": 1 },
                    "hazard_lines": { "type": "integer", "minimum": 0 },
                    "source_lines": { "type": "integer", "minimum": 0 },
                    "findings": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/HazardLineFinding" }
                    },
                    "findings_left_out": { "type": "integer", "minimum": 1 },
                    "baselined_findings": { "type": "integer", "minimum": 1 }
                },
                "additionalProperties": false
            },
            "HazardLineFinding": {
                "type": "object",
                "required": ["id", "path", "line", "rule"],
                "properties": {
                    "id": { "type": "string" },
                    "path": { "type": "string" },
                    "line": { "type": "integer", "minimum": 1 },
                    "rule": { "type": "string" },
                    "owners": { "type": "array", "items": { "type": "string" } }
                },
                "additionalProperties": false
            },
            "PackageHazards": {
                "type": "object",
                "required": ["ratio", "hazard_lines", "source_lines"],
                "properties": {
                    "ratio": { "type": "number", "minimum": 0, "maximum": 1 },
                    "hazard_lines": { "type": "integer", "minimum": 0 },
                    "source_lines": { "type": "integer", "minimum": 0 }
                },
                "additionalProperties": false
            },
            "FileDiagnosticEntry": {
                "type": "object",
                "required": ["path", "line", "kind", "message"],
                "properties": {
                    "path": { "type": "string" },
                    "line": { "type": "integer", "minimum": 1 },
                    "kind": { "type": "string" },
                    "message": { "type": "string" }
                },
                "additionalProperties": false
            },
            "ScopeSummaryEntry": {
                "type": "object",
                "required": ["language", "kind"],
                "properties": {
                    "language": { "type": "string" },
                    "package": { "type": "string" },
                    "dialect": { "type": "string" },
                    "kind": { "type": "string" },
                    "fields": {
                        "type": "object",
                        "additionalProperties": { "type": "string" }
                    }
                },
                "additionalProperties": false
            },
            "BudgetFailure": {
                "type": "object",
                "required": ["metric", "ratio", "budget"],
                "properties": {
                    "source_set": { "type": "string" },
                    "metric": { "type": "string" },
                    "ratio": { "type": "number", "minimum": 0, "maximum": 1 },
                    "budget": { "type": "number", "minimum": 0, "maximum": 1 },
                    "non_baselined": {
                        "type": "boolean",
                        "description": "True when ratio counts only Findings that the Baseline does not hide."
                    }
                },
                "additionalProperties": false
            },
            "RatchetFailure": {
                "type": "object",
                "required": ["metric", "ratio", "ceiling"],
                "properties": {
                    "source_set": { "type": "string" },
                    "metric": { "type": "string" },
                    "ratio": { "type": "number", "minimum": 0, "maximum": 1 },
                    "ceiling": { "type": "number", "minimum": 0, "maximum": 1 }
                },
                "additionalProperties": false
            }
        }
    })
}
