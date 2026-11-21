#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
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
    world: "metric-plugin",
    path: "../../../../../wit",
});

use exports::codeint::plugin::metric::{CodeModel, Finding, Guest, Measurement};

const MAX_FUNCTION_SOURCE_LINES: u32 = 10;

struct SampleLongFunctionsPlugin;

export!(SampleLongFunctionsPlugin);

fn fnv1a_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    const PRIME: u64 = 0x0100_0000_01b3;
    for &byte in bytes {
        hash = (hash ^ u64::from(byte)).wrapping_mul(PRIME);
    }
    hash
}

fn fnv1a_u64(hash: u64, value: u64) -> u64 {
    fnv1a_bytes(hash, &value.to_le_bytes())
}

impl Guest for SampleLongFunctionsPlugin {
    fn key() -> String {
        "long_functions".to_string()
    }

    fn name() -> String {
        "Long functions".to_string()
    }

    fn description() -> String {
        "Share of functions with more than 10 source lines".to_string()
    }

    fn measure(model: CodeModel) -> Measurement {
        let mut total_functions: u64 = 0;
        let mut long_functions: u64 = 0;
        let mut occurrences: BTreeMap<u64, u64> = BTreeMap::new();
        let mut scored_findings: Vec<(u32, Finding)> = Vec::new();

        for file in &model.files {
            for function in &file.model.functions {
                total_functions += 1;
                if function.source_lines <= MAX_FUNCTION_SOURCE_LINES {
                    continue;
                }
                long_functions += 1;
                let mut hash = 0xcbf2_9ce4_8422_2325u64;
                hash = fnv1a_bytes(hash, b"long_functions");
                hash = fnv1a_bytes(hash, file.path.as_bytes());
                for token in &file.model.tokens {
                    if token.line >= function.line && token.line <= function.end_line {
                        hash = fnv1a_u64(hash, token.text_hash);
                    }
                }
                let count = occurrences.entry(hash).or_insert(0);
                let id_hash = fnv1a_u64(hash, *count);
                *count += 1;
                scored_findings.push((
                    function.source_lines,
                    Finding {
                        id: format!("{id_hash:016x}"),
                        path: file.path.clone(),
                        line: function.line,
                        end_line: Some(function.end_line),
                        message: format!(
                            "function has {} source lines, over {MAX_FUNCTION_SOURCE_LINES}",
                            function.source_lines
                        ),
                    },
                ));
            }
        }

        scored_findings.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.path.cmp(&right.1.path))
                .then_with(|| left.1.line.cmp(&right.1.line))
        });

        let ratio = if total_functions == 0 {
            0.0
        } else {
            long_functions as f64 / total_functions as f64
        };

        Measurement {
            ratio,
            numerator: long_functions as f64,
            denominator: total_functions as f64,
            findings: scored_findings
                .into_iter()
                .map(|(_, finding)| finding)
                .collect(),
        }
    }
}
