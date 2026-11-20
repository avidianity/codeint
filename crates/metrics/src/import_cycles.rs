use codeint_model::{CodeModel, RepoPath};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportCycle {
    modules: Vec<RepoPath>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportCycles {
    modules: usize,
    cycles: Vec<ImportCycle>,
}

impl ImportCycle {
    pub fn modules(&self) -> &[RepoPath] {
        &self.modules
    }
}

impl ImportCycles {
    pub fn measure(_model: &CodeModel) -> ImportCycles {
        todo!()
    }

    pub fn modules(&self) -> usize {
        self.modules
    }

    pub fn modules_in_import_cycles(&self) -> usize {
        todo!()
    }

    pub fn cycles(&self) -> &[ImportCycle] {
        &self.cycles
    }
}
