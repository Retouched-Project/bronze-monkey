// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::Result;
use crate::codec::bm_stream::BMStream;
use crate::codec::externals::registry;
use crate::codec::messages::bm_encoding::{BMEncoding, Value};

use serde::{Deserialize, Serialize};

// A single invoke parameter. It carries one tagged value inside an invoke's
// parameter list; the value itself is encoded by BMEncoding, this type is just
// the protocol object (class id 3) wrapping it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BMParameter {
    pub value: Value,
}

impl BMParameter {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_PARAMETER;

    pub fn new(value: Value) -> Self {
        Self { value }
    }

    pub fn read_from<B: AsRef<[u8]>>(input: &mut BMStream<B>) -> Result<Self> {
        Ok(Self {
            value: BMEncoding::decode(input)?,
        })
    }

    pub fn write_to(&self, out: &mut BMStream<Vec<u8>>) -> Result<()> {
        match Self::narrowed(&self.value) {
            Some(narrow) => BMEncoding::encode(&narrow, out),
            None => BMEncoding::encode(&self.value, out),
        }
    }

    fn narrowed(value: &Value) -> Option<Value> {
        Some(match *value {
            Value::I16(v) => Value::I32(v as i32),
            Value::U16(v) => Value::I32(v as i32),
            Value::U32(v) => Value::I32(v as i32),
            Value::F64(v) => Value::F32(v as f32),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::bm_stream::BMStream;
    use crate::codec::object::Object;

    fn round_trip(value: Value) -> Value {
        let param = BMParameter::new(value);
        let mut w = BMStream::new();
        param.write_to(&mut w).unwrap();
        let bytes = w.into_inner();
        let mut r = BMStream::view(&bytes);
        BMParameter::read_from(&mut r).unwrap().value
    }

    #[test]
    fn scalar_value_round_trips() {
        assert!(matches!(round_trip(Value::I32(-9)), Value::I32(-9)));
        assert!(matches!(round_trip(Value::Bool(true)), Value::Bool(true)));
        assert!(matches!(round_trip(Value::F32(2.5)), Value::F32(v) if v == 2.5));
        assert!(matches!(round_trip(Value::String("x".into())), Value::String(s) if s == "x"));
    }

    #[test]
    fn a_value_too_wide_to_dispatch_is_narrowed_to_one_that_is_not() {
        assert!(matches!(round_trip(Value::F64(0.032)), Value::F32(v) if v == 0.032_f32));
        assert!(matches!(round_trip(Value::U32(3)), Value::I32(3)));
        assert!(matches!(round_trip(Value::U16(65535)), Value::I32(65535)));
        assert!(matches!(round_trip(Value::I16(-7)), Value::I32(-7)));
    }

    #[test]
    fn narrowing_an_integer_keeps_every_bit() {
        assert!(matches!(round_trip(Value::U32(u32::MAX)), Value::I32(-1)));
        assert!(matches!(round_trip(Value::U32(i32::MAX as u32)), Value::I32(v) if v == i32::MAX));
    }

    #[test]
    fn object_value_round_trips() {
        let inner = Value::Object(Object::StringLiteral(
            crate::codec::messages::string_literal::StringLiteral {
                value: "hello".into(),
            },
        ));
        match round_trip(inner) {
            Value::Object(Object::StringLiteral(s)) => assert_eq!(s.value, "hello"),
            other => panic!("expected StringLiteral object, got {other:?}"),
        }
    }
}
