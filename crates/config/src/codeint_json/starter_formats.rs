use std::fmt::Write as _;

use codeint_model::RepoPath;

use crate::boundaries::BoundaryPreset;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StructuredSyntax {
    Toml,
    Yaml,
}

pub(super) struct StarterRenderInput<'a> {
    pub(super) schema_address: &'a str,
    pub(super) languages: &'a [(String, Vec<RepoPath>)],
    pub(super) boundary_zones: Option<&'a [(String, String)]>,
    pub(super) boundary_preset: Option<BoundaryPreset>,
    pub(super) adoption: bool,
    pub(super) production_zeros: &'a [&'static str],
    pub(super) test_zeros: &'a [&'static str],
    pub(super) dependency_zeros: &'a [&'static str],
}

pub(super) fn render_structured_starter(
    syntax: StructuredSyntax,
    input: &StarterRenderInput<'_>,
) -> String {
    let mut out = String::new();
    match syntax {
        StructuredSyntax::Toml => write_toml_body(&mut out, input),
        StructuredSyntax::Yaml => write_yaml_body(&mut out, input),
    }
    write_optional_hints(&mut out, syntax, input);
    out
}

fn write_toml_body(out: &mut String, input: &StarterRenderInput<'_>) {
    let _ = writeln!(out, "#:schema {}", input.schema_address);
    out.push_str(
        "\ncoverage_reports = []\n\n[source_sets]\nproduction = []\ntest = []\ngenerated = []\nvendored = []\n\n",
    );
    if !input.dependency_zeros.is_empty() {
        out.push_str("[budgets]\n");
        for key in input.dependency_zeros {
            let _ = writeln!(out, "{key} = 0");
        }
        out.push('\n');
    }
    out.push_str("[budgets.production]\n# test_gap = 0.2\n");
    for key in input.production_zeros {
        let _ = writeln!(out, "{key} = 0");
    }
    out.push_str("\n[budgets.test]\n");
    for key in input.test_zeros {
        let _ = writeln!(out, "{key} = 0");
    }
    out.push_str("\n[budgets.generated]\n\n[budgets.vendored]\n\n[regression_limits]\n");
    if input.adoption {
        out.push_str("per_file = true\n");
    }
    for (plugin, include) in input.languages {
        let includes: Vec<String> = include
            .iter()
            .map(|path| quote_scalar(&globset::escape(path.as_str())))
            .collect();
        let _ = writeln!(
            out,
            "\n[[languages]]\nplugin = {}\ninclude = [{}]\nexclude = []",
            quote_scalar(plugin),
            includes.join(", ")
        );
    }
}

fn write_yaml_body(out: &mut String, input: &StarterRenderInput<'_>) {
    let _ = writeln!(
        out,
        "# yaml-language-server: $schema={}",
        input.schema_address
    );
    out.push_str(
        "source_sets:\n  production: []\n  test: []\n  generated: []\n  vendored: []\nbudgets:\n",
    );
    for key in input.dependency_zeros {
        let _ = writeln!(out, "  {key}: 0");
    }
    write_yaml_budget_section(out, "production", input.production_zeros, true);
    write_yaml_budget_section(out, "test", input.test_zeros, false);
    out.push_str("  generated: {}\n  vendored: {}\n");
    if input.adoption {
        out.push_str("regression_limits:\n  per_file: true\n");
    } else {
        out.push_str("regression_limits: {}\n");
    }
    if !input.languages.is_empty() {
        out.push_str("languages:\n");
        for (plugin, include) in input.languages {
            let _ = writeln!(out, "  - plugin: {}", quote_scalar(plugin));
            if include.is_empty() {
                out.push_str("    include: []\n");
            } else {
                out.push_str("    include:\n");
                for path in include {
                    let escaped = globset::escape(path.as_str());
                    let _ = writeln!(out, "      - {}", quote_scalar(&escaped));
                }
            }
            out.push_str("    exclude: []\n");
        }
    }
    out.push_str("coverage_reports: []\n");
}

fn write_yaml_budget_section(
    out: &mut String,
    name: &str,
    keys: &[&'static str],
    include_test_gap_hint: bool,
) {
    if keys.is_empty() {
        let _ = writeln!(out, "  {name}: {{}}");
        if include_test_gap_hint {
            out.push_str("  # production:\n  #   test_gap: 0.2\n");
        }
        return;
    }
    let _ = writeln!(out, "  {name}:");
    if include_test_gap_hint {
        out.push_str("    # test_gap: 0.2\n");
    }
    for key in keys {
        let _ = writeln!(out, "    {key}: 0");
    }
}

fn write_optional_hints(
    out: &mut String,
    syntax: StructuredSyntax,
    input: &StarterRenderInput<'_>,
) {
    let lead = match syntax {
        StructuredSyntax::Toml => "\n",
        StructuredSyntax::Yaml => "",
    };
    if input.adoption {
        let body = match syntax {
            StructuredSyntax::Toml => {
                "# [new_file_budgets.production]\n# erosion = 0\n# cognitive_complexity = 0\n# hazards = 0\n# boundaries = 0\n"
            }
            StructuredSyntax::Yaml => {
                "# new_file_budgets:\n#   production:\n#     erosion: 0\n#     cognitive_complexity: 0\n#     hazards: 0\n#     boundaries: 0\n"
            }
        };
        let _ = write!(
            out,
            "{lead}# Optional: uncomment to hold every new file to a strict bar in codeint diff.\n{body}"
        );
    }
    if input.languages.iter().any(|(plugin, _)| plugin == "sql") {
        let body = match syntax {
            StructuredSyntax::Toml => "# [sql]\n# dialects = []\n",
            StructuredSyntax::Yaml => "# sql:\n#   dialects: []\n",
        };
        let _ = write!(
            out,
            "{lead}# Optional: map SQL folders to a Dialect with sql.dialects (postgresql, mysql, sqlite, or duckdb).\n{body}"
        );
    }
    write_boundary_hints(
        out,
        syntax,
        lead,
        input.boundary_zones,
        input.boundary_preset,
    );
}

fn write_boundary_hints(
    out: &mut String,
    syntax: StructuredSyntax,
    lead: &str,
    zones: Option<&[(String, String)]>,
    preset: Option<BoundaryPreset>,
) {
    if let Some(zones) = zones
        && !zones.is_empty()
    {
        let _ = writeln!(
            out,
            "{lead}# Optional: uncomment to enforce Import boundaries between discovered Workspace packages."
        );
        match syntax {
            StructuredSyntax::Toml => {
                out.push_str("# [boundaries]\n# rules = []\n");
                for (name, pattern) in zones {
                    let _ = writeln!(out, "# [boundaries.zones.{}]", quote_scalar(name));
                    let _ = writeln!(out, "# patterns = [{}]", quote_scalar(pattern));
                }
            }
            StructuredSyntax::Yaml => {
                out.push_str("# boundaries:\n#   zones:\n");
                for (name, pattern) in zones {
                    let _ = writeln!(out, "#     {}:", quote_scalar(name));
                    let _ = writeln!(out, "#       patterns: [{}]", quote_scalar(pattern));
                }
                out.push_str("#   rules: []\n");
            }
        }
        return;
    }
    if let Some(preset) = preset {
        let name = preset.as_str();
        let _ = writeln!(
            out,
            "{lead}# Optional: uncomment to enable the {name} Import boundaries preset."
        );
        match syntax {
            StructuredSyntax::Toml => {
                let _ = writeln!(out, "# [boundaries]\n# preset = {name:?}");
            }
            StructuredSyntax::Yaml => {
                let _ = writeln!(out, "# boundaries:\n#   preset: {name:?}");
            }
        }
    }
}

fn quote_scalar(value: &str) -> String {
    serde_json::to_string(value).expect("strings serialize to single-line basic double-quoted form")
}
