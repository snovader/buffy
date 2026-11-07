//! The tests' one model of tables: the conformance suite's descriptions,
//! which `cases` parses from `cases.json` and `strategy` generates,
//! with the builder and reader that every test builds and reads them with,
//! and the sizer and writer that write them in place.

#![allow(dead_code)]

mod build;
pub mod cases;
pub mod description;
pub mod read;
pub mod strategy;
mod write;

use proptest::test_runner::{Config, RngAlgorithm};

/// The number of random cases to run, which is fewer under Miri, which
/// interprets the tests to check for undefined behavior about 100 times
/// slower
const fn runs(cases: u32) -> u32 {
    if cfg!(miri) {
        cases.div_ceil(50)
    } else {
        cases
    }
}

/// Runs proptest's 256 cases of each property, or `runs()` of them under
/// Miri. Miri keeps tests from reading files, so failures aren't saved
/// there, and cases come from proptest's `XorShift` generator rather than
/// `ChaCha`, which is slower to seed: each choice between strategies seeds
/// a generator, and generating tables took Miri 5 times as long.
pub fn config() -> Config {
    let mut config = Config::with_cases(runs(256));
    if cfg!(miri) {
        config.failure_persistence = None;
        config.rng_algorithm = RngAlgorithm::XorShift;
    }

    config
}

/// Returns a copy of `bytes` that starts `byte_offset` bytes past a multiple
/// of 8 in a larger buffer of 0xff. Its arrays are aligned in memory when
/// `byte_offset` is a multiple of 8.
pub fn at_byte_offset(bytes: &[u8], byte_offset: usize) -> AtByteOffset {
    let mut backing = vec![0xff; 8 + byte_offset + bytes.len() + 8];
    let start = backing.as_ptr().align_offset(8) + byte_offset;
    backing[start..start + bytes.len()].copy_from_slice(bytes);

    AtByteOffset {
        backing,
        start,
        len: bytes.len(),
    }
}

pub struct AtByteOffset {
    backing: Vec<u8>,
    start: usize,
    len: usize,
}

impl AtByteOffset {
    pub fn bytes(&self) -> &[u8] {
        &self.backing[self.start..self.start + self.len]
    }
}
