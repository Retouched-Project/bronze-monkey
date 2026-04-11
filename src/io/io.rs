// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Read, Write};

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

impl<T: AsRef<[u8]>> DataInput for Cursor<T> {
    fn read_boolean(&mut self) -> Result<bool> {
        Ok(self.read_u8()? != 0)
    }

    fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>> {
        let remaining = self
            .get_ref()
            .as_ref()
            .len()
            .saturating_sub(self.position() as usize);
        if len > remaining {
            return Err("read_bytes requested more bytes than available in cursor".into());
        }
        let mut buf = vec![0u8; len];
        self.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn read_short(&mut self) -> Result<i16> {
        Ok(self.read_i16::<LittleEndian>()?)
    }

    fn read_int(&mut self) -> Result<i32> {
        Ok(self.read_i32::<LittleEndian>()?)
    }

    fn read_unsigned_int(&mut self) -> Result<u32> {
        Ok(self.read_u32::<LittleEndian>()?)
    }

    fn read_float(&mut self) -> Result<f32> {
        Ok(self.read_f32::<LittleEndian>()?)
    }

    fn read_double(&mut self) -> Result<f64> {
        Ok(self.read_f64::<LittleEndian>()?)
    }

    fn read_utf(&mut self) -> Result<String> {
        let len = self.read_i16::<LittleEndian>()?;
        if len < 0 {
            return Err("Negative UTF length".into());
        }
        let len = len as usize;
        if len == 0 {
            return Ok(String::new());
        }

        let remaining = self
            .get_ref()
            .as_ref()
            .len()
            .saturating_sub(self.position() as usize);
        if len > remaining {
            return Err("read_utf requested more bytes than available in cursor".into());
        }

        let mut buf = vec![0u8; len];
        self.read_exact(&mut buf)?;
        Ok(String::from_utf8(buf)?)
    }
}

impl DataOutput for Vec<u8> {
    fn write_boolean(&mut self, b: bool) -> Result<()> {
        self.write_u8(if b { 1 } else { 0 })?;
        Ok(())
    }

    fn write_bytes(&mut self, data: &[u8]) -> Result<()> {
        self.write_all(data)?;
        Ok(())
    }

    fn write_short(&mut self, v: i16) -> Result<()> {
        self.write_i16::<LittleEndian>(v)?;
        Ok(())
    }

    fn write_int(&mut self, v: i32) -> Result<()> {
        self.write_i32::<LittleEndian>(v)?;
        Ok(())
    }

    fn write_unsigned_int(&mut self, v: u32) -> Result<()> {
        self.write_u32::<LittleEndian>(v)?;
        Ok(())
    }

    fn write_float(&mut self, v: f32) -> Result<()> {
        self.write_f32::<LittleEndian>(v)?;
        Ok(())
    }

    fn write_double(&mut self, v: f64) -> Result<()> {
        self.write_f64::<LittleEndian>(v)?;
        Ok(())
    }

    fn write_utf(&mut self, s: &str) -> Result<()> {
        let bytes = s.as_bytes();
        if bytes.len() > i16::MAX as usize {
            return Err("UTF string too long".into());
        }
        self.write_i16::<LittleEndian>(bytes.len() as i16)?;
        self.write_all(bytes)?;
        Ok(())
    }
}
