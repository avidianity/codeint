use std::fmt::Write as _;

use codeint_config::AgentFormat;
use serde::Serialize;

use crate::ReportError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentStatusReport {
    pub config_root: String,
    pub agents: Vec<AgentStatusEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentStatusEntry {
    pub client: &'static str,
    pub name: &'static str,
    pub config_path: String,
    pub detected: bool,
    pub config_exists: bool,
    pub registered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentInstallAction {
    Create,
    Update,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentInstallReport {
    pub config_root: String,
    pub dry_run: bool,
    pub results: Vec<AgentInstallEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentInstallEntry {
    pub client: &'static str,
    pub name: &'static str,
    pub config_path: String,
    pub action: AgentInstallAction,
    pub written: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}

impl AgentStatusReport {
    pub fn render(&self, format: AgentFormat) -> Result<String, ReportError> {
        render_agent_report(format, self, format_status_text)
    }
}

impl AgentInstallReport {
    pub fn render(&self, format: AgentFormat) -> Result<String, ReportError> {
        render_agent_report(format, self, format_install_text)
    }
}

fn render_agent_report<T: Serialize>(
    format: AgentFormat,
    report: &T,
    to_text: fn(&T) -> String,
) -> Result<String, ReportError> {
    match format {
        AgentFormat::Json => serde_json::to_string_pretty(report).map_err(ReportError::Json),
        AgentFormat::Toon => toon_format::encode_default(report).map_err(ReportError::Toon),
        AgentFormat::Text => Ok(to_text(report)),
    }
}

fn format_status_text(report: &AgentStatusReport) -> String {
    let mut lines = vec![format!("config_root: {}", report.config_root)];
    for entry in &report.agents {
        let detection = if entry.detected {
            "detected"
        } else {
            "not detected"
        };
        let registration = if entry.registered {
            "registered"
        } else {
            "not registered"
        };
        lines.push(format!(
            "{} ({}): {}, {} ({})",
            entry.client, entry.name, detection, registration, entry.config_path
        ));
    }
    lines.join("\n")
}

fn format_install_text(report: &AgentInstallReport) -> String {
    let mut buf = String::new();
    let mode = if report.dry_run {
        "dry-run (no files modified)"
    } else {
        "apply"
    };
    let _ = writeln!(buf, "mode: {mode}\nconfig_root: {}", report.config_root);
    for entry in &report.results {
        let verb = install_action_label(entry.action, report.dry_run);
        let _ = writeln!(
            buf,
            "{} ({}): {} ({})",
            entry.client, entry.name, verb, entry.config_path
        );
        if let Some(preview) = &entry.preview {
            for preview_line in preview.lines() {
                let _ = writeln!(buf, "  {preview_line}");
            }
        }
    }
    if let Some(note) = &report.note {
        let _ = writeln!(buf, "note: {note}");
    }
    buf.trim_end_matches('\n').to_owned()
}

fn install_action_label(action: AgentInstallAction, dry_run: bool) -> &'static str {
    match (action, dry_run) {
        (AgentInstallAction::Create, true) => "would create",
        (AgentInstallAction::Create, false) => "created",
        (AgentInstallAction::Update, true) => "would update",
        (AgentInstallAction::Update, false) => "updated",
        (AgentInstallAction::Unchanged, _) => "unchanged (already registered)",
    }
}
