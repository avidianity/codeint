use std::collections::BTreeSet;

use codeint_model::language::{Manifest, ManifestAnalysis, WorkspaceError};
use codeint_model::scan::{analyze_parsed_manifests, manifest_root_excluding};
use globset::{Glob, GlobSet, GlobSetBuilder};

const PYPROJECT_MANIFEST: &str = "pyproject.toml";

enum PythonManifestDoc<'a> {
    Pyproject(toml::Value),
    Auxiliary(&'a Manifest),
}

pub(crate) fn analyze_manifests(manifests: &[Manifest]) -> ManifestAnalysis {
    analyze_parsed_manifests(
        manifests,
        PYPROJECT_MANIFEST,
        parse_python_manifest,
        |all, parsed| {
            let Some((_, _, PythonManifestDoc::Pyproject(doc))) = parsed
                .iter()
                .find(|(_, path, _)| path == PYPROJECT_MANIFEST)
            else {
                return Ok(Vec::new());
            };
            discover_from_doc(doc, all)
        },
        |_| true,
        |parsed| {
            let pyprojects: Vec<(String, String, &toml::Value)> = parsed
                .iter()
                .filter_map(|(root, path, doc)| match doc {
                    PythonManifestDoc::Pyproject(val) => Some((root.clone(), path.clone(), val)),
                    PythonManifestDoc::Auxiliary(_) => None,
                })
                .collect();
            let auxiliary: Vec<&Manifest> = parsed
                .iter()
                .filter_map(|(_, _, doc)| match doc {
                    PythonManifestDoc::Auxiliary(m) => Some(*m),
                    PythonManifestDoc::Pyproject(_) => None,
                })
                .collect();
            crate::dependencies::extract_declared_from_parsed(&pyprojects, &auxiliary)
        },
    )
}

fn parse_python_manifest(
    manifest: &Manifest,
) -> Option<Result<(String, PythonManifestDoc<'_>), WorkspaceError>> {
    if let Some(root) = manifest_root_excluding(
        &manifest.path,
        PYPROJECT_MANIFEST,
        &["vendor", "third_party", ".venv", "venv"],
    ) {
        return Some(
            toml::from_str::<toml::Value>(&manifest.text)
                .map(|doc| (root.to_owned(), PythonManifestDoc::Pyproject(doc)))
                .map_err(|err| WorkspaceError {
                    path: manifest.path.clone(),
                    message: err.to_string(),
                }),
        );
    }
    if crate::dependencies::is_python_dependency_manifest(&manifest.path) {
        let (root, _) = manifest
            .path
            .rsplit_once('/')
            .unwrap_or(("", &manifest.path));
        return Some(Ok((
            root.to_owned(),
            PythonManifestDoc::Auxiliary(manifest),
        )));
    }
    None
}

fn discover_from_doc(
    doc: &toml::Value,
    manifests: &[Manifest],
) -> Result<Vec<String>, WorkspaceError> {
    let Some(workspace) = doc
        .get("tool")
        .and_then(|v| v.get("uv"))
        .and_then(|v| v.get("workspace"))
    else {
        return Ok(Vec::new());
    };
    let members_list = read_string_array(workspace.get("members"));
    if members_list.is_empty() {
        return Ok(Vec::new());
    }
    let members = compile_globs(&members_list, PYPROJECT_MANIFEST)?;
    let excludes = compile_globs(
        &read_string_array(workspace.get("exclude")),
        PYPROJECT_MANIFEST,
    )?;
    let mut found = BTreeSet::new();
    for manifest in manifests {
        if let Some(dir) = manifest.path.strip_suffix("/pyproject.toml")
            && members.is_match(dir)
            && !excludes.is_match(dir)
        {
            found.insert(dir.to_owned());
        }
    }
    Ok(found.into_iter().collect())
}

fn read_string_array(value: Option<&toml::Value>) -> Vec<String> {
    value
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn compile_globs(patterns: &[String], manifest_path: &str) -> Result<GlobSet, WorkspaceError> {
    let mut builder = GlobSetBuilder::new();
    for raw in patterns {
        let normalized = raw.trim_start_matches("./").trim_end_matches('/');
        let glob = Glob::new(normalized).map_err(|err| WorkspaceError {
            path: manifest_path.to_owned(),
            message: err.to_string(),
        })?;
        builder.add(glob);
    }
    builder.build().map_err(|err| WorkspaceError {
        path: manifest_path.to_owned(),
        message: err.to_string(),
    })
}
