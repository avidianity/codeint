use std::collections::{BTreeMap, HashMap};

use codeint_model::{CodeModel, ImportResolution, Package, RepoPath};

use crate::FindingId;
use crate::finding_id::{FindingIds, MetricKey};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportCycle {
    id: FindingId,
    modules: Vec<RepoPath>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnresolvedImport {
    pub importer: RepoPath,
    pub specifier: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportCycles {
    modules: usize,
    cycles: Vec<ImportCycle>,
    unresolved_imports: Vec<UnresolvedImport>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModuleShare {
    pub modules_in_import_cycles: usize,
    pub modules: usize,
}

impl ModuleShare {
    pub fn ratio(&self) -> f64 {
        if self.modules > 0 {
            self.modules_in_import_cycles as f64 / self.modules as f64
        } else {
            0.0
        }
    }
}

impl ImportCycle {
    pub fn id(&self) -> FindingId {
        self.id
    }

    pub fn modules(&self) -> &[RepoPath] {
        &self.modules
    }
}

impl ImportCycles {
    pub fn measure(model: &CodeModel) -> ImportCycles {
        let modules: Vec<&RepoPath> = model.files.iter().map(|file| &file.path).collect();
        let index_of: HashMap<&RepoPath, usize> = modules
            .iter()
            .enumerate()
            .map(|(index, path)| (*path, index))
            .collect();
        let mut imports = vec![Vec::new(); modules.len()];
        for runtime_import in &model.runtime_imports {
            if let ImportResolution::Resolved(target) = &runtime_import.resolution
                && let (Some(&from), Some(&to)) =
                    (index_of.get(&runtime_import.importer), index_of.get(target))
            {
                imports[from].push(to);
            }
        }
        let mut cycle_modules: Vec<Vec<RepoPath>> = strongly_connected_components(&imports)
            .into_iter()
            .filter(|component| match component.as_slice() {
                [single] => imports[*single].contains(single),
                _ => true,
            })
            .map(|component| {
                let mut members: Vec<RepoPath> = component
                    .into_iter()
                    .map(|index| modules[index].clone())
                    .collect();
                members.sort();
                members
            })
            .collect();
        cycle_modules.sort();
        let mut ids = FindingIds::new(MetricKey::ImportCycles);
        let cycles = cycle_modules
            .into_iter()
            .map(|modules| ImportCycle {
                id: ids.next_in_source_order(&modules, []),
                modules,
            })
            .collect();
        ImportCycles {
            modules: modules.len(),
            cycles,
            unresolved_imports: unresolved_imports(model),
        }
    }

    pub fn ratio(&self) -> f64 {
        if self.modules > 0 {
            self.modules_in_import_cycles() as f64 / self.modules as f64
        } else {
            0.0
        }
    }

    pub fn modules(&self) -> usize {
        self.modules
    }

    pub fn modules_in_import_cycles(&self) -> usize {
        self.cycles.iter().map(|cycle| cycle.modules.len()).sum()
    }

    pub fn cycles(&self) -> &[ImportCycle] {
        &self.cycles
    }

    pub fn by_package<'model>(
        &self,
        model: &'model CodeModel,
    ) -> BTreeMap<Package<'model>, ModuleShare> {
        let workspace = &model.workspace;
        let mut shares: BTreeMap<Package<'model>, ModuleShare> = workspace
            .packages()
            .map(|package| (package, ModuleShare::default()))
            .collect();
        for file in &model.files {
            shares
                .entry(workspace.package_of(&file.path))
                .or_default()
                .modules += 1;
        }
        for module in self.cycles.iter().flat_map(|cycle| &cycle.modules) {
            shares
                .entry(workspace.package_of(module))
                .or_default()
                .modules_in_import_cycles += 1;
        }
        shares
    }

    pub fn unresolved_imports(&self) -> &[UnresolvedImport] {
        &self.unresolved_imports
    }
}

fn unresolved_imports(model: &CodeModel) -> Vec<UnresolvedImport> {
    let mut unresolved: Vec<UnresolvedImport> = model
        .runtime_imports
        .iter()
        .filter_map(|runtime_import| match &runtime_import.resolution {
            ImportResolution::Unresolved { reason } => Some(UnresolvedImport {
                importer: runtime_import.importer.clone(),
                specifier: runtime_import.specifier.clone(),
                reason: reason.clone(),
            }),
            ImportResolution::Resolved(_) | ImportResolution::External => None,
        })
        .collect();
    unresolved.sort_by(|left, right| left.importer.cmp(&right.importer));
    unresolved
}

struct Tarjan<'graph> {
    imports: &'graph [Vec<usize>],
    order: Vec<Option<usize>>,
    low_link: Vec<usize>,
    on_stack: Vec<bool>,
    stack: Vec<usize>,
    next_order: usize,
    components: Vec<Vec<usize>>,
}

fn strongly_connected_components(imports: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut tarjan = Tarjan {
        imports,
        order: vec![None; imports.len()],
        low_link: vec![0; imports.len()],
        on_stack: vec![false; imports.len()],
        stack: Vec::new(),
        next_order: 0,
        components: Vec::new(),
    };
    for module in 0..imports.len() {
        if tarjan.order[module].is_none() {
            tarjan.visit_from(module);
        }
    }
    tarjan.components
}

impl Tarjan<'_> {
    fn enter(&mut self, module: usize) {
        self.order[module] = Some(self.next_order);
        self.low_link[module] = self.next_order;
        self.next_order += 1;
        self.stack.push(module);
        self.on_stack[module] = true;
    }

    fn visit_from(&mut self, root: usize) {
        self.enter(root);
        let mut path: Vec<(usize, usize)> = vec![(root, 0)];
        while let Some(&mut (module, ref mut next_import)) = path.last_mut() {
            if let Some(&target) = self.imports[module].get(*next_import) {
                *next_import += 1;
                match self.order[target] {
                    None => {
                        self.enter(target);
                        path.push((target, 0));
                    }
                    Some(target_order) if self.on_stack[target] => {
                        self.low_link[module] = self.low_link[module].min(target_order);
                    }
                    Some(_) => {}
                }
                continue;
            }
            path.pop();
            if let Some(&(parent, _)) = path.last() {
                self.low_link[parent] = self.low_link[parent].min(self.low_link[module]);
            }
            if Some(self.low_link[module]) == self.order[module] {
                self.pop_component(module);
            }
        }
    }

    fn pop_component(&mut self, root: usize) {
        let mut component = Vec::new();
        while let Some(member) = self.stack.pop() {
            self.on_stack[member] = false;
            component.push(member);
            if member == root {
                break;
            }
        }
        self.components.push(component);
    }
}
