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

pub const PLUGIN_INTERFACE_VERSION: &str = "0.5.0";

#[macro_export]
macro_rules! export_language_plugin {
    ($plugin:ty) => {
        const _: () = {
            use $crate::__alloc::string::String as __SdkString;
            use $crate::__alloc::vec::Vec as __SdkVec;
            use $crate::language::{self as __lang, LanguagePlugin as __LangPlugin};

            struct __CodeintLanguageBridge;

            impl $crate::__bindings::language::exports::codeint::plugin::language::Guest
                for __CodeintLanguageBridge
            {
                fn extensions() -> __SdkVec<__SdkString> {
                    $crate::__link_runtime_symbols();
                    <$plugin as __LangPlugin>::extensions()
                }

                fn known_dialects() -> __SdkVec<__SdkString> {
                    <$plugin as __LangPlugin>::known_dialects()
                }

                fn lower(
                    path: __SdkString,
                    text: __SdkString,
                    dialect: ::core::option::Option<__SdkString>,
                ) -> ::core::result::Result<__lang::FileModel, __lang::LowerError> {
                    <$plugin as __LangPlugin>::lower(path, text, dialect)
                }

                fn flagged_line_rules() -> __SdkVec<__SdkString> {
                    <$plugin as __LangPlugin>::flagged_line_rules()
                }

                fn manifest_paths(
                    repository_paths: __SdkVec<__SdkString>,
                ) -> __SdkVec<__SdkString> {
                    <$plugin as __LangPlugin>::manifest_paths(repository_paths)
                }

                fn analyze_manifests(
                    manifests: __SdkVec<__lang::Manifest>,
                ) -> __lang::ManifestAnalysis {
                    <$plugin as __LangPlugin>::analyze_manifests(manifests)
                }

                fn resolve_imports(
                    repository_paths: __SdkVec<__SdkString>,
                    manifests: __SdkVec<__lang::Manifest>,
                    requests: __SdkVec<__lang::ImportRequest>,
                    package_profiles: __SdkVec<__lang::PackageFrameworkProfiles>,
                ) -> __SdkVec<__lang::Resolution> {
                    <$plugin as __LangPlugin>::resolve_imports(
                        repository_paths,
                        manifests,
                        requests,
                        package_profiles,
                    )
                }

                fn entry_points(
                    repository_paths: __SdkVec<__SdkString>,
                    manifests: __SdkVec<__lang::Manifest>,
                ) -> __SdkVec<__SdkString> {
                    <$plugin as __LangPlugin>::entry_points(repository_paths, manifests)
                }

                fn reference_ranges() -> __SdkVec<__lang::ReferenceRange> {
                    <$plugin as __LangPlugin>::reference_ranges()
                }

                fn known_framework_profiles() -> __SdkVec<__SdkString> {
                    <$plugin as __LangPlugin>::known_framework_profiles()
                }

                fn resolve_framework_profiles(
                    package_root: __SdkString,
                    files: __SdkVec<__SdkString>,
                    manifests: __SdkVec<__lang::Manifest>,
                    selection: __lang::FrameworkProfileSelection,
                ) -> ::core::result::Result<__lang::FrameworkProfileResult, __SdkString> {
                    <$plugin as __LangPlugin>::resolve_framework_profiles(
                        package_root,
                        files,
                        manifests,
                        selection,
                    )
                }

                fn has_hazard_rules() -> bool {
                    <$plugin as __LangPlugin>::has_hazard_rules()
                }

                fn hazard_rules() -> __SdkVec<__SdkString> {
                    <$plugin as __LangPlugin>::hazard_rules()
                }

                fn has_imports() -> bool {
                    <$plugin as __LangPlugin>::has_imports()
                }

                fn has_scope_analysis() -> bool {
                    <$plugin as __LangPlugin>::has_scope_analysis()
                }

                fn analyze_scope(
                    package_root: __SdkString,
                    dialect: ::core::option::Option<__SdkString>,
                    artifacts: __SdkVec<__lang::FileScopeArtifact>,
                ) -> __lang::ScopeAnalysis {
                    <$plugin as __LangPlugin>::analyze_scope(package_root, dialect, artifacts)
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
