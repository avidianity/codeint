use std::collections::BTreeMap;

use codeint_config::{
    CliCommandSchema, CliSchema, NewFindingsRule, OutputFormatSchema, RegressionLimits,
    SchemaFormat, configuration_schema,
};
use codeint_model::{Metric, MetricScope, SourceSet};
use serde::Serialize;
use serde_json::Value;

use crate::ReportError;

#[derive(Debug, Clone, Copy)]
pub(crate) struct BuiltInMetricDescriptor {
    pub(crate) name: &'static str,
    pub(crate) short_description: &'static str,
    pub(crate) full_description: &'static str,
    pub(crate) help: &'static str,
}

const BUILT_IN_METRIC_DESCRIPTORS: [BuiltInMetricDescriptor; 12] = [
    BuiltInMetricDescriptor {
        name: Metric::Erosion.display_name(),
        short_description: "A function with cyclomatic complexity over 10",
        full_description: "Erosion is the share of total Function mass held by functions whose cyclomatic complexity is greater than 10. Function mass is cyclomatic complexity times the square root of source lines.",
        help: "Split the function into smaller functions, each with a cyclomatic complexity of 10 or less.",
    },
    BuiltInMetricDescriptor {
        name: Metric::CognitiveComplexity.display_name(),
        short_description: "A function with cognitive complexity over 15",
        full_description: "Cognitive complexity is the share of total cognitive complexity held by functions whose cognitive complexity is greater than 15. Cognitive complexity weights control-flow structures by nesting depth.",
        help: "Flatten nested control flow or split the function into smaller functions, each with a cognitive complexity of 15 or less.",
    },
    BuiltInMetricDescriptor {
        name: Metric::Duplication.display_name(),
        short_description: "A Clone group: code fragments with the same token sequence",
        full_description: "Duplication is the share of source lines that belong to at least one Clone group. A Clone group is two or more code fragments with the same token sequence once identifiers and literals are normalized.",
        help: "Extract the fragments of the Clone group into one shared function or module.",
    },
    BuiltInMetricDescriptor {
        name: Metric::Verbosity.display_name(),
        short_description: "A Flagged line: code that can be deleted or collapsed",
        full_description: "Verbosity is the share of source lines that are Flagged lines, clone lines, or both. A Flagged line matches a verbose-code pattern of the source language, and the result names the rule it matches.",
        help: "Delete or collapse the line as its rule describes; the behavior stays the same.",
    },
    BuiltInMetricDescriptor {
        name: Metric::ImportCycles.display_name(),
        short_description: "An Import cycle among runtime imports",
        full_description: "Import cycles is the share of modules that belong to at least one Import cycle. An Import cycle is a group of modules in which every module can reach every other module through runtime imports.",
        help: "Break the Import cycle, for example by moving the code that the modules share into a module that imports none of them.",
    },
    BuiltInMetricDescriptor {
        name: "Boundaries",
        short_description: "An import or file that breaks configured architectural boundaries",
        full_description: "Boundaries checks runtime imports against configured architectural zones, directional internal allow or deny rules, external-package allow_external or deny_external glob rules, public_files encapsulation, and optional strict_zones coverage.",
        help: "Import through an allowed zone, a target zone's public_files entry point, or an allowed external package, or assign the file to a configured zone.",
    },
    BuiltInMetricDescriptor {
        name: Metric::OrphanFiles.display_name(),
        short_description: "A Production file unreachable from any entry point",
        full_description: "Orphan files measures the share of Production files that cannot be reached through resolved runtime imports from any manifest-declared, program-start, or configured entry point, annotating files imported only by tests as test-only.",
        help: "Delete the unused Production file (and any stale tests that only import it), wire it into an entry point, or add its path glob to entry_points in codeint.json.",
    },
    BuiltInMetricDescriptor {
        name: Metric::TestGap.display_name(),
        short_description: "A Production function whose executable lines tests do not fully execute",
        full_description: "Test gap is the share of total Production Function mass that tests do not execute, computed from LCOV or Cobertura XML Coverage reports.",
        help: "Add tests that execute the uncovered lines of this function, starting with the heaviest untested functions first.",
    },
    BuiltInMetricDescriptor {
        name: Metric::Hazards.display_name(),
        short_description: "A Hazard line: a parsed pattern likely to cause wrong results, slow execution, or a security exposure",
        full_description: "Hazards is the share of source lines that are Hazard lines in languages that have Hazard line rules. Each Hazard line Finding names its path, line, and rule, and never the source text.",
        help: "Rewrite the statement to remove the hazard named by the rule, or turn off the rule in hazards.disabled_rules in codeint.json.",
    },
    BuiltInMetricDescriptor {
        name: Metric::UnusedDependencies.display_name(),
        short_description: "A declared dependency that no file in the package imports",
        full_description: "Unused dependencies is the share of declared runtime and dev dependencies in package manifests that are not imported by any source file in the owning package.",
        help: "Remove the unused dependency from the package manifest, or add it to dependencies.ignore in codeint.json if it is used outside source imports.",
    },
    BuiltInMetricDescriptor {
        name: Metric::UnlistedDependencies.display_name(),
        short_description: "An external package imported without being declared in the package manifest",
        full_description: "Unlisted dependencies is the share of imported external packages that are missing from the owning package manifest.",
        help: "Declare the package in the owning package manifest, or remove the import.",
    },
    BuiltInMetricDescriptor {
        name: Metric::DevInProductionDependencies.display_name(),
        short_description: "A dev-only dependency imported by a Production source file",
        full_description: "Dev dependencies in production is the share of production-imported external packages that are declared only as dev dependencies in the package manifest.",
        help: "Move the dependency to runtime dependencies in the package manifest, or stop importing it from Production files.",
    },
];

pub(crate) const fn built_in_metric_descriptor(metric: Metric) -> BuiltInMetricDescriptor {
    BUILT_IN_METRIC_DESCRIPTORS[metric.index()]
}

const SCHEMA_METRIC_ORDER: [Metric; 11] = [
    Metric::Erosion,
    Metric::Duplication,
    Metric::Verbosity,
    Metric::ImportCycles,
    Metric::CognitiveComplexity,
    Metric::OrphanFiles,
    Metric::Hazards,
    Metric::UnusedDependencies,
    Metric::UnlistedDependencies,
    Metric::DevInProductionDependencies,
    Metric::TestGap,
];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SchemaReport {
    name: String,
    version: String,
    description: String,
    commands: Vec<CliCommandSchema>,
    metrics: Vec<MetricSchema>,
    configuration_keys: Vec<String>,
    source_sets: Vec<&'static str>,
    languages: Vec<LanguageSchemaEntry>,
    output_formats: Vec<OutputFormatSchema>,
    mcp_tools: Vec<McpToolSchema>,
    exit_codes: Vec<ExitCodeSchema>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct LanguageSchemaEntry {
    name: String,
    extensions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    hazard_rules: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MetricSchema {
    key: &'static str,
    name: &'static str,
    short_description: &'static str,
    description: &'static str,
    unit: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_ratio_rise_limit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_new_findings_rule: Option<NewFindingsRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LanguageSchema {
    pub name: String,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct McpToolSchema {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExitCodeSchema {
    pub code: u8,
    pub name: &'static str,
    pub description: &'static str,
}

impl SchemaReport {
    pub fn build(
        cli: CliSchema,
        languages: Vec<LanguageSchema>,
        mcp_tools: Vec<McpToolSchema>,
        exit_codes: Vec<ExitCodeSchema>,
    ) -> SchemaReport {
        let default_limits = RegressionLimits::default();
        let metrics: Vec<MetricSchema> = SCHEMA_METRIC_ORDER
            .into_iter()
            .map(|metric| {
                let descriptor = built_in_metric_descriptor(metric);
                if !metric.supports_diff_regression() {
                    metric_schema_without_regression(metric, descriptor)
                } else {
                    let limit = match metric.scope() {
                        MetricScope::RepositoryWide => {
                            default_limits.limit_for_dependency(metric.key())
                        }
                        MetricScope::AllSourceSets | MetricScope::ProductionOnly => {
                            default_limits.limit(SourceSet::Production, metric)
                        }
                    };
                    metric_schema(metric, descriptor, limit)
                }
            })
            .collect();
        let source_sets = SourceSet::ALL.into_iter().map(SourceSet::key).collect();
        let languages = languages
            .into_iter()
            .map(|language| LanguageSchemaEntry {
                name: language.name,
                extensions: language.extensions,
                hazard_rules: Vec::new(),
            })
            .collect();
        SchemaReport {
            name: cli.name,
            version: cli.version,
            description: cli.description,
            commands: cli.commands,
            metrics,
            configuration_keys: configuration_top_level_keys(),
            source_sets,
            languages,
            output_formats: cli.output_formats,
            mcp_tools,
            exit_codes,
        }
    }

    pub fn with_language_hazard_rules(
        mut self,
        mut rules_by_language: BTreeMap<String, Vec<String>>,
    ) -> SchemaReport {
        for language in &mut self.languages {
            if let Some(rules) = rules_by_language.remove(&language.name) {
                language.hazard_rules = rules;
            }
        }
        self
    }

    pub fn render(&self, format: SchemaFormat) -> Result<String, ReportError> {
        match format {
            SchemaFormat::Json => crate::render_json(self),
            SchemaFormat::Toon => crate::render_toon(self),
        }
    }
}

fn configuration_top_level_keys() -> Vec<String> {
    let schema: Value =
        serde_json::from_str(&configuration_schema()).expect("configuration schema is valid JSON");
    let mut keys: Vec<String> = schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| properties.keys().cloned().collect())
        .unwrap_or_default();
    keys.sort();
    keys
}

fn metric_schema(
    metric: Metric,
    descriptor: BuiltInMetricDescriptor,
    limit: codeint_config::RegressionLimit,
) -> MetricSchema {
    MetricSchema {
        key: metric.key(),
        name: descriptor.name,
        short_description: descriptor.short_description,
        description: descriptor.full_description,
        unit: "ratio",
        default_ratio_rise_limit: Some(limit.ratio_rise.ratio()),
        default_new_findings_rule: Some(limit.new_findings),
    }
}

fn metric_schema_without_regression(
    metric: Metric,
    descriptor: BuiltInMetricDescriptor,
) -> MetricSchema {
    MetricSchema {
        key: metric.key(),
        name: descriptor.name,
        short_description: descriptor.short_description,
        description: descriptor.full_description,
        unit: "ratio",
        default_ratio_rise_limit: None,
        default_new_findings_rule: None,
    }
}
