use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use codeint_model::{CodeModel, ImportResolution, Metric, Package, RepoPath, SourceSet};

use crate::finding_id::{FindingId, FindingIds};
use crate::tooling_paths::ToolingPaths;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OrphanFileKind {
    Unreferenced,
    TestOnly,
}

impl OrphanFileKind {
    pub fn key(self) -> &'static str {
        match self {
            OrphanFileKind::Unreferenced => "unreferenced",
            OrphanFileKind::TestOnly => "test-only",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrphanFileFinding {
    pub id: FindingId,
    pub path: RepoPath,
    pub kind: OrphanFileKind,
    pub test_importers: Vec<RepoPath>,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrphanFiles {
    entry_points: Vec<RepoPath>,
    evaluated_files: usize,
    findings: Vec<OrphanFileFinding>,
}

struct ImportGraphs {
    forward: Vec<Vec<usize>>,
    reverse_non_test: Vec<Vec<usize>>,
    direct_test_importers: Vec<BTreeSet<usize>>,
}

struct PendingOrphan {
    path: RepoPath,
    kind: OrphanFileKind,
    test_importers: Vec<RepoPath>,
    message: String,
}

impl OrphanFiles {
    pub fn measure(model: &CodeModel, entry_points: &[RepoPath]) -> Option<OrphanFiles> {
        OrphanFiles::measure_filtered(model, entry_points, |_| true)
    }

    pub fn measure_filtered(
        model: &CodeModel,
        entry_points: &[RepoPath],
        include_in_report: impl Fn(&RepoPath) -> bool,
    ) -> Option<OrphanFiles> {
        let index_of: HashMap<&RepoPath, usize> = model
            .files
            .iter()
            .enumerate()
            .map(|(index, file)| (&file.path, index))
            .collect();
        let source_sets: Vec<SourceSet> = model
            .files
            .iter()
            .map(|file| file.model.source_set)
            .collect();
        let tooling = ToolingPaths::from_model(model);
        let roots = collect_roots(&index_of, &source_sets, entry_points, &tooling);
        if roots.is_empty() {
            return None;
        }
        let graphs = build_import_graphs(model, &index_of, &source_sets);
        let live = traverse_live(&roots, &graphs.forward, &source_sets);
        let active_packages = active_packages_of(model, &live);
        let reached_by_test = traverse_test_reachable(
            &graphs.direct_test_importers,
            &graphs.forward,
            &live,
            &source_sets,
        );

        let mut evaluated_files = 0usize;
        let mut pending = Vec::new();
        for (index, file) in model.files.iter().enumerate() {
            if !is_evaluated_production_file(
                model,
                file.model.source_set,
                &file.path,
                &tooling,
                active_packages.as_ref(),
                &include_in_report,
            ) {
                continue;
            }
            evaluated_files += 1;
            if live[index] {
                continue;
            }
            pending.push(build_pending_orphan(
                index,
                &file.path,
                reached_by_test[index],
                &graphs,
                &live,
                model,
            ));
        }

        if evaluated_files == 0 {
            return None;
        }

        let mut reported_roots: Vec<RepoPath> = roots
            .iter()
            .map(|&index| model.files[index].path.clone())
            .filter(&include_in_report)
            .collect();
        reported_roots.sort();
        reported_roots.dedup();

        Some(OrphanFiles {
            entry_points: reported_roots,
            evaluated_files,
            findings: assign_finding_ids(pending),
        })
    }

    pub fn merge(measurements: impl IntoIterator<Item = OrphanFiles>) -> Option<OrphanFiles> {
        let mut iter = measurements.into_iter().peekable();
        iter.peek()?;
        let mut entry_points = BTreeSet::new();
        let mut evaluated_files = 0usize;
        let mut findings = Vec::new();
        for measurement in iter {
            evaluated_files += measurement.evaluated_files;
            entry_points.extend(measurement.entry_points);
            findings.extend(measurement.findings);
        }
        if evaluated_files == 0 {
            return None;
        }
        findings.sort_by(|left, right| left.path.cmp(&right.path));
        findings.dedup_by(|left, right| left.path == right.path);
        Some(OrphanFiles {
            entry_points: entry_points.into_iter().collect(),
            evaluated_files,
            findings,
        })
    }

    pub fn ratio(&self) -> f64 {
        if self.evaluated_files > 0 {
            self.findings.len() as f64 / self.evaluated_files as f64
        } else {
            0.0
        }
    }

    pub fn entry_points(&self) -> &[RepoPath] {
        &self.entry_points
    }

    pub fn evaluated_files(&self) -> usize {
        self.evaluated_files
    }

    pub fn findings(&self) -> &[OrphanFileFinding] {
        &self.findings
    }

    pub fn unreferenced_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.kind == OrphanFileKind::Unreferenced)
            .count()
    }

    pub fn test_only_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.kind == OrphanFileKind::TestOnly)
            .count()
    }
}

fn collect_roots(
    index_of: &HashMap<&RepoPath, usize>,
    source_sets: &[SourceSet],
    entry_points: &[RepoPath],
    tooling: &ToolingPaths,
) -> BTreeSet<usize> {
    let mut roots = BTreeSet::new();
    for entry in entry_points {
        if let Some(&index) = index_of.get(entry)
            && source_sets[index] == SourceSet::Production
            && !tooling.is_non_runtime_tooling_path(entry)
        {
            roots.insert(index);
        }
    }
    roots
}

fn build_import_graphs(
    model: &CodeModel,
    index_of: &HashMap<&RepoPath, usize>,
    source_sets: &[SourceSet],
) -> ImportGraphs {
    let count = model.files.len();
    let mut forward = vec![Vec::new(); count];
    let mut reverse_non_test = vec![Vec::new(); count];
    let mut direct_test_importers = vec![BTreeSet::new(); count];
    for runtime_import in &model.runtime_imports {
        let ImportResolution::Resolved(targets) = &runtime_import.resolution else {
            continue;
        };
        let Some(&from) = index_of.get(&runtime_import.importer) else {
            continue;
        };
        for target in targets {
            let Some(&to) = index_of.get(target) else {
                continue;
            };
            if source_sets[from] == SourceSet::Test {
                if source_sets[to] != SourceSet::Test {
                    direct_test_importers[to].insert(from);
                }
            } else {
                forward[from].push(to);
                if source_sets[to] != SourceSet::Test {
                    reverse_non_test[to].push(from);
                }
            }
        }
    }
    ImportGraphs {
        forward,
        reverse_non_test,
        direct_test_importers,
    }
}

fn traverse_live(
    roots: &BTreeSet<usize>,
    forward: &[Vec<usize>],
    source_sets: &[SourceSet],
) -> Vec<bool> {
    let mut visited = vec![false; forward.len()];
    let mut queue = VecDeque::new();
    for &root in roots {
        visited[root] = true;
        queue.push_back(root);
    }
    while let Some(node) = queue.pop_front() {
        for &target in &forward[node] {
            if !visited[target] && source_sets[target] != SourceSet::Test {
                visited[target] = true;
                queue.push_back(target);
            }
        }
    }
    visited
}

fn active_packages_of<'model>(
    model: &'model CodeModel,
    live: &[bool],
) -> Option<HashSet<Package<'model>>> {
    if !model.workspace.has_members() {
        return None;
    }
    let active = live
        .iter()
        .enumerate()
        .filter(|&(_, &is_live)| is_live)
        .map(|(index, _)| model.workspace.package_of(&model.files[index].path))
        .collect();
    Some(active)
}

fn traverse_test_reachable(
    direct_test_importers: &[BTreeSet<usize>],
    forward: &[Vec<usize>],
    live: &[bool],
    source_sets: &[SourceSet],
) -> Vec<bool> {
    let mut reached = vec![false; forward.len()];
    let mut queue = VecDeque::new();
    for (node, importers) in direct_test_importers.iter().enumerate() {
        if !importers.is_empty() && !live[node] && source_sets[node] != SourceSet::Test {
            reached[node] = true;
            queue.push_back(node);
        }
    }
    while let Some(node) = queue.pop_front() {
        for &target in &forward[node] {
            if !reached[target] && !live[target] && source_sets[target] != SourceSet::Test {
                reached[target] = true;
                queue.push_back(target);
            }
        }
    }
    reached
}

fn collect_reaching_test_importers(
    start: usize,
    direct_test_importers: &[BTreeSet<usize>],
    reverse_non_test: &[Vec<usize>],
    live: &[bool],
    model: &CodeModel,
) -> Vec<RepoPath> {
    if !direct_test_importers[start].is_empty() {
        let mut paths: Vec<RepoPath> = direct_test_importers[start]
            .iter()
            .map(|&index| model.files[index].path.clone())
            .collect();
        paths.sort();
        return paths;
    }
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut test_indices = BTreeSet::new();
    visited.insert(start);
    queue.push_back(start);
    while let Some(node) = queue.pop_front() {
        test_indices.extend(direct_test_importers[node].iter().copied());
        for &importer in &reverse_non_test[node] {
            if !live[importer] && visited.insert(importer) {
                queue.push_back(importer);
            }
        }
    }
    let mut paths: Vec<RepoPath> = test_indices
        .into_iter()
        .map(|index| model.files[index].path.clone())
        .collect();
    paths.sort();
    paths
}

fn is_evaluated_production_file<'model>(
    model: &'model CodeModel,
    source_set: SourceSet,
    path: &RepoPath,
    tooling: &ToolingPaths,
    active_packages: Option<&HashSet<Package<'model>>>,
    include_in_report: &impl Fn(&RepoPath) -> bool,
) -> bool {
    if source_set != SourceSet::Production || tooling.is_non_runtime_tooling_path(path) {
        return false;
    }
    if let Some(active) = active_packages
        && !active.contains(&model.workspace.package_of(path))
    {
        return false;
    }
    include_in_report(path)
}

fn build_pending_orphan(
    index: usize,
    path: &RepoPath,
    reached_by_test: bool,
    graphs: &ImportGraphs,
    live: &[bool],
    model: &CodeModel,
) -> PendingOrphan {
    let (kind, test_importers, message) = if reached_by_test {
        let importers = collect_reaching_test_importers(
            index,
            &graphs.direct_test_importers,
            &graphs.reverse_non_test,
            live,
            model,
        );
        let message = format!(
            "Production file {path} is unreachable from Production entry points (only imported by tests)"
        );
        (OrphanFileKind::TestOnly, importers, message)
    } else {
        let message = format!("Production file {path} is unreachable from any entry point");
        (OrphanFileKind::Unreferenced, Vec::new(), message)
    };
    PendingOrphan {
        path: path.clone(),
        kind,
        test_importers,
        message,
    }
}

fn assign_finding_ids(mut pending: Vec<PendingOrphan>) -> Vec<OrphanFileFinding> {
    pending.sort_by(|left, right| left.path.cmp(&right.path));
    let mut ids = FindingIds::new(Metric::OrphanFiles);
    pending
        .into_iter()
        .map(|item| {
            let id = ids.next_in_source_order([&item.path], []);
            OrphanFileFinding {
                id,
                path: item.path,
                kind: item.kind,
                test_importers: item.test_importers,
                message: item.message,
            }
        })
        .collect()
}
