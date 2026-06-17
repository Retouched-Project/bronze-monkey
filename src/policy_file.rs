// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

pub const CROSS_DOMAIN_POLICY: &str =
    r#"<?xml version="1.0"?><cross-domain-policy><allow-access-from domain="*" to-ports="1008-49151" /></cross-domain-policy>"#;

pub fn is_policy_request(bytes: &[u8]) -> bool {
    bytes.starts_with(b"<policy-file-request")
}
