// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::types::coded::crosses_as_its_code;

crosses_as_its_code! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum LogLevel {
        Error = 1,
        Warn = 2,
        Info = 3,
        Debug = 4,
        Trace = 5,
    }
}

impl From<LogLevel> for log::LevelFilter {
    fn from(l: LogLevel) -> Self {
        match l {
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Trace => log::LevelFilter::Trace,
        }
    }
}

impl From<log::Level> for LogLevel {
    fn from(l: log::Level) -> Self {
        match l {
            log::Level::Error => LogLevel::Error,
            log::Level::Warn => LogLevel::Warn,
            log::Level::Info => LogLevel::Info,
            log::Level::Debug => LogLevel::Debug,
            log::Level::Trace => LogLevel::Trace,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRecord {
    pub seq: u64,
    pub level: LogLevel,
    pub target: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LogDrain {
    pub records: Vec<LogRecord>,
    pub dropped: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct LogConfig {
    pub level: LogLevel,
    pub capacity: usize,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            capacity: 1024,
        }
    }
}

const MIN_CAPACITY: usize = 16;

struct Ring {
    buf: VecDeque<LogRecord>,
    cap: usize,
    dropped: u64,
    seq: u64,
}

impl Ring {
    fn new(cap: usize) -> Self {
        let cap = cap.max(MIN_CAPACITY);
        Self {
            buf: VecDeque::with_capacity(cap),
            cap,
            dropped: 0,
            seq: 0,
        }
    }

    fn push(&mut self, level: LogLevel, target: String, message: String) {
        let seq = self.seq;
        self.seq = self.seq.wrapping_add(1);
        if self.buf.len() >= self.cap {
            self.buf.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.buf.push_back(LogRecord {
            seq,
            level,
            target,
            message,
        });
    }

    fn drain(&mut self) -> LogDrain {
        LogDrain {
            records: self.buf.drain(..).collect(),
            dropped: std::mem::take(&mut self.dropped),
        }
    }
}

static RING: OnceLock<Mutex<Ring>> = OnceLock::new();

struct BmLogger;
static LOGGER: BmLogger = BmLogger;

impl log::Log for BmLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        if let Some(ring) = RING.get() {
            ring.lock().unwrap().push(
                record.level().into(),
                record.target().to_string(),
                record.args().to_string(),
            );
        }
    }

    fn flush(&self) {}
}

pub fn install(config: LogConfig) -> bool {
    RING.get_or_init(|| Mutex::new(Ring::new(config.capacity)));
    let installed = log::set_logger(&LOGGER).is_ok();
    log::set_max_level(config.level.into());
    crate::log_library_loaded();
    installed
}

pub fn set_level(level: LogLevel) {
    log::set_max_level(level.into());
}

pub fn take_logs() -> LogDrain {
    match RING.get() {
        Some(ring) => ring.lock().unwrap().drain(),
        None => LogDrain::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_and_drops_oldest() {
        let mut ring = Ring::new(MIN_CAPACITY);
        for i in 0..(MIN_CAPACITY + 4) {
            ring.push(LogLevel::Info, "t".into(), format!("m{i}"));
        }
        let drain = ring.drain();
        assert_eq!(drain.records.len(), MIN_CAPACITY);
        assert_eq!(drain.dropped, 4);
        // the four oldest are gone, so the first surviving seq is 4
        assert_eq!(drain.records.first().unwrap().seq, 4);
        assert_eq!(drain.records.last().unwrap().message, "m19");
    }

    #[test]
    fn drain_resets_dropped_and_empties() {
        let mut ring = Ring::new(MIN_CAPACITY);
        for i in 0..(MIN_CAPACITY + 2) {
            ring.push(LogLevel::Warn, "t".into(), format!("m{i}"));
        }
        assert_eq!(ring.drain().dropped, 2);
        let second = ring.drain();
        assert!(second.records.is_empty());
        assert_eq!(second.dropped, 0);
    }

    #[test]
    fn seq_is_monotonic() {
        let mut ring = Ring::new(MIN_CAPACITY);
        for _ in 0..5 {
            ring.push(LogLevel::Debug, "t".into(), "m".into());
        }
        let seqs: Vec<u64> = ring.drain().records.iter().map(|r| r.seq).collect();
        assert_eq!(seqs, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn every_level_filters_at_its_own_level() {
        for (level, filter) in [
            (LogLevel::Error, log::LevelFilter::Error),
            (LogLevel::Warn, log::LevelFilter::Warn),
            (LogLevel::Info, log::LevelFilter::Info),
            (LogLevel::Debug, log::LevelFilter::Debug),
            (LogLevel::Trace, log::LevelFilter::Trace),
        ] {
            assert_eq!(log::LevelFilter::from(level), filter);
        }
    }
}
