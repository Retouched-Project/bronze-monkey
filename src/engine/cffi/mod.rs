// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use std::os::raw::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};

mod entry;
mod entry_make;
mod marshal_in;
mod marshal_out;
pub mod types;

pub use entry::*;
pub use entry_make::*;
pub use types::*;

#[inline]
fn catch_bool<F: FnOnce() -> bool>(f: F) -> bool {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(false)
}

#[inline]
fn catch_ptr<T, F: FnOnce() -> *mut T>(f: F) -> *mut T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(std::ptr::null_mut())
}

#[inline]
fn catch_void<F: FnOnce()>(f: F) {
    let _ = catch_unwind(AssertUnwindSafe(f));
}

fn req_str(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let c_str = unsafe { std::ffi::CStr::from_ptr(ptr) };
    c_str.to_str().ok().map(str::to_owned)
}

fn opt_str(ptr: *const c_char) -> Option<Option<String>> {
    if ptr.is_null() {
        return Some(None);
    }
    let c_str = unsafe { std::ffi::CStr::from_ptr(ptr) };
    c_str.to_str().ok().map(|s| Some(s.to_owned()))
}

#[inline]
fn set_string_field<F>(s: String, setter: F)
where
    F: FnOnce(*const c_char) -> bool,
{
    let c = std::ffi::CString::new(s).unwrap();
    setter(c.as_ptr());
}
