use std::collections::BTreeMap;

use codeint_config::{
    CliCommandSchema, CliSchema, NewFindingsRule, OutputFormatSchema, RegressionLimits,
    SchemaFormat, configuration_schema,
};
use codeint_metrics::{
    DEV_IN_PRODUCTION_DEPENDENCIES_METRIC_KEY, HAZARDS_METRIC_KEY, TEST_GAP_METRIC_KEY,
    UNLISTED_DEPENDENCIES_METRIC_KEY, UNUSED_DEPENDENCIES_METRIC_KEY,
};
use codeint_model::{Metric, SourceSet};
use serde::Serialize;
use serde_json::Value;

use crate::{COGNITIVE_COMPLEXITY_KEY, ReportError};

pub(crate) const BUILT_IN_METRICS: [Metric; 4] = [
    Metric::Erosion,
    Metric::Duplication,
    Metric::Verbosity,
    Metric::ImportCycles,
];

const SOURCE_SETS: [SourceSet; 4] = [
    SourceSet::Production,
    SourceSet::Test,
    SourceSet::Generated,
    SourceSet::Vendored,
];

#[derive(Debug, Clone, Copy)]
pub(crate) struct BuiltInMetricDescriptor {
    pub(crate) name: &'static str,
    pub(crate) short_description: &'static str,
    pub(crate) full_description: &'static str,
    pub(crate) help: &'static str,
}

pub(crate) fn built_in_metric_descriptor(metric: Metric) -> BuiltInMetricDescriptor {
    match metric {
        Metric::Erosion => BuiltInMetricDescriptor {
            name: "Erosion",
            short_description: "A function with cyclomatic complexity over 10",
            full_description: "Erosion is the share of total Function mass held by functions whose cyclomatic complexity is greater than 10. Function mass is cyclomatic complexity times the square root of source lines.",
            help: "Split the function into smaller functions, each with a cyclomatic complexity of 10 or less.",
        },
        Metric::Duplication => BuiltInMetricDescriptor {
            name: "Duplication",
            short_description: "A Clone group: code fragments with the same token sequence",
            full_description: "Duplication is the share of source lines that belong to at least one Clone group. A Clone group is two or more code fragments with the same token sequence once identifiers and literals are normalized.",
            help: "Extract the fragments of the Clone group into one shared function or module.",
        },
        Metric::Verbosity => BuiltInMetricDescriptor {
            name: "Verbosity",
            short_description: "A Flagged line: code that can be deleted or collapsed",
            full_description: "Verbosity is the share of source lines that are Flagged lines, clone lines, or both. A Flagged line matches a verbose-code pattern of the source language, and the result names the rule it matches.",
            help: "Delete or collapse the line as its rule describes; the behavior stays the same.",
        },
        Metric::ImportCycles => BuiltInMetricDescriptor {
            name: "Import cycles",
            short_description: "An Import cycle among runtime imports",
            full_description: "Import cycles is the share of modules that belong to at least one Import cycle. An Import cycle is a group of modules in which every module can reach every other module through runtime imports.",
            help: "Break the Import cycle, for example by moving the code that the modules share into a module that imports none of them.",
        },
    }
}

pub(crate) const COGNITIVE_COMPLEXITY_DESCRIPTOR: BuiltInMetricDescriptor =
    BuiltInMetricDescriptor {
        name: "Cognitive complexity",
        short_description: "A function with cognitive complexity over 15",
        full_description: "Cognitive complexity is the share of total cognitive complexity held by functions whose cognitive complexity is greater than 15. Cognitive complexity weights control-flow structures by nesting depth.",
        help: "Flatten nested control flow or split the function into smaller functions, each with a cognitive complexity of 15 or less.",
    };

pub(crate) const ORPHAN_FILES_DESCRIPTOR: BuiltInMetricDescriptor = BuiltInMetricDescriptor {
    name: "Orphan files",
    short_description: "A Production file unreachable from any entry point",
    full_description: "Orphan files measures the share of Production files that cannot be reached through resolved runtime imports from any manifest-declared, program-start, or configured entry point, annotating files imported only by tests as test-only.",
    help: "Delete the unused Production file (and any stale tests that only import it), wire it into an entry point, or add its path glob to entry_points in codeint.json.",
};

pub(crate) const UNUSED_DEPENDENCIES_DESCRIPTOR: BuiltInMetricDescriptor =
    BuiltInMetricDescriptor {
        name: "Unused dependencies",
        short_description: "A declared dependency that no file in the package imports",
        full_description: "Unused dependencies is the share of declared runtime and dev dependencies in package manifests that are not imported by any source file in the owning package.",
        help: "Remove the unused dependency from the package manifest, or add it to dependencies.ignore in codeint.json if it is used outside source imports.",
    };

pub(crate) const UNLISTED_DEPENDENCIES_DESCRIPTOR: BuiltInMetricDescriptor =
    BuiltInMetricDescriptor {
        name: "Unlisted dependencies",
        short_description: "An external package imported without being declared in the package manifest",
        full_description: "Unlisted dependencies is the share of imported external packages that are missing from the owning package manifest.",
        help: "Declare the package in the owning package manifest, or remove the import.",
    };

pub(crate) const DEV_IN_PRODUCTION_DEPENDENCIES_DESCRIPTOR: BuiltInMetricDescriptor =
    BuiltInMetricDescriptor {
        name: "Dev dependencies in production",
        short_description: "A dev-only dependency imported by a Production source file",
        full_description: "Dev dependencies in production is the share of production-imported external packages that are declared only as dev dependencies in the package manifest.",
        help: "Move the dependency to runtime dependencies in the package manifest, or stop importing it from Production files.",
    };

pub(crate) const TEST_GAP_DESCRIPTOR: BuiltInMetricDescriptor = BuiltInMetricDescriptor {
    name: "Test gap",
    short_description: "A Production function whose executable lines tests do not fully execute",
    full_description: "Test gap is the share of total Production Function mass that tests do not execute, computed from LCOV or Cobertura XML Coverage reports.",
    help: "Add tests that execute the uncovered lines of this function, starting with the heaviest untested functions first.",
};

pub(crate) const HAZARDS_DESCRIPTOR: BuiltInMetricDescriptor = BuiltInMetricDescriptor {
    name: "Hazards",
    short_description: "A Hazard line: a parsed pattern likely to cause wrong results, slow execution, or a security exposure",
    full_description: "Hazards is the share of source lines that are Hazard lines in languages that have Hazard line rules. Each Hazard line Finding names its path, line, and rule, and never the source text.",
    help: "Rewrite the statement to remove the hazard named by the rule, or turn off the rule in hazards.disabled_rules in codeint.json.",
};

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
        let mut metrics: Vec<MetricSchema> = BUILT_IN_METRICS
            .into_iter()
            .map(|metric| {
                let descriptor = built_in_metric_descriptor(metric);
                let limit = default_limits.limit(SourceSet::Production, metric);
                metric_schema(metric.key(), descriptor, limit)
            })
            .collect();
        for (key, descriptor) in [
            (COGNITIVE_COMPLEXITY_KEY, COGNITIVE_COMPLEXITY_DESCRIPTOR),
            ("orphan_files", ORPHAN_FILES_DESCRIPTOR),
            (HAZARDS_METRIC_KEY, HAZARDS_DESCRIPTOR),
        ] {
            let limit = default_limits.limit_for_key(SourceSet::Production, key);
            metrics.push(metric_schema(key, descriptor, limit));
        }
        for (key, descriptor) in [
            (
                UNUSED_DEPENDENCIES_METRIC_KEY,
                UNUSED_DEPENDENCIES_DESCRIPTOR,
            ),
            (
                UNLISTED_DEPENDENCIES_METRIC_KEY,
                UNLISTED_DEPENDENCIES_DESCRIPTOR,
            ),
            (
                DEV_IN_PRODUCTION_DEPENDENCIES_METRIC_KEY,
                DEV_IN_PRODUCTION_DEPENDENCIES_DESCRIPTOR,
            ),
        ] {
            let limit = default_limits.limit_for_dependency(key);
            metrics.push(metric_schema(key, descriptor, limit));
        }
        metrics.push(metric_schema_without_regression(
            TEST_GAP_METRIC_KEY,
            TEST_GAP_DESCRIPTOR,
        ));
        let source_sets = SOURCE_SETS.into_iter().map(SourceSet::key).collect();
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
    key: &'static str,
    descriptor: BuiltInMetricDescriptor,
    limit: codeint_config::RegressionLimit,
) -> MetricSchema {
    MetricSchema {
        key,
        name: descriptor.name,
        short_description: descriptor.short_description,
        description: descriptor.full_description,
        unit: "ratio",
        default_ratio_rise_limit: Some(limit.ratio_rise.ratio()),
        default_new_findings_rule: Some(limit.new_findings),
    }
}

fn metric_schema_without_regression(
    key: &'static str,
    descriptor: BuiltInMetricDescriptor,
) -> MetricSchema {
    MetricSchema {
        key,
        name: descriptor.name,
        short_description: descriptor.short_description,
        description: descriptor.full_description,
        unit: "ratio",
        default_ratio_rise_limit: None,
        default_new_findings_rule: None,
    }
}
