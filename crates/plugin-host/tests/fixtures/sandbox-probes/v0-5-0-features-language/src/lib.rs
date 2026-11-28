#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "language-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::language::{
    DeclaredDep, DepKind, DependencyManifestOwner, FileDiagnostic, FileHazards, FileModel,
    FileScopeArtifact, FlaggedLine, FrameworkProfileResult, FrameworkProfileSelection, Guest,
    HazardLine, ImportRequest, LowerError, Manifest, ManifestAnalysis, PackageDependency,
    ReferenceRange, Resolution, ScopeAnalysis, ScopeSummary, SourceSet, SummaryField,
    WorkspaceError,
};

struct V05FeaturesLanguagePlugin;

export!(V05FeaturesLanguagePlugin);

const FLAGGED_MARKERS: [(&str, &str); 2] = [
    ("NOOP_FLAG", "redundant-noop"),
    ("UNLISTED_FLAG", "unlisted-flag-rule"),
];

fn collect_flagged_lines(text: &str) -> Vec<FlaggedLine> {
    text.lines()
        .enumerate()
        .flat_map(|(index, line)| {
            let line_number = (index + 1) as u32;
            FLAGGED_MARKERS
                .iter()
                .filter(move |(marker, _)| line.contains(marker))
                .map(move |(_, rule)| FlaggedLine {
                    line: line_number,
                    rule: (*rule).to_string(),
                })
        })
        .collect()
}

const DIAG_MARKERS: [(&str, Option<u32>, &str, &str, usize); 6] = [
    ("DIAG_NOTE", None, "note", "v05 diagnostic", 1),
    ("DIAG_DUP", None, "note", "dup diagnostic", 2),
    ("DIAG_LINE_ZERO", Some(0), "note", "zero line", 1),
    (
        "DIAG_LINE_HUGE",
        Some(4_000_000_000),
        "note",
        "huge line",
        1,
    ),
    ("DIAG_EMPTY_KIND", None, "   ", "blank kind", 1),
    ("DIAG_EMPTY_MSG", None, "note", "   ", 1),
];

fn collect_diagnostics(text: &str) -> Vec<FileDiagnostic> {
    let mut diagnostics = Vec::new();
    for (index, line) in text.lines().enumerate() {
        push_line_diagnostics(line, (index + 1) as u32, &mut diagnostics);
    }
    diagnostics
}

fn push_line_diagnostics(line: &str, line_no: u32, out: &mut Vec<FileDiagnostic>) {
    for &(marker, fixed_line, kind, message, repeat) in &DIAG_MARKERS {
        if line.contains(marker) {
            for _ in 0..repeat {
                out.push(FileDiagnostic {
                    line: fixed_line.unwrap_or(line_no),
                    kind: kind.to_string(),
                    message: message.to_string(),
                });
            }
        }
    }
}

fn build_scope_artifact(text: &str, dialect: Option<&str>) -> Vec<u8> {
    if text.contains("SCOPE_BAD_LINE") {
        return b"BAD_LINE".to_vec();
    }
    if text.contains("SCOPE_BAD_PATH") {
        return b"BAD_PATH".to_vec();
    }
    if text.contains("SCOPE_RISK") {
        return b"RISK".to_vec();
    }
    format!("DIALECT:{}", dialect.unwrap_or("none")).into_bytes()
}

fn analyze_one_manifest(manifest: Manifest, analysis: &mut ManifestAnalysis) {
    if manifest.text.starts_with("not a valid manifest") {
        analysis.invalid_manifests.push(WorkspaceError {
            path: manifest.path,
            message: "invalid v05 manifest".to_string(),
        });
        return;
    }
    let package_root = manifest
        .path
        .rsplit_once('/')
        .map_or("", |(dir, _)| dir)
        .to_string();
    if !package_root.is_empty() {
        analysis.workspace_packages.push(package_root.clone());
        analysis
            .root_dependency_inheritance
            .push(package_root.clone());
    }
    analysis.dependency_manifests.push(DependencyManifestOwner {
        package_root: package_root.clone(),
        path: manifest.path,
    });
    analysis.declared_dependencies.push(DeclaredDep {
        package_root: package_root.clone(),
        name: "v05-dep".to_string(),
        kind: DepKind::Runtime,
    });
    analysis.manifest_used_dependencies.push(PackageDependency {
        package_root,
        name: "v05-dep".to_string(),
    });
}

fn scope_hazard_for_artifact(
    artifact: &FileScopeArtifact,
    expected_dialect: &[u8],
    dialect_mismatch: &mut bool,
) -> Option<FileHazards> {
    if artifact.artifact == b"BAD_LINE" {
        return Some(FileHazards {
            path: artifact.path.clone(),
            hazard_lines: vec![HazardLine {
                line: 4_000_000_000,
                rule: "cross-file-risk".to_string(),
            }],
        });
    }
    if artifact.artifact == b"BAD_PATH" {
        return Some(FileHazards {
            path: "not/in/artifacts.hst".to_string(),
            hazard_lines: vec![HazardLine {
                line: 1,
                rule: "cross-file-risk".to_string(),
            }],
        });
    }
    if artifact.artifact == b"RISK" {
        return Some(FileHazards {
            path: artifact.path.clone(),
            hazard_lines: vec![HazardLine {
                line: 1,
                rule: "cross-file-risk".to_string(),
            }],
        });
    }
    if artifact.artifact.starts_with(b"DIALECT:")
        && artifact.artifact.as_slice() != expected_dialect
    {
        *dialect_mismatch = true;
    }
    None
}

impl Guest for V05FeaturesLanguagePlugin {
    fn extensions() -> Vec<String> {
        vec!["v05".to_string()]
    }

    fn known_dialects() -> Vec<String> {
        vec!["alpha".to_string(), "beta".to_string()]
    }

    fn flagged_line_rules() -> Vec<String> {
        vec!["redundant-noop".to_string()]
    }

    fn lower(path: String, text: String, dialect: Option<String>) -> Result<FileModel, LowerError> {
        if !path.ends_with(".v05") {
            return Err(LowerError::UnsupportedFile(path));
        }
        let source_lines = probe_runtime::count_non_empty_lines(&text);
        let flagged_lines = collect_flagged_lines(&text);
        let diagnostics = collect_diagnostics(&text);
        let scope_artifact = Some(build_scope_artifact(&text, dialect.as_deref()));
        Ok(probe_runtime::blank_file_model!(
            SourceSet::Production,
            source_lines,
            flagged_lines: flagged_lines,
            is_tooling_file: path.ends_with(".config.v05"),
            diagnostics: diagnostics,
            scope_artifact: scope_artifact,
        ))
    }

    probe_runtime::filter_manifest_paths!("package.json");

    fn analyze_manifests(manifests: Vec<Manifest>) -> ManifestAnalysis {
        let mut analysis = probe_runtime::empty_manifest_analysis!();
        for manifest in manifests {
            analyze_one_manifest(manifest, &mut analysis);
        }
        analysis
    }

    probe_runtime::unresolved_imports_stub!();

    probe_runtime::empty_entry_points_and_ranges!();

    probe_runtime::empty_framework_profile_stubs!();

    fn has_hazard_rules() -> bool {
        true
    }

    fn hazard_rules() -> Vec<String> {
        vec!["cross-file-risk".to_string()]
    }

    fn has_imports() -> bool {
        true
    }

    fn has_scope_analysis() -> bool {
        true
    }

    fn analyze_scope(
        _package_root: String,
        dialect: Option<String>,
        artifacts: Vec<FileScopeArtifact>,
    ) -> ScopeAnalysis {
        let expected_dialect = format!("DIALECT:{}", dialect.as_deref().unwrap_or("none"));
        let mut extra_hazards = Vec::new();
        let mut dialect_mismatch = false;
        for artifact in &artifacts {
            if let Some(hazards) = scope_hazard_for_artifact(
                artifact,
                expected_dialect.as_bytes(),
                &mut dialect_mismatch,
            ) {
                extra_hazards.push(hazards);
            }
        }
        let summary_kind = if dialect_mismatch {
            String::new()
        } else {
            "v05-scope".to_string()
        };
        ScopeAnalysis {
            extra_hazards,
            summaries: vec![ScopeSummary {
                kind: summary_kind,
                fields: vec![
                    SummaryField {
                        key: "dialect".to_string(),
                        value: dialect.unwrap_or_else(|| "none".to_string()),
                    },
                    SummaryField {
                        key: "artifacts".to_string(),
                        value: format!("{}", artifacts.len()),
                    },
                ],
            }],
        }
    }
}
