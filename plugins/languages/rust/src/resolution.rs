use std::collections::HashSet;

use codeint_model::language::{ImportRequest, Manifest, Resolution};

use crate::imports::{MOD_PREFIX, USE_PREFIX};
use crate::workspaces::CARGO_MANIFEST;

const STD_CRATES: [&str; 4] = ["std", "core", "alloc", "proc_macro"];

pub(crate) fn manifest_paths(repository_paths: &[String]) -> Vec<String> {
    repository_paths
        .iter()
        .filter(|path| *path == CARGO_MANIFEST || path.ends_with(&format!("/{CARGO_MANIFEST}")))
        .cloned()
        .collect()
}

pub(crate) struct RustResolver {
    files: HashSet<String>,
    crate_roots: Vec<String>,
}

impl RustResolver {
    pub(crate) fn new(repository_paths: &[String], manifests: &[Manifest]) -> RustResolver {
        let files: HashSet<String> = repository_paths.iter().cloned().collect();
        let mut crate_roots: Vec<String> = manifests
            .iter()
            .filter_map(|manifest| {
                if manifest.path == CARGO_MANIFEST {
                    Some(String::new())
                } else {
                    manifest
                        .path
                        .strip_suffix(&format!("/{CARGO_MANIFEST}"))
                        .map(str::to_owned)
                }
            })
            .collect();
        crate_roots.sort_by_key(|root| std::cmp::Reverse(root.len()));
        RustResolver { files, crate_roots }
    }

    pub(crate) fn resolve(&self, request: &ImportRequest) -> Resolution {
        if let Some(mod_name) = request.specifier.strip_prefix(MOD_PREFIX) {
            return self.resolve_mod(&request.importer, mod_name);
        }
        if let Some(use_path) = request.specifier.strip_prefix(USE_PREFIX) {
            return self.resolve_use(&request.importer, use_path);
        }
        Resolution::External(String::new())
    }

    fn resolve_mod(&self, importer: &str, mod_name: &str) -> Resolution {
        let base_dir = module_child_dir(importer);
        match self.find_module_file(&base_dir, &[mod_name]) {
            Some(target) if target != importer => Resolution::Resolved(target),
            Some(_) => Resolution::External(String::new()),
            None => Resolution::Unresolved(format!(
                "no module file found for `mod {mod_name}` from {importer}"
            )),
        }
    }

    fn resolve_use(&self, importer: &str, use_path: &str) -> Resolution {
        let segments: Vec<&str> = use_path.split("::").filter(|s| !s.is_empty()).collect();
        let Some(&first) = segments.first() else {
            return Resolution::External(String::new());
        };
        if STD_CRATES.contains(&first) {
            return Resolution::Builtin;
        }
        match first {
            "crate" => {
                let Some(src_dir) = self.crate_src_dir(importer) else {
                    return Resolution::External(String::new());
                };
                self.resolve_longest_module(importer, &src_dir, &segments[1..])
            }
            "self" => {
                let dir = module_child_dir(importer);
                self.resolve_longest_module(importer, &dir, &segments[1..])
            }
            "super" => {
                let mut current = module_parent_dir(importer);
                let mut idx = 1usize;
                while idx < segments.len() && segments[idx] == "super" {
                    current = parent_dir(&current);
                    idx += 1;
                }
                if idx == segments.len() {
                    return self
                        .resolve_dir_to_file(&current)
                        .filter(|target| target != importer)
                        .map_or_else(|| Resolution::External(String::new()), Resolution::Resolved);
                }
                self.resolve_longest_module(importer, &current, &segments[idx..])
            }
            _ => Resolution::External(String::new()),
        }
    }

    fn resolve_longest_module(
        &self,
        importer: &str,
        base_dir: &str,
        segments: &[&str],
    ) -> Resolution {
        for len in (1..=segments.len()).rev() {
            if let Some(target) = self.find_module_file(base_dir, &segments[..len]) {
                return if target == importer {
                    Resolution::External(String::new())
                } else {
                    Resolution::Resolved(target)
                };
            }
        }
        if let Some(root_file) = self.resolve_dir_to_file(base_dir)
            && root_file != importer
        {
            return Resolution::Resolved(root_file);
        }
        Resolution::External(String::new())
    }

    fn find_module_file(&self, base_dir: &str, segments: &[&str]) -> Option<String> {
        if segments.is_empty() {
            return None;
        }
        let rel = segments.join("/");
        let prefix = if base_dir.is_empty() {
            rel
        } else {
            format!("{base_dir}/{rel}")
        };
        let direct = format!("{prefix}.rs");
        if self.files.contains(&direct) {
            return Some(direct);
        }
        let mod_rs = format!("{prefix}/mod.rs");
        if self.files.contains(&mod_rs) {
            return Some(mod_rs);
        }
        None
    }

    fn resolve_dir_to_file(&self, dir: &str) -> Option<String> {
        [
            join(dir, "lib.rs"),
            join(dir, "main.rs"),
            join(dir, "mod.rs"),
            format!("{dir}.rs"),
        ]
        .into_iter()
        .find(|candidate| self.files.contains(candidate))
    }

    fn crate_src_dir(&self, importer: &str) -> Option<String> {
        for root in &self.crate_roots {
            let src = if root.is_empty() {
                "src".to_owned()
            } else {
                format!("{root}/src")
            };
            if importer.starts_with(&format!("{src}/")) {
                return Some(src);
            }
        }
        importer
            .find("/src/")
            .map(|idx| importer[..idx + 4].to_owned())
            .or_else(|| importer.starts_with("src/").then(|| "src".to_owned()))
    }
}

fn join(dir: &str, file: &str) -> String {
    if dir.is_empty() {
        file.to_owned()
    } else {
        format!("{dir}/{file}")
    }
}

fn parent_dir(path: &str) -> String {
    path.rsplit_once('/')
        .map_or(String::new(), |(parent, _)| parent.to_owned())
}

fn module_child_dir(importer: &str) -> String {
    let file_name = importer.rsplit_once('/').map_or(importer, |(_, name)| name);
    let dir = parent_dir(importer);
    let parent_name = dir.rsplit_once('/').map_or(dir.as_str(), |(_, name)| name);
    if matches!(file_name, "lib.rs" | "main.rs" | "mod.rs")
        || matches!(parent_name, "tests" | "benches" | "examples" | "bin")
    {
        dir
    } else {
        let stem = file_name.strip_suffix(".rs").unwrap_or(file_name);
        join(&dir, stem)
    }
}

fn module_parent_dir(importer: &str) -> String {
    let file_name = importer.rsplit_once('/').map_or(importer, |(_, name)| name);
    let dir = parent_dir(importer);
    if file_name == "mod.rs" {
        parent_dir(&dir)
    } else {
        dir
    }
}
