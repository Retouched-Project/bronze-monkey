// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

/*
 The version/handshake packet is used to negotiate the protocol version between BM clients (and the server).
 It is the first packet sent by each client.

 The first 4 bytes is the little-endian payload size for the 2 version fields (8 bytes).
 The next 4 bytes is the version field (major, minor, build).
 The next 4 bytes is the minimum version field (major, minor, build).

 The version fields are encoded using the BMVersion struct.
 Example (BMVersion, current 1.7.0, minimum 0.9.0):
 08 00 00 00 | 00 00 07 01 | 00 00 09 00
*/

use super::bm_version::BMVersion;
use std::panic::{AssertUnwindSafe, catch_unwind};

pub const CURRENT_MAJOR: u8 = 1;
pub const CURRENT_MINOR: u8 = 7;
pub const CURRENT_BUILD: u16 = 0;
pub const MIN_MAJOR: u8 = 0;
pub const MIN_MINOR: u8 = 9;
pub const MIN_BUILD: u16 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handshake {
    pub current: BMVersion,
    pub minimum: BMVersion,
}

impl Handshake {
    pub fn new(current: BMVersion, minimum: BMVersion) -> Self {
        Self { current, minimum }
    }

    pub fn default_version() -> Self {
        Self {
            current: BMVersion::new(CURRENT_MAJOR, CURRENT_MINOR, CURRENT_BUILD),
            minimum: BMVersion::new(MIN_MAJOR, MIN_MINOR, MIN_BUILD),
        }
    }

    /// The two version fields on their own, without the length in front.
    pub fn to_message(&self) -> [u8; 8] {
        let mut buf = [0u8; 8];
        buf[0..4].copy_from_slice(&self.current.to_u32().to_le_bytes());
        buf[4..8].copy_from_slice(&self.minimum.to_u32().to_le_bytes());
        buf
    }

    pub fn to_bytes(&self) -> [u8; 12] {
        let mut buf = [0u8; 12];
        buf[0..4].copy_from_slice(&8u32.to_le_bytes());
        buf[4..8].copy_from_slice(&self.current.to_u32().to_le_bytes());
        buf[8..12].copy_from_slice(&self.minimum.to_u32().to_le_bytes());
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 12 {
            return None;
        }
        let size = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
        if size != 8 {
            return None;
        }
        Self::from_message(&bytes[4..12])
    }

    /// Reads the two version fields on their own, with no length in front.
    /// A real packet is far longer than this, so the length alone identifies it.
    pub fn from_message(message: &[u8]) -> Option<Self> {
        if message.len() != 8 {
            return None;
        }
        let current = BMVersion::from_u32(u32::from_le_bytes(message[0..4].try_into().ok()?));
        let minimum = BMVersion::from_u32(u32::from_le_bytes(message[4..8].try_into().ok()?));
        Some(Self { current, minimum })
    }
}

impl Default for Handshake {
    fn default() -> Self {
        Self::default_version()
    }
}

pub fn handshake_bytes(current: Option<BMVersion>, minimum: Option<BMVersion>) -> [u8; 12] {
    let defaults = Handshake::default_version();
    let cur = current.unwrap_or(defaults.current);
    let min = minimum.unwrap_or(defaults.minimum);
    Handshake::new(cur, min).to_bytes()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_handshake_bytes(
    cur_major: u8,
    cur_minor: u8,
    cur_build: u16,
    min_major: u8,
    min_minor: u8,
    min_build: u16,
    out_ptr: *mut u8,
    out_len: usize,
) -> usize {
    catch_unwind(AssertUnwindSafe(|| {
        if out_ptr.is_null() || out_len < 12 {
            return 0usize;
        }
        let current = BMVersion::new(cur_major, cur_minor, cur_build);
        let minimum = BMVersion::new(min_major, min_minor, min_build);
        let bytes = handshake_bytes(Some(current), Some(minimum));
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_ptr, 12);
        }
        12usize
    }))
    .unwrap_or(0)
}
