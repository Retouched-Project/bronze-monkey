// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::bm_stream::BMStream;
use crate::codec::externals::registry;
use crate::codec::Result;
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
        BMEncoding::encode(&self.value, out)
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
        assert!(matches!(round_trip(Value::F64(2.5)), Value::F64(v) if v == 2.5));
        assert!(matches!(round_trip(Value::String("x".into())), Value::String(s) if s == "x"));
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
