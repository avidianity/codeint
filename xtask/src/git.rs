use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) enum DiffSource {
    Staged,
    Range(String),
}

impl DiffSource {
    pub(crate) fn from_args(args: &[String]) -> Result<DiffSource, String> {
        match args {
            [flag] if flag == "--staged" => Ok(DiffSource::Staged),
            [flag, base] if flag == "--base" => Ok(DiffSource::Range(base.clone())),
            _ => Err("expected --staged or --base {ref}".to_string()),
        }
    }

    fn diff_args(&self) -> Vec<String> {
        match self {
            DiffSource::Staged => vec!["--cached".to_string()],
            DiffSource::Range(base) => vec![format!("{base}...HEAD")],
        }
    }
}

pub(crate) struct FileChange {
    pub(crate) status: char,
    pub(crate) path: String,
}

impl FileChange {
    pub(crate) fn is_deleted(&self) -> bool {
        self.status == 'D'
    }
}

#[derive(Default)]
pub(crate) struct LineDiff {
    pub(crate) added: Vec<(usize, String)>,
    pub(crate) removed: Vec<String>,
}

pub(crate) fn repository_root() -> Result<PathBuf, String> {
    let output = run(Path::new("."), &["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(output.trim()))
}

pub(crate) fn run(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub(crate) fn tracked_files(root: &Path) -> Result<Vec<String>, String> {
    Ok(run(root, &["ls-files", "-z"])?
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .collect())
}

pub(crate) fn changed_files(root: &Path, source: &DiffSource) -> Result<Vec<FileChange>, String> {
    let mut args = vec![
        "diff".to_string(),
        "--name-status".to_string(),
        "--no-renames".to_string(),
        "-z".to_string(),
    ];
    args.extend(source.diff_args());
    let output = run(root, &as_strs(&args))?;
    let fields: Vec<&str> = output
        .split('\0')
        .filter(|field| !field.is_empty())
        .collect();
    Ok(fields
        .chunks(2)
        .filter_map(|pair| match pair {
            [status, path] => Some(FileChange {
                status: status.chars().next().unwrap_or('M'),
                path: (*path).to_string(),
            }),
            _ => None,
        })
        .collect())
}

pub(crate) fn line_diff(root: &Path, source: &DiffSource, path: &str) -> Result<LineDiff, String> {
    let mut args = vec![
        "diff".to_string(),
        "-U0".to_string(),
        "--no-color".to_string(),
    ];
    args.extend(source.diff_args());
    args.push("--".to_string());
    args.push(path.to_string());
    Ok(parse_unified_diff(&run(root, &as_strs(&args))?))
}

pub(crate) fn parse_unified_diff(diff: &str) -> LineDiff {
    let mut result = LineDiff::default();
    let mut next_new_line = 0usize;
    for line in diff.lines() {
        if let Some(header) = line.strip_prefix("@@ ") {
            next_new_line = new_start_of(header).unwrap_or(0);
        } else if line.starts_with("+++") || line.starts_with("---") {
            continue;
        } else if let Some(text) = line.strip_prefix('+') {
            result.added.push((next_new_line, text.to_string()));
            next_new_line += 1;
        } else if let Some(text) = line.strip_prefix('-') {
            result.removed.push(text.to_string());
        }
    }
    result
}

fn new_start_of(header: &str) -> Option<usize> {
    let new_range = header.split(' ').find(|part| part.starts_with('+'))?;
    new_range
        .trim_start_matches('+')
        .split(',')
        .next()?
        .parse()
        .ok()
}

pub(crate) fn new_content(root: &Path, source: &DiffSource, path: &str) -> Result<String, String> {
    match source {
        DiffSource::Staged => run(root, &["show", &format!(":{path}")]),
        DiffSource::Range(_) => run(root, &["show", &format!("HEAD:{path}")]),
    }
}

pub(crate) fn base_content(root: &Path, source: &DiffSource, path: &str) -> Option<String> {
    let base = match source {
        DiffSource::Staged => "HEAD".to_string(),
        DiffSource::Range(base) => run(root, &["merge-base", base, "HEAD"])
            .ok()?
            .trim()
            .to_string(),
    };
    run(root, &["show", &format!("{base}:{path}")]).ok()
}

pub(crate) fn as_strs(args: &[String]) -> Vec<&str> {
    args.iter().map(String::as_str).collect()
}
