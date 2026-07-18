// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use uuid::Builder;

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).expect("random source unavailable");
    b
}

pub fn generate_device_id() -> String {
    Builder::from_random_bytes(random_bytes::<16>())
        .into_uuid()
        .as_simple()
        .to_string()
}

pub fn generate_app_id() -> String {
    hex::encode(random_bytes::<12>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_id_is_lowercase_uuid_v4() {
        let id = generate_device_id();
        assert_eq!(id.len(), 32);
        assert_eq!(id.as_bytes()[12], b'4');
        assert!(
            id.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert_ne!(generate_device_id(), generate_device_id());
    }

    #[test]
    fn app_id_is_24_lowercase_hex() {
        let id = generate_app_id();
        assert_eq!(id.len(), 24);
        assert!(
            id.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert_ne!(generate_app_id(), generate_app_id());
    }
}
