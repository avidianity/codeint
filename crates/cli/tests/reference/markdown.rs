use std::ops::Range;
use std::path::PathBuf;

pub(crate) struct Page {
    lines: Vec<String>,
}

pub(crate) struct FencedBlock {
    pub(crate) language: String,
    pub(crate) attributes: Vec<String>,
    pub(crate) content: String,
    pub(crate) content_lines: Range<usize>,
    pub(crate) heading_line: Option<usize>,
}

pub(crate) fn reference_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/reference.md")
}

struct Fence {
    marker: String,
    opened_at: usize,
    info: String,
}

impl Page {
    pub(crate) fn read() -> Page {
        let path = reference_path();
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        Page {
            lines: text.lines().map(str::to_owned).collect(),
        }
    }

    pub(crate) fn fenced_blocks(&self) -> Vec<FencedBlock> {
        let mut blocks = Vec::new();
        let mut heading_line = None;
        let mut open: Option<Fence> = None;
        for (index, line) in self.lines.iter().enumerate() {
            match &open {
                Some(fence) if line.trim_end() == fence.marker => {
                    let mut words = fence.info.split_whitespace().map(str::to_owned);
                    let content_lines = fence.opened_at + 1..index;
                    blocks.push(FencedBlock {
                        language: words.next().unwrap_or_default(),
                        attributes: words.collect(),
                        content: self.lines[content_lines.clone()]
                            .iter()
                            .map(|content| format!("{content}\n"))
                            .collect(),
                        content_lines,
                        heading_line,
                    });
                    open = None;
                }
                Some(_) => {}
                None => {
                    if let Some(marker) = fence_marker(line) {
                        open = Some(Fence {
                            info: line[marker.len()..].trim().to_owned(),
                            marker,
                            opened_at: index,
                        });
                    } else if heading_level(line).is_some() {
                        heading_line = Some(index);
                    }
                }
            }
        }
        blocks
    }

    pub(crate) fn section(&self, heading: &str) -> &[String] {
        let headings = self.heading_lines();
        let (start, level) = headings
            .iter()
            .find(|(index, _)| self.lines[*index] == heading)
            .copied()
            .unwrap_or_else(|| panic!("docs/reference.md has no heading {heading:?}"));
        let end = headings
            .iter()
            .find(|(index, other)| *index > start && *other <= level)
            .map_or(self.lines.len(), |(index, _)| *index);
        &self.lines[start + 1..end]
    }

    pub(crate) fn with_replaced_contents(&self, replacements: &[(Range<usize>, String)]) -> String {
        let mut sorted: Vec<&(Range<usize>, String)> = replacements.iter().collect();
        sorted.sort_by_key(|(range, _)| range.start);
        let mut text = String::new();
        let mut kept_from = 0;
        for (range, content) in sorted {
            self.lines[kept_from..range.start]
                .iter()
                .for_each(|line| text.push_str(&format!("{line}\n")));
            text.push_str(content);
            kept_from = range.end;
        }
        self.lines[kept_from..]
            .iter()
            .for_each(|line| text.push_str(&format!("{line}\n")));
        text
    }

    fn heading_lines(&self) -> Vec<(usize, usize)> {
        let mut inside_fence: Option<String> = None;
        let mut headings = Vec::new();
        for (index, line) in self.lines.iter().enumerate() {
            match &inside_fence {
                Some(marker) if line.trim_end() == marker => inside_fence = None,
                Some(_) => {}
                None => {
                    if let Some(marker) = fence_marker(line) {
                        inside_fence = Some(marker);
                    } else if let Some(level) = heading_level(line) {
                        headings.push((index, level));
                    }
                }
            }
        }
        headings
    }
}

fn fence_marker(line: &str) -> Option<String> {
    let backticks = line
        .chars()
        .take_while(|character| *character == '`')
        .count();
    (backticks >= 3).then(|| "`".repeat(backticks))
}

fn heading_level(line: &str) -> Option<usize> {
    let hashes = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    (hashes > 0 && line[hashes..].starts_with(' ')).then_some(hashes)
}

pub(crate) fn table_rows(lines: &[String], first_header: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut in_matching_table = false;
    let mut previous_was_table = false;
    for line in lines {
        let is_table = line.starts_with('|');
        if !is_table {
            in_matching_table = false;
        } else if !previous_was_table {
            in_matching_table = cells(line).first().map(String::as_str) == Some(first_header);
        } else if in_matching_table && !line.starts_with("| -") {
            rows.push(cells(line));
        }
        previous_was_table = is_table;
    }
    rows
}

fn cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split(" | ")
        .map(|cell| cell.trim().to_owned())
        .collect()
}

pub(crate) fn code_spans(text: &str) -> Vec<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}
