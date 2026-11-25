#[cfg(all(
    target_arch = "wasm32",
    target_os = "wasi",
    target_env = "p2",
    feature = "std"
))]
compile_error!(
    "codeint-plugin-sdk does not support the `std` feature on `wasm32-wasip2` because WASI imports are rejected by codeint's zero-capability sandbox; build `wasm32-wasip2` without `std`, or target `wasm32-unknown-unknown` with `features = [\"std\"]` and `wasm-tools component new`"
);

#[doc(hidden)]
pub mod __bindings {
    pub mod language {
        wit_bindgen::generate!({
            world: "language-plugin",
            path: "../../wit",
            pub_export_macro: true,
            export_macro_name: "__export_language_world",
            default_bindings_module: "codeint_plugin_sdk::__bindings::language",
            runtime_path: "codeint_plugin_sdk::__wit_bindgen::rt",
            type_section_suffix: "-language",
            additional_derives: [PartialEq],
        });
    }

    pub mod metric {
        wit_bindgen::generate!({
            world: "metric-plugin",
            path: "../../wit",
            pub_export_macro: true,
            export_macro_name: "__export_metric_world",
            default_bindings_module: "codeint_plugin_sdk::__bindings::metric",
            runtime_path: "codeint_plugin_sdk::__wit_bindgen::rt",
            type_section_suffix: "-metric",
            additional_derives: [PartialEq],
        });
    }
}

#[cfg(all(
    target_arch = "wasm32",
    not(feature = "std"),
    feature = "default-allocator"
))]
#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

#[cfg(all(
    target_arch = "wasm32",
    not(feature = "std"),
    feature = "panic-handler"
))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    core::arch::wasm32::unreachable()
}

#[cfg(all(
    target_arch = "wasm32",
    target_os = "wasi",
    target_env = "p2",
    not(feature = "std")
))]
mod wasip2_shims {
    use core::alloc::Layout;
    use core::arch::wasm32;
    use core::ffi::{c_int, c_void};

    #[unsafe(export_name = "cabi_realloc")]
    unsafe extern "C" fn cabi_realloc(
        old_ptr: *mut u8,
        old_size: usize,
        align: usize,
        new_size: usize,
    ) -> *mut u8 {
        if new_size == 0 {
            if old_size != 0
                && let Ok(old_layout) = Layout::from_size_align(old_size, align)
            {
                unsafe { alloc::alloc::dealloc(old_ptr, old_layout) };
            }
            return align as *mut u8;
        }
        let Ok(layout) = Layout::from_size_align(new_size, align) else {
            wasm32::unreachable()
        };
        let ptr = if old_size == 0 {
            unsafe { alloc::alloc::alloc(layout) }
        } else {
            let Ok(old_layout) = Layout::from_size_align(old_size, align) else {
                wasm32::unreachable()
            };
            unsafe { alloc::alloc::realloc(old_ptr, old_layout, new_size) }
        };
        if ptr.is_null() {
            wasm32::unreachable()
        }
        ptr
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn memcmp(left: *const c_void, right: *const c_void, count: usize) -> c_int {
        let left_bytes = left.cast::<u8>();
        let right_bytes = right.cast::<u8>();
        let mut index = 0;
        while index < count {
            let a = unsafe { *left_bytes.add(index) };
            let b = unsafe { *right_bytes.add(index) };
            if a != b {
                return c_int::from(a) - c_int::from(b);
            }
            index += 1;
        }
        0
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn memcpy(
        dest: *mut c_void,
        src: *const c_void,
        count: usize,
    ) -> *mut c_void {
        let dest_bytes = dest.cast::<u8>();
        let src_bytes = src.cast::<u8>();
        for index in 0..count {
            unsafe { *dest_bytes.add(index) = *src_bytes.add(index) };
        }
        dest
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn memmove(
        dest: *mut c_void,
        src: *const c_void,
        count: usize,
    ) -> *mut c_void {
        let dest_bytes = dest.cast::<u8>();
        let src_bytes = src.cast::<u8>();
        if (dest_bytes as usize) < (src_bytes as usize) {
            for index in 0..count {
                unsafe { *dest_bytes.add(index) = *src_bytes.add(index) };
            }
        } else {
            for index in (0..count).rev() {
                unsafe { *dest_bytes.add(index) = *src_bytes.add(index) };
            }
        }
        dest
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn memset(dest: *mut c_void, value: c_int, count: usize) -> *mut c_void {
        let dest_bytes = dest.cast::<u8>();
        let byte = value as u8;
        for index in 0..count {
            unsafe { *dest_bytes.add(index) = byte };
        }
        dest
    }

    pub(super) fn anchor_symbols() {
        #[used]
        static LINK_CABI_REALLOC: unsafe extern "C" fn(*mut u8, usize, usize, usize) -> *mut u8 =
            cabi_realloc;
        #[used]
        static LINK_MEMCMP: unsafe extern "C" fn(*const c_void, *const c_void, usize) -> c_int =
            memcmp;
        #[used]
        static LINK_MEMCPY: unsafe extern "C" fn(*mut c_void, *const c_void, usize) -> *mut c_void =
            memcpy;
        #[used]
        static LINK_MEMMOVE: unsafe extern "C" fn(
            *mut c_void,
            *const c_void,
            usize,
        ) -> *mut c_void = memmove;
        #[used]
        static LINK_MEMSET: unsafe extern "C" fn(*mut c_void, c_int, usize) -> *mut c_void = memset;
    }
}

pub fn link_runtime_symbols() {
    #[cfg(all(
        target_arch = "wasm32",
        target_os = "wasi",
        target_env = "p2",
        not(feature = "std")
    ))]
    wasip2_shims::anchor_symbols();
}
