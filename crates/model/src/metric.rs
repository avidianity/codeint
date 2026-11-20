use crate::SourceSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Metric {
    Erosion,
    Duplication,
    Verbosity,
    ImportCycles,
}

impl Metric {
    pub fn key(self) -> &'static str {
        todo!()
    }
}

impl SourceSet {
    pub fn key(self) -> &'static str {
        todo!()
    }
}
