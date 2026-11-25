use std::collections::BTreeMap;

use codeint_lang_dart::Dart;
use codeint_lang_go::Go;
use codeint_lang_php::Php;
use codeint_lang_python::Python;
use codeint_lang_rust::Rust;
use codeint_lang_ts::{TypeScriptAndJavaScript, react_reference_ranges, vue_reference_ranges};
use codeint_model::{CodeModel, InvalidReferenceRange, LanguagePlugin, Manifest, ReferenceRanges};

use crate::sql::BuiltInSql;

pub const TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME: &str = "typescript-javascript";
pub const SQL_PLUGIN_NAME: &str = "sql";

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

const BUILT_IN_LANGUAGE_PLUGINS: [NamedLanguagePlugin<'static>; 7] = [
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
        name: SQL_PLUGIN_NAME,
        plugin: &BuiltInSql,
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

pub fn manifests_declare_framework_reference_ranges(
    plugin_name: &str,
    manifests: &[Manifest],
) -> bool {
    plugin_name == TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME
        && (vue_reference_ranges(manifests).is_some()
            || react_reference_ranges(manifests).is_some())
}

pub fn select_language_reference_ranges(
    named: &NamedLanguagePlugin<'_>,
    model: &CodeModel,
    manifests: &[Manifest],
) -> Result<ReferenceRanges, InvalidReferenceRange> {
    if named.name == TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME {
        let has_vue = model
            .files
            .iter()
            .any(|file| file.path.as_str().ends_with(".vue"));
        let has_jsx_or_tsx = model.files.iter().any(|file| {
            file.path.as_str().ends_with(".tsx") || file.path.as_str().ends_with(".jsx")
        });
        let react_ranges = has_jsx_or_tsx
            .then(|| react_reference_ranges(manifests))
            .flatten();
        if has_vue {
            if react_ranges.is_none()
                && let Some(ranges) = vue_reference_ranges(manifests)
            {
                return ReferenceRanges::new(ranges);
            }
        } else if let Some(ranges) = react_ranges {
            return ReferenceRanges::new(ranges);
        }
    }
    named.plugin.reference_ranges()
}
