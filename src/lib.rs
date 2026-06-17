// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

pub mod codec;
pub mod controls;
pub mod devices;
pub mod engine;
mod ffi_macros;
pub mod policy;
pub mod policy_file;
pub mod types;

#[deprecated(
    since = "2.0.0",
    note = "use `bronze_monkey::codec::externals` instead"
)]
pub use codec::externals;
#[deprecated(
    since = "2.0.0",
    note = "use `bronze_monkey::codec::messages` instead"
)]
pub use codec::messages;

#[deprecated(
    since = "2.0.0",
    note = "use `bronze_monkey::codec::io` / `bronze_monkey::codec::object` instead"
)]
pub mod io {
    #[deprecated(since = "2.0.0", note = "use `bronze_monkey::codec::io` instead")]
    pub use crate::codec::io;
    #[deprecated(since = "2.0.0", note = "use `bronze_monkey::codec::object` instead")]
    pub use crate::codec::object;
}

#[cfg(feature = "pyo3")]
pub use engine::python::*;
pub use codec::externals::bm_stream;

use base64::prelude::*;
use std::cell::RefCell;
use std::ffi::CString;
#[cfg(not(target_arch = "wasm32"))]
use std::os::raw::c_char;
#[cfg(not(target_arch = "wasm32"))]
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = RefCell::new(None);
}

static LIB_LOGGED: AtomicBool = AtomicBool::new(false);
static PENDING_LOG: Mutex<Option<String>> = Mutex::new(None);

pub fn set_last_error(e: impl std::fmt::Display) {
    LAST_ERROR.with(|prev| {
        *prev.borrow_mut() = Some(CString::new(e.to_string()).unwrap());
    });
}

pub fn log_library_loaded(context: &str) {
    if LIB_LOGGED.load(Ordering::SeqCst) {
        return;
    }
    let msg = format!("bronze-monkey library loaded ({})", context);
    if try_log(&msg) {
        LIB_LOGGED.store(true, Ordering::SeqCst);
    } else {
        let mut pending = PENDING_LOG.lock().unwrap();
        *pending = Some(msg);
    }
}

pub fn flush_pending_log() {
    if LIB_LOGGED.load(Ordering::SeqCst) {
        return;
    }
    let pending = { PENDING_LOG.lock().unwrap().take() };
    if let Some(msg) = pending {
        if try_log(&msg) {
            LIB_LOGGED.store(true, Ordering::SeqCst);
        } else {
            let mut pending = PENDING_LOG.lock().unwrap();
            *pending = Some(msg);
        }
    }
}

fn try_log(msg: &str) -> bool {
    #[cfg(feature = "pyo3")]
    {
        if crate::engine::python::try_python_log(msg) {
            return true;
        }
    }
    eprintln!("{}", msg);
    true
}

pub fn base64_decode(input: &str) -> Result<Vec<u8>, base64::DecodeError> {
    BASE64_STANDARD.decode(input)
}

#[cfg(not(target_arch = "wasm32"))]
#[unsafe(no_mangle)]
pub extern "C" fn bm_library_init() -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        log_library_loaded("ffi");
        true
    }))
    .unwrap_or(false)
}

#[cfg(not(target_arch = "wasm32"))]
#[unsafe(no_mangle)]
pub extern "C" fn bm_get_last_error(buf: *mut c_char, len: usize) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        if buf.is_null() || len == 0 {
            return -1;
        }

        let last_error = LAST_ERROR.with(|prev| prev.borrow_mut().take());
        if let Some(e) = last_error {
            let bytes = e.as_bytes_with_nul();
            let n = bytes.len().min(len);
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, n);
            }
            if n < len {
                unsafe {
                    *buf.add(n) = 0;
                }
            }
            return n as i32;
        }
        0
    }))
    .unwrap_or(-1)
}
