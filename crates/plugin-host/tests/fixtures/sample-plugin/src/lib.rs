#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
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

wit_bindgen::generate!({
    world: "language-plugin",
    path: "../../../../../wit",
});

use exports::codeint::plugin::language::{
    DeclaredDep, DepKind, FileModel, FlaggedLine, Function, Guest, ImportRequest, LowerError,
    Manifest, ReferenceRange, Resolution, SourceSet, Token, TokenKind, WorkspaceError,
};

struct SampleMiniPlugin;

export!(SampleMiniPlugin);

fn fnv1a_64(text: &str) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;
    text.as_bytes().iter().fold(OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(PRIME)
    })
}

fn strip_comment(line: &str) -> &str {
    let mut in_string = false;
    for (index, byte) in line.bytes().enumerate() {
        match byte {
            b'"' => in_string = !in_string,
            b'#' if !in_string => return &line[..index],
            _ => {}
        }
    }
    line
}

fn source_set_of(path: &str) -> SourceSet {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    if file_name.ends_with(".test.mini") || path.split('/').any(|segment| segment == "tests") {
        SourceSet::Test
    } else {
        SourceSet::Production
    }
}

fn tokenize_line(code: &str, line_number: u32, is_import: bool, tokens: &mut Vec<Token>) {
    let bytes = code.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if byte == b'"' {
            let start = index;
            index += 1;
            while index < bytes.len() && bytes[index] != b'"' {
                index += 1;
            }
            if index < bytes.len() {
                index += 1;
            }
            let text = &code[start..index];
            tokens.push(Token {
                kind: if is_import {
                    TokenKind::ImportOrExport
                } else {
                    TokenKind::Literal
                },
                text_hash: fnv1a_64(text),
                line: line_number,
            });
            continue;
        }
        if byte.is_ascii_digit() {
            let start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            let text = &code[start..index];
            tokens.push(Token {
                kind: if is_import {
                    TokenKind::ImportOrExport
                } else {
                    TokenKind::Literal
                },
                text_hash: fnv1a_64(text),
                line: line_number,
            });
            continue;
        }
        if byte.is_ascii_alphabetic() || byte == b'_' {
            let start = index;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }
            let text = &code[start..index];
            let kind = if is_import {
                TokenKind::ImportOrExport
            } else if matches!(
                text,
                "fn" | "if" | "else" | "while" | "return" | "let" | "pass"
            ) {
                TokenKind::Other
            } else {
                TokenKind::Identifier
            };
            tokens.push(Token {
                kind,
                text_hash: fnv1a_64(text),
                line: line_number,
            });
            continue;
        }
        let text = &code[index..index + 1];
        tokens.push(Token {
            kind: if is_import {
                TokenKind::ImportOrExport
            } else {
                TokenKind::Other
            },
            text_hash: fnv1a_64(text),
            line: line_number,
        });
        index += 1;
    }
}

fn extract_import_specifier(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix("import ")?.trim();
    let inner = rest.strip_prefix('"')?.strip_suffix('"')?;
    Some(inner.to_string())
}

fn extract_function_name(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix("fn")?.trim_start();
    let end = rest
        .find(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .unwrap_or(rest.len());
    let name = &rest[..end];
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

struct OpenFunction {
    slot: usize,
    name: Option<String>,
    line: u32,
    brace_depth: u32,
    decision_points: u32,
    cognitive_complexity: u32,
    source_lines: u32,
}

fn is_workspace_directive(trimmed: &str) -> bool {
    ["dep:", "dev-dep:", "opt-dep:", "entry:"]
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
}

fn push_function_header(
    trimmed: &str,
    line_number: u32,
    functions: &mut Vec<Function>,
    stack: &mut Vec<OpenFunction>,
) {
    let opens_fn =
        (trimmed.starts_with("fn ") || trimmed.starts_with("fn(")) && trimmed.contains('{');
    if !opens_fn {
        return;
    }
    let slot = functions.len();
    let name = extract_function_name(trimmed);
    functions.push(Function {
        line: line_number,
        end_line: line_number,
        decision_points: 0,
        source_lines: 0,
        name: name.clone(),
        cognitive_complexity: 0,
        coverage: None,
    });
    stack.push(OpenFunction {
        slot,
        name,
        line: line_number,
        brace_depth: 0,
        decision_points: 0,
        cognitive_complexity: 0,
        source_lines: 0,
    });
}

fn update_function_stack(
    code: &str,
    trimmed: &str,
    line_number: u32,
    functions: &mut [Function],
    stack: &mut Vec<OpenFunction>,
) -> Result<(), LowerError> {
    let Some(current) = stack.last_mut() else {
        if code.contains('}') {
            return Err(LowerError::Unparsable(format!(
                "unmatched closing brace at line {line_number}"
            )));
        }
        return Ok(());
    };
    current.source_lines += 1;
    let nesting = current.brace_depth.saturating_sub(1);
    for word in trimmed.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_') {
        if matches!(word, "if" | "while") {
            current.decision_points += 1;
            current.cognitive_complexity += 1 + nesting;
        }
    }
    for byte in code.bytes() {
        if byte == b'{' {
            current.brace_depth += 1;
        } else if byte == b'}' {
            if current.brace_depth == 0 {
                return Err(LowerError::Unparsable(format!(
                    "unmatched closing brace at line {line_number}"
                )));
            }
            current.brace_depth -= 1;
        }
    }
    if current.brace_depth == 0 {
        let finished = stack.pop().unwrap();
        functions[finished.slot] = Function {
            line: finished.line,
            end_line: line_number,
            decision_points: finished.decision_points,
            source_lines: finished.source_lines,
            name: finished.name,
            cognitive_complexity: finished.cognitive_complexity,
            coverage: None,
        };
    }
    Ok(())
}

impl Guest for SampleMiniPlugin {
    fn extensions() -> Vec<String> {
        vec!["mini".to_string()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        let file_name = path.rsplit('/').next().unwrap_or(&path);
        if !file_name.ends_with(".mini") {
            return Err(LowerError::UnsupportedFile(format!(
                "{path} does not end with .mini"
            )));
        }
        if file_name.ends_with(".d.mini") {
            return Err(LowerError::DeclarationOnly);
        }

        let mut source_lines: u32 = 0;
        let mut functions: Vec<Function> = Vec::new();
        let mut stack: Vec<OpenFunction> = Vec::new();
        let mut tokens: Vec<Token> = Vec::new();
        let mut flagged_lines: Vec<FlaggedLine> = Vec::new();
        let mut runtime_imports: Vec<String> = Vec::new();

        for (zero_based, raw_line) in text.lines().enumerate() {
            let line_number = u32::try_from(zero_based + 1).map_err(|_| LowerError::TooLarge)?;
            let code = strip_comment(raw_line);
            let trimmed = code.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.contains("SYNTAX_ERROR") {
                return Err(LowerError::Unparsable(format!(
                    "syntax error at line {line_number}"
                )));
            }
            source_lines = source_lines.checked_add(1).ok_or(LowerError::TooLarge)?;

            let is_import = if let Some(specifier) = extract_import_specifier(trimmed) {
                runtime_imports.push(specifier);
                true
            } else {
                false
            };

            if trimmed == "pass;" {
                flagged_lines.push(FlaggedLine {
                    line: line_number,
                    rule: "redundant-pass".to_string(),
                });
            }

            tokenize_line(code, line_number, is_import, &mut tokens);
            push_function_header(trimmed, line_number, &mut functions, &mut stack);
            update_function_stack(code, trimmed, line_number, &mut functions, &mut stack)?;
        }

        if !stack.is_empty() {
            return Err(LowerError::Unparsable(
                "unclosed function block".to_string(),
            ));
        }

        Ok(FileModel {
            source_set: source_set_of(&path),
            source_lines,
            functions,
            tokens,
            flagged_lines,
            hazard_lines: Vec::new(),
            runtime_imports,
            program_start: false,
        })
    }

    fn manifest_paths(repository_paths: Vec<String>) -> Vec<String> {
        repository_paths
            .into_iter()
            .filter(|path| path == "mini.workspace")
            .collect()
    }

    fn workspace_packages(manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        let mut packages = Vec::new();
        for manifest in manifests {
            if manifest.path != "mini.workspace" {
                continue;
            }
            for line in manifest.text.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || is_workspace_directive(trimmed)
                {
                    continue;
                }
                if let Some(message) = trimmed.strip_prefix("INVALID:") {
                    return Err(WorkspaceError {
                        path: manifest.path,
                        message: message.trim().to_string(),
                    });
                }
                packages.push(trimmed.to_string());
            }
        }
        Ok(packages)
    }

    fn resolve_imports(
        repository_paths: Vec<String>,
        _manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
    ) -> Vec<Resolution> {
        requests
            .into_iter()
            .map(|request| {
                if request.specifier.starts_with("std:") {
                    return Resolution::Builtin;
                }
                if !request.specifier.starts_with("./") {
                    return Resolution::External(request.specifier);
                }
                let relative = request.specifier.trim_start_matches("./");
                let folder = match request.importer.rsplit_once('/') {
                    Some((dir, _)) => dir,
                    None => "",
                };
                let candidate = if folder.is_empty() {
                    relative.to_string()
                } else {
                    format!("{folder}/{relative}")
                };
                let with_ext = if candidate.ends_with(".mini") {
                    candidate.clone()
                } else {
                    format!("{candidate}.mini")
                };
                if repository_paths.iter().any(|path| path == &candidate) {
                    Resolution::Resolved(candidate)
                } else if repository_paths.iter().any(|path| path == &with_ext) {
                    Resolution::Resolved(with_ext)
                } else {
                    Resolution::Unresolved(format!("cannot find {with_ext}"))
                }
            })
            .collect()
    }

    fn declared_dependencies(manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        let mut deps = Vec::new();
        for manifest in manifests {
            if manifest.path != "mini.workspace" {
                continue;
            }
            for line in manifest.text.lines() {
                let trimmed = line.trim();
                let entry = if let Some(name) = trimmed.strip_prefix("dep:") {
                    Some((name.trim(), DepKind::Runtime))
                } else if let Some(name) = trimmed.strip_prefix("dev-dep:") {
                    Some((name.trim(), DepKind::Dev))
                } else {
                    trimmed
                        .strip_prefix("opt-dep:")
                        .map(|name| (name.trim(), DepKind::Optional))
                };
                if let Some((name, kind)) = entry
                    && !name.is_empty()
                {
                    deps.push(DeclaredDep {
                        package_root: String::new(),
                        name: name.to_string(),
                        kind,
                    });
                }
            }
        }
        deps
    }

    fn entry_points(repository_paths: Vec<String>, manifests: Vec<Manifest>) -> Vec<String> {
        let mut entries = Vec::new();
        for manifest in manifests {
            if manifest.path != "mini.workspace" {
                continue;
            }
            for line in manifest.text.lines() {
                if let Some(entry) = line.trim().strip_prefix("entry:") {
                    let candidate = entry.trim();
                    if repository_paths.iter().any(|path| path == candidate) {
                        entries.push(candidate.to_string());
                    }
                }
            }
        }
        entries
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        vec![ReferenceRange {
            metric: "erosion".to_string(),
            low: 0.0,
            high: 0.5,
            source: "sample mini language fixture".to_string(),
        }]
    }

    fn has_hazard_rules() -> bool {
        false
    }

    fn hazard_rules() -> Vec<String> {
        Vec::new()
    }
}
