use std::collections::{BTreeSet, HashMap, VecDeque};

use codeint_model::{CodeModel, ImportResolution, RepoPath, SourceSet};

#[derive(Debug, thiserror::Error)]
pub enum ImpactError {
    #[error("{path} is not in the measured Code model")]
    TargetNotInCodeModel { path: RepoPath },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImpactTarget {
    pub path: RepoPath,
    pub source_set: SourceSet,
    pub package: Option<String>,
    pub fan_in: usize,
    pub fan_out: usize,
    pub transitive_import_weight: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImpactDependent {
    pub path: RepoPath,
    pub distance: usize,
    pub source_set: SourceSet,
    pub package: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImpactSummary {
    pub direct_dependents: usize,
    pub transitive_dependents: usize,
    pub total_dependents: usize,
    pub fan_in: usize,
    pub fan_out: usize,
    pub transitive_import_weight: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImpactAnalysis {
    pub targets: Vec<ImpactTarget>,
    pub depth_limit: Option<usize>,
    pub summary: ImpactSummary,
    pub dependents: Vec<ImpactDependent>,
    pub affected_packages: Vec<String>,
}

impl ImpactAnalysis {
    pub fn compute(
        model: &CodeModel,
        targets: &[RepoPath],
        max_depth: Option<usize>,
    ) -> Result<ImpactAnalysis, ImpactError> {
        let index_of: HashMap<&RepoPath, usize> = model
            .files
            .iter()
            .enumerate()
            .map(|(index, file)| (&file.path, index))
            .collect();

        let mut unique_target_indices: BTreeSet<usize> = BTreeSet::new();
        for target in targets {
            let Some(&index) = index_of.get(target) else {
                return Err(ImpactError::TargetNotInCodeModel {
                    path: target.clone(),
                });
            };
            unique_target_indices.insert(index);
        }

        let mut forward: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); model.files.len()];
        let mut reverse: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); model.files.len()];
        for runtime_import in &model.runtime_imports {
            if let ImportResolution::Resolved(target) = &runtime_import.resolution
                && let (Some(&from), Some(&to)) =
                    (index_of.get(&runtime_import.importer), index_of.get(target))
                && from != to
            {
                forward[from].insert(to);
                reverse[to].insert(from);
            }
        }

        let has_workspace_members = model.workspace.has_members();
        let package_name = |path: &RepoPath| -> Option<String> {
            has_workspace_members.then(|| model.workspace.package_of(path).to_string())
        };

        let mut target_rows: Vec<ImpactTarget> = unique_target_indices
            .iter()
            .map(|&index| {
                let file = &model.files[index];
                ImpactTarget {
                    path: file.path.clone(),
                    source_set: file.model.source_set,
                    package: package_name(&file.path),
                    fan_in: reverse[index].len(),
                    fan_out: forward[index].len(),
                    transitive_import_weight: reverse_reachable_count(&reverse, index, max_depth),
                }
            })
            .collect();
        target_rows.sort_by(|left, right| left.path.cmp(&right.path));

        let distances = multi_source_reverse_bfs(&reverse, &unique_target_indices, max_depth);
        let mut dependents: Vec<ImpactDependent> = distances
            .into_iter()
            .enumerate()
            .filter_map(|(index, distance)| {
                let distance = distance?;
                if distance == 0 {
                    return None;
                }
                let file = &model.files[index];
                Some(ImpactDependent {
                    path: file.path.clone(),
                    distance,
                    source_set: file.model.source_set,
                    package: package_name(&file.path),
                })
            })
            .collect();
        dependents.sort_by(|left, right| {
            left.distance
                .cmp(&right.distance)
                .then_with(|| left.path.cmp(&right.path))
        });

        let direct_dependents = dependents.iter().filter(|dep| dep.distance == 1).count();
        let transitive_dependents = dependents.len() - direct_dependents;
        let total_dependents = dependents.len();

        let fan_out = unique_target_indices
            .iter()
            .flat_map(|&index| forward[index].iter().copied())
            .filter(|target_import| !unique_target_indices.contains(target_import))
            .collect::<BTreeSet<usize>>()
            .len();

        let affected_packages: Vec<String> = if has_workspace_members {
            dependents
                .iter()
                .filter_map(|dep| dep.package.clone())
                .collect::<BTreeSet<String>>()
                .into_iter()
                .collect()
        } else {
            Vec::new()
        };

        Ok(ImpactAnalysis {
            targets: target_rows,
            depth_limit: max_depth,
            summary: ImpactSummary {
                direct_dependents,
                transitive_dependents,
                total_dependents,
                fan_in: direct_dependents,
                fan_out,
                transitive_import_weight: total_dependents,
            },
            dependents,
            affected_packages,
        })
    }

    pub fn scoped_to_package(mut self, package: &str) -> ImpactAnalysis {
        self.dependents
            .retain(|dep| dep.package.as_deref() == Some(package));
        let direct_dependents = self
            .dependents
            .iter()
            .filter(|dep| dep.distance == 1)
            .count();
        let total_dependents = self.dependents.len();
        let transitive_dependents = total_dependents - direct_dependents;
        self.summary.direct_dependents = direct_dependents;
        self.summary.transitive_dependents = transitive_dependents;
        self.summary.total_dependents = total_dependents;
        self.summary.fan_in = direct_dependents;
        self.summary.transitive_import_weight = total_dependents;
        self.affected_packages = self
            .dependents
            .iter()
            .filter_map(|dep| dep.package.clone())
            .collect::<BTreeSet<String>>()
            .into_iter()
            .collect();
        self
    }
}

fn reverse_reachable_count(
    reverse: &[BTreeSet<usize>],
    start: usize,
    max_depth: Option<usize>,
) -> usize {
    let mut visited = vec![false; reverse.len()];
    visited[start] = true;
    let mut queue = VecDeque::new();
    queue.push_back((start, 0_usize));
    let mut count = 0;
    while let Some((node, depth)) = queue.pop_front() {
        if max_depth.is_some_and(|limit| depth >= limit) {
            continue;
        }
        for &neighbor in &reverse[node] {
            if !visited[neighbor] {
                visited[neighbor] = true;
                count += 1;
                queue.push_back((neighbor, depth + 1));
            }
        }
    }
    count
}

fn multi_source_reverse_bfs(
    reverse: &[BTreeSet<usize>],
    starts: &BTreeSet<usize>,
    max_depth: Option<usize>,
) -> Vec<Option<usize>> {
    let mut distances = vec![None; reverse.len()];
    let mut queue = VecDeque::new();
    for &start in starts {
        distances[start] = Some(0);
        queue.push_back(start);
    }
    while let Some(node) = queue.pop_front() {
        let depth = distances[node].unwrap_or(0);
        if max_depth.is_some_and(|limit| depth >= limit) {
            continue;
        }
        for &neighbor in &reverse[node] {
            if distances[neighbor].is_none() {
                distances[neighbor] = Some(depth + 1);
                queue.push_back(neighbor);
            }
        }
    }
    distances
}
