use std::collections::{BTreeSet, HashMap, HashSet};

use codeint_model::language::{ImportRequest, Manifest, Resolution, SourceSet};
use codeint_model::scan::normalize_relative_path;
use toml::Value;

use crate::dependencies::normalize_crate_name;
use crate::imports::{MOD_PATH_PREFIX, MOD_PREFIX, PATH_PREFIX, USE_LOCAL_PREFIX, USE_PREFIX};
use crate::workspaces::{CARGO_MANIFEST, workspace_members_under};

const STD_CRATES: [&str; 5] = ["std", "core", "alloc", "proc_macro", "test"];
const PROD_DEP_SECTIONS: [&str; 2] = ["dependencies", "build-dependencies"];
const DEV_DEP_SECTION: &str = "dev-dependencies";
const DEP_SECTIONS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

enum ReexportFollow {
    Resolved(Resolution),
    NotFound,
    Cycle,
}

#[derive(Default)]
struct PackageDeclaredDeps {
    production: HashSet<String>,
    dev: HashSet<String>,
}

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
    target_root_files: HashSet<String>,
    build_script_files: HashSet<String>,
    target_root_dirs: Vec<String>,
    package_local_crates: HashMap<String, HashMap<String, Option<String>>>,
    workspace_local_crates: HashMap<String, HashMap<String, Option<String>>>,
    package_declared_deps: HashMap<String, PackageDeclaredDeps>,
    mod_parents: HashMap<String, Vec<String>>,
    use_bindings: HashMap<String, HashMap<String, Option<String>>>,
}

impl RustResolver {
    pub(crate) fn new(
        repository_paths: &[String],
        manifests: &[Manifest],
        requests: &[ImportRequest],
    ) -> RustResolver {
        let files: HashSet<String> = repository_paths.iter().cloned().collect();
        let (parsed, mut crate_roots) = parse_cargo_manifests(manifests);
        crate_roots.sort_by_key(|root| std::cmp::Reverse(root.len()));
        let (package_local_crates, workspace_local_crates) =
            build_scoped_local_crate_maps(&files, &parsed, &crate_roots);
        let (target_root_files, build_script_files, target_root_dirs) =
            collect_target_roots(&files, &parsed);
        let package_declared_deps = collect_package_declared_deps(&parsed);
        let mut resolver = RustResolver {
            files,
            crate_roots,
            target_root_files,
            build_script_files,
            target_root_dirs,
            package_local_crates,
            workspace_local_crates,
            package_declared_deps,
            mod_parents: HashMap::new(),
            use_bindings: HashMap::new(),
        };
        resolver.index_requests(requests);
        resolver
    }

    fn index_requests(&mut self, requests: &[ImportRequest]) {
        let mut mod_parents: HashMap<String, Vec<String>> = HashMap::new();
        let mut use_bindings: HashMap<String, HashMap<String, Option<String>>> = HashMap::new();
        for request in requests {
            if let Some(child) = self.resolved_mod_child(request) {
                let parents = mod_parents.entry(child).or_default();
                if !parents.contains(&request.importer) {
                    parents.push(request.importer.clone());
                }
            } else if let Some(use_spec) = request.specifier.strip_prefix(USE_PREFIX) {
                record_use_binding(&request.importer, use_spec, &mut use_bindings);
            }
        }
        self.mod_parents = mod_parents;
        self.use_bindings = use_bindings;
    }

    fn resolved_mod_child(&self, request: &ImportRequest) -> Option<String> {
        let resolution = if let Some(mod_path) = request.specifier.strip_prefix(MOD_PATH_PREFIX) {
            self.resolve_mod_path(&request.importer, mod_path)
        } else {
            let mod_name = request.specifier.strip_prefix(MOD_PREFIX)?;
            self.resolve_mod(&request.importer, mod_name)
        };
        match resolution {
            Resolution::Resolved(child) => Some(child),
            _ => None,
        }
    }

    pub(crate) fn resolve(&self, request: &ImportRequest) -> Resolution {
        if let Some(mod_path) = request.specifier.strip_prefix(MOD_PATH_PREFIX) {
            return self.resolve_mod_path(&request.importer, mod_path);
        }
        if let Some(mod_name) = request.specifier.strip_prefix(MOD_PREFIX) {
            return self.resolve_mod(&request.importer, mod_name);
        }
        if let Some(use_spec) = request
            .specifier
            .strip_prefix(USE_PREFIX)
            .or_else(|| request.specifier.strip_prefix(USE_LOCAL_PREFIX))
        {
            let (raw_path, _) = split_use_alias(use_spec);
            let use_path = strip_glob_suffix(raw_path);
            let mut visited = HashSet::new();
            return self
                .resolve_use(&request.importer, &request.importer, use_path, &mut visited)
                .unwrap_or(Resolution::Builtin);
        }
        if let Some(head) = request.specifier.strip_prefix(PATH_PREFIX) {
            return self.resolve_qualified_path(&request.importer, head);
        }
        Resolution::Unresolved(format!(
            "unrecognized Rust import specifier {:?}",
            request.specifier
        ))
    }

    fn resolve_mod(&self, importer: &str, mod_name: &str) -> Resolution {
        let base_dir = self.module_child_dir(importer);
        let segments: Vec<&str> = mod_name.split("::").filter(|s| !s.is_empty()).collect();
        match self.find_module_file(&base_dir, &segments) {
            Some(target) if target != importer => Resolution::Resolved(target),
            Some(_) => Resolution::Builtin,
            None => Resolution::Unresolved(format!(
                "no module file found for `mod {mod_name}` from {importer}"
            )),
        }
    }

    fn resolve_mod_path(&self, importer: &str, rel_path: &str) -> Resolution {
        if rel_path.is_empty() || rel_path.starts_with('/') || rel_path.contains('\\') {
            return Resolution::Unresolved(format!(
                "no module file found for `#[path = {rel_path:?}]` from {importer}"
            ));
        }
        let pkg_root = self.owning_crate_root(importer);
        let child_dir = self.module_child_dir(importer);
        let parent = parent_dir(importer);
        for base in [child_dir.as_str(), parent.as_str()] {
            if let Some(candidate) = normalize_relative_path(base, rel_path)
                && is_within_package_root(&candidate, pkg_root)
                && self.files.contains(&candidate)
            {
                return if candidate == importer {
                    Resolution::Builtin
                } else {
                    Resolution::Resolved(candidate)
                };
            }
        }
        Resolution::Unresolved(format!(
            "no module file found for `#[path = {rel_path:?}]` from {importer}"
        ))
    }

    fn resolve_qualified_path(&self, importer: &str, head: &str) -> Resolution {
        let crate_ident = normalize_crate_name(head);
        if crate_ident.is_empty() || STD_CRATES.contains(&crate_ident.as_str()) {
            return Resolution::Builtin;
        }
        if let Some(lib_target) = self.find_local_crate(importer, &crate_ident) {
            return match lib_target {
                Some(path) if !self.is_self_or_mod_ancestor(importer, path) => {
                    Resolution::Resolved(path.to_owned())
                }
                _ => Resolution::Builtin,
            };
        }
        if self.is_declared_dependency(importer, &crate_ident) {
            return Resolution::External(crate_ident);
        }
        Resolution::Builtin
    }

    fn resolve_use(
        &self,
        original_importer: &str,
        context_file: &str,
        use_path: &str,
        visited: &mut HashSet<(String, String)>,
    ) -> Option<Resolution> {
        let segments: Vec<&str> = use_path.split("::").filter(|s| !s.is_empty()).collect();
        let Some(&first) = segments.first() else {
            return Some(Resolution::Unresolved(format!(
                "empty Rust use path {use_path:?}"
            )));
        };
        let head = strip_raw_prefix(first);
        if STD_CRATES.contains(&head) {
            return Some(Resolution::Builtin);
        }
        match head {
            "crate" => match self.unambiguous_mod_root(context_file) {
                Some(root_file) => self.resolve_in_module_file(
                    original_importer,
                    root_file,
                    &segments[1..],
                    visited,
                ),
                None => match self.crate_src_dir(context_file) {
                    Some(src_dir) => self.resolve_longest_module(
                        original_importer,
                        &src_dir,
                        &segments[1..],
                        visited,
                    ),
                    None => Some(Resolution::Builtin),
                },
            },
            "self" => self.resolve_in_module_file(
                original_importer,
                context_file,
                &segments[1..],
                visited,
            ),
            "super" => self.resolve_super_use(original_importer, context_file, &segments, visited),
            _ => self.resolve_top_level_use(
                original_importer,
                context_file,
                head,
                &segments,
                visited,
            ),
        }
    }

    fn resolve_super_use(
        &self,
        original_importer: &str,
        context_file: &str,
        segments: &[&str],
        visited: &mut HashSet<(String, String)>,
    ) -> Option<Resolution> {
        let super_count = segments.iter().take_while(|&&seg| seg == "super").count();
        let rest = &segments[super_count..];
        let (current_file, taken) = self.walk_mod_parents(context_file, super_count);
        if taken == super_count {
            return self.resolve_in_module_file(original_importer, current_file, rest, visited);
        }
        let mut current_dir = module_parent_dir(current_file);
        for _ in (taken + 1)..super_count {
            current_dir = parent_dir(&current_dir);
        }
        if rest.is_empty() {
            return Some(
                self.resolve_dir_to_file(&current_dir)
                    .map_or(Resolution::Builtin, |target| {
                        self.finalize_intra_crate_target(original_importer, target)
                    }),
            );
        }
        self.resolve_longest_module(original_importer, &current_dir, rest, visited)
    }

    fn walk_mod_parents<'a>(&'a self, start: &'a str, max_steps: usize) -> (&'a str, usize) {
        let mut current = start;
        let mut seen = HashSet::new();
        seen.insert(current);
        let mut taken = 0usize;
        while taken < max_steps {
            let Some(parents) = self.mod_parents.get(current) else {
                break;
            };
            if parents.len() != 1 {
                break;
            }
            let parent = parents[0].as_str();
            if !seen.insert(parent) {
                break;
            }
            current = parent;
            taken += 1;
        }
        (current, taken)
    }

    fn unambiguous_mod_root<'a>(&'a self, file: &'a str) -> Option<&'a str> {
        let (top, _) = self.walk_mod_parents(file, usize::MAX);
        if self.mod_parents.contains_key(top) {
            return None;
        }
        self.is_crate_root_file(top).then_some(top)
    }

    fn is_primary_crate_root_file(&self, file: &str) -> bool {
        let file_name = file.rsplit_once('/').map_or(file, |(_, name)| name);
        self.target_root_files.contains(file)
            || matches!(file_name, "lib.rs" | "main.rs" | "build.rs")
    }

    fn is_crate_root_file(&self, file: &str) -> bool {
        if self.is_primary_crate_root_file(file) {
            return true;
        }
        let file_name = file.rsplit_once('/').map_or(file, |(_, name)| name);
        if file_name == "mod.rs" {
            return false;
        }
        let dir = parent_dir(file);
        let pkg_root = self.owning_crate_root(file);
        let rel_dir = if pkg_root.is_empty() {
            dir.as_str()
        } else {
            dir.strip_prefix(pkg_root)
                .and_then(|rest| rest.strip_prefix('/'))
                .unwrap_or("")
        };
        matches!(rel_dir, "tests" | "benches" | "examples" | "src/bin")
    }

    fn resolve_top_level_use(
        &self,
        original_importer: &str,
        context_file: &str,
        head: &str,
        segments: &[&str],
        visited: &mut HashSet<(String, String)>,
    ) -> Option<Resolution> {
        if head.bytes().next().is_some_and(|b| b.is_ascii_uppercase()) {
            return Some(Resolution::Builtin);
        }
        let crate_ident = normalize_crate_name(head);
        if let Some(lib_target) = self.find_local_crate(context_file, &crate_ident) {
            return match lib_target {
                Some(path) if path == original_importer => Some(Resolution::Builtin),
                Some(path) if self.is_mod_ancestor(original_importer, path) => {
                    self.resolve_in_module_file(original_importer, path, &segments[1..], visited)
                }
                Some(path) => Some(Resolution::Resolved(path.to_owned())),
                None => Some(Resolution::Builtin),
            };
        }
        let child_dir = self.module_child_dir(context_file);
        if let Some(resolution) =
            self.resolve_existing_submodule(original_importer, &child_dir, segments, visited)
        {
            return Some(resolution);
        }
        Some(Resolution::External(crate_ident))
    }

    fn is_declared_dependency(&self, importer: &str, crate_ident: &str) -> bool {
        let pkg_root = self.owning_crate_root(importer);
        let Some(deps) = self.package_declared_deps.get(pkg_root) else {
            return false;
        };
        if deps.production.contains(crate_ident) {
            return true;
        }
        crate::source_set::classify(importer, "") == SourceSet::Test
            && deps.dev.contains(crate_ident)
    }

    fn find_local_crate(&self, importer: &str, crate_ident: &str) -> Option<Option<&str>> {
        let pkg_root = self.owning_crate_root(importer);
        if let Some(pkg_map) = self.package_local_crates.get(pkg_root)
            && let Some(target) = pkg_map.get(crate_ident)
        {
            return Some(target.as_deref());
        }
        let mut cursor = pkg_root;
        loop {
            if let Some(ws_map) = self.workspace_local_crates.get(cursor) {
                return ws_map.get(crate_ident).map(Option::as_deref);
            }
            if cursor.is_empty() {
                break;
            }
            cursor = cursor.rsplit_once('/').map_or("", |(parent, _)| parent);
        }
        None
    }

    fn owning_crate_root(&self, importer: &str) -> &str {
        for root in &self.crate_roots {
            if root.is_empty() {
                return "";
            }
            if importer.starts_with(&format!("{root}/")) {
                return root.as_str();
            }
        }
        ""
    }

    fn resolve_in_module_file(
        &self,
        original_importer: &str,
        module_file: &str,
        segments: &[&str],
        visited: &mut HashSet<(String, String)>,
    ) -> Option<Resolution> {
        let child_dir = self.module_child_dir(module_file);
        if let Some(resolution) =
            self.resolve_existing_submodule(original_importer, &child_dir, segments, visited)
        {
            return Some(resolution);
        }
        self.resolve_module_reexport_or_self(original_importer, module_file, segments, visited)
    }

    fn resolve_longest_module(
        &self,
        original_importer: &str,
        base_dir: &str,
        segments: &[&str],
        visited: &mut HashSet<(String, String)>,
    ) -> Option<Resolution> {
        if let Some(resolution) =
            self.resolve_existing_submodule(original_importer, base_dir, segments, visited)
        {
            return Some(resolution);
        }
        let Some(root_file) = self.resolve_dir_to_file(base_dir) else {
            return Some(Resolution::Builtin);
        };
        self.resolve_module_reexport_or_self(original_importer, &root_file, segments, visited)
    }

    fn resolve_module_reexport_or_self(
        &self,
        original_importer: &str,
        module_file: &str,
        segments: &[&str],
        visited: &mut HashSet<(String, String)>,
    ) -> Option<Resolution> {
        if !segments.is_empty() {
            match self.follow_reexport(original_importer, module_file, segments, visited) {
                ReexportFollow::Resolved(followed) => return Some(followed),
                ReexportFollow::Cycle => return None,
                ReexportFollow::NotFound => {}
            }
        }
        Some(self.finalize_intra_crate_target(original_importer, module_file.to_owned()))
    }

    fn resolve_existing_submodule(
        &self,
        original_importer: &str,
        base_dir: &str,
        segments: &[&str],
        visited: &mut HashSet<(String, String)>,
    ) -> Option<Resolution> {
        for len in (1..=segments.len()).rev() {
            if let Some(target) = self.find_module_file(base_dir, &segments[..len]) {
                let remaining = &segments[len..];
                if !remaining.is_empty()
                    && let ReexportFollow::Resolved(followed) =
                        self.follow_reexport(original_importer, &target, remaining, visited)
                {
                    return Some(followed);
                }
                return Some(self.finalize_intra_crate_target(original_importer, target));
            }
        }
        None
    }

    fn follow_reexport(
        &self,
        original_importer: &str,
        module_file: &str,
        segments: &[&str],
        visited: &mut HashSet<(String, String)>,
    ) -> ReexportFollow {
        let symbol = strip_raw_prefix(segments[0]);
        let Some(target_path) = self
            .use_bindings
            .get(module_file)
            .and_then(|bindings| bindings.get(symbol))
            .and_then(Option::as_deref)
        else {
            return ReexportFollow::NotFound;
        };
        if !visited.insert((module_file.to_owned(), symbol.to_owned())) {
            return ReexportFollow::Cycle;
        }
        let combined = if segments.len() > 1 {
            format!("{target_path}::{}", segments[1..].join("::"))
        } else {
            target_path.to_owned()
        };
        match self.resolve_use(original_importer, module_file, &combined, visited) {
            Some(resolution) => ReexportFollow::Resolved(resolution),
            None => ReexportFollow::Cycle,
        }
    }

    fn finalize_intra_crate_target(&self, importer: &str, target: String) -> Resolution {
        if self.is_self_or_mod_ancestor(importer, &target) {
            Resolution::Builtin
        } else {
            Resolution::Resolved(target)
        }
    }

    fn is_self_or_mod_ancestor(&self, importer: &str, candidate: &str) -> bool {
        importer == candidate || self.is_mod_ancestor(importer, candidate)
    }

    fn is_mod_ancestor(&self, importer: &str, candidate: &str) -> bool {
        let mut stack = vec![importer];
        let mut seen = HashSet::new();
        seen.insert(importer);
        while let Some(node) = stack.pop() {
            let Some(parents) = self.mod_parents.get(node) else {
                continue;
            };
            for parent in parents {
                if parent == candidate {
                    return true;
                }
                if seen.insert(parent.as_str()) {
                    stack.push(parent.as_str());
                }
            }
        }
        false
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
        let standard = [
            join(dir, "lib.rs"),
            join(dir, "main.rs"),
            join(dir, "mod.rs"),
            format!("{dir}.rs"),
        ]
        .into_iter()
        .find(|candidate| self.files.contains(candidate));
        if standard.is_some() {
            return standard;
        }
        self.target_root_files
            .iter()
            .filter(|file| parent_dir(file) == dir)
            .min()
            .cloned()
    }

    fn crate_src_dir(&self, importer: &str) -> Option<String> {
        if self.build_script_files.contains(importer) {
            return Some(parent_dir(importer));
        }
        for dir in &self.target_root_dirs {
            if dir.is_empty() {
                if !importer.contains('/') {
                    return Some(String::new());
                }
            } else if importer.starts_with(&format!("{dir}/")) {
                return Some(dir.clone());
            }
        }
        importer
            .find("/src/")
            .map(|idx| importer[..idx + 4].to_owned())
            .or_else(|| importer.starts_with("src/").then(|| "src".to_owned()))
    }

    fn module_child_dir(&self, importer: &str) -> String {
        let file_name = importer.rsplit_once('/').map_or(importer, |(_, name)| name);
        let dir = parent_dir(importer);
        let parent_name = dir.rsplit_once('/').map_or(dir.as_str(), |(_, name)| name);
        if self.target_root_files.contains(importer)
            || matches!(file_name, "lib.rs" | "main.rs" | "mod.rs" | "build.rs")
            || matches!(parent_name, "tests" | "benches" | "examples" | "bin")
        {
            dir
        } else {
            let stem = file_name.strip_suffix(".rs").unwrap_or(file_name);
            join(&dir, stem)
        }
    }
}

type ScopedCrateMaps = (
    HashMap<String, HashMap<String, Option<String>>>,
    HashMap<String, HashMap<String, Option<String>>>,
);

type TargetRoots = (HashSet<String>, HashSet<String>, Vec<String>);

fn collect_target_roots(
    files: &HashSet<String>,
    parsed: &HashMap<String, (String, Value)>,
) -> TargetRoots {
    let mut root_files = HashSet::new();
    let mut build_scripts = HashSet::new();
    let mut root_dirs = BTreeSet::new();
    for (root, (_, doc)) in parsed {
        root_dirs.insert(join(root, "src"));
        record_target_root(
            files,
            root,
            doc.get("lib")
                .and_then(|v| v.get("path"))
                .and_then(Value::as_str),
            &mut root_files,
            &mut root_dirs,
        );
        if let Some(bins) = doc.get("bin").and_then(Value::as_array) {
            for bin in bins {
                record_target_root(
                    files,
                    root,
                    bin.get("path").and_then(Value::as_str),
                    &mut root_files,
                    &mut root_dirs,
                );
            }
        }
        record_build_script_root(files, root, doc, &mut root_files, &mut build_scripts);
    }
    let mut sorted_dirs: Vec<String> = root_dirs.into_iter().collect();
    sorted_dirs.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    (root_files, build_scripts, sorted_dirs)
}

fn record_target_root(
    files: &HashSet<String>,
    root: &str,
    rel_path: Option<&str>,
    root_files: &mut HashSet<String>,
    root_dirs: &mut BTreeSet<String>,
) {
    let Some(path) = rel_path.and_then(|rel| safe_manifest_path(root, rel)) else {
        return;
    };
    if files.contains(&path) {
        root_dirs.insert(parent_dir(&path));
        root_files.insert(path);
    }
}

fn record_build_script_root(
    files: &HashSet<String>,
    root: &str,
    doc: &Value,
    root_files: &mut HashSet<String>,
    build_scripts: &mut HashSet<String>,
) {
    let Some(package) = doc.get("package") else {
        return;
    };
    let rel = match package.get("build") {
        None | Some(Value::Boolean(true)) => "build.rs",
        Some(Value::Boolean(false)) => return,
        Some(Value::String(explicit)) => explicit.as_str(),
        Some(_) => return,
    };
    if let Some(path) = safe_manifest_path(root, rel)
        && files.contains(&path)
    {
        root_files.insert(path.clone());
        build_scripts.insert(path);
    }
}

fn safe_manifest_path(root: &str, rel: &str) -> Option<String> {
    let trimmed = rel.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.contains('\\')
        || trimmed.split('/').any(|seg| seg == "..")
    {
        return None;
    }
    normalize_relative_path(root, trimmed).filter(|path| !path.is_empty())
}

fn is_within_package_root(candidate: &str, pkg_root: &str) -> bool {
    pkg_root.is_empty() || candidate.starts_with(&format!("{pkg_root}/"))
}

fn collect_package_declared_deps(
    parsed: &HashMap<String, (String, Value)>,
) -> HashMap<String, PackageDeclaredDeps> {
    let mut out = HashMap::new();
    for (root, (_, doc)) in parsed {
        if doc.get("package").is_none() {
            continue;
        }
        let mut deps = PackageDeclaredDeps::default();
        collect_dep_names_from_doc(doc, &mut deps);
        if !deps.production.is_empty() || !deps.dev.is_empty() {
            out.insert(root.clone(), deps);
        }
    }
    out
}

fn collect_dep_names_from_doc(doc: &Value, out: &mut PackageDeclaredDeps) {
    collect_dep_sections_from_table(doc, out);
    if let Some(targets) = doc.get("target").and_then(Value::as_table) {
        for cfg_table in targets.values() {
            collect_dep_sections_from_table(cfg_table, out);
        }
    }
}

fn collect_dep_sections_from_table(table: &Value, out: &mut PackageDeclaredDeps) {
    for section in PROD_DEP_SECTIONS {
        if let Some(deps) = table.get(section).and_then(Value::as_table) {
            insert_dep_keys(deps, &mut out.production);
        }
    }
    if let Some(deps) = table.get(DEV_DEP_SECTION).and_then(Value::as_table) {
        insert_dep_keys(deps, &mut out.dev);
    }
}

fn insert_dep_keys(table: &toml::map::Map<String, Value>, out: &mut HashSet<String>) {
    for raw_key in table.keys() {
        let normalized = normalize_crate_name(raw_key);
        if !normalized.is_empty() {
            out.insert(normalized);
        }
    }
}

fn build_scoped_local_crate_maps(
    files: &HashSet<String>,
    parsed: &HashMap<String, (String, Value)>,
    manifest_roots: &[String],
) -> ScopedCrateMaps {
    let mut package_maps = HashMap::new();
    let mut workspace_maps = HashMap::new();
    for (root, (text, doc)) in parsed {
        let pkg_map = build_package_crate_map(files, root, doc);
        if !pkg_map.is_empty() {
            package_maps.insert(root.clone(), pkg_map);
        }
        if doc.get("workspace").is_some() {
            let ws_map = build_workspace_crate_map(files, root, text, doc, parsed, manifest_roots);
            workspace_maps.insert(root.clone(), ws_map);
        }
    }
    (package_maps, workspace_maps)
}

fn parse_cargo_manifests(
    manifests: &[Manifest],
) -> (HashMap<String, (String, Value)>, Vec<String>) {
    let mut parsed = HashMap::new();
    let mut manifest_roots = Vec::new();
    for manifest in manifests {
        let Some(root) = manifest_dir(&manifest.path) else {
            continue;
        };
        if root.split('/').any(|seg| seg == "target") {
            continue;
        }
        if let Ok(doc) = toml::from_str::<Value>(&manifest.text) {
            manifest_roots.push(root.clone());
            parsed.insert(root, (manifest.text.clone(), doc));
        }
    }
    (parsed, manifest_roots)
}

fn build_package_crate_map(
    files: &HashSet<String>,
    root: &str,
    doc: &Value,
) -> HashMap<String, Option<String>> {
    let mut pkg_map = HashMap::new();
    insert_crate_entry(files, root, doc, &mut pkg_map);
    collect_manifest_path_deps(files, root, doc, &mut pkg_map);
    pkg_map
}

fn build_workspace_crate_map(
    files: &HashSet<String>,
    root: &str,
    text: &str,
    doc: &Value,
    parsed: &HashMap<String, (String, Value)>,
    manifest_roots: &[String],
) -> HashMap<String, Option<String>> {
    let mut ws_map = HashMap::new();
    insert_crate_entry(files, root, doc, &mut ws_map);
    collect_workspace_dep_paths(files, root, doc, &mut ws_map);
    for member_root in workspace_members_under(root, text, manifest_roots) {
        if let Some((_, member_doc)) = parsed.get(&member_root) {
            insert_crate_entry(files, &member_root, member_doc, &mut ws_map);
        }
    }
    ws_map
}

fn manifest_dir(path: &str) -> Option<String> {
    if path == CARGO_MANIFEST {
        Some(String::new())
    } else {
        path.strip_suffix(&format!("/{CARGO_MANIFEST}"))
            .map(str::to_owned)
    }
}

fn insert_crate_entry(
    files: &HashSet<String>,
    root: &str,
    doc: &Value,
    out: &mut HashMap<String, Option<String>>,
) {
    let lib_rel = doc
        .get("lib")
        .and_then(|lib| lib.get("path"))
        .and_then(Value::as_str)
        .unwrap_or("src/lib.rs");
    let lib_path = safe_manifest_path(root, lib_rel).filter(|candidate| files.contains(candidate));
    let crate_name = doc
        .get("lib")
        .and_then(|lib| lib.get("name"))
        .and_then(Value::as_str)
        .or_else(|| {
            doc.get("package")
                .and_then(|pkg| pkg.get("name"))
                .and_then(Value::as_str)
        });
    if let Some(raw_name) = crate_name {
        let ident = normalize_crate_name(raw_name);
        if !ident.is_empty() {
            out.insert(ident, lib_path);
        }
    }
}

fn collect_workspace_dep_paths(
    files: &HashSet<String>,
    ws_root: &str,
    doc: &Value,
    out: &mut HashMap<String, Option<String>>,
) {
    let Some(ws_deps) = doc
        .get("workspace")
        .and_then(|ws| ws.get("dependencies"))
        .and_then(Value::as_table)
    else {
        return;
    };
    collect_path_dep_table(files, ws_root, ws_deps, out);
}

fn collect_manifest_path_deps(
    files: &HashSet<String>,
    root: &str,
    doc: &Value,
    out: &mut HashMap<String, Option<String>>,
) {
    for section in DEP_SECTIONS {
        if let Some(table) = doc.get(section).and_then(Value::as_table) {
            collect_path_dep_table(files, root, table, out);
        }
    }
    if let Some(targets) = doc.get("target").and_then(Value::as_table) {
        for cfg_table in targets.values() {
            for section in DEP_SECTIONS {
                if let Some(table) = cfg_table.get(section).and_then(Value::as_table) {
                    collect_path_dep_table(files, root, table, out);
                }
            }
        }
    }
}

fn collect_path_dep_table(
    files: &HashSet<String>,
    base_dir: &str,
    table: &toml::map::Map<String, Value>,
    out: &mut HashMap<String, Option<String>>,
) {
    for (dep_key, spec) in table {
        let Some(dep_table) = spec.as_table() else {
            continue;
        };
        let Some(rel_path) = dep_table.get("path").and_then(Value::as_str) else {
            continue;
        };
        let lib_path = normalize_relative_path(base_dir, rel_path).and_then(|target_dir| {
            let candidate = join(&target_dir, "src/lib.rs");
            files.contains(&candidate).then_some(candidate)
        });
        let key_ident = normalize_crate_name(dep_key);
        if !key_ident.is_empty() {
            out.entry(key_ident).or_insert_with(|| lib_path.clone());
        }
        if let Some(pkg) = dep_table.get("package").and_then(Value::as_str) {
            let pkg_ident = normalize_crate_name(pkg);
            if !pkg_ident.is_empty() {
                out.entry(pkg_ident).or_insert(lib_path);
            }
        }
    }
}

fn split_use_alias(use_spec: &str) -> (&str, Option<&str>) {
    match use_spec.rsplit_once(" as ") {
        Some((path, alias)) if !alias.is_empty() && !alias.contains("::") => (path, Some(alias)),
        _ => (use_spec, None),
    }
}

fn strip_glob_suffix(use_path: &str) -> &str {
    use_path.strip_suffix("::*").unwrap_or(use_path)
}

fn strip_raw_prefix(segment: &str) -> &str {
    segment.strip_prefix("r#").unwrap_or(segment)
}

fn record_use_binding(
    importer: &str,
    use_spec: &str,
    bindings: &mut HashMap<String, HashMap<String, Option<String>>>,
) {
    let (use_path, explicit_alias) = split_use_alias(use_spec);
    if explicit_alias == Some("_") || use_path.ends_with("::*") || use_path == "*" {
        return;
    }
    let bound_name = match explicit_alias {
        Some(alias) => strip_raw_prefix(alias),
        None => {
            let Some(last) = use_path.rsplit("::").find(|s| !s.is_empty()) else {
                return;
            };
            strip_raw_prefix(last)
        }
    };
    if bound_name.is_empty() || matches!(bound_name, "crate" | "self" | "super" | "_") {
        return;
    }
    let file_bindings = bindings.entry(importer.to_owned()).or_default();
    match file_bindings.get_mut(bound_name) {
        None => {
            file_bindings.insert(bound_name.to_owned(), Some(use_path.to_owned()));
        }
        Some(existing) if existing.as_deref() != Some(use_path) => {
            *existing = None;
        }
        Some(_) => {}
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

fn module_parent_dir(importer: &str) -> String {
    let file_name = importer.rsplit_once('/').map_or(importer, |(_, name)| name);
    let dir = parent_dir(importer);
    if file_name == "mod.rs" {
        parent_dir(&dir)
    } else {
        dir
    }
}
