// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::types::coded::crosses_as_its_code;

crosses_as_its_code! {
    #[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
    pub enum DeviceType {
        #[default]
        Any = 0,
        Unity = 1,
        IPhone = 2,
        Flash = 3,
        Android = 4,
        Native = 5,
        Palm = 6,
        Server = 7,
    }
}

impl DeviceType {
    pub fn is_game(self) -> bool {
        matches!(
            self,
            DeviceType::Unity | DeviceType::Flash | DeviceType::Native
        )
    }

    pub fn is_controller(self) -> bool {
        matches!(
            self,
            DeviceType::Android | DeviceType::IPhone | DeviceType::Palm
        )
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
    fn a_device_hosts_or_drives_a_session_but_never_both() {
        for &kind in DeviceType::ALL {
            assert!(
                !(kind.is_game() && kind.is_controller()),
                "{} claims both roles",
                kind.label()
            );
        }
    }

    #[test]
    fn a_device_of_unknown_or_serving_type_claims_neither_role() {
        for kind in [DeviceType::Any, DeviceType::Server] {
            assert!(!kind.is_game(), "{} should not host", kind.label());
            assert!(!kind.is_controller(), "{} should not drive", kind.label());
        }
    }
}
