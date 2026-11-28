use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use codeint_model::{
    CodeModel, ImportResolution, Metric, Package, RepoPath, SourceSet, UniversalToolingPaths,
};

use crate::finding_id::{FindingContext, FindingId, FindingIds};

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
        OrphanFiles::measure_with_explicit_entry_points(model, entry_points, &[])
    }

    pub fn measure_with_explicit_entry_points(
        model: &CodeModel,
        entry_points: &[RepoPath],
        explicit_entry_points: &[RepoPath],
    ) -> Option<OrphanFiles> {
        OrphanFiles::measure_filtered_with_explicit_entry_points(
            model,
            entry_points,
            explicit_entry_points,
            |_| true,
        )
    }

    pub fn measure_filtered(
        model: &CodeModel,
        entry_points: &[RepoPath],
        include_in_report: impl Fn(&RepoPath) -> bool,
    ) -> Option<OrphanFiles> {
        OrphanFiles::measure_filtered_with_explicit_entry_points(
            model,
            entry_points,
            &[],
            include_in_report,
        )
    }

    pub fn measure_filtered_with_explicit_entry_points(
        model: &CodeModel,
        entry_points: &[RepoPath],
        explicit_entry_points: &[RepoPath],
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
        let tooling = UniversalToolingPaths::from_model(model);
        let is_tooling: Vec<bool> = model
            .files
            .iter()
            .map(|file| tooling.is_tooling_file(file))
            .collect();
        let explicit_roots: HashSet<usize> = explicit_entry_points
            .iter()
            .filter_map(|entry| index_of.get(entry).copied())
            .filter(|&index| source_sets[index] == SourceSet::Production)
            .collect();
        let roots = collect_roots(
            &index_of,
            &source_sets,
            entry_points,
            &explicit_roots,
            &is_tooling,
        );
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
            let excluded_tooling = is_tooling[index] && !explicit_roots.contains(&index);
            if !is_evaluated_production_file(
                model,
                file.model.source_set,
                excluded_tooling,
                &file.path,
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
    explicit_roots: &HashSet<usize>,
    is_tooling: &[bool],
) -> BTreeSet<usize> {
    let mut roots: BTreeSet<usize> = explicit_roots.iter().copied().collect();
    for entry in entry_points {
        if let Some(&index) = index_of.get(entry)
            && source_sets[index] == SourceSet::Production
            && !is_tooling[index]
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
        .map(|members| collect_direct_scc_importers(members, direct_test_importers))
        .collect();
    let mut seen_target_scc = vec![usize::MAX; sccs.len()];
    for scc_id in (0..sccs.len()).rev() {
        if scc_importers[scc_id].is_empty() {
            continue;
        }
        propagate_scc_importers(
            scc_id,
            &sccs[scc_id],
            forward,
            reached_by_test,
            &scc_of,
            &mut scc_importers,
            &mut seen_target_scc,
        );
    }
    ReachingTestImporters {
        scc_of,
        scc_importers,
    }
}

fn collect_direct_scc_importers(
    members: &[usize],
    direct_test_importers: &[BTreeSet<usize>],
) -> BTreeSet<usize> {
    let mut importers = BTreeSet::new();
    for &member in members {
        importers.extend(direct_test_importers[member].iter().copied());
    }
    importers
}

fn propagate_scc_importers(
    scc_id: usize,
    members: &[usize],
    forward: &[Vec<usize>],
    reached_by_test: &[bool],
    scc_of: &[usize],
    scc_importers: &mut [BTreeSet<usize>],
    seen_target_scc: &mut [usize],
) {
    let (lower, upper) = scc_importers.split_at_mut(scc_id);
    let source_importers = &upper[0];
    for &member in members {
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

struct TarjanState<'a> {
    forward: &'a [Vec<usize>],
    reached_by_test: &'a [bool],
    next_index: usize,
    index: Vec<Option<usize>>,
    lowlink: Vec<usize>,
    on_stack: Vec<bool>,
    tarjan_stack: Vec<usize>,
    call_stack: Vec<(usize, usize)>,
    scc_of: Vec<usize>,
    sccs: Vec<Vec<usize>>,
}

impl<'a> TarjanState<'a> {
    fn new(forward: &'a [Vec<usize>], reached_by_test: &'a [bool], count: usize) -> Self {
        Self {
            forward,
            reached_by_test,
            next_index: 0,
            index: vec![None; count],
            lowlink: vec![0; count],
            on_stack: vec![false; count],
            tarjan_stack: Vec::new(),
            call_stack: Vec::new(),
            scc_of: vec![0; count],
            sccs: Vec::new(),
        }
    }

    fn visit_node(&mut self, node: usize) {
        self.index[node] = Some(self.next_index);
        self.lowlink[node] = self.next_index;
        self.next_index += 1;
        self.tarjan_stack.push(node);
        self.on_stack[node] = true;
        self.call_stack.push((node, 0));
    }

    fn step_current_frame(&mut self) {
        let frame_idx = self.call_stack.len() - 1;
        let (current, start_edge) = self.call_stack[frame_idx];
        for (offset, &target) in self.forward[current][start_edge..].iter().enumerate() {
            if !self.reached_by_test[target] {
                continue;
            }
            if let Some(target_index) = self.index[target] {
                if self.on_stack[target] {
                    self.lowlink[current] = self.lowlink[current].min(target_index);
                }
            } else {
                self.call_stack[frame_idx].1 = start_edge + offset + 1;
                self.visit_node(target);
                return;
            }
        }
        self.call_stack.pop();
        self.finish_node(current);
    }

    fn finish_node(&mut self, current: usize) {
        if Some(self.lowlink[current]) == self.index[current] {
            self.pop_scc_component(current);
        }
        if let Some(&(parent, _)) = self.call_stack.last() {
            self.lowlink[parent] = self.lowlink[parent].min(self.lowlink[current]);
        }
    }

    fn pop_scc_component(&mut self, root: usize) {
        let scc_id = self.sccs.len();
        let mut members = Vec::new();
        while let Some(member) = self.tarjan_stack.pop() {
            self.on_stack[member] = false;
            self.scc_of[member] = scc_id;
            members.push(member);
            if member == root {
                break;
            }
        }
        self.sccs.push(members);
    }
}

fn tarjan_test_reachable_sccs(
    forward: &[Vec<usize>],
    reached_by_test: &[bool],
    count: usize,
) -> (Vec<usize>, Vec<Vec<usize>>) {
    let mut state = TarjanState::new(forward, reached_by_test, count);
    for (start, &reached) in reached_by_test.iter().enumerate().take(count) {
        if !reached || state.index[start].is_some() {
            continue;
        }
        state.visit_node(start);
        while !state.call_stack.is_empty() {
            state.step_current_frame();
        }
    }
    (state.scc_of, state.sccs)
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
    excluded_tooling: bool,
    path: &RepoPath,
    active_packages: Option<&HashSet<Package<'model>>>,
    include_in_report: &impl Fn(&RepoPath) -> bool,
) -> bool {
    if source_set != SourceSet::Production || excluded_tooling {
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
