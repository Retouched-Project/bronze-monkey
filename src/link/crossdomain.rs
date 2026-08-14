// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey
//
//! Cross domain policy
//!
//! A Flash or Unity client may ask for policy before it will open any socket,
//! and will not proceed without one. The request arrives in place of a normal message,
//! so every connection has to be able to recognise it.

pub const REQUEST: &[u8] = b"<policy-file-request/>\0";

macro_rules! policy_xml {
    () => {
        "<?xml version=\"1.0\"?><cross-domain-policy><allow-access-from domain=\"*\" to-ports=\"1008-49151\" /></cross-domain-policy>"
    };
}

pub const XML: &str = policy_xml!();

pub const RESPONSE: &[u8] = concat!(policy_xml!(), "\0").as_bytes();

pub const PREFIX_LEN: usize = 16;

pub fn is_policy_request(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let compared = bytes.len().min(PREFIX_LEN);
    bytes[..compared] == REQUEST[..compared]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link::framing::{MAX_MESSAGE_LEN, frame};

    #[test]
    fn the_socket_reply_is_the_xml_plus_a_terminator() {
        assert_eq!(&RESPONSE[..XML.len()], XML.as_bytes());
        assert_eq!(RESPONSE.last(), Some(&0));
        assert_eq!(RESPONSE.len(), XML.len() + 1);
    }

    #[test]
    fn a_whole_request_is_recognised() {
        assert!(is_policy_request(REQUEST));
    }

    #[test]
    fn a_partial_request_is_enough() {
        assert!(is_policy_request(b"<policy-file-req"));
        assert!(is_policy_request(b"<pol"));
        assert!(is_policy_request(b"<"));
    }

    #[test]
    fn nothing_is_not_a_request() {
        assert!(!is_policy_request(&[]));
    }

    #[test]
    fn other_angle_brackets_are_not_a_request() {
        assert!(!is_policy_request(b"<html><body>"));
        assert!(!is_policy_request(RESPONSE));
    }

    #[test]
    fn a_framed_message_is_never_mistaken_for_one() {
        let framed = frame(&[0u8; 64]);
        assert!(!is_policy_request(&framed));
    }

    #[test]
    fn a_length_prefix_cannot_begin_with_an_angle_bracket() {
        let claimed = u32::from_le_bytes(*b"<pol") as usize;
        assert!(
            claimed > MAX_MESSAGE_LEN,
            "a frame starting with '<pol' would claim {claimed} bytes"
        );
    }

    #[test]
    fn trailing_bytes_do_not_matter() {
        let mut noisy = REQUEST.to_vec();
        noisy.extend_from_slice(b"and then some");
        assert!(is_policy_request(&noisy));
    }
}
