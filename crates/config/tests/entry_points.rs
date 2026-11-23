use std::path::{Path, PathBuf};

use codeint_config::{Configuration, NewFindingsRule, read_configuration};
use codeint_model::{RepoPath, SourceSet};

struct TempFolder(PathBuf);

impl TempFolder {
    fn new(name: &str) -> TempFolder {
        let path =
            std::env::temp_dir().join(format!("codeint-config-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create temp dir");
        TempFolder(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempFolder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn parses_entry_points_orphan_files_budget_and_regression_limits() {
    let dir = TempFolder::new("entry-points-valid");
    std::fs::write(
        dir.path().join("codeint.json"),
        r#"{
  "entry_points": ["src/pages/**/*.tsx", "scripts/cli.ts"],
  "budgets": {
    "production": {
      "orphan_files": 0.0
    }
  },
  "regression_limits": {
    "orphan_files": {
      "ratio_rise": 0.02
    },
    "production": {
      "orphan_files": {
        "new_findings": "any_new_finding"
      }
    }
  }
}"#,
    )
    .expect("write codeint.json");

    let config = read_configuration(dir.path()).expect("valid configuration");
    config
        .validate_metrics(&[])
        .expect("orphan_files is a known built-in metric");

    let budget = config
        .budgets
        .budget_for_key(SourceSet::Production, "orphan_files")
        .expect("production orphan_files budget");
    assert_eq!(budget.ratio(), 0.0);

    let prod_limit = config
        .regression_limits
        .limit_for_key(SourceSet::Production, "orphan_files");
    assert_eq!(prod_limit.ratio_rise.ratio(), 0.02);
    assert_eq!(prod_limit.new_findings, NewFindingsRule::AnyNewFinding);

    let default_config = Configuration::default();
    let default_limit = default_config
        .regression_limits
        .limit_for_key(SourceSet::Production, "orphan_files");
    assert_eq!(default_limit.new_findings, NewFindingsRule::AnyNewFinding);

    assert!(!config.entry_points.is_empty());
    assert!(
        config
            .entry_points
            .matches(&RepoPath::new("src/pages/blog/post.tsx").expect("path"))
    );
    assert!(
        config
            .entry_points
            .matches(&RepoPath::new("scripts/cli.ts").expect("path"))
    );
    assert!(
        !config
            .entry_points
            .matches(&RepoPath::new("src/lib/unused.ts").expect("path"))
    );
}

#[test]
fn rejects_invalid_entry_points_glob() {
    let dir = TempFolder::new("entry-points-invalid");
    std::fs::write(
        dir.path().join("codeint.json"),
        r#"{ "entry_points": ["src/[unclosed"] }"#,
    )
    .expect("write codeint.json");

    let error = read_configuration(dir.path()).expect_err("invalid glob must fail");
    assert!(
        error.to_string().contains("entry_points[0]"),
        "unexpected error: {error}"
    );
}
