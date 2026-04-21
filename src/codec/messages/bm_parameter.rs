// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::io::{DataInput, DataOutput, Result};
use crate::codec::object::Object;
use crate::codec::messages::bm_encoding::{BMEncoding, Value};
use byteorder::{LittleEndian, WriteBytesExt};
use std::io::Cursor;

const TAG_OBJECT: u32 = 1;
const TAG_I32: u32 = 2;
const TAG_U32: u32 = 3;
const TAG_I16: u32 = 4;
const TAG_U16: u32 = 5;
const TAG_F32: u32 = 6;
const TAG_F64: u32 = 7;
const TAG_BOOL: u32 = 8;
const TAG_STRING: u32 = 9;

#[derive(Debug, Clone, Default)]
pub struct BMParameter {
    pub tag: u32,
    pub i16_v: i16,
    pub u16_v: u16,
    pub i32_v: i32,
    pub u32_v: u32,
    pub f32_v: f32,
    pub f64_v: f64,
    pub bool_v: bool,
    pub str_v: String,
    pub obj_v: Vec<u8>,
}

impl BMParameter {
    pub fn to_value(&self) -> Result<Value> {
        match self.tag {
            TAG_BOOL => Ok(Value::Bool(self.bool_v)),
            TAG_I16 => Ok(Value::I16(self.i16_v)),
            TAG_U16 => Ok(Value::U16(self.u16_v)),
            TAG_I32 => Ok(Value::I32(self.i32_v)),
            TAG_U32 => Ok(Value::U32(self.u32_v)),
            TAG_F32 => Ok(Value::F32(self.f32_v)),
            TAG_F64 => Ok(Value::F64(self.f64_v)),
            TAG_STRING => Ok(Value::String(self.str_v.clone())),
            TAG_OBJECT => {
                if self.obj_v.is_empty() {
                    return Err("object payload missing".into());
                }
                let mut cur = Cursor::new(&self.obj_v);
                let obj = Object::decode(&mut cur)?;
                Ok(Value::Object(obj))
            }
            _ => Err("unknown BMParameter tag".into()),
        }
    }

    pub fn from_value(v: Value) -> Result<Self> {
        let mut out = BMParameter::default();
        match v {
            Value::Bool(b) => {
                out.tag = TAG_BOOL;
                out.bool_v = b;
            }
            Value::I16(x) => {
                out.tag = TAG_I16;
                out.i16_v = x;
            }
            Value::U16(x) => {
                out.tag = TAG_U16;
                out.u16_v = x;
            }
            Value::I32(x) => {
                out.tag = TAG_I32;
                out.i32_v = x;
            }
            Value::U32(x) => {
                out.tag = TAG_U32;
                out.u32_v = x;
            }
            Value::F32(x) => {
                out.tag = TAG_F32;
                out.f32_v = x;
            }
            Value::F64(x) => {
                out.tag = TAG_F64;
                out.f64_v = x;
            }
            Value::String(s) => {
                out.tag = TAG_STRING;
                out.str_v = s;
            }
            Value::Object(o) => {
                let mut w = VecOutput::default();
                o.encode(&mut w)?;
                out.obj_v = w.buf;
                out.tag = TAG_OBJECT;
            }
        }
        Ok(out)
    }

    pub fn read_external(input: &mut dyn DataInput) -> Result<Self> {
        let val = BMEncoding::decode(input)?;
        BMParameter::from_value(val)
    }

    pub fn write_external(&self, out: &mut dyn DataOutput) -> Result<()> {
        let v = self.to_value()?;
        BMEncoding::encode(&v, out)
    }
}

#[derive(Default)]
pub(crate) struct VecOutput {
    pub buf: Vec<u8>,
}
impl DataOutput for VecOutput {
    fn write_boolean(&mut self, b: bool) -> Result<()> {
        self.buf.push(if b { 1 } else { 0 });
        Ok(())
    }
    fn write_bytes(&mut self, data: &[u8]) -> Result<()> {
        self.buf.extend_from_slice(data);
        Ok(())
    }
    fn write_short(&mut self, v: i16) -> Result<()> {
        self.buf.write_i16::<LittleEndian>(v)?;
        Ok(())
    }
    fn write_int(&mut self, v: i32) -> Result<()> {
        self.buf.write_i32::<LittleEndian>(v)?;
        Ok(())
    }
    fn write_unsigned_int(&mut self, v: u32) -> Result<()> {
        self.buf.write_u32::<LittleEndian>(v)?;
        Ok(())
    }
    fn write_float(&mut self, v: f32) -> Result<()> {
        self.buf.write_f32::<LittleEndian>(v)?;
        Ok(())
    }
    fn write_double(&mut self, v: f64) -> Result<()> {
        self.buf.write_f64::<LittleEndian>(v)?;
        Ok(())
    }
    fn write_utf(&mut self, s: &str) -> Result<()> {
        if s.len() > i16::MAX as usize {
            return Err("utf too long".into());
        }
        self.buf.write_i16::<LittleEndian>(s.len() as i16)?;
        self.buf.extend_from_slice(s.as_bytes());
        Ok(())
    }
}
