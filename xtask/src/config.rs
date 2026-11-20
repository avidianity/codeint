use std::path::{Path, PathBuf};

use toml::{Table, Value};

pub(crate) struct Invocation {
    pub(crate) command: String,
    pub(crate) args: Vec<String>,
}

pub(crate) fn invocation() -> Invocation {
    let mut words = std::env::args().skip(1);
    let command = words.next().unwrap_or_default();
    Invocation {
        command,
        args: words.collect(),
    }
}

pub(crate) fn github_output_file() -> Option<PathBuf> {
    std::env::var_os("GITHUB_OUTPUT").map(PathBuf::from)
}

pub(crate) struct TierRule {
    pub(crate) tier: u8,
    pub(crate) paths: Vec<String>,
}

pub(crate) struct DuplicatePass {
    pub(crate) min_lines: u64,
    pub(crate) min_tokens: u64,
}

pub(crate) struct FeedbackBudgets {
    pub(crate) hook_seconds: f64,
    pub(crate) relevant_seconds: f64,
    pub(crate) ci_gate_seconds: f64,
    pub(crate) warn_ratio: f64,
}

pub(crate) struct Settings {
    pub(crate) rules_file: String,
    pub(crate) entry_files: Vec<String>,
    pub(crate) size_budget_bytes: u64,
    pub(crate) skills_folder: String,
    pub(crate) skills_links: Vec<String>,
    pub(crate) skills_lock_file: String,
    pub(crate) project_skills: Vec<String>,
    pub(crate) commit_types: Vec<String>,
    pub(crate) commit_scopes: Vec<String>,
    pub(crate) placeholder_paths: Vec<String>,
    pub(crate) record_folders: Vec<String>,
    pub(crate) config_modules: Vec<String>,
    pub(crate) approved_suppressions: Vec<String>,
    pub(crate) test_input_paths: Vec<String>,
    pub(crate) duplicate_passes: Vec<DuplicatePass>,
    pub(crate) behavior_source: Vec<String>,
    pub(crate) behavior_tests: Vec<String>,
    pub(crate) tier_default: u8,
    pub(crate) tier_rules: Vec<TierRule>,
    pub(crate) budgets: FeedbackBudgets,
}

impl Settings {
    pub(crate) fn is_third_party_skill(&self, path: &str) -> bool {
        path.strip_prefix(self.skills_folder.as_str())
            .and_then(|rest| rest.strip_prefix('/'))
            .and_then(|rest| rest.split('/').next())
            .is_some_and(|skill| !self.project_skills.iter().any(|project| project == skill))
    }
}

pub(crate) fn load_settings(root: &Path) -> Result<Settings, String> {
    let path = root.join("checks.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let table: Table = text
        .parse()
        .map_err(|error| format!("checks.toml is not valid TOML: {error}"))?;
    let tier_rules = lookup(&table, "tier.rules")?
        .as_array()
        .ok_or("checks.toml: tier.rules must be an array of tables")?
        .iter()
        .map(|rule| {
            let tier = rule
                .get("tier")
                .and_then(Value::as_integer)
                .and_then(|tier| u8::try_from(tier).ok())
                .ok_or("checks.toml: each tier rule needs an integer tier")?;
            let paths = string_list(rule.get("paths"), "tier.rules.paths")?;
            Ok(TierRule { tier, paths })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let duplicate_passes = lookup(&table, "duplicates.passes")?
        .as_array()
        .ok_or("checks.toml: duplicates.passes must be an array of tables")?
        .iter()
        .map(|pass| {
            let number = |key: &str| {
                pass.get(key)
                    .and_then(Value::as_integer)
                    .and_then(|number| u64::try_from(number).ok())
                    .ok_or(format!(
                        "checks.toml: each duplicates pass needs a positive integer {key}"
                    ))
            };
            Ok(DuplicatePass {
                min_lines: number("min_lines")?,
                min_tokens: number("min_tokens")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Settings {
        rules_file: string(&table, "rules.file")?,
        entry_files: strings(&table, "rules.entry_files")?,
        size_budget_bytes: integer(&table, "rules.size_budget_bytes")?,
        skills_folder: string(&table, "skills.folder")?,
        skills_links: strings(&table, "skills.links")?,
        skills_lock_file: string(&table, "skills.lock_file")?,
        project_skills: strings(&table, "skills.project")?,
        commit_types: strings(&table, "commit.types")?,
        commit_scopes: strings(&table, "commit.scopes")?,
        placeholder_paths: strings(&table, "placeholders.paths")?,
        record_folders: strings(&table, "records.folders")?,
        config_modules: strings(&table, "config_access.modules")?,
        approved_suppressions: strings(&table, "suppressions.approved")?,
        test_input_paths: strings(&table, "test_input.paths")?,
        duplicate_passes,
        behavior_source: strings(&table, "behavior.source")?,
        behavior_tests: strings(&table, "behavior.tests")?,
        tier_default: u8::try_from(integer(&table, "tier.default")?)
            .map_err(|_| "checks.toml: tier.default must be 0 to 4".to_string())?,
        tier_rules,
        budgets: FeedbackBudgets {
            hook_seconds: float(&table, "feedback_budget.hook_seconds")?,
            relevant_seconds: float(&table, "feedback_budget.relevant_seconds")?,
            ci_gate_seconds: float(&table, "feedback_budget.ci_gate_seconds")?,
            warn_ratio: float(&table, "feedback_budget.warn_ratio")?,
        },
    })
}

fn lookup<'a>(table: &'a Table, dotted: &str) -> Result<&'a Value, String> {
    let mut parts = dotted.split('.');
    let first = parts.next().unwrap_or_default();
    let mut value = table
        .get(first)
        .ok_or_else(|| format!("checks.toml lacks {dotted}"))?;
    for part in parts {
        value = value
            .get(part)
            .ok_or_else(|| format!("checks.toml lacks {dotted}"))?;
    }
    Ok(value)
}

fn string(table: &Table, dotted: &str) -> Result<String, String> {
    lookup(table, dotted)?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("checks.toml: {dotted} must be a string"))
}

fn strings(table: &Table, dotted: &str) -> Result<Vec<String>, String> {
    string_list(Some(lookup(table, dotted)?), dotted)
}

fn string_list(value: Option<&Value>, dotted: &str) -> Result<Vec<String>, String> {
    value
        .and_then(Value::as_array)
        .ok_or_else(|| format!("checks.toml: {dotted} must be a list of strings"))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("checks.toml: {dotted} must hold only strings"))
        })
        .collect()
}

fn integer(table: &Table, dotted: &str) -> Result<u64, String> {
    lookup(table, dotted)?
        .as_integer()
        .and_then(|number| u64::try_from(number).ok())
        .ok_or_else(|| format!("checks.toml: {dotted} must be a positive integer"))
}

fn float(table: &Table, dotted: &str) -> Result<f64, String> {
    let value = lookup(table, dotted)?;
    value
        .as_float()
        .or_else(|| value.as_integer().map(|number| number as f64))
        .ok_or_else(|| format!("checks.toml: {dotted} must be a number"))
}

#[cfg(test)]
mod tests;
