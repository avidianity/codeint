use std::collections::BTreeSet;

use codeint_model::language::{Manifest, WorkspaceError};
use globset::{Glob, GlobSet, GlobSetBuilder};

pub(crate) fn discover(manifests: &[Manifest]) -> Result<Vec<String>, WorkspaceError> {
    let Some(root) = manifests.iter().find(|m| m.path == "pyproject.toml") else {
        return Ok(Vec::new());
    };
    let doc: toml::Value = toml::from_str(&root.text).map_err(|err| WorkspaceError {
        path: root.path.clone(),
        message: err.to_string(),
    })?;
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
    let members = compile_globs(&members_list, &root.path)?;
    let excludes = compile_globs(&read_string_array(workspace.get("exclude")), &root.path)?;
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
