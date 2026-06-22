// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

pub type Error = Box<dyn std::error::Error + Send + Sync + 'static>;
pub type Result<T> = std::result::Result<T, Error>;

pub trait DataInput {
    fn read_boolean(&mut self) -> Result<bool>;
    fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>>;
    fn read_short(&mut self) -> Result<i16>;
    fn read_int(&mut self) -> Result<i32>;
    fn read_unsigned_int(&mut self) -> Result<u32>;
    fn read_float(&mut self) -> Result<f32>;
    fn read_double(&mut self) -> Result<f64>;
    fn read_utf(&mut self) -> Result<String>;
}

pub trait DataOutput {
    fn write_boolean(&mut self, b: bool) -> Result<()>;
    fn write_bytes(&mut self, data: &[u8]) -> Result<()>;
    fn write_short(&mut self, v: i16) -> Result<()>;
    fn write_int(&mut self, v: i32) -> Result<()>;
    fn write_unsigned_int(&mut self, v: u32) -> Result<()>;
    fn write_float(&mut self, v: f32) -> Result<()>;
    fn write_double(&mut self, v: f64) -> Result<()>;
    fn write_utf(&mut self, s: &str) -> Result<()>;
}
