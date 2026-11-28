mod common;
mod history_support;
#[path = "../wasm_fixtures/mod.rs"]
mod wasm_fixtures;

#[cfg(unix)]
mod action;
mod agent;
mod baseline;
mod boundaries;
mod churn;
mod codeowners;
mod config;
mod config_formats;
mod coupling;
mod dependencies;
mod diff;
mod doctor;
#[cfg(unix)]
mod evaluation_runner;
mod format_config_and_text;
mod go;
mod guard;
mod hazards;
mod impact;
mod init;
mod init_adopt;
#[cfg(unix)]
mod install;
mod invalid_manifest;
mod mcp;
mod mcp_cache;
mod npm_packages;
mod orphan_files;
mod package_filter;
mod planted_defects;
mod plugin_test;
mod ratchet;
mod report;
mod report_schemas;
mod schema;
mod sdk;
mod shell;
mod sql;
mod test_gap;
mod trace;
mod trend;
mod update;
