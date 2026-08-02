// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::Result;
use crate::codec::bm_stream::BMStream;
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
    pub fn decode<B: AsRef<[u8]>>(input: &mut BMStream<B>) -> Result<Value> {
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

    pub fn encode(value: &Value, out: &mut BMStream<Vec<u8>>) -> Result<()> {
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
