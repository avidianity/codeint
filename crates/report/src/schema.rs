use codeint_config::{
    CliCommandSchema, CliSchema, NewFindingsRule, OutputFormatSchema, RegressionLimits,
    SchemaFormat,
};
use codeint_model::{Metric, SourceSet};
use serde::Serialize;
use serde_json::Value;

use crate::ReportError;

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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SchemaReport {
    name: String,
    version: String,
    description: String,
    commands: Vec<CliCommandSchema>,
    metrics: Vec<MetricSchema>,
    source_sets: Vec<&'static str>,
    languages: Vec<LanguageSchema>,
    output_formats: Vec<OutputFormatSchema>,
    mcp_tools: Vec<McpToolSchema>,
    exit_codes: Vec<ExitCodeSchema>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MetricSchema {
    key: &'static str,
    name: &'static str,
    short_description: &'static str,
    description: &'static str,
    unit: &'static str,
    default_ratio_rise_limit: f64,
    default_new_findings_rule: NewFindingsRule,
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
        let metrics = BUILT_IN_METRICS
            .into_iter()
            .map(|metric| {
                let descriptor = built_in_metric_descriptor(metric);
                let limit = default_limits.limit(SourceSet::Production, metric);
                MetricSchema {
                    key: metric.key(),
                    name: descriptor.name,
                    short_description: descriptor.short_description,
                    description: descriptor.full_description,
                    unit: "ratio",
                    default_ratio_rise_limit: limit.ratio_rise.ratio(),
                    default_new_findings_rule: limit.new_findings,
                }
            })
            .collect();
        let source_sets = SOURCE_SETS.into_iter().map(SourceSet::key).collect();
        SchemaReport {
            name: cli.name,
            version: cli.version,
            description: cli.description,
            commands: cli.commands,
            metrics,
            source_sets,
            languages,
            output_formats: cli.output_formats,
            mcp_tools,
            exit_codes,
        }
    }

    pub fn render(&self, format: SchemaFormat) -> Result<String, ReportError> {
        match format {
            SchemaFormat::Json => crate::render_json(self),
            SchemaFormat::Toon => crate::render_toon(self),
        }
    }
}
