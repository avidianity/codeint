use std::collections::BTreeMap;

use codeint_lang_dart::Dart;
use codeint_lang_go::Go;
use codeint_lang_php::Php;
use codeint_lang_python::Python;
use codeint_lang_rust::Rust;
use codeint_lang_shell::Shell;
use codeint_lang_sql::Sql;
use codeint_lang_ts::TypeScriptAndJavaScript;
use codeint_model::LanguagePlugin;

pub const TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME: &str = "typescript-javascript";

const BUILT_IN_SQL: Sql = Sql::new();

#[derive(Clone, Copy)]
pub struct NamedLanguagePlugin<'plugin> {
    pub name: &'plugin str,
    pub plugin: &'plugin dyn LanguagePlugin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuiltInLanguageDescriptor {
    pub name: String,
    pub extensions: Vec<String>,
}

const BUILT_IN_LANGUAGE_PLUGINS: [NamedLanguagePlugin<'static>; 8] = [
    NamedLanguagePlugin {
        name: TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME,
        plugin: &TypeScriptAndJavaScript,
    },
    NamedLanguagePlugin {
        name: "rust",
        plugin: &Rust,
    },
    NamedLanguagePlugin {
        name: "php",
        plugin: &Php,
    },
    NamedLanguagePlugin {
        name: "dart",
        plugin: &Dart,
    },
    NamedLanguagePlugin {
        name: "python",
        plugin: &Python,
    },
    NamedLanguagePlugin {
        name: "go",
        plugin: &Go,
    },
    NamedLanguagePlugin {
        name: "sql",
        plugin: &BUILT_IN_SQL,
    },
    NamedLanguagePlugin {
        name: "shell",
        plugin: &Shell,
    },
];

pub fn built_in_language_plugins() -> &'static [NamedLanguagePlugin<'static>] {
    &BUILT_IN_LANGUAGE_PLUGINS
}

pub fn built_in_language_plugin(name: &str) -> Option<NamedLanguagePlugin<'static>> {
    BUILT_IN_LANGUAGE_PLUGINS
        .iter()
        .copied()
        .find(|named| named.name == name)
}

pub fn built_in_language_names() -> Vec<&'static str> {
    BUILT_IN_LANGUAGE_PLUGINS
        .iter()
        .map(|named| named.name)
        .collect()
}

pub fn built_in_language_descriptors() -> Vec<BuiltInLanguageDescriptor> {
    BUILT_IN_LANGUAGE_PLUGINS
        .iter()
        .map(|named| BuiltInLanguageDescriptor {
            name: named.name.to_owned(),
            extensions: named.plugin.extensions(),
        })
        .collect()
}

pub fn built_in_language_hazard_rules() -> BTreeMap<String, Vec<String>> {
    BUILT_IN_LANGUAGE_PLUGINS
        .iter()
        .filter(|named| named.plugin.has_hazard_rules())
        .map(|named| (named.name.to_owned(), named.plugin.hazard_rules()))
        .collect()
}

pub fn built_in_language_flagged_line_rules() -> BTreeMap<String, Vec<String>> {
    BUILT_IN_LANGUAGE_PLUGINS
        .iter()
        .map(|named| (named.name.to_owned(), named.plugin.flagged_line_rules()))
        .collect()
}

pub fn built_in_language_framework_profiles() -> BTreeMap<String, Vec<String>> {
    BUILT_IN_LANGUAGE_PLUGINS
        .iter()
        .filter_map(|named| {
            let profiles = named.plugin.known_framework_profiles();
            (!profiles.is_empty()).then(|| (named.name.to_owned(), profiles))
        })
        .collect()
}

pub fn built_in_language_dialects() -> BTreeMap<String, Vec<String>> {
    BUILT_IN_LANGUAGE_PLUGINS
        .iter()
        .map(|named| (named.name.to_owned(), named.plugin.known_dialects()))
        .collect()
}
