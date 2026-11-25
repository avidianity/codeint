mod common;
mod history_support;
#[path = "../wasm_fixtures/mod.rs"]
mod wasm_fixtures;

mod agent;
mod boundaries;
mod churn;
mod codeowners;
mod config;
mod coupling;
mod dependencies;
mod diff;
mod doctor;
mod go;
mod guard;
mod hazards;
mod impact;
mod init;
#[cfg(unix)]
mod install;
mod mcp;
mod npm_packages;
mod orphan_files;
mod package_filter;
mod plugin_test;
mod ratchet;
mod report;
mod schema;
mod sdk;
mod sql;
mod test_gap;
mod trace;
mod trend;
mod update;
