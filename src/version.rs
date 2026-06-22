// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::bm_version::BMVersion;
use crate::codec::externals::handshake::Handshake;
use serde::Serialize;

// Versions a consumer can query and display: the bronze-monkey library version
// and the Brass Monkey protocol (handshake) version it speaks.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct VersionInfo {
    pub library: String,
    pub sdk: String,
    pub sdk_minimum: String,
}

pub fn version_info() -> VersionInfo {
    let handshake = Handshake::default_version();
    VersionInfo {
        library: env!("CARGO_PKG_VERSION").to_string(),
        sdk: label(handshake.current),
        sdk_minimum: label(handshake.minimum),
    }
}

fn label(v: BMVersion) -> String {
    format!("{}.{}.{}", v.major, v.minor, v.build)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::externals::handshake;

    #[test]
    fn reports_library_and_sdk_versions() {
        let info = version_info();
        assert_eq!(info.library, env!("CARGO_PKG_VERSION"));
        assert_eq!(
            info.sdk,
            format!(
                "{}.{}.{}",
                handshake::CURRENT_MAJOR,
                handshake::CURRENT_MINOR,
                handshake::CURRENT_BUILD
            )
        );
        assert_eq!(
            info.sdk_minimum,
            format!(
                "{}.{}.{}",
                handshake::MIN_MAJOR,
                handshake::MIN_MINOR,
                handshake::MIN_BUILD
            )
        );
    }
}
