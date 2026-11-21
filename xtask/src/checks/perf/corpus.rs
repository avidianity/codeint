use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::config::PerformanceBudget;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CorpusSpec {
    pub(crate) seed: u64,
    pub(crate) packages: usize,
    pub(crate) files_per_package: usize,
    pub(crate) functions_per_file: usize,
}

impl CorpusSpec {
    pub(crate) fn from_budget(budget: PerformanceBudget) -> CorpusSpec {
        CorpusSpec {
            seed: budget.seed,
            packages: budget.packages,
            files_per_package: budget.files_per_package,
            functions_per_file: budget.functions_per_file,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CorpusSummary {
    pub(crate) files: usize,
    pub(crate) lines: u64,
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> SplitMix64 {
        SplitMix64 { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn range(&mut self, upper: usize) -> usize {
        if upper <= 1 {
            return 0;
        }
        (self.next_u64() % upper as u64) as usize
    }

    fn chance(&mut self, percent: u64) -> bool {
        (self.next_u64() % 100) < percent
    }
}

const VERBS: [&str; 8] = [
    "compute",
    "resolve",
    "transform",
    "evaluate",
    "aggregate",
    "normalize",
    "dispatch",
    "reconcile",
];

const NOUNS: [&str; 8] = [
    "Ledger", "Batch", "Window", "Segment", "Signal", "Metric", "Snapshot", "Queue",
];

pub(crate) fn write_corpus(root: &Path, spec: CorpusSpec) -> Result<CorpusSummary, String> {
    let mut rng = SplitMix64::new(spec.seed);
    write_file(
        &root.join("package.json"),
        "{\n  \"name\": \"synthetic-corpus\",\n  \"private\": true,\n  \"workspaces\": [\"packages/*\"]\n}\n",
    )?;
    let mut files = 0usize;
    let mut lines = 0u64;
    for package_index in 0..spec.packages {
        let package_name = format!("pkg-{package_index:02}");
        let package_dir = root.join("packages").join(&package_name);
        let src_dir = package_dir.join("src");
        fs::create_dir_all(&src_dir)
            .map_err(|error| format!("cannot create {}: {error}", src_dir.display()))?;
        let manifest = format!(
            "{{\n  \"name\": \"@synthetic/{package_name}\",\n  \"version\": \"0.1.0\",\n  \"type\": \"module\"\n}}\n"
        );
        write_file(&package_dir.join("package.json"), &manifest)?;

        for file_index in 0..spec.files_per_package {
            let content = render_module(
                &mut rng,
                package_index,
                file_index,
                spec.files_per_package,
                spec.functions_per_file,
            );
            lines += content.lines().count() as u64;
            files += 1;
            let file_path = src_dir.join(format!("mod-{file_index:03}.ts"));
            write_file(&file_path, &content)?;
        }

        let test_content = render_test_file(&mut rng, package_index);
        lines += test_content.lines().count() as u64;
        files += 1;
        write_file(&src_dir.join("mod-000.test.ts"), &test_content)?;
    }
    Ok(CorpusSummary { files, lines })
}

fn write_file(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn render_module(
    rng: &mut SplitMix64,
    package_index: usize,
    file_index: usize,
    files_per_package: usize,
    functions_per_file: usize,
) -> String {
    let mut out = String::with_capacity(functions_per_file * 512);
    if files_per_package > 1 {
        let target = if file_index == 0 {
            1
        } else if file_index == 1 {
            0
        } else {
            file_index - 1
        };
        let _ = writeln!(
            out,
            "import {{ anchor_{target:03} }} from \"./mod-{target:03}.ts\";"
        );
        let type_target = (file_index + 1) % files_per_package;
        let _ = writeln!(
            out,
            "import type {{ RecordState_{type_target:03} }} from \"./mod-{type_target:03}.ts\";"
        );
    }
    let _ = writeln!(
        out,
        "\nexport interface RecordState_{file_index:03} {{\n  id: number;\n  weight: number;\n  label: string;\n  active: boolean;\n}}\n"
    );
    if files_per_package > 1 {
        let target = if file_index == 0 {
            1
        } else if file_index == 1 {
            0
        } else {
            file_index - 1
        };
        let _ = writeln!(
            out,
            "export function anchor_{file_index:03}(value: number): number {{\n  if (value <= 0) {{\n    return 0;\n  }}\n  return value + (value > 1000 ? anchor_{target:03}(0) : {package_index});\n}}\n"
        );
    } else {
        let _ = writeln!(
            out,
            "export function anchor_{file_index:03}(value: number): number {{\n  return value + {package_index};\n}}\n"
        );
    }

    for fn_index in 0..functions_per_file {
        let verb = VERBS[rng.range(VERBS.len())];
        let noun = NOUNS[rng.range(NOUNS.len())];
        let name = format!("{verb}{noun}_{file_index:03}_{fn_index:02}");
        let salt = (rng.next_u64() % 97) + 3;
        if fn_index % 5 == 0 {
            append_clone_function(&mut out, &name, fn_index, salt);
        } else if rng.chance(20) {
            append_eroded_function(&mut out, &name, salt);
        } else {
            append_standard_function(&mut out, rng, &name, salt);
        }
    }
    out
}

fn append_clone_function(out: &mut String, name: &str, variant: usize, salt: u64) {
    let acc = format!("acc_{variant}");
    let step = format!("step_{variant}");
    let limit = salt + 12;
    let _ = write!(
        out,
        "export function {name}(items: readonly number[], factor: number): number {{\n  let {acc} = 0;\n  for (let idx = 0; idx < items.length; idx += 1) {{\n    const {step} = items[idx] * factor;\n    if ({step} > {limit}) {{\n      {acc} += {step} - {salt};\n    }} else if ({step} < 0) {{\n      {acc} -= {step};\n    }} else {{\n      {acc} += {step} + 1;\n    }}\n    if ({acc} > 5000) {{\n      {acc} = Math.floor({acc} / 2);\n    }}\n  }}\n  return {acc};\n}}\n\n"
    );
}

fn append_eroded_function(out: &mut String, name: &str, salt: u64) {
    let _ = write!(
        out,
        "export function {name}(mode: number, primary: number, secondary: number, flag: boolean): number {{\n  let total = primary + {salt};\n  if (mode === 0 && flag) {{\n    total += secondary * 2;\n  }} else if (mode === 1 || secondary > 10) {{\n    total -= secondary;\n  }} else if (mode === 2 && primary > secondary) {{\n    total += primary - secondary;\n  }} else if (mode === 3) {{\n    total = flag ? total + 7 : total - 7;\n  }} else if (mode === 4) {{\n    total = primary > 0 && secondary > 0 ? total * 2 : total;\n  }} else {{\n    total += 1;\n  }}\n  switch (mode % 4) {{\n    case 0:\n      return total > 100 ? total - 10 : total;\n    case 1:\n      return flag ? total + 3 : total;\n    case 2:\n      return total < 0 ? 0 : total;\n    default:\n      return total;\n  }}\n}}\n\n"
    );
}

fn append_standard_function(out: &mut String, rng: &mut SplitMix64, name: &str, salt: u64) {
    let threshold = (rng.next_u64() % 40) + 5;
    let include_verbose = rng.chance(30);
    let _ = writeln!(
        out,
        "export function {name}(values: readonly number[], bias: number): number {{"
    );
    let _ = writeln!(out, "  let score = bias + {salt};");
    if include_verbose {
        let _ = writeln!(out, "  const scratch: any = values.length;");
        let _ = writeln!(out, "  if (scratch < 0) {{");
        let _ = writeln!(out, "  }}");
    }
    let _ = writeln!(out, "  const scaled = values.map((entry) => {{");
    let _ = writeln!(out, "    if (entry > {threshold}) {{");
    let _ = writeln!(out, "      return entry + {salt};");
    let _ = writeln!(out, "    }} else {{");
    let _ = writeln!(out, "      return entry * 2;");
    let _ = writeln!(out, "    }}");
    let _ = writeln!(out, "  }});");
    let _ = writeln!(out, "  for (const item of scaled) {{");
    let _ = writeln!(out, "    score += item > 0 ? item : 0;");
    let _ = writeln!(out, "  }}");
    let _ = writeln!(out, "  return score;");
    let _ = writeln!(out, "}}\n");
}

fn render_test_file(rng: &mut SplitMix64, package_index: usize) -> String {
    let sample = (rng.next_u64() % 50) + 1;
    format!(
        "import {{ anchor_000 }} from \"./mod-000.ts\";\n\nexport function verifyPackage_{package_index:02}(): boolean {{\n  const actual = anchor_000({sample});\n  return actual >= {package_index};\n}}\n"
    )
}
