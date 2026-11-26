use std::fmt::Write as _;

use codeint_config::DoctorFormat;
use serde::Serialize;

use crate::{ReportError, render_text_report};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorStatus {
    Pass,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorCheck {
    pub id: String,
    pub status: DoctorStatus,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remedy: Option<String>,
}

impl DoctorCheck {
    pub fn pass(id: impl Into<String>, summary: impl Into<String>) -> DoctorCheck {
        DoctorCheck {
            id: id.into(),
            status: DoctorStatus::Pass,
            summary: summary.into(),
            detail: None,
            remedy: None,
        }
    }

    pub fn warn(id: impl Into<String>, summary: impl Into<String>) -> DoctorCheck {
        DoctorCheck {
            id: id.into(),
            status: DoctorStatus::Warn,
            summary: summary.into(),
            detail: None,
            remedy: None,
        }
    }

    pub fn error(id: impl Into<String>, summary: impl Into<String>) -> DoctorCheck {
        DoctorCheck {
            id: id.into(),
            status: DoctorStatus::Error,
            summary: summary.into(),
            detail: None,
            remedy: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> DoctorCheck {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_remedy(mut self, remedy: impl Into<String>) -> DoctorCheck {
        self.remedy = Some(remedy.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorSummary {
    pub passed: usize,
    pub warnings: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorReport {
    pub version: String,
    pub path: String,
    pub status: DoctorStatus,
    pub summary: DoctorSummary,
    pub checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    pub fn new(
        version: impl Into<String>,
        path: impl Into<String>,
        checks: Vec<DoctorCheck>,
    ) -> DoctorReport {
        let passed = checks
            .iter()
            .filter(|check| check.status == DoctorStatus::Pass)
            .count();
        let warnings = checks
            .iter()
            .filter(|check| check.status == DoctorStatus::Warn)
            .count();
        let errors = checks
            .iter()
            .filter(|check| check.status == DoctorStatus::Error)
            .count();
        let status = if errors > 0 {
            DoctorStatus::Error
        } else if warnings > 0 {
            DoctorStatus::Warn
        } else {
            DoctorStatus::Pass
        };
        DoctorReport {
            version: version.into(),
            path: path.into(),
            status,
            summary: DoctorSummary {
                passed,
                warnings,
                errors,
            },
            checks,
        }
    }

    pub fn has_blocking_errors(&self) -> bool {
        self.summary.errors > 0
    }

    pub fn blocking_error_messages(&self) -> Vec<String> {
        self.checks
            .iter()
            .filter(|check| check.status == DoctorStatus::Error)
            .map(|check| format!("{}: {}", check.id, check.summary))
            .collect()
    }

    pub fn render(&self, format: DoctorFormat) -> Result<String, ReportError> {
        render_text_report(self, format.into(), DoctorReport::render_text)
    }

    fn render_text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "codeint doctor {} ({})", self.version, self.path);
        let _ = writeln!(
            out,
            "summary: {} passed, {} {}, {} blocking {}",
            self.summary.passed,
            self.summary.warnings,
            plural(self.summary.warnings, "warning", "warnings"),
            self.summary.errors,
            plural(self.summary.errors, "error", "errors"),
        );
        self.write_section(&mut out, "Passing checks:", DoctorStatus::Pass, "PASS");
        self.write_section(&mut out, "Warnings:", DoctorStatus::Warn, "WARN");
        self.write_section(&mut out, "Blocking errors:", DoctorStatus::Error, "ERROR");
        out.trim_end_matches('\n').to_owned()
    }

    fn write_section(&self, out: &mut String, heading: &str, status: DoctorStatus, badge: &str) {
        let items: Vec<&DoctorCheck> = self
            .checks
            .iter()
            .filter(|check| check.status == status)
            .collect();
        if items.is_empty() {
            return;
        }
        let _ = writeln!(out);
        let _ = writeln!(out, "{heading}");
        for check in items {
            let _ = writeln!(out, "  [{badge}] {}: {}", check.id, check.summary);
            if let Some(detail) = &check.detail {
                for (index, line) in detail.lines().enumerate() {
                    let label = if index == 0 { "detail: " } else { "        " };
                    let _ = writeln!(out, "    {label}{line}");
                }
            }
            if let Some(remedy) = &check.remedy {
                let _ = writeln!(out, "    remedy: {remedy}");
            }
        }
    }
}

fn plural<'a>(count: usize, singular: &'a str, plural_word: &'a str) -> &'a str {
    if count == 1 { singular } else { plural_word }
}
