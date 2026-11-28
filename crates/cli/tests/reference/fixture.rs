use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

use crate::examples::CommandLine;

pub(crate) const SHOWN_ROOT: &str = "/home/me/shop";

const GIT_IDENTITY: [(&str, &str); 6] = [
    ("GIT_CONFIG_GLOBAL", "/dev/null"),
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_AUTHOR_NAME", "Reference fixture"),
    ("GIT_AUTHOR_EMAIL", "reference@example.invalid"),
    ("GIT_COMMITTER_NAME", "Reference fixture"),
    ("GIT_COMMITTER_EMAIL", "reference@example.invalid"),
];

const SIGN: &str = "export function sign(value: number): string {
  if (value < 0) {
    return \"negative\";
  } else {
    return \"positive\";
  }
}
";

const SIMPLE_PARSE: &str = "export function parse(value: number): number {
  return value > 0 && value < 12 ? value : 0;
}
";

const TOTAL: &str = "export function total(prices: number[], discount: number): number {
  let sum = 0;
  for (const price of prices) {
    sum += price;
  }
  const reduced = sum - discount;
  const rounded = Math.round(reduced * 100) / 100;
  const floor = 0;
  const result = Math.max(rounded, floor);
  return result;
}
";

const PLUGIN_SAMPLES: [&str; 4] = [
    "sample_mini_plugin.wasm",
    "sample_long_functions_plugin.wasm",
    "out_of_range_ratio_metric_plugin.wasm",
    "legacy_v0_3_0_language_plugin.wasm",
];

fn add_plugin_samples(root: &Path) {
    let folder = root.join("plugins");
    fs::create_dir_all(&folder).expect("create the plugins folder");
    for file_name in PLUGIN_SAMPLES {
        let source = crate::wasm_fixtures::wasm_fixture_path(file_name);
        fs::copy(&source, folder.join(file_name)).expect("copy a sample Wasm Plugin");
    }
    write(root, "plugins/sample.mini", "alpha\nbeta\n");
}

pub(crate) struct Template {
    folder: TempDir,
}

pub(crate) struct Run {
    pub(crate) output: Output,
    root: TempDir,
    _result_cache: TempDir,
}

impl Template {
    pub(crate) fn build() -> Template {
        let template = Template {
            folder: tempfile::tempdir().expect("create the fixture folder"),
        };
        let root = template.folder.path();
        git(
            root,
            &["init", "--quiet", "--initial-branch", "main"],
            "2026-11-01T09:00:00Z",
        );
        write(root, "src/sign.ts", SIGN);
        write(root, "src/types.d.ts", "export type Sign = string;\n");
        write(root, "src/broken.ts", "export const broken = ;\n");
        commit(root, "Add sign", "2026-11-02T09:00:00Z");
        write(
            root,
            "src/parse.ts",
            &branching_function("parse", "value: number", "value === ", 11),
        );
        commit(root, "Add parse", "2026-11-09T09:00:00Z");
        git(
            root,
            &["update-ref", "refs/remotes/origin/main", "main"],
            "2026-11-09T09:00:00Z",
        );
        git(
            root,
            &["checkout", "--quiet", "-b", "feature"],
            "2026-11-09T09:00:00Z",
        );
        write(root, "src/parse.ts", SIMPLE_PARSE);
        write(root, "src/total.ts", TOTAL);
        commit(root, "Simplify parse", "2026-11-16T09:00:00Z");
        write(
            root,
            "src/route.ts",
            &branching_function("route", "path: string", "path === \"/", 15),
        );
        write(
            root,
            "coverage/lcov.info",
            "SF:src/sign.ts\nDA:2,1\nDA:3,1\nDA:5,0\nend_of_record\n",
        );
        let saved = Command::new(env!("CARGO_BIN_EXE_codeint"))
            .args(["check", "--format", "json", "--output", "report.json"])
            .current_dir(root)
            .output()
            .expect("generate sample report.json");
        assert!(
            saved.status.success(),
            "generate sample report.json failed: {}",
            String::from_utf8_lossy(&saved.stderr)
        );
        add_plugin_samples(root);
        template
    }

    pub(crate) fn run(&self, configuration: Option<&str>, command: &CommandLine) -> Run {
        let root = tempfile::tempdir().expect("create the example folder");
        copy_folder(self.folder.path(), root.path());
        if let Some(text) = configuration {
            fs::write(root.path().join("codeint.json"), text).expect("write codeint.json");
        }
        let result_cache = tempfile::tempdir().expect("create the result cache folder");
        let output = Command::new(env!("CARGO_BIN_EXE_codeint"))
            .args(&command.arguments)
            .current_dir(root.path())
            .env("CODEINT_CACHE_DIR", result_cache.path())
            .envs(
                command
                    .environment
                    .iter()
                    .map(|(name, value)| (name, value)),
            )
            .output()
            .expect("run codeint");
        Run {
            output,
            root,
            _result_cache: result_cache,
        }
    }
}

impl Run {
    pub(crate) fn root(&self) -> &Path {
        self.root.path()
    }

    pub(crate) fn shown(&self, text: &str) -> String {
        let canonical = fs::canonicalize(self.root())
            .expect("canonicalize the example folder")
            .display()
            .to_string();
        text.replace(&canonical, SHOWN_ROOT)
            .replace(&self.root().display().to_string(), SHOWN_ROOT)
    }
}

fn branching_function(name: &str, parameter: &str, comparison: &str, branches: usize) -> String {
    let closing = if comparison.ends_with('/') { "\"" } else { "" };
    let body: String = (1..=branches)
        .map(|value| format!("  if ({comparison}{value}{closing}) return {value};\n"))
        .collect();
    format!("export function {name}({parameter}): number {{\n{body}  return 0;\n}}\n")
}

fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("a fixture file sits in a folder"))
        .expect("create the fixture folder");
    fs::write(path, text).expect("write the fixture file");
}

fn commit(root: &Path, message: &str, date: &str) {
    git(root, &["add", "--all"], date);
    git(root, &["commit", "--quiet", "--message", message], date);
}

fn git(root: &Path, arguments: &[&str], date: &str) {
    let output = Command::new("git")
        .args(["-c", "gc.auto=0"])
        .args(arguments)
        .current_dir(root)
        .envs(GIT_IDENTITY)
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn copy_file(source: &Path, target: &Path) {
    if source.extension().is_some_and(|ext| ext == "wasm") && fs::hard_link(source, target).is_ok()
    {
        return;
    }
    if let Err(error) = fs::copy(source, target)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        panic!("copy a fixture file: {error}");
    }
}

fn copy_folder(from: &Path, to: &Path) {
    let entries = match fs::read_dir(from) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => panic!("list the fixture folder: {error}"),
    };
    for entry in entries {
        copy_dir_entry(entry, to);
    }
}

fn copy_dir_entry(entry: std::io::Result<fs::DirEntry>, to: &Path) {
    let entry = match entry {
        Ok(entry) => entry,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => panic!("read a fixture folder entry: {error}"),
    };
    let file_type = match entry.file_type() {
        Ok(file_type) => file_type,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => panic!("read the entry type: {error}"),
    };
    let target = to.join(entry.file_name());
    if file_type.is_dir() {
        fs::create_dir_all(&target).expect("create a copied folder");
        copy_folder(&entry.path(), &target);
    } else {
        copy_file(&entry.path(), &target);
    }
}
