// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::io::io::{DataInput, DataOutput, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Read, Write};

pub struct BMStream {
    cursor: Cursor<Vec<u8>>,
}

impl BMStream {
    pub fn new() -> Self {
        Self {
            cursor: Cursor::new(Vec::with_capacity(256)),
        }
    }

    pub fn with_bytes(bytes: Vec<u8>) -> Self {
        Self {
            cursor: Cursor::new(bytes),
        }
    }

    pub fn get_data(&self) -> &[u8] {
        self.cursor.get_ref()
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.cursor.into_inner()
    }
}

impl DataInput for BMStream {
    fn read_boolean(&mut self) -> Result<bool> {
        Ok(self.cursor.read_u8()? != 0)
    }
    fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0; len];
        self.cursor.read_exact(&mut buf)?;
        Ok(buf)
    }
    fn read_short(&mut self) -> Result<i16> {
        Ok(self.cursor.read_i16::<LittleEndian>()?)
    }
    fn read_int(&mut self) -> Result<i32> {
        Ok(self.cursor.read_i32::<LittleEndian>()?)
    }
    fn read_unsigned_int(&mut self) -> Result<u32> {
        Ok(self.cursor.read_u32::<LittleEndian>()?)
    }
    fn read_float(&mut self) -> Result<f32> {
        Ok(self.cursor.read_f32::<LittleEndian>()?)
    }
    fn read_double(&mut self) -> Result<f64> {
        Ok(self.cursor.read_f64::<LittleEndian>()?)
    }
    fn read_utf(&mut self) -> Result<String> {
        let len = self.read_short()? as usize;
        let bytes = self.read_bytes(len)?;
        Ok(String::from_utf8(bytes)?)
    }
}

impl DataOutput for BMStream {
    fn write_boolean(&mut self, b: bool) -> Result<()> {
        Ok(self.cursor.write_u8(if b { 1 } else { 0 })?)
    }
    fn write_bytes(&mut self, data: &[u8]) -> Result<()> {
        Ok(self.cursor.write_all(data)?)
    }
    fn write_short(&mut self, v: i16) -> Result<()> {
        Ok(self.cursor.write_i16::<LittleEndian>(v)?)
    }
    fn write_int(&mut self, v: i32) -> Result<()> {
        Ok(self.cursor.write_i32::<LittleEndian>(v)?)
    }
    fn write_unsigned_int(&mut self, v: u32) -> Result<()> {
        Ok(self.cursor.write_u32::<LittleEndian>(v)?)
    }
    fn write_float(&mut self, v: f32) -> Result<()> {
        Ok(self.cursor.write_f32::<LittleEndian>(v)?)
    }
    fn write_double(&mut self, v: f64) -> Result<()> {
        Ok(self.cursor.write_f64::<LittleEndian>(v)?)
    }
    fn write_utf(&mut self, s: &str) -> Result<()> {
        let bytes = s.as_bytes();
        if bytes.len() > i16::MAX as usize {
            return Err("UTF string too long".into());
        }
        self.write_short(bytes.len() as i16)?;
        self.write_bytes(bytes)
    }
}
