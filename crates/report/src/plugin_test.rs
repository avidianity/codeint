use std::fmt::Write as _;

use codeint_config::PluginTestFormat;
use serde::Serialize;

use crate::{ReportError, render_text_report};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginTestStatus {
    Pass,
    Fail,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginTestCheck {
    pub id: String,
    pub status: PluginTestStatus,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginTestSummary {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginTestReport {
    pub plugin: String,
    pub kind: String,
    pub passed: bool,
    pub summary: PluginTestSummary,
    pub checks: Vec<PluginTestCheck>,
}

impl PluginTestReport {
    pub fn new(
        plugin: impl Into<String>,
        kind: impl Into<String>,
        checks: Vec<PluginTestCheck>,
    ) -> PluginTestReport {
        let count =
            |status: PluginTestStatus| checks.iter().filter(|check| check.status == status).count();
        let summary = PluginTestSummary {
            passed: count(PluginTestStatus::Pass),
            failed: count(PluginTestStatus::Fail),
            skipped: count(PluginTestStatus::Skipped),
        };
        PluginTestReport {
            plugin: plugin.into(),
            kind: kind.into(),
            passed: summary.failed == 0,
            summary,
            checks,
        }
    }

    pub fn failure_messages(&self) -> Vec<String> {
        self.checks
            .iter()
            .filter(|check| check.status == PluginTestStatus::Fail)
            .map(|check| format!("Conformance check {} failed: {}", check.id, check.message))
            .collect()
    }

    pub fn render(&self, format: PluginTestFormat) -> Result<String, ReportError> {
        render_text_report(self, format.into(), PluginTestReport::render_text)
    }

    fn render_text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "codeint plugin test {} ({})", self.plugin, self.kind);
        let _ = writeln!(
            out,
            "summary: {} passed, {} failed, {} skipped",
            self.summary.passed, self.summary.failed, self.summary.skipped
        );
        let _ = writeln!(out);
        for check in &self.checks {
            let badge = match check.status {
                PluginTestStatus::Pass => "PASS",
                PluginTestStatus::Fail => "FAIL",
                PluginTestStatus::Skipped => "SKIP",
            };
            let _ = writeln!(out, "[{badge}] {}: {}", check.id, check.message);
            if let Some(hint) = &check.hint {
                let _ = writeln!(out, "    fix: {hint}");
            }
        }
        out.trim_end_matches('\n').to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(id: &str, status: PluginTestStatus) -> PluginTestCheck {
        PluginTestCheck {
            id: id.to_owned(),
            status,
            message: format!("{id} message"),
            hint: (status == PluginTestStatus::Fail).then(|| "fix it".to_owned()),
        }
    }

    #[test]
    fn a_failed_check_fails_the_report_and_is_listed_in_every_format() {
        let report = PluginTestReport::new(
            "a.wasm",
            "metric",
            vec![
                check("component", PluginTestStatus::Pass),
                check("metric-key", PluginTestStatus::Fail),
                check("limits", PluginTestStatus::Skipped),
            ],
        );
        assert!(!report.passed);
        assert_eq!(report.summary.failed, 1);
        assert_eq!(report.failure_messages().len(), 1);
        let text = report.render(PluginTestFormat::Text).expect("text");
        assert!(
            text.contains("[FAIL] metric-key: metric-key message"),
            "{text}"
        );
        assert!(text.contains("fix: fix it"), "{text}");
        let json = report.render(PluginTestFormat::Json).expect("json");
        assert!(
            json.contains("\"passed\":false") || json.contains("\"passed\": false"),
            "{json}"
        );
        assert!(
            report
                .render(PluginTestFormat::Toon)
                .expect("toon")
                .contains("metric-key")
        );
    }

    #[test]
    fn all_passing_checks_pass_the_report() {
        let report = PluginTestReport::new(
            "a.wasm",
            "language",
            vec![check("component", PluginTestStatus::Pass)],
        );
        assert!(report.passed);
        assert!(report.failure_messages().is_empty());
    }
}
