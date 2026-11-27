use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use codeint_model::{CodeModel, ImportResolution, Metric, Package, RepoPath, SourceSet};

use crate::finding_id::{FindingContext, FindingId, FindingIds};
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
    pub context: FindingContext,
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
    direct_test_importers: Vec<BTreeSet<usize>>,
}

struct ReachingTestImporters {
    scc_of: Vec<usize>,
    scc_importers: Vec<BTreeSet<usize>>,
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
        let reaching = compute_reaching_test_importers(
            &graphs.direct_test_importers,
            &graphs.forward,
            &reached_by_test,
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
                &graphs.direct_test_importers,
                &reaching,
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
            }
        }
    }
    ImportGraphs {
        forward,
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

fn compute_reaching_test_importers(
    direct_test_importers: &[BTreeSet<usize>],
    forward: &[Vec<usize>],
    reached_by_test: &[bool],
) -> ReachingTestImporters {
    let count = forward.len();
    let (scc_of, sccs) = tarjan_test_reachable_sccs(forward, reached_by_test, count);
    let mut scc_importers: Vec<BTreeSet<usize>> = sccs
        .iter()
        .map(|members| {
            let mut importers = BTreeSet::new();
            for &member in members {
                importers.extend(direct_test_importers[member].iter().copied());
            }
            importers
        })
        .collect();
    let mut seen_target_scc = vec![usize::MAX; sccs.len()];
    for scc_id in (0..sccs.len()).rev() {
        if scc_importers[scc_id].is_empty() {
            continue;
        }
        let (lower, upper) = scc_importers.split_at_mut(scc_id);
        let source_importers = &upper[0];
        for &member in &sccs[scc_id] {
            for &target in &forward[member] {
                if !reached_by_test[target] {
                    continue;
                }
                let target_scc = scc_of[target];
                if target_scc != scc_id && seen_target_scc[target_scc] != scc_id {
                    seen_target_scc[target_scc] = scc_id;
                    lower[target_scc].extend(source_importers.iter().copied());
                }
            }
        }
    }
    ReachingTestImporters {
        scc_of,
        scc_importers,
    }
}

fn tarjan_test_reachable_sccs(
    forward: &[Vec<usize>],
    reached_by_test: &[bool],
    count: usize,
) -> (Vec<usize>, Vec<Vec<usize>>) {
    let mut next_index = 0usize;
    let mut index = vec![None; count];
    let mut lowlink = vec![0usize; count];
    let mut on_stack = vec![false; count];
    let mut tarjan_stack = Vec::new();
    let mut call_stack = Vec::new();
    let mut scc_of = vec![0usize; count];
    let mut sccs = Vec::new();

    for start in 0..count {
        if !reached_by_test[start] || index[start].is_some() {
            continue;
        }
        index[start] = Some(next_index);
        lowlink[start] = next_index;
        next_index += 1;
        tarjan_stack.push(start);
        on_stack[start] = true;
        call_stack.push((start, 0usize));

        while let Some((node, edge_idx)) = call_stack.last_mut() {
            let current = *node;
            let mut descended = false;
            while *edge_idx < forward[current].len() {
                let target = forward[current][*edge_idx];
                *edge_idx += 1;
                if !reached_by_test[target] {
                    continue;
                }
                if let Some(target_index) = index[target] {
                    if on_stack[target] {
                        lowlink[current] = lowlink[current].min(target_index);
                    }
                } else {
                    index[target] = Some(next_index);
                    lowlink[target] = next_index;
                    next_index += 1;
                    tarjan_stack.push(target);
                    on_stack[target] = true;
                    call_stack.push((target, 0));
                    descended = true;
                    break;
                }
            }
            if descended {
                continue;
            }
            call_stack.pop();
            if Some(lowlink[current]) == index[current] {
                let scc_id = sccs.len();
                let mut members = Vec::new();
                while let Some(member) = tarjan_stack.pop() {
                    on_stack[member] = false;
                    scc_of[member] = scc_id;
                    members.push(member);
                    if member == current {
                        break;
                    }
                }
                sccs.push(members);
            }
            if let Some(&(parent, _)) = call_stack.last() {
                lowlink[parent] = lowlink[parent].min(lowlink[current]);
            }
        }
    }
    (scc_of, sccs)
}

fn sorted_importer_paths(indices: &BTreeSet<usize>, model: &CodeModel) -> Vec<RepoPath> {
    let mut paths: Vec<RepoPath> = indices
        .iter()
        .map(|&index| model.files[index].path.clone())
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
    direct_test_importers: &[BTreeSet<usize>],
    reaching: &ReachingTestImporters,
    model: &CodeModel,
) -> PendingOrphan {
    let (kind, test_importers, message) = if reached_by_test {
        let importer_indices = if !direct_test_importers[index].is_empty() {
            &direct_test_importers[index]
        } else {
            &reaching.scc_importers[reaching.scc_of[index]]
        };
        let importers = sorted_importer_paths(importer_indices, model);
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
            let (id, content_hash) = ids.next_with_content_hash([&item.path], []);
            OrphanFileFinding {
                id,
                context: FindingContext::content_only(content_hash),
                path: item.path,
                kind: item.kind,
                test_importers: item.test_importers,
                message: item.message,
            }
        })
        .collect()
}
