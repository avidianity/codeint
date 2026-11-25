#![no_std]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::arch::wasm32;
use core::cell::UnsafeCell;

struct BumpAllocator {
    offset: UnsafeCell<usize>,
    limit: UnsafeCell<usize>,
}

unsafe impl Sync for BumpAllocator {}

impl BumpAllocator {
    const fn new() -> BumpAllocator {
        BumpAllocator {
            offset: UnsafeCell::new(0),
            limit: UnsafeCell::new(0),
        }
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align().max(8);
        let size = layout.size();
        let current = unsafe { *self.offset.get() };
        let limit = unsafe { *self.limit.get() };
        let aligned = (current + align - 1) & !(align - 1);
        let end = match aligned.checked_add(size) {
            Some(end) => end,
            None => return core::ptr::null_mut(),
        };
        if end <= limit && current != 0 {
            unsafe { *self.offset.get() = end };
            return aligned as *mut u8;
        }
        let needed = size + align;
        let pages = needed.div_ceil(65536).max(1);
        let prev_pages = wasm32::memory_grow(0, pages);
        if prev_pages == usize::MAX {
            return core::ptr::null_mut();
        }
        let base = prev_pages * 65536;
        let new_limit = base + pages * 65536;
        let start = (base + align - 1) & !(align - 1);
        unsafe {
            *self.offset.get() = start + size;
            *self.limit.get() = new_limit;
        }
        start as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator::new();

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    wasm32::unreachable()
}

#[unsafe(export_name = "cabi_realloc")]
pub unsafe extern "C" fn cabi_realloc(
    old_ptr: *mut u8,
    old_size: usize,
    align: usize,
    new_size: usize,
) -> *mut u8 {
    if new_size == 0 {
        return align as *mut u8;
    }
    let layout = match Layout::from_size_align(new_size, align) {
        Ok(layout) => layout,
        Err(_) => wasm32::unreachable(),
    };
    let ptr = if old_size == 0 {
        unsafe { alloc::alloc::alloc(layout) }
    } else {
        let old_layout = match Layout::from_size_align(old_size, align) {
            Ok(layout) => layout,
            Err(_) => wasm32::unreachable(),
        };
        unsafe { alloc::alloc::realloc(old_ptr, old_layout, new_size) }
    };
    if ptr.is_null() {
        wasm32::unreachable()
    }
    ptr
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(left: *const u8, right: *const u8, count: usize) -> i32 {
    let mut index = 0;
    while index < count {
        let a = unsafe { *left.add(index) };
        let b = unsafe { *right.add(index) };
        if a != b {
            return i32::from(a) - i32::from(b);
        }
        index += 1;
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, count: usize) -> *mut u8 {
    let mut index = 0;
    while index < count {
        unsafe { *dest.add(index) = *src.add(index) };
        index += 1;
    }
    dest
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, count: usize) -> *mut u8 {
    if (dest as usize) < (src as usize) {
        let mut index = 0;
        while index < count {
            unsafe { *dest.add(index) = *src.add(index) };
            index += 1;
        }
    } else {
        let mut index = count;
        while index > 0 {
            index -= 1;
            unsafe { *dest.add(index) = *src.add(index) };
        }
    }
    dest
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(dest: *mut u8, value: i32, count: usize) -> *mut u8 {
    let byte = value as u8;
    let mut index = 0;
    while index < count {
        unsafe { *dest.add(index) = byte };
        index += 1;
    }
    dest
}

pub fn fnv1a_64(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

pub fn ratio(numerator: f64, denominator: f64) -> f64 {
    if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}

pub fn count_non_empty_lines(text: &str) -> u32 {
    text.lines().filter(|line| !line.trim().is_empty()).count() as u32
}

pub fn parse_fn_name(rest: &str) -> Option<alloc::string::String> {
    rest.split(|ch: char| ch == '(' || ch.is_whitespace())
        .next()
        .filter(|name| !name.is_empty())
        .map(alloc::string::ToString::to_string)
}

pub fn extract_manifest_entries(
    repository_paths: &[alloc::string::String],
    manifest_texts: &[&str],
) -> alloc::vec::Vec<alloc::string::String> {
    let mut entries = alloc::vec::Vec::new();
    for text in manifest_texts {
        for line in text.lines() {
            if let Some(entry) = line.trim().strip_prefix("entry:") {
                let candidate = entry.trim();
                if repository_paths.iter().any(|path| path == candidate) {
                    entries.push(alloc::string::ToString::to_string(candidate));
                }
            }
        }
    }
    entries
}

#[macro_export]
macro_rules! common_empty_language_metadata_stubs {
    () => {
        fn manifest_paths(
            _repository_paths: ::alloc::vec::Vec<::alloc::string::String>,
        ) -> ::alloc::vec::Vec<::alloc::string::String> {
            ::alloc::vec::Vec::new()
        }

        fn workspace_packages(
            _manifests: ::alloc::vec::Vec<Manifest>,
        ) -> ::core::result::Result<::alloc::vec::Vec<::alloc::string::String>, WorkspaceError> {
            ::core::result::Result::Ok(::alloc::vec::Vec::new())
        }

        fn declared_dependencies(
            _manifests: ::alloc::vec::Vec<Manifest>,
        ) -> ::alloc::vec::Vec<DeclaredDep> {
            ::alloc::vec::Vec::new()
        }

        fn entry_points(
            _repository_paths: ::alloc::vec::Vec<::alloc::string::String>,
            _manifests: ::alloc::vec::Vec<Manifest>,
        ) -> ::alloc::vec::Vec<::alloc::string::String> {
            ::alloc::vec::Vec::new()
        }

        fn reference_ranges() -> ::alloc::vec::Vec<ReferenceRange> {
            ::alloc::vec::Vec::new()
        }
    };
}

#[macro_export]
macro_rules! empty_resolve_imports_stub {
    ($($extra_param:ident : $extra_ty:ty),* $(,)?) => {
        fn resolve_imports(
            _repository_paths: ::alloc::vec::Vec<::alloc::string::String>,
            _manifests: ::alloc::vec::Vec<Manifest>,
            _requests: ::alloc::vec::Vec<ImportRequest>,
            $($extra_param: $extra_ty),*
        ) -> ::alloc::vec::Vec<Resolution> {
            ::alloc::vec::Vec::new()
        }
    };
}

#[macro_export]
macro_rules! legacy_language_metadata_stubs {
    () => {
        $crate::common_empty_language_metadata_stubs!();
        $crate::empty_resolve_imports_stub!();
    };
}

#[macro_export]
macro_rules! empty_framework_profile_stubs {
    () => {
        fn known_framework_profiles() -> ::alloc::vec::Vec<::alloc::string::String> {
            ::alloc::vec::Vec::new()
        }

        fn resolve_framework_profiles(
            _package_root: ::alloc::string::String,
            _files: ::alloc::vec::Vec<::alloc::string::String>,
            _manifests: ::alloc::vec::Vec<Manifest>,
            _selection: FrameworkProfileSelection,
        ) -> ::core::result::Result<FrameworkProfileResult, ::alloc::string::String> {
            ::core::result::Result::Ok(FrameworkProfileResult {
                active_profiles: ::alloc::vec::Vec::new(),
                entry_points: ::alloc::vec::Vec::new(),
                generated_paths: ::alloc::vec::Vec::new(),
                reference_ranges: ::alloc::vec::Vec::new(),
                reference_range_note: ::core::option::Option::None,
            })
        }
    };
}

#[macro_export]
macro_rules! empty_language_metadata_stubs {
    () => {
        $crate::common_empty_language_metadata_stubs!();
        $crate::empty_resolve_imports_stub!(_active_profiles: ::alloc::vec::Vec<::alloc::string::String>);
        $crate::empty_framework_profile_stubs!();
    };
}

#[macro_export]
macro_rules! metric_metadata {
    ($key:expr, $name:expr, $description:expr, $version:expr) => {
        fn key() -> ::alloc::string::String {
            ::alloc::string::ToString::to_string($key)
        }

        fn name() -> ::alloc::string::String {
            ::alloc::string::ToString::to_string($name)
        }

        fn description() -> ::alloc::string::String {
            ::alloc::string::ToString::to_string($description)
        }

        fn version() -> u32 {
            $version
        }
    };
}

#[macro_export]
macro_rules! capability_probe_language_tail {
    () => {
        fn lower(_path: ::alloc::string::String, _text: ::alloc::string::String) -> ::core::result::Result<FileModel, LowerError> {
            ::core::result::Result::Err(LowerError::DeclarationOnly)
        }

        $crate::empty_language_metadata_stubs!();

        $crate::hazard_free_language_facts!(true);
    };
}

#[macro_export]
macro_rules! plain_language_tail {
    () => {
        $crate::empty_language_metadata_stubs!();

        $crate::hazard_free_language_facts!(false);
    };
}

#[macro_export]
macro_rules! panic_no_imports_stubs {
    () => {
        fn manifest_paths(
            _repository_paths: ::alloc::vec::Vec<::alloc::string::String>,
        ) -> ::alloc::vec::Vec<::alloc::string::String> {
            panic!("manifest-paths must not be called when has-imports is false");
        }

        fn workspace_packages(
            _manifests: ::alloc::vec::Vec<Manifest>,
        ) -> ::core::result::Result<::alloc::vec::Vec<::alloc::string::String>, WorkspaceError> {
            panic!("workspace-packages must not be called when has-imports is false");
        }

        fn resolve_imports(
            _repository_paths: ::alloc::vec::Vec<::alloc::string::String>,
            _manifests: ::alloc::vec::Vec<Manifest>,
            _requests: ::alloc::vec::Vec<ImportRequest>,
            _active_profiles: ::alloc::vec::Vec<::alloc::string::String>,
        ) -> ::alloc::vec::Vec<Resolution> {
            panic!("resolve-imports must not be called when has-imports is false");
        }

        fn declared_dependencies(
            _manifests: ::alloc::vec::Vec<Manifest>,
        ) -> ::alloc::vec::Vec<DeclaredDep> {
            panic!("declared-dependencies must not be called when has-imports is false");
        }

        fn entry_points(
            _repository_paths: ::alloc::vec::Vec<::alloc::string::String>,
            _manifests: ::alloc::vec::Vec<Manifest>,
        ) -> ::alloc::vec::Vec<::alloc::string::String> {
            panic!("entry-points must not be called when has-imports is false");
        }
    };
}

#[macro_export]
macro_rules! language_fact_findings {
    ($languages:expr, $prefix:expr, $flag_name:expr, $on_label:expr, $off_label:expr, $extract:expr) => {{
        let mut enabled: usize = 0;
        let mut findings = ::alloc::vec::Vec::new();
        for facts in $languages {
            let active = ($extract)(facts);
            let status = if active {
                enabled += 1;
                $on_label
            } else {
                $off_label
            };
            findings.push(Finding {
                id: ::alloc::format!("{}:{}:{status}", $prefix, facts.language),
                path: ::alloc::format!("languages/{}", facts.language),
                line: 1,
                end_line: None,
                message: ::alloc::format!("language={} {}={active}", facts.language, $flag_name),
            });
        }
        (enabled, findings)
    }};
}

#[macro_export]
macro_rules! blank_file_model {
    ($source_set:expr, $source_lines:expr $(, $field:ident : $val:expr)* $(,)?) => {{
        let mut model = FileModel {
            source_set: $source_set,
            source_lines: $source_lines,
            functions: ::alloc::vec::Vec::new(),
            tokens: ::alloc::vec::Vec::new(),
            flagged_lines: ::alloc::vec::Vec::new(),
            hazard_lines: ::alloc::vec::Vec::new(),
            runtime_imports: ::alloc::vec::Vec::new(),
            program_start: false,
        };
        $(model.$field = $val;)*
        model
    }};
}

#[macro_export]
macro_rules! fixture_erosion_ranges {
    ($source:expr, $has_imports:expr) => {
        fn reference_ranges() -> ::alloc::vec::Vec<ReferenceRange> {
            ::alloc::vec![ReferenceRange {
                metric: ::alloc::string::ToString::to_string("erosion"),
                metric_version: 1,
                low: 0.0,
                high: 0.2,
                source: ::alloc::string::ToString::to_string($source),
            }]
        }

        $crate::empty_framework_profile_stubs!();

        $crate::hazard_free_language_facts!($has_imports);
    };
}

pub fn parse_workspace_manifest_lines(
    manifests: &[(&str, &str)],
    skip_prefix: &str,
) -> Result<alloc::vec::Vec<alloc::string::String>, (alloc::string::String, alloc::string::String)>
{
    let mut packages = alloc::vec::Vec::new();
    for &(path, text) in manifests {
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with(skip_prefix) {
                continue;
            }
            if let Some(problem) = trimmed.strip_prefix("INVALID:") {
                return Err((
                    alloc::string::ToString::to_string(path),
                    alloc::string::ToString::to_string(problem.trim()),
                ));
            }
            packages.push(alloc::string::ToString::to_string(trimmed));
        }
    }
    Ok(packages)
}

#[macro_export]
macro_rules! faulty_import_language {
    (
        extensions: $extensions:expr,
        manifest_paths: $manifest_paths:expr,
        workspace_packages: |$manifests:ident| $workspace_packages:expr,
        framework_profiles: |$selection:ident| $framework_profiles:expr $(,)?
    ) => {
        fn extensions() -> ::alloc::vec::Vec<::alloc::string::String> {
            $extensions
        }

        $crate::probe_extension_lower!();

        fn manifest_paths(
            _repository_paths: ::alloc::vec::Vec<::alloc::string::String>,
        ) -> ::alloc::vec::Vec<::alloc::string::String> {
            $manifest_paths
        }

        fn workspace_packages(
            $manifests: ::alloc::vec::Vec<Manifest>,
        ) -> ::core::result::Result<::alloc::vec::Vec<::alloc::string::String>, WorkspaceError> {
            $workspace_packages
        }

        fn resolve_imports(
            _repository_paths: ::alloc::vec::Vec<::alloc::string::String>,
            _manifests: ::alloc::vec::Vec<Manifest>,
            requests: ::alloc::vec::Vec<ImportRequest>,
            _active_profiles: ::alloc::vec::Vec<::alloc::string::String>,
        ) -> ::alloc::vec::Vec<Resolution> {
            requests
                .iter()
                .map(|_| Resolution::Unresolved(::alloc::string::ToString::to_string("probe")))
                .collect()
        }

        fn declared_dependencies(
            _manifests: ::alloc::vec::Vec<Manifest>,
        ) -> ::alloc::vec::Vec<DeclaredDep> {
            ::alloc::vec::Vec::new()
        }

        fn entry_points(
            _repository_paths: ::alloc::vec::Vec<::alloc::string::String>,
            _manifests: ::alloc::vec::Vec<Manifest>,
        ) -> ::alloc::vec::Vec<::alloc::string::String> {
            ::alloc::vec::Vec::new()
        }

        fn reference_ranges() -> ::alloc::vec::Vec<ReferenceRange> {
            ::alloc::vec::Vec::new()
        }

        fn known_framework_profiles() -> ::alloc::vec::Vec<::alloc::string::String> {
            ::alloc::vec![::alloc::string::ToString::to_string("alpha")]
        }

        fn resolve_framework_profiles(
            _package_root: ::alloc::string::String,
            _files: ::alloc::vec::Vec<::alloc::string::String>,
            _manifests: ::alloc::vec::Vec<Manifest>,
            $selection: FrameworkProfileSelection,
        ) -> ::core::result::Result<FrameworkProfileResult, ::alloc::string::String> {
            $framework_profiles;
            ::core::result::Result::Ok(FrameworkProfileResult {
                active_profiles: ::alloc::vec::Vec::new(),
                entry_points: ::alloc::vec::Vec::new(),
                generated_paths: ::alloc::vec::Vec::new(),
                reference_ranges: ::alloc::vec::Vec::new(),
                reference_range_note: ::core::option::Option::None,
            })
        }

        $crate::hazard_free_language_facts!(true);
    };
}

#[macro_export]
macro_rules! profile_resolving_language {
    (
        manifest_paths: $manifest_paths:expr,
        profiles: $profiles:expr,
        resolve: |$manifests:ident, $selection:ident| $resolve:expr $(,)?
    ) => {
        fn extensions() -> ::alloc::vec::Vec<::alloc::string::String> {
            ::alloc::vec![::alloc::string::ToString::to_string("probe")]
        }

        $crate::probe_extension_lower!();

        fn manifest_paths(
            _repository_paths: ::alloc::vec::Vec<::alloc::string::String>,
        ) -> ::alloc::vec::Vec<::alloc::string::String> {
            $manifest_paths
        }

        fn workspace_packages(
            _manifests: ::alloc::vec::Vec<Manifest>,
        ) -> ::core::result::Result<::alloc::vec::Vec<::alloc::string::String>, WorkspaceError> {
            ::core::result::Result::Ok(::alloc::vec::Vec::new())
        }

        fn declared_dependencies(
            _manifests: ::alloc::vec::Vec<Manifest>,
        ) -> ::alloc::vec::Vec<DeclaredDep> {
            ::alloc::vec::Vec::new()
        }

        fn entry_points(
            _repository_paths: ::alloc::vec::Vec<::alloc::string::String>,
            _manifests: ::alloc::vec::Vec<Manifest>,
        ) -> ::alloc::vec::Vec<::alloc::string::String> {
            ::alloc::vec::Vec::new()
        }

        fn reference_ranges() -> ::alloc::vec::Vec<ReferenceRange> {
            ::alloc::vec::Vec::new()
        }

        fn resolve_imports(
            _repository_paths: ::alloc::vec::Vec<::alloc::string::String>,
            _manifests: ::alloc::vec::Vec<Manifest>,
            requests: ::alloc::vec::Vec<ImportRequest>,
            _active_profiles: ::alloc::vec::Vec<::alloc::string::String>,
        ) -> ::alloc::vec::Vec<Resolution> {
            requests
                .iter()
                .map(|_| Resolution::Unresolved(::alloc::string::ToString::to_string("probe")))
                .collect()
        }

        fn known_framework_profiles() -> ::alloc::vec::Vec<::alloc::string::String> {
            $profiles
                .iter()
                .map(|name| ::alloc::string::ToString::to_string(name))
                .collect()
        }

        fn resolve_framework_profiles(
            _package_root: ::alloc::string::String,
            _files: ::alloc::vec::Vec<::alloc::string::String>,
            $manifests: ::alloc::vec::Vec<Manifest>,
            $selection: FrameworkProfileSelection,
        ) -> ::core::result::Result<FrameworkProfileResult, ::alloc::string::String> {
            let rejection: ::core::option::Option<::alloc::string::String> = $resolve;
            if let ::core::option::Option::Some(message) = rejection {
                return ::core::result::Result::Err(message);
            }
            ::core::result::Result::Ok(FrameworkProfileResult {
                active_profiles: ::alloc::vec::Vec::new(),
                entry_points: ::alloc::vec::Vec::new(),
                generated_paths: ::alloc::vec::Vec::new(),
                reference_ranges: ::alloc::vec::Vec::new(),
                reference_range_note: ::core::option::Option::None,
            })
        }

        $crate::hazard_free_language_facts!(true);
    };
}

#[macro_export]
macro_rules! probe_claiming_language_stubs {
    () => {
        fn extensions() -> ::alloc::vec::Vec<::alloc::string::String> {
            ::alloc::vec![::alloc::string::ToString::to_string("probe")]
        }

        $crate::probe_extension_lower!();

        $crate::common_empty_language_metadata_stubs!();

        fn known_framework_profiles() -> ::alloc::vec::Vec<::alloc::string::String> {
            ::alloc::vec![::alloc::string::ToString::to_string("alpha")]
        }

        $crate::hazard_free_language_facts!(true);
    };
}

#[macro_export]
macro_rules! hazard_free_language_facts {
    ($has_imports:expr) => {
        fn has_hazard_rules() -> bool {
            false
        }

        fn hazard_rules() -> ::alloc::vec::Vec<::alloc::string::String> {
            ::alloc::vec::Vec::new()
        }

        fn has_imports() -> bool {
            $has_imports
        }
    };
}

#[macro_export]
macro_rules! probe_extension_lower {
    () => {
        fn lower(
            path: ::alloc::string::String,
            text: ::alloc::string::String,
        ) -> ::core::result::Result<FileModel, LowerError> {
            if path.ends_with(".probe") {
                let lines = $crate::count_non_empty_lines(&text);
                ::core::result::Result::Ok($crate::blank_file_model!(SourceSet::Production, lines))
            } else {
                ::core::result::Result::Err(LowerError::UnsupportedFile(path))
            }
        }
    };
}
