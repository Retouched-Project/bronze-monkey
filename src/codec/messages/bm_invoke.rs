// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::registry;
use crate::codec::io::{DataInput, DataOutput, Result};
use crate::codec::object::Object;
use crate::codec::messages::bm_encoding::Value;
use crate::codec::messages::bm_parameter::VecOutput;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Read};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
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
