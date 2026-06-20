// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use std::panic::{AssertUnwindSafe, catch_unwind};

mod entry;

pub use entry::*;

#[inline]
fn catch_bool<F: FnOnce() -> bool>(f: F) -> bool {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(false)
}

#[inline]
fn catch_i32<F: FnOnce() -> i32>(f: F) -> i32 {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(-1)
}

#[inline]
fn catch_ptr<T, F: FnOnce() -> *mut T>(f: F) -> *mut T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(std::ptr::null_mut())
}

#[inline]
fn catch_void<F: FnOnce()>(f: F) {
    let _ = catch_unwind(AssertUnwindSafe(f));
}
