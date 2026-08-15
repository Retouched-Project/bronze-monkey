// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
#[serde(into = "i32", try_from = "i32")]
pub enum DeviceType {
    #[default]
    Any,
    Unity,
    IPhone,
    Flash,
    Android,
    Native,
    Palm,
    Server,
}

impl From<DeviceType> for i32 {
    fn from(value: DeviceType) -> Self {
        value.code()
    }
}

impl TryFrom<i32> for DeviceType {
    type Error = DeviceTypeError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Self::for_value(value)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DeviceTypeError {
    OutOfRange(i32),
}

impl std::fmt::Display for DeviceTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceTypeError::OutOfRange(v) => write!(f, "DeviceType out of range: {v}"),
        }
    }
}

impl std::error::Error for DeviceTypeError {}

impl DeviceType {
    /// Every device type, for callers that publish the whole table.
    pub const ALL: [DeviceType; 8] = [
        DeviceType::Any,
        DeviceType::Unity,
        DeviceType::IPhone,
        DeviceType::Flash,
        DeviceType::Android,
        DeviceType::Native,
        DeviceType::Palm,
        DeviceType::Server,
    ];

    pub fn code(self) -> i32 {
        match self {
            DeviceType::Any => 0,
            DeviceType::Unity => 1,
            DeviceType::IPhone => 2,
            DeviceType::Flash => 3,
            DeviceType::Android => 4,
            DeviceType::Native => 5,
            DeviceType::Palm => 6,
            DeviceType::Server => 7,
        }
    }

    pub fn for_value(v: i32) -> Result<Self, DeviceTypeError> {
        Ok(match v {
            0 => DeviceType::Any,
            1 => DeviceType::Unity,
            2 => DeviceType::IPhone,
            3 => DeviceType::Flash,
            4 => DeviceType::Android,
            5 => DeviceType::Native,
            6 => DeviceType::Palm,
            7 => DeviceType::Server,
            _ => return Err(DeviceTypeError::OutOfRange(v)),
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            DeviceType::Any => "ANY",
            DeviceType::Unity => "UNITY",
            DeviceType::IPhone => "IPHONE",
            DeviceType::Flash => "FLASH",
            DeviceType::Android => "ANDROID",
            DeviceType::Native => "NATIVE",
            DeviceType::Palm => "PALM",
            DeviceType::Server => "SERVER",
        }
    }
}

impl std::fmt::Display for DeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[DeviceType {}]", self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_lists_every_code_in_order() {
        for (index, kind) in DeviceType::ALL.iter().enumerate() {
            assert_eq!(
                kind.code(),
                index as i32,
                "{} is out of place",
                kind.label()
            );
        }
        // A variant added without updating ALL would leave this code reachable.
        assert!(DeviceType::for_value(DeviceType::ALL.len() as i32).is_err());
    }
}
