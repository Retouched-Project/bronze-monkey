// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey
//
//! The link layer
//!
//! Everything a connection has to settle before anyone knows who is on the
//! other end: where messages begin and end, and which versions are in play.
//! None of it can belong to the engine, which addresses peers by device id and
//! so cannot speak until an identity exists.

pub mod crossdomain;
pub mod framing;
pub mod negotiation;

pub use framing::{Framer, LENGTH_PREFIX_LEN, MAX_MESSAGE_LEN, frame};
pub use negotiation::{HandshakeOutcome, Handshaker, LinkRole, VersionCheck};
