// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey
//
//! Cross domain policy
//!
//! A Flash or Unity client may ask for policy before it will open any socket,
//! and will not proceed without one. The request arrives in place of a normal message,
//! so every connection has to be able to recognise it.
//!
//! A caller that can look at the socket without consuming needs only
//! [`is_policy_request`] and [`RESPONSE`]. One that is handed bytes a chunk at a
//! time wants [`Sniffer`], which holds the opening bytes back until it knows.

use serde::{Deserialize, Serialize};

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

/// What a [`Sniffer`] made of the bytes it was given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Sniff {
    Wait,
    Answer,
    Passthrough {
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Watch {
    #[default]
    Open,
    Ordinary,
    Answered,
}

/// Recognises a policy request at the head of a connection.
///
/// A request only ever arrives first, and a client that sends one sends nothing
/// else, so the sniffer stops looking the moment it can tell. Until then it
/// keeps what it has seen: a request can be split across chunks, and bytes that
/// turn out to be ordinary traffic have to reach the framer intact.
///
/// The stop matters as much as the match. A four byte length prefix of 60 also
/// begins with `<`, so a sniffer that kept looking would eventually mistake an
/// ordinary message for a request and hang up on a live session.
#[derive(Debug, Default)]
pub struct Sniffer {
    buffer: Vec<u8>,
    watch: Watch,
}

impl Sniffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// True while the answer is still open. Once it is false the caller can
    /// pass bytes on directly and skip the sniffer for the rest of the
    /// connection.
    pub fn is_watching(&self) -> bool {
        self.watch == Watch::Open
    }

    /// Whether the connection that just dropped was one we hung up on after
    /// answering a request, rather than a peer going away. Watches again either
    /// way, since whatever comes next is a new connection.
    pub fn hung_up(&mut self) -> bool {
        let ours = self.watch == Watch::Answered;
        self.reset();
        ours
    }

    /// Offers the next bytes off the wire.
    pub fn feed(&mut self, data: &[u8]) -> Sniff {
        match self.watch {
            Watch::Ordinary => {
                return Sniff::Passthrough {
                    data: data.to_vec(),
                };
            }
            Watch::Answered => return Sniff::Wait,
            Watch::Open => {}
        }

        self.buffer.extend_from_slice(data);
        if self.buffer.is_empty() {
            return Sniff::Wait;
        }
        if !is_policy_request(&self.buffer) {
            self.watch = Watch::Ordinary;
            return Sniff::Passthrough {
                data: std::mem::take(&mut self.buffer),
            };
        }
        if self.buffer.len() < PREFIX_LEN {
            return Sniff::Wait;
        }

        self.watch = Watch::Answered;
        self.buffer.clear();
        Sniff::Answer
    }

    /// Starts over, for a sniffer reused across connections.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.watch = Watch::Open;
    }
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

    #[test]
    fn a_sniffer_answers_a_whole_request() {
        let mut sniffer = Sniffer::new();
        assert_eq!(sniffer.feed(REQUEST), Sniff::Answer);
        assert!(!sniffer.is_watching());
    }

    #[test]
    fn a_sniffer_answers_a_request_that_arrives_in_pieces() {
        let mut sniffer = Sniffer::new();
        assert_eq!(sniffer.feed(b"<pol"), Sniff::Wait);
        assert_eq!(sniffer.feed(b"icy-fi"), Sniff::Wait);
        assert!(sniffer.is_watching());
        assert_eq!(sniffer.feed(b"le-request/>\0"), Sniff::Answer);
    }

    #[test]
    fn a_sniffer_passes_a_sixty_byte_message_through() {
        // The length prefix of a 60 byte message is 0x3C 0x00 0x00 0x00, so it
        // opens with the same '<' a request does. Answering this one would
        // hang up on a healthy connection.
        let framed = frame(&[7u8; 60]);
        let mut sniffer = Sniffer::new();
        assert_eq!(
            sniffer.feed(&framed),
            Sniff::Passthrough {
                data: framed.clone()
            }
        );
        assert!(!sniffer.is_watching());
        assert!(!sniffer.hung_up());
    }

    #[test]
    fn held_back_bytes_come_back_in_order() {
        let framed = frame(&[7u8; 60]);
        let mut sniffer = Sniffer::new();
        assert_eq!(sniffer.feed(&framed[..1]), Sniff::Wait);
        assert_eq!(
            sniffer.feed(&framed[1..]),
            Sniff::Passthrough {
                data: framed.clone()
            }
        );
    }

    #[test]
    fn an_empty_chunk_decides_nothing() {
        let mut sniffer = Sniffer::new();
        assert_eq!(sniffer.feed(&[]), Sniff::Wait);
        assert!(sniffer.is_watching());
        assert_eq!(sniffer.feed(REQUEST), Sniff::Answer);
    }

    #[test]
    fn ordinary_traffic_is_never_reconsidered() {
        let mut sniffer = Sniffer::new();
        assert!(matches!(
            sniffer.feed(&frame(b"hello")),
            Sniff::Passthrough { .. }
        ));
        // Even a verbatim request now belongs to whoever is reading the stream.
        assert_eq!(
            sniffer.feed(REQUEST),
            Sniff::Passthrough {
                data: REQUEST.to_vec()
            }
        );
    }

    #[test]
    fn a_request_is_answered_only_once() {
        let mut sniffer = Sniffer::new();
        assert_eq!(sniffer.feed(REQUEST), Sniff::Answer);
        assert_eq!(sniffer.feed(REQUEST), Sniff::Wait);
        assert_eq!(sniffer.feed(b"leftovers"), Sniff::Wait);
    }

    #[test]
    fn a_reset_sniffer_watches_again() {
        let mut sniffer = Sniffer::new();
        assert_eq!(sniffer.feed(REQUEST), Sniff::Answer);
        sniffer.reset();
        assert!(sniffer.is_watching());
        assert_eq!(sniffer.feed(REQUEST), Sniff::Answer);
    }

    #[test]
    fn a_hang_up_is_claimed_once() {
        let mut sniffer = Sniffer::new();
        assert_eq!(sniffer.feed(REQUEST), Sniff::Answer);
        assert!(sniffer.hung_up());
        // The next drop belongs to whoever was actually on the connection.
        assert!(!sniffer.hung_up());
    }

    #[test]
    fn claiming_a_hang_up_watches_again() {
        let mut sniffer = Sniffer::new();
        assert_eq!(sniffer.feed(REQUEST), Sniff::Answer);
        assert!(sniffer.hung_up());
        assert!(sniffer.is_watching());
        assert_eq!(sniffer.feed(REQUEST), Sniff::Answer);
    }

    #[test]
    fn an_ordinary_connection_was_not_hung_up_on() {
        let mut sniffer = Sniffer::new();
        let framed = frame(&[7u8; 60]);
        assert!(matches!(sniffer.feed(&framed), Sniff::Passthrough { .. }));
        assert!(!sniffer.hung_up());
        // A link that carried real traffic is watched again from its next byte.
        assert!(sniffer.is_watching());
    }

    #[test]
    fn a_reset_sniffer_forgets_a_half_read_request() {
        let mut sniffer = Sniffer::new();
        assert_eq!(sniffer.feed(b"<policy"), Sniff::Wait);
        sniffer.reset();
        let framed = frame(&[7u8; 60]);
        assert_eq!(
            sniffer.feed(&framed),
            Sniff::Passthrough {
                data: framed.clone()
            }
        );
    }
}
