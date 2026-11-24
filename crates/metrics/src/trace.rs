use std::collections::{BTreeMap, VecDeque};
use std::error::Error;
use std::fmt;

use codeint_model::{CodeModel, ImportResolution, RepoPath};

#[cfg(test)]
mod tests;

pub const DEFAULT_MAX_TRACE_PATHS: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceError {
    UnknownModule { path: RepoPath },
    InvalidMaxPaths,
}

impl fmt::Display for TraceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TraceError::UnknownModule { path } => {
                write!(formatter, "file {path} is not in the measured Code model")
            }
            TraceError::InvalidMaxPaths => write!(formatter, "max_paths must be at least 1"),
        }
    }
}

impl Error for TraceError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceResult {
    pub from: RepoPath,
    pub to: RepoPath,
    pub connected: bool,
    pub shortest_distance: Option<usize>,
    pub paths: Vec<Vec<RepoPath>>,
    pub truncated: bool,
}

pub fn trace_imports(
    model: &CodeModel,
    from: &RepoPath,
    to: &RepoPath,
    all_shortest: bool,
    max_paths: usize,
) -> Result<TraceResult, TraceError> {
    if max_paths == 0 {
        return Err(TraceError::InvalidMaxPaths);
    }
    let mut modules: Vec<&RepoPath> = model.files.iter().map(|file| &file.path).collect();
    modules.sort();
    modules.dedup();
    let index_of: BTreeMap<&RepoPath, usize> = modules
        .iter()
        .enumerate()
        .map(|(index, path)| (*path, index))
        .collect();
    let Some(&from_idx) = index_of.get(from) else {
        return Err(TraceError::UnknownModule { path: from.clone() });
    };
    let Some(&to_idx) = index_of.get(to) else {
        return Err(TraceError::UnknownModule { path: to.clone() });
    };
    if from_idx == to_idx {
        return Ok(TraceResult {
            from: from.clone(),
            to: to.clone(),
            connected: true,
            shortest_distance: Some(0),
            paths: vec![vec![from.clone()]],
            truncated: false,
        });
    }
    let adjacency = build_adjacency(model, &index_of, modules.len());
    let (dist, parents) = shortest_path_dag(&adjacency, from_idx, to_idx);
    let Some(target_distance) = dist[to_idx] else {
        return Ok(TraceResult {
            from: from.clone(),
            to: to.clone(),
            connected: false,
            shortest_distance: None,
            paths: Vec::new(),
            truncated: false,
        });
    };
    let forward = build_forward_shortest_dag(&parents, to_idx, modules.len());
    let limit = if all_shortest { max_paths } else { 1 };
    let (index_paths, truncated) = collect_shortest_paths(&forward, from_idx, to_idx, limit);
    let paths = index_paths
        .into_iter()
        .map(|chain| {
            chain
                .into_iter()
                .map(|index| modules[index].clone())
                .collect()
        })
        .collect();
    Ok(TraceResult {
        from: from.clone(),
        to: to.clone(),
        connected: true,
        shortest_distance: Some(target_distance),
        paths,
        truncated: all_shortest && truncated,
    })
}

fn build_adjacency(
    model: &CodeModel,
    index_of: &BTreeMap<&RepoPath, usize>,
    node_count: usize,
) -> Vec<Vec<usize>> {
    let mut adjacency = vec![Vec::new(); node_count];
    for runtime_import in &model.runtime_imports {
        if let ImportResolution::Resolved(targets) = &runtime_import.resolution
            && let Some(&from) = index_of.get(&runtime_import.importer)
        {
            for target in targets {
                if let Some(&to) = index_of.get(target) {
                    adjacency[from].push(to);
                }
            }
        }
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable();
        neighbors.dedup();
    }
    adjacency
}

fn shortest_path_dag(
    adjacency: &[Vec<usize>],
    from_idx: usize,
    to_idx: usize,
) -> (Vec<Option<usize>>, Vec<Vec<usize>>) {
    let node_count = adjacency.len();
    let mut dist = vec![None; node_count];
    let mut parents = vec![Vec::new(); node_count];
    let mut queue = VecDeque::new();
    dist[from_idx] = Some(0);
    queue.push_back(from_idx);
    while let Some(current) = queue.pop_front() {
        let current_dist = dist[current].unwrap_or(0);
        if let Some(target_dist) = dist[to_idx]
            && current_dist >= target_dist
        {
            continue;
        }
        for &next in &adjacency[current] {
            match dist[next] {
                None => {
                    dist[next] = Some(current_dist + 1);
                    parents[next].push(current);
                    queue.push_back(next);
                }
                Some(next_dist) if next_dist == current_dist + 1 => {
                    parents[next].push(current);
                }
                Some(_) => {}
            }
        }
    }
    (dist, parents)
}

fn build_forward_shortest_dag(
    parents: &[Vec<usize>],
    to_idx: usize,
    node_count: usize,
) -> Vec<Vec<usize>> {
    let mut forward = vec![Vec::new(); node_count];
    let mut visited = vec![false; node_count];
    let mut queue = VecDeque::new();
    visited[to_idx] = true;
    queue.push_back(to_idx);
    while let Some(node) = queue.pop_front() {
        for &parent in &parents[node] {
            forward[parent].push(node);
            if !visited[parent] {
                visited[parent] = true;
                queue.push_back(parent);
            }
        }
    }
    for children in &mut forward {
        children.sort_unstable();
        children.dedup();
    }
    forward
}

fn collect_shortest_paths(
    forward: &[Vec<usize>],
    from_idx: usize,
    to_idx: usize,
    limit: usize,
) -> (Vec<Vec<usize>>, bool) {
    let mut collected = Vec::new();
    let mut truncated = false;
    let mut current_path = vec![from_idx];
    let mut stack: Vec<(usize, usize)> = vec![(from_idx, 0)];
    while let Some(&mut (node, ref mut next_child)) = stack.last_mut() {
        if node == to_idx {
            if collected.len() < limit {
                collected.push(current_path.clone());
            } else {
                truncated = true;
                break;
            }
            stack.pop();
            current_path.pop();
            continue;
        }
        if let Some(&child) = forward[node].get(*next_child) {
            *next_child += 1;
            current_path.push(child);
            stack.push((child, 0));
        } else {
            stack.pop();
            current_path.pop();
        }
    }
    (collected, truncated)
}
