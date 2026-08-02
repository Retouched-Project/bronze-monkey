// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::Result;
use crate::codec::bm_stream::BMStream;
use crate::codec::externals::registry;
use crate::codec::messages::bm_encoding::Value;
use crate::codec::messages::bm_parameter::BMParameter;
use crate::codec::object::Object;

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

    pub fn read_from<B: AsRef<[u8]>>(input: &mut BMStream<B>) -> Result<Self> {
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
                Object::BMParameter(p) => params.push((*p).value),
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

    pub fn write_to(&self, out: &mut BMStream<Vec<u8>>) -> Result<()> {
        out.write_int(self.id)?;
        out.write_utf(&self.method)?;
        out.write_utf(self.return_method.as_deref().unwrap_or(""))?;
        out.write_int(self.params.len() as i32)?;
        for p in &self.params {
            let obj = Object::BMParameter(Box::new(BMParameter::new(p.clone())));
            obj.encode_with_marker(out)?;
        }
        Ok(())
    }

    pub fn to_object_bytes(&self) -> Result<Vec<u8>> {
        let mut tmp = BMStream::new();
        let obj = Object::BMInvoke(self.clone());
        obj.encode_with_marker(&mut tmp)?;
        Ok(tmp.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invoke_round_trips_through_object_codec() {
        let invoke = BMInvoke {
            id: 7,
            method: "doThing".to_string(),
            return_method: Some("onThing".to_string()),
            params: vec![
                Value::I32(-3),
                Value::String("hi".to_string()),
                Value::Bool(true),
                Value::F64(1.25),
            ],
        };

        let bytes = invoke.to_object_bytes().unwrap();
        let mut cur = BMStream::view(&bytes);
        let decoded = match Object::decode(&mut cur).unwrap() {
            Object::BMInvoke(i) => i,
            other => panic!("expected BMInvoke, got {other:?}"),
        };

        assert_eq!(decoded.id, invoke.id);
        assert_eq!(decoded.method, invoke.method);
        assert_eq!(decoded.return_method, invoke.return_method);
        assert_eq!(decoded.params.len(), invoke.params.len());
        assert!(matches!(decoded.params[0], Value::I32(-3)));
        assert!(matches!(&decoded.params[1], Value::String(s) if s == "hi"));
        assert!(matches!(decoded.params[2], Value::Bool(true)));
        assert!(matches!(decoded.params[3], Value::F64(v) if v == 1.25));
    }

    #[test]
    fn empty_return_method_round_trips_as_none() {
        let invoke = BMInvoke {
            id: 0,
            method: "ping".to_string(),
            return_method: None,
            params: vec![],
        };
        let bytes = invoke.to_object_bytes().unwrap();
        let mut cur = BMStream::view(&bytes);
        let decoded = match Object::decode(&mut cur).unwrap() {
            Object::BMInvoke(i) => i,
            other => panic!("expected BMInvoke, got {other:?}"),
        };
        assert_eq!(decoded.return_method, None);
        assert!(decoded.params.is_empty());
    }

    // Golden bytes computed by hand from the wire format.
    #[test]
    fn invoke_golden_bytes() {
        let invoke = BMInvoke {
            id: 1,
            method: "x".to_string(),
            return_method: None,
            params: vec![Value::I32(5)],
        };
        let bytes = invoke.to_object_bytes().unwrap();
        let expected: &[u8] = &[
            0x01, 0x00, 0x40, // "@" object marker
            0x04, 0x00, // class id 4 (BMInvoke)
            0x01, 0x00, 0x00, 0x00, // id = 1
            0x01, 0x00, 0x78, // method "x"
            0x00, 0x00, // return method "" (None)
            0x01, 0x00, 0x00, 0x00, // param count = 1
            0x01, 0x00, 0x40, // "@" object marker (parameter)
            0x03, 0x00, // class id 3 (BMParameter)
            0x01, 0x00, 0x69, // BMEncoding tag "i"
            0x05, 0x00, 0x00, 0x00, // i32 value = 5
        ];
        assert_eq!(bytes, expected);
    }
}
