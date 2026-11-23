use std::collections::{BTreeMap, BTreeSet};

use codeint_model::{
    CodeModel, DeclaredDep, DepKind, ImportResolution, RepoPath, SourceSet, token_text_hash,
};

use crate::finding_id::{FindingId, FindingIds};
use crate::tooling_paths::ToolingPaths;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnusedDependencyKind {
    Runtime,
    Dev,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnusedDependencyFinding {
    pub id: FindingId,
    pub package_root: String,
    pub manifest: RepoPath,
    pub dependency: String,
    pub kind: UnusedDependencyKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnlistedDependencyFinding {
    pub id: FindingId,
    pub package_root: String,
    pub manifest: RepoPath,
    pub dependency: String,
    pub importers: Vec<RepoPath>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DevInProductionDependencyFinding {
    pub id: FindingId,
    pub package_root: String,
    pub manifest: RepoPath,
    pub dependency: String,
    pub importers: Vec<RepoPath>,
}

pub const UNUSED_DEPENDENCIES_METRIC_KEY: &str = "unused_dependencies";
pub const UNLISTED_DEPENDENCIES_METRIC_KEY: &str = "unlisted_dependencies";
pub const DEV_IN_PRODUCTION_DEPENDENCIES_METRIC_KEY: &str = "dev_in_production_dependencies";

impl UnusedDependencyKind {
    pub fn key(self) -> &'static str {
        match self {
            UnusedDependencyKind::Runtime => "runtime",
            UnusedDependencyKind::Dev => "dev",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DependencyHygiene {
    has_manifests: bool,
    declared_count: usize,
    imported_count: usize,
    production_imported_count: usize,
    unused: Vec<UnusedDependencyFinding>,
    unlisted: Vec<UnlistedDependencyFinding>,
    dev_in_production: Vec<DevInProductionDependencyFinding>,
}

#[derive(Clone, Copy, Default)]
struct DeclaredKinds {
    runtime: bool,
    dev: bool,
    optional: bool,
}

impl DeclaredKinds {
    fn record(&mut self, kind: DepKind) {
        match kind {
            DepKind::Runtime => self.runtime = true,
            DepKind::Dev => self.dev = true,
            DepKind::Optional => self.optional = true,
        }
    }

    fn unused_kind(self) -> Option<UnusedDependencyKind> {
        if self.runtime {
            Some(UnusedDependencyKind::Runtime)
        } else if self.dev {
            Some(UnusedDependencyKind::Dev)
        } else {
            None
        }
    }

    fn is_dev_only(self) -> bool {
        self.dev && !self.runtime && !self.optional
    }
}

#[derive(Default)]
struct PackageImports {
    used: BTreeSet<String>,
    checked: BTreeMap<String, BTreeSet<RepoPath>>,
    production: BTreeMap<String, BTreeSet<RepoPath>>,
}

struct MatchedImportFinding {
    id: FindingId,
    package_root: String,
    manifest: RepoPath,
    dependency: String,
    importers: Vec<RepoPath>,
}

impl DependencyHygiene {
    pub fn measure(model: &CodeModel, ignored: &BTreeSet<String>) -> DependencyHygiene {
        DependencyHygiene::measure_filtered(model, ignored, |_| true)
    }

    pub fn measure_filtered(
        model: &CodeModel,
        ignored: &BTreeSet<String>,
        include_package: impl Fn(&str) -> bool,
    ) -> DependencyHygiene {
        let manifests = manifest_map(model);
        if !manifests.keys().map(String::as_str).any(&include_package) {
            return DependencyHygiene::default();
        }
        let declared = collect_declared(&model.declared_dependencies, ignored);
        let js_workspace_members = collect_js_workspace_members(model, &manifests);
        let imports = collect_imports(model, &manifests, &declared, &js_workspace_members, ignored);
        let tables = DependencyTables {
            manifests: &manifests,
            declared: &declared,
            js_workspace_members: &js_workspace_members,
            imports: &imports,
        };
        let (declared_count, unused) = tables.build_unused(&include_package);
        let (imported_count, unlisted_raw) = tables.build_imported_findings(
            UNLISTED_DEPENDENCIES_METRIC_KEY,
            &include_package,
            |pkg| &pkg.checked,
            |declared_kinds| declared_kinds.is_none(),
        );
        let (production_imported_count, dev_raw) = tables.build_imported_findings(
            DEV_IN_PRODUCTION_DEPENDENCIES_METRIC_KEY,
            &include_package,
            |pkg| &pkg.production,
            |declared_kinds| declared_kinds.is_some_and(DeclaredKinds::is_dev_only),
        );
        DependencyHygiene {
            has_manifests: true,
            declared_count,
            imported_count,
            production_imported_count,
            unused,
            unlisted: unlisted_raw
                .into_iter()
                .map(|item| UnlistedDependencyFinding {
                    id: item.id,
                    package_root: item.package_root,
                    manifest: item.manifest,
                    dependency: item.dependency,
                    importers: item.importers,
                })
                .collect(),
            dev_in_production: dev_raw
                .into_iter()
                .map(|item| DevInProductionDependencyFinding {
                    id: item.id,
                    package_root: item.package_root,
                    manifest: item.manifest,
                    dependency: item.dependency,
                    importers: item.importers,
                })
                .collect(),
        }
    }

    pub fn merge(results: impl IntoIterator<Item = DependencyHygiene>) -> DependencyHygiene {
        let mut merged = DependencyHygiene::default();
        for item in results {
            merged.has_manifests |= item.has_manifests;
            merged.declared_count += item.declared_count;
            merged.imported_count += item.imported_count;
            merged.production_imported_count += item.production_imported_count;
            merged.unused.extend(item.unused);
            merged.unlisted.extend(item.unlisted);
            merged.dev_in_production.extend(item.dev_in_production);
        }
        sort_by_key_triple(&mut merged.unused, |item| {
            (&item.package_root, &item.manifest, &item.dependency)
        });
        sort_by_key_triple(&mut merged.unlisted, |item| {
            (&item.package_root, &item.manifest, &item.dependency)
        });
        sort_by_key_triple(&mut merged.dev_in_production, |item| {
            (&item.package_root, &item.manifest, &item.dependency)
        });
        merged
    }

    pub fn has_manifests(&self) -> bool {
        self.has_manifests
    }

    pub fn has_activity(&self) -> bool {
        self.has_manifests
            && (self.declared_count > 0
                || self.imported_count > 0
                || !self.unused.is_empty()
                || !self.unlisted.is_empty()
                || !self.dev_in_production.is_empty())
    }

    pub fn declared_count(&self) -> usize {
        self.declared_count
    }

    pub fn imported_count(&self) -> usize {
        self.imported_count
    }

    pub fn production_imported_count(&self) -> usize {
        self.production_imported_count
    }

    pub fn unused(&self) -> &[UnusedDependencyFinding] {
        &self.unused
    }

    pub fn unlisted(&self) -> &[UnlistedDependencyFinding] {
        &self.unlisted
    }

    pub fn dev_in_production(&self) -> &[DevInProductionDependencyFinding] {
        &self.dev_in_production
    }

    pub fn unused_ratio(&self) -> f64 {
        ratio(self.unused.len(), self.declared_count)
    }

    pub fn unlisted_ratio(&self) -> f64 {
        ratio(self.unlisted.len(), self.imported_count)
    }

    pub fn dev_in_production_ratio(&self) -> f64 {
        ratio(self.dev_in_production.len(), self.production_imported_count)
    }
}

fn sort_by_key_triple<T>(items: &mut [T], key: impl Fn(&T) -> (&String, &RepoPath, &String)) {
    items.sort_by(|left, right| key(left).cmp(&key(right)));
}

fn manifest_map(model: &CodeModel) -> BTreeMap<String, RepoPath> {
    let mut manifests = BTreeMap::new();
    for entry in &model.dependency_manifests {
        manifests
            .entry(entry.package_root.clone())
            .or_insert_with(|| entry.path.clone());
    }
    for dep in &model.declared_dependencies {
        manifests
            .entry(dep.package_root.clone())
            .or_insert_with(|| fallback_manifest_path(&dep.package_root));
    }
    manifests
}

fn fallback_manifest_path(package_root: &str) -> RepoPath {
    let raw = if package_root.is_empty() {
        "manifest".to_owned()
    } else {
        format!("{package_root}/manifest")
    };
    RepoPath::new(raw).expect("validated package root produces a valid fallback path")
}

fn collect_declared(
    declared: &[DeclaredDep],
    ignored: &BTreeSet<String>,
) -> BTreeMap<String, BTreeMap<String, DeclaredKinds>> {
    let mut by_package: BTreeMap<String, BTreeMap<String, DeclaredKinds>> = BTreeMap::new();
    for dep in declared {
        if ignored.contains(&dep.name) {
            continue;
        }
        by_package
            .entry(dep.package_root.clone())
            .or_default()
            .entry(dep.name.clone())
            .or_default()
            .record(dep.kind);
    }
    by_package
}

fn collect_js_workspace_members(
    model: &CodeModel,
    manifests: &BTreeMap<String, RepoPath>,
) -> BTreeSet<String> {
    if !is_package_json(manifests, "") {
        return BTreeSet::new();
    }
    model
        .workspace
        .member_roots()
        .map(RepoPath::as_str)
        .filter(|owner| !owner.is_empty() && is_package_json(manifests, owner))
        .map(str::to_owned)
        .collect()
}

fn is_package_json(manifests: &BTreeMap<String, RepoPath>, root: &str) -> bool {
    manifests
        .get(root)
        .is_some_and(|path| path.file_name() == "package.json")
}

fn is_hoisted_from_root(
    owner: &str,
    package_name: &str,
    declared: &BTreeMap<String, BTreeMap<String, DeclaredKinds>>,
    js_workspace_members: &BTreeSet<String>,
) -> bool {
    js_workspace_members.contains(owner)
        && !declared
            .get(owner)
            .is_some_and(|deps| deps.contains_key(package_name))
        && declared
            .get("")
            .is_some_and(|deps| deps.contains_key(package_name))
}

fn collect_imports(
    model: &CodeModel,
    manifests: &BTreeMap<String, RepoPath>,
    declared: &BTreeMap<String, BTreeMap<String, DeclaredKinds>>,
    js_workspace_members: &BTreeSet<String>,
    ignored: &BTreeSet<String>,
) -> BTreeMap<String, PackageImports> {
    let tooling = ToolingPaths::from_model(model);
    let source_sets: BTreeMap<&RepoPath, SourceSet> = model
        .files
        .iter()
        .map(|file| (&file.path, file.model.source_set))
        .collect();
    let mut by_package: BTreeMap<String, PackageImports> = BTreeMap::new();
    for import in &model.runtime_imports {
        let ImportResolution::External(package_name) = &import.resolution else {
            continue;
        };
        if ignored.contains(package_name) {
            continue;
        }
        let Some(&source_set) = source_sets.get(&import.importer) else {
            continue;
        };
        if source_set == SourceSet::Vendored {
            continue;
        }
        let Some(owner) = package_root_of(&import.importer, manifests) else {
            continue;
        };
        record_package_import(
            by_package.entry(owner.to_owned()).or_default(),
            source_set,
            package_name,
            &import.importer,
            &tooling,
        );
        if is_hoisted_from_root(owner, package_name, declared, js_workspace_members) {
            by_package
                .entry(String::new())
                .or_default()
                .used
                .insert(package_name.to_owned());
        }
    }
    record_manifest_used_dependencies(model, manifests, ignored, &mut by_package);
    by_package
}

fn record_manifest_used_dependencies(
    model: &CodeModel,
    manifests: &BTreeMap<String, RepoPath>,
    ignored: &BTreeSet<String>,
    by_package: &mut BTreeMap<String, PackageImports>,
) {
    for entry in &model.manifest_used_dependencies {
        if ignored.contains(&entry.name) || !manifests.contains_key(&entry.package_root) {
            continue;
        }
        by_package
            .entry(entry.package_root.clone())
            .or_default()
            .used
            .insert(entry.name.clone());
    }
}

fn record_package_import(
    entry: &mut PackageImports,
    source_set: SourceSet,
    package_name: &str,
    importer: &RepoPath,
    tooling: &ToolingPaths,
) {
    entry.used.insert(package_name.to_owned());
    if matches!(source_set, SourceSet::Production | SourceSet::Test) {
        entry
            .checked
            .entry(package_name.to_owned())
            .or_default()
            .insert(importer.clone());
    }
    if source_set == SourceSet::Production && !tooling.is_non_runtime_tooling_path(importer) {
        entry
            .production
            .entry(package_name.to_owned())
            .or_default()
            .insert(importer.clone());
    }
}

fn package_root_of<'a>(
    path: &RepoPath,
    manifests: &'a BTreeMap<String, RepoPath>,
) -> Option<&'a str> {
    let mut folder = path.as_str();
    while let Some((parent, _)) = folder.rsplit_once('/') {
        if let Some((root, _)) = manifests.get_key_value(parent) {
            return Some(root.as_str());
        }
        folder = parent;
    }
    manifests.get_key_value("").map(|(root, _)| root.as_str())
}

fn dependency_finding_id(
    ids: &mut FindingIds,
    manifest: &RepoPath,
    package_root: &str,
    dependency: &str,
) -> FindingId {
    ids.next_in_source_order(
        [manifest],
        [token_text_hash(package_root), token_text_hash(dependency)],
    )
}

struct DependencyTables<'a> {
    manifests: &'a BTreeMap<String, RepoPath>,
    declared: &'a BTreeMap<String, BTreeMap<String, DeclaredKinds>>,
    js_workspace_members: &'a BTreeSet<String>,
    imports: &'a BTreeMap<String, PackageImports>,
}

impl DependencyTables<'_> {
    fn effective_declared(&self, package_root: &str, name: &str) -> Option<DeclaredKinds> {
        if let Some(kinds) = self
            .declared
            .get(package_root)
            .and_then(|deps| deps.get(name))
        {
            return Some(*kinds);
        }
        if self.js_workspace_members.contains(package_root) {
            return self
                .declared
                .get("")
                .and_then(|deps| deps.get(name))
                .copied();
        }
        None
    }

    fn build_unused(
        &self,
        include_package: impl Fn(&str) -> bool,
    ) -> (usize, Vec<UnusedDependencyFinding>) {
        let mut ids = FindingIds::for_key(UNUSED_DEPENDENCIES_METRIC_KEY);
        let mut declared_count = 0usize;
        let findings = self
            .declared
            .iter()
            .filter(|(root, _)| include_package(root))
            .flat_map(|(root, deps)| deps.iter().map(move |(dep, kinds)| (root, dep, kinds)))
            .filter_map(|(package_root, dependency, kinds)| {
                let kind = kinds.unused_kind()?;
                declared_count += 1;
                let used = self.imports.get(package_root);
                if used.is_some_and(|entry| entry.used.contains(dependency)) {
                    return None;
                }
                let manifest = &self.manifests[package_root];
                Some(UnusedDependencyFinding {
                    id: dependency_finding_id(&mut ids, manifest, package_root, dependency),
                    package_root: package_root.clone(),
                    manifest: manifest.clone(),
                    dependency: dependency.clone(),
                    kind,
                })
            })
            .collect();
        (declared_count, findings)
    }

    fn build_imported_findings(
        &self,
        metric_key: &'static str,
        include_package: impl Fn(&str) -> bool,
        select: impl Fn(&PackageImports) -> &BTreeMap<String, BTreeSet<RepoPath>>,
        should_flag: impl Fn(Option<DeclaredKinds>) -> bool,
    ) -> (usize, Vec<MatchedImportFinding>) {
        let mut ids = FindingIds::for_key(metric_key);
        let mut total = 0usize;
        let mut findings = Vec::new();
        for (package_root, package_imports) in self.imports {
            if !include_package(package_root) {
                continue;
            }
            let manifest = &self.manifests[package_root];
            for (dependency, importers) in select(package_imports) {
                total += 1;
                let declared_kinds = self.effective_declared(package_root, dependency);
                if should_flag(declared_kinds) {
                    findings.push(MatchedImportFinding {
                        id: dependency_finding_id(&mut ids, manifest, package_root, dependency),
                        package_root: package_root.clone(),
                        manifest: manifest.clone(),
                        dependency: dependency.clone(),
                        importers: importers.iter().cloned().collect(),
                    });
                }
            }
        }
        (total, findings)
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}
