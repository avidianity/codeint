use serde_json::{Value, json};

pub fn report_schema() -> String {
    let schema = report_schema_value();
    format!("{schema:#}\n")
}

fn report_schema_value() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "codeint check JSON report",
        "description": "The JSON output of `codeint check --format json`.",
        "type": "object",
        "required": ["files", "source_lines", "source_sets"],
        "properties": {
            "files": { "type": "integer", "minimum": 0 },
            "source_lines": { "type": "integer", "minimum": 0 },
            "import_metrics_note": { "type": "string" },
            "source_sets": { "$ref": "#/$defs/SourceSetSections" },
            "test_gap_note": {
                "type": "string",
                "description": "One-line explanation shown when Test gap is absent for the Production Source set."
            },
            "coverage_unmatched_note": {
                "type": "string",
                "description": "One-line note listing every Coverage report path that matched no repository file unambiguously."
            },
            "dependencies": { "type": "object" },
            "languages": {
                "type": "object",
                "additionalProperties": { "$ref": "#/$defs/MeasuredSection" }
            },
            "skipped": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["path", "reason"],
                    "properties": {
                        "path": { "type": "string" },
                        "reason": { "type": "string" }
                    }
                }
            },
            "declaration_only": { "type": "array", "items": { "type": "string" } },
            "submodules": { "type": "array", "items": { "type": "string" } },
            "over_budget": {
                "type": "array",
                "items": { "$ref": "#/$defs/BudgetFailure" }
            }
        },
        "$defs": {
            "MeasuredSection": {
                "type": "object",
                "required": ["files", "source_lines", "source_sets"],
                "properties": {
                    "files": { "type": "integer", "minimum": 0 },
                    "source_lines": { "type": "integer", "minimum": 0 },
                    "import_metrics_note": { "type": "string" },
                    "source_sets": { "$ref": "#/$defs/SourceSetSections" },
                    "test_gap_note": { "type": "string" },
                    "dependencies": { "type": "object" }
                }
            },
            "SourceSetSections": {
                "type": "object",
                "properties": {
                    "production": { "$ref": "#/$defs/SourceSetSection" },
                    "test": { "$ref": "#/$defs/NonProductionSourceSetSection" },
                    "generated": { "$ref": "#/$defs/NonProductionSourceSetSection" },
                    "vendored": { "$ref": "#/$defs/NonProductionSourceSetSection" }
                },
                "additionalProperties": false
            },
            "SourceSetSection": {
                "type": "object",
                "required": ["files", "source_lines", "erosion", "cognitive_complexity", "duplication", "verbosity"],
                "properties": {
                    "files": { "type": "integer", "minimum": 0 },
                    "source_lines": { "type": "integer", "minimum": 0 },
                    "erosion": { "type": "object" },
                    "cognitive_complexity": { "type": "object" },
                    "duplication": { "type": "object" },
                    "verbosity": { "type": "object" },
                    "import_cycles": { "type": "object" },
                    "boundaries": { "type": "object" },
                    "orphan_files": { "type": "object" },
                    "test_gap": { "$ref": "#/$defs/TestGapSection" },
                    "packages": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/PackageSection" }
                    }
                }
            },
            "NonProductionSourceSetSection": {
                "type": "object",
                "required": ["files", "source_lines", "erosion", "cognitive_complexity", "duplication", "verbosity"],
                "properties": {
                    "files": { "type": "integer", "minimum": 0 },
                    "source_lines": { "type": "integer", "minimum": 0 },
                    "erosion": { "type": "object" },
                    "cognitive_complexity": { "type": "object" },
                    "duplication": { "type": "object" },
                    "verbosity": { "type": "object" },
                    "import_cycles": { "type": "object" },
                    "boundaries": { "type": "object" },
                    "test_gap": false,
                    "packages": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/PackageSection" }
                    }
                }
            },
            "TestGapSection": {
                "type": "object",
                "description": "Test gap for the Production Source set; omitted when no Coverage report applies.",
                "required": ["ratio", "untested_function_mass", "total_function_mass", "measured_functions", "findings"],
                "properties": {
                    "ratio": { "type": "number", "minimum": 0, "maximum": 1 },
                    "untested_function_mass": { "type": "number", "minimum": 0 },
                    "total_function_mass": { "type": "number", "minimum": 0 },
                    "measured_functions": { "type": "integer", "minimum": 0 },
                    "findings": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/TestGapFinding" }
                    },
                    "findings_left_out": { "type": "integer", "minimum": 1 }
                },
                "additionalProperties": false
            },
            "TestGapFinding": {
                "type": "object",
                "required": ["id", "path", "line", "end_line", "function_mass", "untested_share"],
                "properties": {
                    "id": { "type": "string" },
                    "path": { "type": "string" },
                    "line": { "type": "integer", "minimum": 1 },
                    "end_line": { "type": "integer", "minimum": 1 },
                    "name": { "type": "string" },
                    "function_mass": { "type": "number", "minimum": 0 },
                    "untested_share": { "type": "number", "minimum": 0, "maximum": 1 },
                    "owners": { "type": "array", "items": { "type": "string" } }
                },
                "additionalProperties": false
            },
            "PackageSection": {
                "type": "object",
                "required": ["path", "files", "source_lines", "erosion", "cognitive_complexity", "duplication", "verbosity"],
                "properties": {
                    "path": { "type": "string" },
                    "files": { "type": "integer", "minimum": 0 },
                    "source_lines": { "type": "integer", "minimum": 0 },
                    "erosion": { "type": "object" },
                    "cognitive_complexity": { "type": "object" },
                    "duplication": { "type": "object" },
                    "verbosity": { "type": "object" },
                    "import_cycles": { "type": "object" },
                    "test_gap": { "$ref": "#/$defs/PackageTestGap" }
                },
                "additionalProperties": false
            },
            "PackageTestGap": {
                "type": "object",
                "required": ["ratio", "untested_function_mass", "total_function_mass"],
                "properties": {
                    "ratio": { "type": "number", "minimum": 0, "maximum": 1 },
                    "untested_function_mass": { "type": "number", "minimum": 0 },
                    "total_function_mass": { "type": "number", "minimum": 0 }
                },
                "additionalProperties": false
            },
            "BudgetFailure": {
                "type": "object",
                "required": ["metric", "ratio", "budget"],
                "properties": {
                    "source_set": { "type": "string" },
                    "metric": { "type": "string" },
                    "ratio": { "type": "number", "minimum": 0, "maximum": 1 },
                    "budget": { "type": "number", "minimum": 0, "maximum": 1 }
                },
                "additionalProperties": false
            }
        }
    })
}
