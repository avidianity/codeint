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
        match self {
            Metric::Erosion => "erosion",
            Metric::Duplication => "duplication",
            Metric::Verbosity => "verbosity",
            Metric::ImportCycles => "import_cycles",
        }
    }
}

impl SourceSet {
    pub fn key(self) -> &'static str {
        match self {
            SourceSet::Production => "production",
            SourceSet::Test => "test",
            SourceSet::Generated => "generated",
            SourceSet::Vendored => "vendored",
        }
    }
}
