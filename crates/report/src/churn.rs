use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use codeint_config::{ChurnFormat, ChurnWindow, FindingsShown};
use codeint_metrics::{Duplication, Erosion, MinimumCloneTokens, MinimumDistinctTokens};
use codeint_model::{CodeModel, RepoPath, SourceSet};
use serde::Serialize;

use crate::{Decimal, FindingKind, Findings, MeasuredLanguage, ReportError};

mod markdown;
mod text;

const SHALLOW_WARNING: &str = "Repository is a shallow clone; commit churn counts stop at the shallow boundary and may be incomplete. Fetch full history with `git fetch --unshallow`.";

pub struct ChurnHistoryInput {
    pub window: ChurnWindow,
    pub commits_analyzed: usize,
    pub shallow: bool,
    pub touches_by_file: BTreeMap<RepoPath, usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
enum SourceSetKey {
    Production,
    Test,
    Generated,
    Vendored,
}

impl SourceSetKey {
    fn from_source_set(source_set: SourceSet) -> SourceSetKey {
        match source_set {
            SourceSet::Production => SourceSetKey::Production,
            SourceSet::Test => SourceSetKey::Test,
            SourceSet::Generated => SourceSetKey::Generated,
            SourceSet::Vendored => SourceSetKey::Vendored,
        }
    }

    fn heading(self) -> &'static str {
        match self {
            SourceSetKey::Production => "Production",
            SourceSetKey::Test => "Test",
            SourceSetKey::Generated => "Generated",
            SourceSetKey::Vendored => "Vendored",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ChurnReport {
    window: String,
    commits_analyzed: usize,
    shallow: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    warning: Option<String>,
    source_sets: BTreeMap<SourceSetKey, SourceSetChurnSection>,
}

#[derive(Debug, Serialize)]
struct SourceSetChurnSection {
    files: usize,
    erosion: ErosionChurnSection,
    duplication: DuplicationChurnSection,
}

#[derive(Debug, Serialize)]
struct ErosionChurnSection {
    ratio: Decimal,
    eroded_function_mass: Decimal,
    total_function_mass: Decimal,
    #[serde(flatten)]
    findings: Findings<ErodedChurnFile>,
}

#[derive(Debug, Serialize)]
struct ErodedChurnFile {
    path: String,
    commits: usize,
    eroded_functions: usize,
    eroded_function_mass: Decimal,
    max_cyclomatic_complexity: u32,
}

#[derive(Debug, Serialize)]
struct DuplicationChurnSection {
    ratio: Decimal,
    clone_lines: u64,
    #[serde(flatten)]
    findings: Findings<DuplicatedChurnFile>,
}

#[derive(Debug, Serialize)]
struct DuplicatedChurnFile {
    path: String,
    commits: usize,
    clone_lines: u64,
    clone_groups: usize,
}

impl FindingKind for ErodedChurnFile {
    const LIST_KEY: &'static str = "findings";
}

impl FindingKind for DuplicatedChurnFile {
    const LIST_KEY: &'static str = "findings";
}

impl ChurnReport {
    pub fn summarize(
        languages: Vec<MeasuredLanguage>,
        history: &ChurnHistoryInput,
        findings: FindingsShown,
    ) -> ChurnReport {
        let model = CodeModel::merge(languages.into_iter().map(|lang| lang.model));
        let mut source_sets = BTreeMap::new();
        for (source_set, part) in model.into_source_sets().iter() {
            if !part.files.is_empty() {
                source_sets.insert(
                    SourceSetKey::from_source_set(source_set),
                    SourceSetChurnSection::new(part, &history.touches_by_file, findings),
                );
            }
        }
        let warning = history.shallow.then(|| SHALLOW_WARNING.to_owned());
        ChurnReport {
            window: history.window.to_string(),
            commits_analyzed: history.commits_analyzed,
            shallow: history.shallow,
            warning,
            source_sets,
        }
    }

    pub fn render(&self, format: ChurnFormat) -> Result<String, ReportError> {
        render_churn_format(self, format, text::render, markdown::render)
    }

    fn named_source_sets(&self) -> impl Iterator<Item = (&'static str, &SourceSetChurnSection)> {
        self.source_sets
            .iter()
            .map(|(key, section)| (key.heading(), section))
    }
}

impl SourceSetChurnSection {
    fn new(
        model: &CodeModel,
        touches_by_file: &BTreeMap<RepoPath, usize>,
        shown: FindingsShown,
    ) -> SourceSetChurnSection {
        let erosion = Erosion::measure(model);
        let duplication = Duplication::measure(
            model,
            MinimumCloneTokens::DEFAULT,
            MinimumDistinctTokens::DEFAULT,
        );
        SourceSetChurnSection {
            files: model.files.len(),
            erosion: ErosionChurnSection::new(&erosion, touches_by_file, shown),
            duplication: DuplicationChurnSection::new(&duplication, touches_by_file, shown),
        }
    }
}

struct ErosionAccumulator {
    eroded_functions: usize,
    eroded_function_mass: f64,
    max_cyclomatic_complexity: u32,
}

impl ErosionChurnSection {
    fn new(
        erosion: &Erosion,
        touches_by_file: &BTreeMap<RepoPath, usize>,
        shown: FindingsShown,
    ) -> ErosionChurnSection {
        let mut by_file: BTreeMap<&RepoPath, ErosionAccumulator> = BTreeMap::new();
        for func in erosion.eroded_functions() {
            let entry = by_file.entry(&func.path).or_insert(ErosionAccumulator {
                eroded_functions: 0,
                eroded_function_mass: 0.0,
                max_cyclomatic_complexity: 0,
            });
            entry.eroded_functions += 1;
            entry.eroded_function_mass += func.function_mass;
            entry.max_cyclomatic_complexity = entry
                .max_cyclomatic_complexity
                .max(func.cyclomatic_complexity);
        }
        let mut rows: Vec<ErodedChurnFile> = by_file
            .into_iter()
            .map(|(path, acc)| ErodedChurnFile {
                path: path.to_string(),
                commits: touches_by_file.get(path).copied().unwrap_or(0),
                eroded_functions: acc.eroded_functions,
                eroded_function_mass: Decimal::rounded(acc.eroded_function_mass),
                max_cyclomatic_complexity: acc.max_cyclomatic_complexity,
            })
            .collect();
        rows.sort_by(compare_eroded_rows);
        ErosionChurnSection {
            ratio: Decimal::rounded(erosion.ratio()),
            eroded_function_mass: Decimal::rounded(erosion.eroded_function_mass()),
            total_function_mass: Decimal::rounded(erosion.total_function_mass()),
            findings: Findings::worst_first(rows.into_iter(), shown),
        }
    }
}

fn compare_eroded_rows(left: &ErodedChurnFile, right: &ErodedChurnFile) -> Ordering {
    right
        .commits
        .cmp(&left.commits)
        .then_with(|| {
            right
                .eroded_function_mass
                .0
                .total_cmp(&left.eroded_function_mass.0)
        })
        .then_with(|| left.path.cmp(&right.path))
}

struct DuplicationAccumulator {
    lines: BTreeSet<u32>,
    groups: BTreeSet<usize>,
}

impl DuplicationChurnSection {
    fn new(
        duplication: &Duplication,
        touches_by_file: &BTreeMap<RepoPath, usize>,
        shown: FindingsShown,
    ) -> DuplicationChurnSection {
        let mut by_file: BTreeMap<&RepoPath, DuplicationAccumulator> = BTreeMap::new();
        for (group_index, group) in duplication.clone_groups.iter().enumerate() {
            for fragment in &group.fragments {
                let entry =
                    by_file
                        .entry(&fragment.path)
                        .or_insert_with(|| DuplicationAccumulator {
                            lines: BTreeSet::new(),
                            groups: BTreeSet::new(),
                        });
                entry.lines.extend(fragment.start_line..=fragment.end_line);
                entry.groups.insert(group_index);
            }
        }
        let mut rows: Vec<DuplicatedChurnFile> = by_file
            .into_iter()
            .map(|(path, acc)| DuplicatedChurnFile {
                path: path.to_string(),
                commits: touches_by_file.get(path).copied().unwrap_or(0),
                clone_lines: acc.lines.len() as u64,
                clone_groups: acc.groups.len(),
            })
            .collect();
        rows.sort_by(compare_duplicated_rows);
        DuplicationChurnSection {
            ratio: Decimal::rounded(duplication.ratio()),
            clone_lines: duplication.clone_lines.count(),
            findings: Findings::worst_first(rows.into_iter(), shown),
        }
    }
}

pub(crate) fn render_churn_format<T: Serialize>(
    report: &T,
    format: ChurnFormat,
    render_text: impl FnOnce(&T) -> String,
    render_markdown: impl FnOnce(&T) -> String,
) -> Result<String, ReportError> {
    match format {
        ChurnFormat::Text => Ok(render_text(report)),
        ChurnFormat::Toon => toon_format::encode_default(report).map_err(ReportError::Toon),
        ChurnFormat::Json => serde_json::to_string_pretty(report).map_err(ReportError::Json),
        ChurnFormat::Markdown => Ok(render_markdown(report)),
    }
}

fn compare_duplicated_rows(left: &DuplicatedChurnFile, right: &DuplicatedChurnFile) -> Ordering {
    right
        .commits
        .cmp(&left.commits)
        .then_with(|| right.clone_lines.cmp(&left.clone_lines))
        .then_with(|| left.path.cmp(&right.path))
}
