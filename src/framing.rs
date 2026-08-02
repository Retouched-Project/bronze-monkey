// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey
//
//! Stream framing
//!
//! Stream transports carry messages back to back, so each one is written with
//! its length in front. Datagram transports already preserve message
//! boundaries and carry no length at all. That is the whole difference between
//! the two, and it lives here rather than in every program that uses the
//! library.
//!
//! Feed a stream whatever arrives, in whatever sized pieces it arrives, and
//! take back whole messages ready for the engine.

use crate::codec::Result;

/// Longest message that will be accepted before the stream is treated as
/// corrupt. Control schemes are the only large messages and they
/// use BMByteChunk to split messages into smaller chunks anyway.
pub const MAX_MESSAGE_LEN: usize = 5 * 1024 * 1024;

pub const LENGTH_PREFIX_LEN: usize = 4;

/// Writes a message with the length prefix a stream transport needs.
pub fn frame(message: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(LENGTH_PREFIX_LEN + message.len());
    out.extend_from_slice(&(message.len() as u32).to_le_bytes());
    out.extend_from_slice(message);
    out
}

/// Reassembles messages from a stream that arrives in arbitrary pieces.
#[derive(Debug)]
pub struct Framer {
    buffer: Vec<u8>,
    max_len: usize,
}

impl Default for Framer {
    fn default() -> Self {
        Self::new()
    }
}

impl Framer {
    pub fn new() -> Self {
        Self::with_max_len(MAX_MESSAGE_LEN)
    }

    /// Uses a limit of its own, for callers that want to accept less than the
    /// library's ceiling.
    pub fn with_max_len(max_len: usize) -> Self {
        Self {
            buffer: Vec::new(),
            max_len: max_len.min(MAX_MESSAGE_LEN),
        }
    }

    /// The longest message this framer will accept.
    pub fn max_len(&self) -> usize {
        self.max_len
    }

    /// Adds bytes and returns every message they completed. A message that is
    /// still incomplete stays buffered for the next call.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>> {
        self.buffer.extend_from_slice(bytes);
        let mut messages = Vec::new();
        let mut offset = 0;

        loop {
            let Some(header) = self.buffer.get(offset..offset + LENGTH_PREFIX_LEN) else {
                break;
            };
            let len = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
            if len > self.max_len {
                self.buffer.clear();
                return Err(format!("message length {len} exceeds the maximum").into());
            }
            let start = offset + LENGTH_PREFIX_LEN;
            let Some(message) = self.buffer.get(start..start + len) else {
                break;
            };
            messages.push(message.to_vec());
            offset = start + len;
        }

        if offset > 0 {
            self.buffer.drain(..offset);
        }
        Ok(messages)
    }

    /// Drops anything half read, for when a connection restarts.
    pub fn reset(&mut self) {
        self.buffer.clear();
    }

    /// Bytes held back waiting for the rest of their message.
    pub fn pending(&self) -> usize {
        self.buffer.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn framed(bodies: &[&[u8]]) -> Vec<u8> {
        bodies.iter().flat_map(|b| frame(b)).collect()
    }

    #[test]
    fn splits_messages_that_arrive_together() {
        let mut f = Framer::new();
        let out = f.feed(&framed(&[b"one", b"two", b"three"])).unwrap();
        assert_eq!(
            out,
            vec![b"one".to_vec(), b"two".to_vec(), b"three".to_vec()]
        );
        assert_eq!(f.pending(), 0);
    }

    #[test]
    fn joins_a_message_split_across_reads() {
        let stream = framed(&[b"hello"]);
        let mut f = Framer::new();
        // One byte at a time, the worst a stream can do.
        for (i, byte) in stream.iter().enumerate() {
            let out = f.feed(&[*byte]).unwrap();
            if i + 1 == stream.len() {
                assert_eq!(out, vec![b"hello".to_vec()]);
            } else {
                assert!(out.is_empty(), "message completed early at byte {i}");
            }
        }
        assert_eq!(f.pending(), 0);
    }

    #[test]
    fn keeps_a_trailing_partial_message() {
        let stream = framed(&[b"done", b"partial"]);
        let cut = stream.len() - 3;
        let mut f = Framer::new();
        assert_eq!(f.feed(&stream[..cut]).unwrap(), vec![b"done".to_vec()]);
        assert!(f.pending() > 0);
        assert_eq!(f.feed(&stream[cut..]).unwrap(), vec![b"partial".to_vec()]);
        assert_eq!(f.pending(), 0);
    }

    #[test]
    fn empty_messages_survive() {
        let mut f = Framer::new();
        assert_eq!(
            f.feed(&framed(&[b"", b"x"])).unwrap(),
            vec![vec![], b"x".to_vec()]
        );
    }

    #[test]
    fn a_silly_length_is_rejected_rather_than_buffered() {
        let mut f = Framer::new();
        let mut bytes = (MAX_MESSAGE_LEN as u32 + 1).to_le_bytes().to_vec();
        bytes.extend_from_slice(b"whatever");
        assert!(f.feed(&bytes).is_err());
        // The stream is discarded rather than left to resynchronise on garbage.
        assert_eq!(f.pending(), 0);
    }

    #[test]
    fn reset_drops_a_partial_message() {
        let stream = framed(&[b"abc"]);
        let mut f = Framer::new();
        f.feed(&stream[..5]).unwrap();
        assert!(f.pending() > 0);
        f.reset();
        assert_eq!(f.pending(), 0);
        // What follows is read as a fresh message, not glued to the old one.
        assert_eq!(f.feed(&stream).unwrap(), vec![b"abc".to_vec()]);
    }

    #[test]
    fn a_tighter_limit_is_honoured() {
        let mut f = Framer::with_max_len(16);
        assert_eq!(f.max_len(), 16);
        assert!(f.feed(&frame(&[0u8; 8])).is_ok());
        assert!(f.feed(&frame(&[0u8; 17])).is_err());
    }

    #[test]
    fn a_limit_cannot_be_raised_past_the_ceiling() {
        assert_eq!(Framer::with_max_len(usize::MAX).max_len(), MAX_MESSAGE_LEN);
    }

    #[test]
    fn frame_and_feed_agree() {
        let mut f = Framer::new();
        let body = vec![0u8, 1, 2, 3, 255];
        assert_eq!(f.feed(&frame(&body)).unwrap(), vec![body]);
    }
}
