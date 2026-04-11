// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

#[macro_export]
macro_rules! ffi_pod {
    (
        $cname:ident,
        $field_enum:ident,
        make = $make_fn:ident,
        get  = $get_fn:ident,
        { $( $field:ident : $typ:ty ),+ $(,)? }
    ) => {
        #[repr(C)]
        #[derive(Clone, Copy, Debug)]
        #[allow(dead_code)]
        #[allow(non_camel_case_types)]
        pub struct $cname {
            $( pub $field: $typ, )+
        }

        #[repr(u32)]
        #[derive(Clone, Copy, Debug)]
        #[allow(dead_code)]
        #[allow(non_camel_case_types)]
        pub enum $field_enum {
            $( $field, )+
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn $make_fn(out: *mut $cname, $( $field: $typ ),+ ) -> bool {
            if out.is_null() { return false; }
            let v = $cname { $( $field: $field ),+ };
            unsafe { *out = v; }
            true
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn $get_fn(out: *const $cname, field: $field_enum, val_out: *mut u64) -> bool {
            if out.is_null() || val_out.is_null() { return false; }
            let s = unsafe { &*out };
            let v = match field {
                $( $field_enum::$field => s.$field as u64, )+
            };
            unsafe { *val_out = v; }
            true
        }
    };
}

#[macro_export]
macro_rules! ffi_pod_getter {
    ($name:ident, $struct_name:ty, $field:ident, $typ:ty) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name(p: *const $struct_name) -> $typ {
            if p.is_null() {
                return <$typ>::default();
            }
            unsafe { (*p).$field }
        }
    };
}

#[macro_export]
macro_rules! ffi_cstring_accessors {
    (
        $struct:ty, $ptr_field:ident, $len_field:ident,
        set_inner = $set_inner:ident,
        set = $set:ident,
        get_len = $get_len:ident,
        get = $get:ident,
        free_field = $free_field:ident
    ) => {
        #[inline]
        pub fn $set_inner(s: &mut $struct, in_ptr: *const ::std::os::raw::c_char) -> bool {
            if in_ptr.is_null() {
                return false;
            }
            let c = unsafe { ::std::ffi::CStr::from_ptr(in_ptr) };
            let owned = match ::std::ffi::CString::new(c.to_bytes()) {
                Ok(v) => v,
                Err(_) => return false,
            };
            if !s.$ptr_field.is_null() {
                unsafe {
                    let _ = ::std::ffi::CString::from_raw(s.$ptr_field);
                }
            }
            s.$len_field = owned.as_bytes().len();
            s.$ptr_field = owned.into_raw();
            true
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn $set(s: *mut $struct, in_ptr: *const ::std::os::raw::c_char) -> bool {
            if s.is_null() {
                return false;
            }
            let s = unsafe { &mut *s };
            $set_inner(s, in_ptr)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn $get_len(s: *const $struct) -> usize {
            if s.is_null() {
                return 0;
            }
            let s = unsafe { &*s };
            s.$len_field
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn $get(
            s: *const $struct,
            out: *mut ::std::os::raw::c_char,
            out_len: usize,
        ) -> usize {
            if s.is_null() || out.is_null() || out_len == 0 {
                return 0;
            }
            let s = unsafe { &*s };
            if s.$ptr_field.is_null() {
                return 0;
            }
            let bytes = unsafe { ::std::ffi::CStr::from_ptr(s.$ptr_field).to_bytes() };
            let n = bytes.len().min(out_len.saturating_sub(1));
            unsafe {
                ::std::ptr::copy_nonoverlapping(bytes.as_ptr(), out as *mut u8, n);
                *out.add(n) = 0;
            }
            n
        }

        #[inline]
        pub fn $free_field(s: &mut $struct) {
            if !s.$ptr_field.is_null() {
                unsafe {
                    let _ = ::std::ffi::CString::from_raw(s.$ptr_field);
                }
                s.$ptr_field = ::core::ptr::null_mut();
                s.$len_field = 0;
            }
        }
    };
}

#[macro_export]
macro_rules! ffi_vec_u8_accessors {
    (
        $struct:ty, $ptr_field:ident, $len_field:ident, $cap_field:ident,
        set_inner = $set_inner:ident,
        set = $set:ident,
        get_len = $get_len:ident,
        get = $get:ident,
        free_field = $free_field:ident
    ) => {
        #[inline]
        pub fn $set_inner(s: &mut $struct, in_ptr: *const u8, in_len: usize) -> bool {
            if !s.$ptr_field.is_null() {
                unsafe {
                    let _ =
                        ::std::vec::Vec::from_raw_parts(s.$ptr_field, s.$len_field, s.$cap_field);
                }
                s.$ptr_field = ::core::ptr::null_mut();
                s.$len_field = 0;
                s.$cap_field = 0;
            }
            if in_ptr.is_null() || in_len == 0 {
                return true;
            }
            let src = unsafe { ::std::slice::from_raw_parts(in_ptr, in_len) };
            let mut v = ::std::vec::Vec::with_capacity(in_len);
            v.extend_from_slice(src);
            s.$len_field = v.len();
            s.$cap_field = v.capacity();
            s.$ptr_field = v.as_mut_ptr();
            ::std::mem::forget(v);
            true
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn $set(s: *mut $struct, in_ptr: *const u8, in_len: usize) -> bool {
            if s.is_null() {
                return false;
            }
            let s = unsafe { &mut *s };
            $set_inner(s, in_ptr, in_len)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn $get_len(s: *const $struct) -> usize {
            if s.is_null() {
                return 0;
            }
            let s = unsafe { &*s };
            s.$len_field
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn $get(s: *const $struct, out: *mut u8, out_len: usize) -> usize {
            if s.is_null() || out.is_null() || out_len == 0 {
                return 0;
            }
            let s = unsafe { &*s };
            if s.$ptr_field.is_null() || s.$len_field == 0 {
                return 0;
            }
            let n = s.$len_field.min(out_len);
            unsafe {
                ::std::ptr::copy_nonoverlapping(s.$ptr_field, out, n);
            }
            n
        }

        #[inline]
        pub fn $free_field(s: &mut $struct) {
            if !s.$ptr_field.is_null() {
                unsafe {
                    let _ =
                        ::std::vec::Vec::from_raw_parts(s.$ptr_field, s.$len_field, s.$cap_field);
                }
                s.$ptr_field = ::core::ptr::null_mut();
                s.$len_field = 0;
                s.$cap_field = 0;
            }
        }
    };
}

#[macro_export]
macro_rules! ffi_free_struct {
    ($struct:ty, $free_fn:ident, $( $free_field_fn:ident ),+ $(,)? ) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $free_fn(p: *mut $struct) {
            if p.is_null() { return; }
            let s = unsafe { &mut *p };
            $( $free_field_fn(s); )+
        }
    };
}
