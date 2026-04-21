// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

/*
 BMVersion packs the version fields into a 32-bit integer as follows:
 vInt = ((1 & 0xFF) << 24) | ((7 & 0xFF) << 16) | (0 & 0xFFFF)
 The fields are:
 - Major: 8 bits (0-255)
 - Minor: 8 bits (0-255)
 - Build: 16 bits (0-65535)

 For example:
 Version 1.7.0 is represented as 0x01070000
 The resulting array is:
 [0x00, 0x00, 0x07, 0x01]

 We can also encode a minimum version using the same scheme.
 For example:
 Minimum version 0.9.0 is represented as 0x00090000
 The resulting array is:
 [0x00, 0x00, 0x09, 0x00]

 The bytes are written in little-endian order.
*/

#[derive(Clone, Copy, Debug, Default)]
pub struct BMVersion {
    pub major: u8,
    pub minor: u8,
    pub build: u16,
}

impl BMVersion {
    pub fn new(major: u8, minor: u8, build: u16) -> Self {
        Self {
            major,
            minor,
            build,
        }
    }

    pub fn to_u32(&self) -> u32 {
        ((self.major as u32) << 24) | ((self.minor as u32) << 16) | (self.build as u32 & 0xFFFF)
    }

    pub fn from_u32(v: u32) -> Self {
        let major = ((v >> 24) & 0xFF) as u8;
        let minor = ((v >> 16) & 0xFF) as u8;
        let build = (v & 0xFFFF) as u16;
        Self {
            major,
            minor,
            build,
        }
    }

    pub fn to_bytes(&self) -> [u8; 4] {
        self.to_u32().to_le_bytes()
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 4 {
            return None;
        }
        let mut arr = [0u8; 4];
        arr.copy_from_slice(&bytes[0..4]);
        Some(Self::from_u32(u32::from_le_bytes(arr)))
    }

    pub fn encode(major: u8, minor: u8, build: u16) -> u32 {
        Self::new(major, minor, build).to_u32()
    }

    pub fn major_from(version: u32) -> u8 {
        ((version >> 24) & 0xFF) as u8
    }

    pub fn minor_from(version: u32) -> u8 {
        ((version >> 16) & 0xFF) as u8
    }

    pub fn build_from(version: u32) -> u16 {
        (version & 0xFFFF) as u16
    }
}
