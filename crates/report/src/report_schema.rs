use serde_json::Value;

mod validate;

pub use validate::{validate_json_against_schema, validate_report_json};

const CHECK_REPORT_SCHEMA: &str = include_str!("report_schema/check.json");
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
        return serde_json::from_str(CHECK_REPORT_SCHEMA).unwrap_or(Value::Null);
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
