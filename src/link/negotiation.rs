// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey
//
//! Version negotiation
//!
//! A connection opens with a version exchange. Which side speaks first depends
//! on its role, not on who dialled: a server announces itself, a controller
//! waits and answers.
//!
//! The exchange carries no device id, so the engine cannot tell which peer sent
//! one. It is tracked per connection instead, like a [`crate::link::framing::Framer`].

use crate::codec::externals::bm_version::BMVersion;
use crate::codec::externals::handshake::Handshake;

use serde::{Deserialize, Serialize};

/// Which side of a connection speaks first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkRole {
    Initiator,
    Responder,
}

impl LinkRole {
    pub fn code(self) -> i32 {
        match self {
            LinkRole::Initiator => 0,
            LinkRole::Responder => 1,
        }
    }

    pub fn from_code(v: i32) -> Option<Self> {
        match v {
            0 => Some(LinkRole::Initiator),
            1 => Some(LinkRole::Responder),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionCheck {
    Compatible,
    LocalTooOld,
    RemoteTooOld,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandshakeOutcome {
    Passthrough,
    Received {
        current: BMVersion,
        minimum: BMVersion,
        check: VersionCheck,
        reply: Option<Vec<u8>>,
    },
}

/// Tracks the version exchange for one connection.
#[derive(Debug)]
pub struct Handshaker {
    role: LinkRole,
    local: Handshake,
    sent: bool,
    received: bool,
}

impl Handshaker {
    pub fn new(role: LinkRole) -> Self {
        Self::with_version(role, Handshake::default_version())
    }

    /// Announces a version other than the library's own, for tests and for
    /// standing in as an older build.
    pub fn with_version(role: LinkRole, local: Handshake) -> Self {
        Self {
            role,
            local,
            sent: false,
            received: false,
        }
    }

    pub fn role(&self) -> LinkRole {
        self.role
    }

    /// What to send once the connection is up. A responder sends nothing.
    pub fn on_connect(&mut self) -> Option<Vec<u8>> {
        if self.role != LinkRole::Initiator || self.sent {
            return None;
        }
        self.sent = true;
        Some(self.local.to_message().to_vec())
    }

    /// Sorts one message from the wire, answering it if an answer is owed.
    pub fn on_message(&mut self, message: &[u8]) -> HandshakeOutcome {
        let Some(remote) = Handshake::from_message(message) else {
            return HandshakeOutcome::Passthrough;
        };
        self.received = true;

        // Only answer if we have not spoken yet, or two openers would trade
        // versions forever.
        let reply = if self.sent {
            None
        } else {
            self.sent = true;
            Some(self.local.to_message().to_vec())
        };

        HandshakeOutcome::Received {
            current: remote.current,
            minimum: remote.minimum,
            check: self.compare(&remote),
            reply,
        }
    }

    fn compare(&self, remote: &Handshake) -> VersionCheck {
        if !self.local.current.is_at_least(&remote.minimum) {
            VersionCheck::LocalTooOld
        } else if !remote.current.is_at_least(&self.local.minimum) {
            VersionCheck::RemoteTooOld
        } else {
            VersionCheck::Compatible
        }
    }

    pub fn is_complete(&self) -> bool {
        self.sent && self.received
    }

    /// Forgets the exchange so a reconnect starts over.
    pub fn reset(&mut self) {
        self.sent = false;
        self.received = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn versions(cur: (u8, u8, u16), min: (u8, u8, u16)) -> Handshake {
        Handshake::new(
            BMVersion::new(cur.0, cur.1, cur.2),
            BMVersion::new(min.0, min.1, min.2),
        )
    }

    fn received(outcome: HandshakeOutcome) -> (VersionCheck, Option<Vec<u8>>) {
        match outcome {
            HandshakeOutcome::Received { check, reply, .. } => (check, reply),
            HandshakeOutcome::Passthrough => panic!("expected a version exchange"),
        }
    }

    #[test]
    fn an_initiator_opens_and_then_stays_quiet() {
        let mut h = Handshaker::new(LinkRole::Initiator);
        assert_eq!(h.on_connect().expect("an initiator opens").len(), 8);
        assert!(h.on_connect().is_none(), "it opens only once");

        let (_, reply) = received(h.on_message(&Handshake::default_version().to_message()));
        assert!(reply.is_none(), "having spoken, it owes no answer");
        assert!(h.is_complete());
    }

    #[test]
    fn a_responder_waits_and_answers_once() {
        let mut h = Handshaker::new(LinkRole::Responder);
        assert!(h.on_connect().is_none(), "a responder never opens");
        assert!(!h.is_complete());

        let (_, reply) = received(h.on_message(&Handshake::default_version().to_message()));
        assert_eq!(reply.expect("a responder answers").len(), 8);
        assert!(h.is_complete());

        let (_, second) = received(h.on_message(&Handshake::default_version().to_message()));
        assert!(second.is_none(), "it answers only once");
    }

    #[test]
    fn ordinary_messages_pass_through() {
        let mut h = Handshaker::new(LinkRole::Responder);
        // Length is the only thing that marks a version exchange.
        assert_eq!(h.on_message(&[0u8; 40]), HandshakeOutcome::Passthrough);
        assert_eq!(h.on_message(&[]), HandshakeOutcome::Passthrough);
        assert!(!h.is_complete());
    }

    #[test]
    fn two_openers_do_not_trade_versions_forever() {
        // A game and a registry server both announce themselves.
        let mut a = Handshaker::new(LinkRole::Initiator);
        let mut b = Handshaker::new(LinkRole::Initiator);
        let from_a = a.on_connect().unwrap();
        let from_b = b.on_connect().unwrap();

        assert!(received(a.on_message(&from_b)).1.is_none());
        assert!(received(b.on_message(&from_a)).1.is_none());
        assert!(a.is_complete() && b.is_complete());
    }

    #[test]
    fn a_server_and_a_controller_complete_in_order() {
        let mut server = Handshaker::new(LinkRole::Initiator);
        let mut controller = Handshaker::new(LinkRole::Responder);

        let opening = server.on_connect().expect("server opens");
        assert!(controller.on_connect().is_none());

        let answer = received(controller.on_message(&opening))
            .1
            .expect("controller answers");
        assert!(received(server.on_message(&answer)).1.is_none());
        assert!(server.is_complete() && controller.is_complete());
    }

    #[test]
    fn matching_versions_are_compatible() {
        let mut h = Handshaker::new(LinkRole::Responder);
        let (check, _) = received(h.on_message(&Handshake::default_version().to_message()));
        assert_eq!(check, VersionCheck::Compatible);
    }

    #[test]
    fn either_side_can_be_the_old_one() {
        // We are 1.7, they will not talk below 2.0.
        let mut ours =
            Handshaker::with_version(LinkRole::Responder, versions((1, 7, 0), (0, 9, 0)));
        let (check, _) = received(ours.on_message(&versions((2, 0, 0), (2, 0, 0)).to_message()));
        assert_eq!(check, VersionCheck::LocalTooOld);

        // We will not talk below 1.7, they are 0.9.
        let mut ours =
            Handshaker::with_version(LinkRole::Responder, versions((1, 7, 0), (1, 7, 0)));
        let (check, _) = received(ours.on_message(&versions((0, 9, 0), (0, 9, 0)).to_message()));
        assert_eq!(check, VersionCheck::RemoteTooOld);
    }

    #[test]
    fn the_shipped_pair_accepts_the_oldest_supported_build() {
        // 1.7.0 current, 0.9.0 minimum, against a peer at exactly 0.9.0.
        let mut ours = Handshaker::new(LinkRole::Responder);
        let (check, _) = received(ours.on_message(&versions((0, 9, 0), (0, 9, 0)).to_message()));
        assert_eq!(check, VersionCheck::Compatible);
    }

    #[test]
    fn the_build_number_is_not_compared() {
        let mut ours =
            Handshaker::with_version(LinkRole::Responder, versions((1, 7, 0), (1, 7, 0)));
        let (check, _) =
            received(ours.on_message(&versions((1, 7, 999), (1, 7, 999)).to_message()));
        assert_eq!(check, VersionCheck::Compatible);
    }

    #[test]
    fn reset_reopens_the_exchange() {
        let mut h = Handshaker::new(LinkRole::Initiator);
        h.on_connect().unwrap();
        h.on_message(&Handshake::default_version().to_message());
        assert!(h.is_complete());

        h.reset();
        assert!(!h.is_complete());
        assert!(h.on_connect().is_some(), "it opens again on reconnect");
    }
}
