use std::collections::BTreeMap;

use codeint_model::{Package, RepoPath, SourceSetModel};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LineShare {
    pub lines: u64,
    pub source_lines: u64,
}

impl LineShare {
    pub fn ratio(&self) -> f64 {
        if self.source_lines == 0 {
            0.0
        } else {
            self.lines as f64 / self.source_lines as f64
        }
    }
}

pub(crate) fn line_shares_by_package<'model, 'path>(
    model: &'model SourceSetModel,
    lines_by_file: impl IntoIterator<Item = (&'path RepoPath, u64)>,
) -> BTreeMap<Package<'model>, LineShare> {
    let workspace = &model.workspace;
    let mut shares: BTreeMap<Package<'model>, LineShare> = workspace
        .packages()
        .map(|package| (package, LineShare::default()))
        .collect();
    for file in &model.files {
        shares
            .entry(workspace.package_of(&file.path))
            .or_default()
            .source_lines += u64::from(file.model.source_lines);
    }
    for (path, lines) in lines_by_file {
        shares.entry(workspace.package_of(path)).or_default().lines += lines;
    }
    shares
}
