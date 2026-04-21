// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::registry;
use crate::codec::io::{DataInput, DataOutput, Result};
use crate::codec::object::Object;
use crate::codec::messages::bm_encoding::Value;
use crate::codec::messages::bm_parameter::VecOutput;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Read};
use std::os::raw::c_char;
use std::ptr;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct BMInvoke {
    pub id: i32,
    pub method: String,
    pub return_method: Option<String>,
    pub params: Vec<Value>,
}

impl BMInvoke {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_INVOKE;

    pub fn read_from(input: &mut dyn DataInput) -> Result<Self> {
        let id = input.read_int()?;
        let method = input.read_utf()?;
        let mut ret = input.read_utf()?;
        if ret.is_empty() {
            ret = String::new();
        }
        let count = input.read_int()?;
        if count < 0 {
            return Err("negative param count".into());
        }
        let initial_cap = (count as usize).min(1024);
        let mut params = Vec::with_capacity(initial_cap);
        for _ in 0..count {
            let obj = Object::decode(input)?;
            match obj {
                Object::BMParameter(v) => params.push(*v),
                other => params.push(Value::Object(other)),
            }
        }
        Ok(Self {
            id,
            method,
            return_method: if ret.is_empty() { None } else { Some(ret) },
            params,
        })
    }

    pub fn write_to(&self, out: &mut dyn DataOutput) -> Result<()> {
        out.write_int(self.id)?;
        out.write_utf(&self.method)?;
        out.write_utf(self.return_method.as_deref().unwrap_or(""))?;
        out.write_int(self.params.len() as i32)?;
        for p in &self.params {
            let obj = Object::BMParameter(Box::new(p.clone()));
            obj.encode_with_marker(out)?;
        }
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        buf.write_i32::<LittleEndian>(self.id)?;
        write_utf_bytes(&mut buf, &self.method)?;
        write_utf_bytes(&mut buf, self.return_method.as_deref().unwrap_or(""))?;
        buf.write_i32::<LittleEndian>(self.params.len() as i32)?;
        for p in &self.params {
            let mut tmp = VecOutput::default();
            let obj = Object::BMParameter(Box::new(p.clone()));
            obj.encode_with_marker(&mut tmp)?;
            buf.extend_from_slice(&tmp.buf);
        }
        Ok(buf)
    }

    pub fn to_object_bytes(&self) -> Result<Vec<u8>> {
        let mut tmp = VecOutput::default();
        let obj = Object::BMInvoke(self.clone());
        obj.encode_with_marker(&mut tmp)?;
        Ok(tmp.buf)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut cur = Cursor::new(bytes);
        let id = cur.read_i32::<LittleEndian>()?;
        let method = read_utf_bytes(&mut cur)?;
        let ret = read_utf_bytes(&mut cur)?;
        let count = cur.read_i32::<LittleEndian>()?;
        if count < 0 {
            return Err("negative param count".into());
        }
        let initial_cap = (count as usize).min(1024);
        let mut params = Vec::with_capacity(initial_cap);
        for _ in 0..count {
            let obj = Object::decode(&mut cur)?;
            match obj {
                Object::BMParameter(v) => params.push(*v),
                other => params.push(Value::Object(other)),
            }
        }
        Ok(Self {
            id,
            method,
            return_method: if ret.is_empty() { None } else { Some(ret) },
            params,
        })
    }
}

fn write_utf_bytes(buf: &mut Vec<u8>, s: &str) -> Result<()> {
    if s.len() > i16::MAX as usize {
        return Err("utf too long".into());
    }
    buf.write_i16::<LittleEndian>(s.len() as i16)?;
    buf.extend_from_slice(s.as_bytes());
    Ok(())
}

fn read_utf_bytes(cur: &mut Cursor<&[u8]>) -> Result<String> {
    let len = cur.read_i16::<LittleEndian>()? as usize;
    let mut v = vec![0u8; len];
    cur.read_exact(&mut v)?;
    Ok(String::from_utf8(v)?)
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct BMInvokeC {
    pub id: i32,
    pub method_ptr: *mut c_char,
    pub method_len: usize,
    pub return_method_ptr: *mut c_char,
    pub return_method_len: usize,
    pub params_json_ptr: *mut c_char,
    pub params_json_len: usize,
}

crate::ffi_cstring_accessors!(
    BMInvokeC,
    method_ptr,
    method_len,
    set_inner = bm_invoke_set_method_inner,
    set = bm_invoke_set_method,
    get_len = bm_invoke_get_method_len,
    get = bm_invoke_get_method,
    free_field = bm_invoke_free_method
);

crate::ffi_cstring_accessors!(
    BMInvokeC,
    return_method_ptr,
    return_method_len,
    set_inner = bm_invoke_set_return_method_inner,
    set = bm_invoke_set_return_method,
    get_len = bm_invoke_get_return_method_len,
    get = bm_invoke_get_return_method,
    free_field = bm_invoke_free_return_method
);

crate::ffi_cstring_accessors!(
    BMInvokeC,
    params_json_ptr,
    params_json_len,
    set_inner = bm_invoke_set_params_json_inner,
    set = bm_invoke_set_params_json,
    get_len = bm_invoke_get_params_json_len,
    get = bm_invoke_get_params_json,
    free_field = bm_invoke_free_params_json
);

crate::ffi_free_struct!(
    BMInvokeC,
    bm_invoke_free,
    bm_invoke_free_method,
    bm_invoke_free_return_method,
    bm_invoke_free_params_json
);

#[unsafe(no_mangle)]
pub extern "C" fn bm_invoke_new() -> *mut BMInvokeC {
    Box::into_raw(Box::new(BMInvokeC {
        id: 0,
        method_ptr: ptr::null_mut(),
        method_len: 0,
        return_method_ptr: ptr::null_mut(),
        return_method_len: 0,
        params_json_ptr: ptr::null_mut(),
        params_json_len: 0,
    }))
}

impl BMInvokeC {
    pub fn to_rust(&self) -> Option<BMInvoke> {
        let method = if self.method_len == 0 {
            String::new()
        } else {
            let bytes = unsafe {
                std::slice::from_raw_parts(self.method_ptr as *const u8, self.method_len)
            };
            String::from_utf8(bytes.to_vec()).ok()?
        };
        let return_method = if self.return_method_len == 0 {
            None
        } else {
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    self.return_method_ptr as *const u8,
                    self.return_method_len,
                )
            };
            Some(String::from_utf8(bytes.to_vec()).ok()?)
        };
        let params = Vec::new();
        Some(BMInvoke {
            id: self.id,
            method,
            return_method,
            params,
        })
    }
}
