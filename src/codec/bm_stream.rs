// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::Result;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Read, Write};

pub struct BMStream<B = Vec<u8>> {
    cursor: Cursor<B>,
    depth: usize,
}

impl BMStream<Vec<u8>> {
    pub fn new() -> Self {
        Self {
            cursor: Cursor::new(Vec::with_capacity(256)),
            depth: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            cursor: Cursor::new(Vec::with_capacity(capacity)),
            depth: 0,
        }
    }

    pub fn with_bytes(bytes: Vec<u8>) -> Self {
        Self {
            cursor: Cursor::new(bytes),
            depth: 0,
        }
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.cursor.into_inner()
    }
}

impl<'a> BMStream<&'a [u8]> {
    pub fn view(bytes: &'a [u8]) -> Self {
        Self {
            cursor: Cursor::new(bytes),
            depth: 0,
        }
    }
}

impl Default for BMStream<Vec<u8>> {
    fn default() -> Self {
        Self::new()
    }
}

impl<B: AsRef<[u8]>> BMStream<B> {
    pub fn get_data(&self) -> &[u8] {
        self.cursor.get_ref().as_ref()
    }

    pub fn position(&self) -> usize {
        self.cursor.position() as usize
    }

    fn remaining(&self) -> usize {
        self.cursor
            .get_ref()
            .as_ref()
            .len()
            .saturating_sub(self.position())
    }

    #[inline]
    pub fn enter_nested(&mut self) -> Result<()> {
        if self.depth >= 64 {
            return Err("Maximum nesting depth exceeded".into());
        }
        self.depth += 1;
        Ok(())
    }

    #[inline]
    pub fn exit_nested(&mut self) {
        if self.depth > 0 {
            self.depth -= 1;
        }
    }

    #[inline]
    pub fn read_boolean(&mut self) -> Result<bool> {
        Ok(self.cursor.read_u8()? != 0)
    }

    #[inline]
    pub fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>> {
        if len > self.remaining() {
            return Err("read_bytes requested more bytes than available".into());
        }
        let mut buf = vec![0u8; len];
        self.cursor.read_exact(&mut buf)?;
        Ok(buf)
    }

    #[inline]
    pub fn read_short(&mut self) -> Result<i16> {
        Ok(self.cursor.read_i16::<LittleEndian>()?)
    }

    #[inline]
    pub fn read_int(&mut self) -> Result<i32> {
        Ok(self.cursor.read_i32::<LittleEndian>()?)
    }

    #[inline]
    pub fn read_unsigned_int(&mut self) -> Result<u32> {
        Ok(self.cursor.read_u32::<LittleEndian>()?)
    }

    #[inline]
    pub fn read_float(&mut self) -> Result<f32> {
        Ok(self.cursor.read_f32::<LittleEndian>()?)
    }

    #[inline]
    pub fn read_double(&mut self) -> Result<f64> {
        Ok(self.cursor.read_f64::<LittleEndian>()?)
    }

    pub fn read_utf(&mut self) -> Result<String> {
        let len = self.read_short()?;
        if len < 0 {
            return Err("Negative UTF length".into());
        }
        let len = len as usize;
        if len == 0 {
            return Ok(String::new());
        }
        if len > self.remaining() {
            return Err("read_utf requested more bytes than available".into());
        }
        let mut buf = vec![0u8; len];
        self.cursor.read_exact(&mut buf)?;
        Ok(String::from_utf8(buf)?)
    }
}

impl BMStream<Vec<u8>> {
    #[inline]
    pub fn write_boolean(&mut self, b: bool) -> Result<()> {
        Ok(self.cursor.write_u8(if b { 1 } else { 0 })?)
    }

    #[inline]
    pub fn write_bytes(&mut self, data: &[u8]) -> Result<()> {
        Ok(self.cursor.write_all(data)?)
    }

    #[inline]
    pub fn write_short(&mut self, v: i16) -> Result<()> {
        Ok(self.cursor.write_i16::<LittleEndian>(v)?)
    }

    #[inline]
    pub fn write_int(&mut self, v: i32) -> Result<()> {
        Ok(self.cursor.write_i32::<LittleEndian>(v)?)
    }

    #[inline]
    pub fn write_unsigned_int(&mut self, v: u32) -> Result<()> {
        Ok(self.cursor.write_u32::<LittleEndian>(v)?)
    }

    #[inline]
    pub fn write_float(&mut self, v: f32) -> Result<()> {
        Ok(self.cursor.write_f32::<LittleEndian>(v)?)
    }

    #[inline]
    pub fn write_double(&mut self, v: f64) -> Result<()> {
        Ok(self.cursor.write_f64::<LittleEndian>(v)?)
    }

    pub fn write_utf(&mut self, s: &str) -> Result<()> {
        let bytes = s.as_bytes();
        if bytes.len() > i16::MAX as usize {
            return Err("UTF string too long".into());
        }
        self.write_short(bytes.len() as i16)?;
        self.write_bytes(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_round_trip() {
        let mut w = BMStream::new();
        w.write_boolean(true).unwrap();
        w.write_short(-1234).unwrap();
        w.write_int(-77).unwrap();
        w.write_unsigned_int(4_000_000_000).unwrap();
        w.write_float(1.5).unwrap();
        w.write_double(-2.25).unwrap();
        w.write_utf("hello").unwrap();
        let bytes = w.into_inner();

        let mut r = BMStream::view(&bytes);
        assert!(r.read_boolean().unwrap());
        assert_eq!(r.read_short().unwrap(), -1234);
        assert_eq!(r.read_int().unwrap(), -77);
        assert_eq!(r.read_unsigned_int().unwrap(), 4_000_000_000);
        assert_eq!(r.read_float().unwrap(), 1.5);
        assert_eq!(r.read_double().unwrap(), -2.25);
        assert_eq!(r.read_utf().unwrap(), "hello");
    }

    #[test]
    fn golden_little_endian_bytes() {
        let mut w = BMStream::new();
        w.write_int(1).unwrap();
        w.write_utf("@").unwrap();
        // i32(1) LE = 01 00 00 00; utf "@" = i16 len(1) LE + '@' = 01 00 40
        assert_eq!(w.get_data(), &[0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x40]);
    }

    #[test]
    fn empty_utf_round_trips() {
        let mut w = BMStream::new();
        w.write_utf("").unwrap();
        let bytes = w.into_inner();
        assert_eq!(bytes, vec![0x00, 0x00]);
        let mut r = BMStream::view(&bytes);
        assert_eq!(r.read_utf().unwrap(), "");
    }

    #[test]
    fn owned_and_borrowed_reads_agree() {
        let mut w = BMStream::new();
        w.write_int(42).unwrap();
        w.write_utf("xy").unwrap();
        let bytes = w.into_inner();

        let mut owned = BMStream::with_bytes(bytes.clone());
        let mut borrowed = BMStream::view(&bytes);
        assert_eq!(owned.read_int().unwrap(), borrowed.read_int().unwrap());
        assert_eq!(owned.read_utf().unwrap(), borrowed.read_utf().unwrap());
    }

    #[test]
    fn read_past_end_errors() {
        let bytes = [0x01, 0x00];
        let mut r = BMStream::view(&bytes);
        assert_eq!(r.read_short().unwrap(), 1);
        assert!(r.read_int().is_err());
    }

    #[test]
    fn position_tracks_message_offset() {
        let mut w = BMStream::new();
        w.write_boolean(true).unwrap();
        w.write_utf("ab").unwrap();
        let bytes = w.into_inner();
        let mut r = BMStream::view(&bytes);
        r.read_boolean().unwrap();
        r.read_utf().unwrap();
        assert_eq!(r.position(), bytes.len());
    }
}
