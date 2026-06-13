// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::io::{DataInput, DataOutput, Result};
use crate::codec::object::Object;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    String(String),
    Bool(bool),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    F32(f32),
    F64(f64),
    Object(Object),
}

pub struct BMEncoding;

impl BMEncoding {
    pub fn decode(input: &mut dyn DataInput) -> Result<Value> {
        let tag = input.read_utf()?;
        Ok(match tag.as_str() {
            "@" => Value::Object(Object::decode(input)?),
            "i" => Value::I32(input.read_int()?),
            "I" => Value::U32(input.read_unsigned_int()?),
            "s" => Value::I16(input.read_short()?),
            "S" => Value::U16(input.read_short()? as u16),
            "f" => Value::F32(input.read_float()?),
            "d" => Value::F64(input.read_double()?),
            "B" => Value::Bool(input.read_boolean()?),
            "*" => Value::String(input.read_utf()?),
            t => return Err(format!("unknown tag: {t}").into()),
        })
    }

    pub fn encode(value: &Value, out: &mut dyn DataOutput) -> Result<()> {
        match value {
            Value::Object(o) => {
                out.write_utf("@")?;
                o.encode_with_marker(out)
            }
            Value::I32(v) => {
                out.write_utf("i")?;
                out.write_int(*v)
            }
            Value::U32(v) => {
                out.write_utf("I")?;
                out.write_unsigned_int(*v)
            }
            Value::I16(v) => {
                out.write_utf("s")?;
                out.write_short(*v)
            }
            Value::U16(v) => {
                out.write_utf("S")?;
                out.write_short(*v as i16)
            }
            Value::F32(v) => {
                out.write_utf("f")?;
                out.write_float(*v)
            }
            Value::F64(v) => {
                out.write_utf("d")?;
                out.write_double(*v)
            }
            Value::Bool(b) => {
                out.write_utf("B")?;
                out.write_boolean(*b)
            }
            Value::String(s) => {
                out.write_utf("*")?;
                out.write_utf(s)
            }
        }
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueTagC {
    String = 0,
    Bool = 1,
    I16 = 2,
    U16 = 3,
    I32 = 4,
    U32 = 5,
    F32 = 6,
    F64 = 7,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ValueC {
    pub tag: ValueTagC,
    pub string_ptr: *const std::os::raw::c_char,
    pub bool_value: bool,
    pub int_value: i64,
    pub float_value: f64,
}

impl ValueC {
    pub fn to_rust(&self) -> Option<Value> {
        Some(match self.tag {
            ValueTagC::String => {
                if self.string_ptr.is_null() {
                    return None;
                }
                let c_str = unsafe { std::ffi::CStr::from_ptr(self.string_ptr) };
                Value::String(c_str.to_str().ok()?.to_owned())
            }
            ValueTagC::Bool => Value::Bool(self.bool_value),
            ValueTagC::I16 => Value::I16(i16::try_from(self.int_value).ok()?),
            ValueTagC::U16 => Value::U16(u16::try_from(self.int_value).ok()?),
            ValueTagC::I32 => Value::I32(i32::try_from(self.int_value).ok()?),
            ValueTagC::U32 => Value::U32(u32::try_from(self.int_value).ok()?),
            ValueTagC::F32 => Value::F32(self.float_value as f32),
            ValueTagC::F64 => Value::F64(self.float_value),
        })
    }
}

pub(crate) fn values_from_c(ptr: *const ValueC, len: usize) -> Option<Vec<Value>> {
    if len == 0 {
        return Some(Vec::new());
    }
    if ptr.is_null() {
        return None;
    }
    let items = unsafe { std::slice::from_raw_parts(ptr, len) };
    let mut out = Vec::with_capacity(len);
    for item in items {
        out.push(item.to_rust()?);
    }
    Some(out)
}
