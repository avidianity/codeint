#![cfg_attr(not(any(feature = "std", test)), no_std)]

extern crate alloc;
extern crate self as codeint_plugin_sdk;

pub mod hash;
pub mod language;
pub mod metric;
pub mod prelude;
mod runtime;
#[cfg(test)]
mod tests;

pub use hash::{Fnv1a64, fnv1a_64};
pub use language::LanguagePlugin;
pub use metric::MetricPlugin;

#[doc(hidden)]
pub mod __alloc {
    pub use alloc::string;
    pub use alloc::vec;
}
#[doc(hidden)]
pub use runtime::__bindings;
#[doc(hidden)]
pub use runtime::link_runtime_symbols as __link_runtime_symbols;
#[doc(hidden)]
pub use wit_bindgen as __wit_bindgen;

pub const PLUGIN_INTERFACE_VERSION: &str = "0.4.0";

#[macro_export]
macro_rules! export_language_plugin {
    ($plugin:ty) => {
        const _: () = {
            struct __CodeintLanguageBridge;

            impl $crate::__bindings::language::exports::codeint::plugin::language::Guest
                for __CodeintLanguageBridge
            {
                fn extensions() -> $crate::__alloc::vec::Vec<$crate::__alloc::string::String> {
                    $crate::__link_runtime_symbols();
                    <$plugin as $crate::language::LanguagePlugin>::extensions()
                }

                fn lower(
                    path: $crate::__alloc::string::String,
                    text: $crate::__alloc::string::String,
                ) -> ::core::result::Result<
                    $crate::language::FileModel,
                    $crate::language::LowerError,
                > {
                    <$plugin as $crate::language::LanguagePlugin>::lower(path, text)
                }

                fn manifest_paths(
                    repository_paths: $crate::__alloc::vec::Vec<$crate::__alloc::string::String>,
                ) -> $crate::__alloc::vec::Vec<$crate::__alloc::string::String> {
                    <$plugin as $crate::language::LanguagePlugin>::manifest_paths(repository_paths)
                }

                fn workspace_packages(
                    manifests: $crate::__alloc::vec::Vec<$crate::language::Manifest>,
                ) -> ::core::result::Result<
                    $crate::__alloc::vec::Vec<$crate::__alloc::string::String>,
                    $crate::language::WorkspaceError,
                > {
                    <$plugin as $crate::language::LanguagePlugin>::workspace_packages(manifests)
                }

                fn resolve_imports(
                    repository_paths: $crate::__alloc::vec::Vec<$crate::__alloc::string::String>,
                    manifests: $crate::__alloc::vec::Vec<$crate::language::Manifest>,
                    requests: $crate::__alloc::vec::Vec<$crate::language::ImportRequest>,
                    active_profiles: $crate::__alloc::vec::Vec<$crate::__alloc::string::String>,
                ) -> $crate::__alloc::vec::Vec<$crate::language::Resolution> {
                    <$plugin as $crate::language::LanguagePlugin>::resolve_imports(
                        repository_paths,
                        manifests,
                        requests,
                        active_profiles,
                    )
                }

                fn declared_dependencies(
                    manifests: $crate::__alloc::vec::Vec<$crate::language::Manifest>,
                ) -> $crate::__alloc::vec::Vec<$crate::language::DeclaredDep> {
                    <$plugin as $crate::language::LanguagePlugin>::declared_dependencies(manifests)
                }

                fn entry_points(
                    repository_paths: $crate::__alloc::vec::Vec<$crate::__alloc::string::String>,
                    manifests: $crate::__alloc::vec::Vec<$crate::language::Manifest>,
                ) -> $crate::__alloc::vec::Vec<$crate::__alloc::string::String> {
                    <$plugin as $crate::language::LanguagePlugin>::entry_points(
                        repository_paths,
                        manifests,
                    )
                }

                fn reference_ranges() -> $crate::__alloc::vec::Vec<$crate::language::ReferenceRange>
                {
                    <$plugin as $crate::language::LanguagePlugin>::reference_ranges()
                }

                fn known_framework_profiles()
                -> $crate::__alloc::vec::Vec<$crate::__alloc::string::String> {
                    <$plugin as $crate::language::LanguagePlugin>::known_framework_profiles()
                }

                fn resolve_framework_profiles(
                    package_root: $crate::__alloc::string::String,
                    files: $crate::__alloc::vec::Vec<$crate::__alloc::string::String>,
                    manifests: $crate::__alloc::vec::Vec<$crate::language::Manifest>,
                    selection: $crate::language::FrameworkProfileSelection,
                ) -> ::core::result::Result<
                    $crate::language::FrameworkProfileResult,
                    $crate::__alloc::string::String,
                > {
                    <$plugin as $crate::language::LanguagePlugin>::resolve_framework_profiles(
                        package_root,
                        files,
                        manifests,
                        selection,
                    )
                }

                fn has_hazard_rules() -> bool {
                    <$plugin as $crate::language::LanguagePlugin>::has_hazard_rules()
                }

                fn hazard_rules() -> $crate::__alloc::vec::Vec<$crate::__alloc::string::String> {
                    <$plugin as $crate::language::LanguagePlugin>::hazard_rules()
                }

                fn has_imports() -> bool {
                    <$plugin as $crate::language::LanguagePlugin>::has_imports()
                }
            }

            $crate::__bindings::language::__export_language_world!(
                __CodeintLanguageBridge with_types_in $crate::__bindings::language
            );
        };
    };
}

#[macro_export]
macro_rules! export_metric_plugin {
    ($plugin:ty) => {
        const _: () = {
            struct __CodeintMetricBridge;

            impl $crate::__bindings::metric::exports::codeint::plugin::metric::Guest
                for __CodeintMetricBridge
            {
                fn key() -> $crate::__alloc::string::String {
                    $crate::__link_runtime_symbols();
                    <$plugin as $crate::metric::MetricPlugin>::key()
                }

                fn name() -> $crate::__alloc::string::String {
                    <$plugin as $crate::metric::MetricPlugin>::name()
                }

                fn description() -> $crate::__alloc::string::String {
                    <$plugin as $crate::metric::MetricPlugin>::description()
                }

                fn version() -> u32 {
                    <$plugin as $crate::metric::MetricPlugin>::version()
                }

                fn measure(model: $crate::metric::CodeModel) -> $crate::metric::Measurement {
                    <$plugin as $crate::metric::MetricPlugin>::measure(model)
                }
            }

            $crate::__bindings::metric::__export_metric_world!(
                __CodeintMetricBridge with_types_in $crate::__bindings::metric
            );
        };
    };
}
