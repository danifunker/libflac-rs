//! A pure-Rust port of the **libFLAC 1.4.3** encoder, built to produce frame
//! output **byte-identical** to the C reference at the configuration CHD/MAME
//! uses: compression level 8, 2-channel / 16-bit / 44.1 kHz, streamable subset
//! off, MD5 off, explicit block size (typically 2048).
//!
//! This is bit-exactness-first work: a "working" encoder that emits valid FLAC is
//! **not** the goal if it differs from the C output by a single byte. The crate
//! is continuously differential-tested against the real libFLAC, compiled from
//! source as a dev-only oracle (the `cref` feature) — see `CLAUDE.md` and
//! `ROADMAP.md` for the milestone status.
//!
//! The library itself is pure Rust with zero runtime dependencies; the C
//! reference is only ever compiled as a test oracle and is excluded from the
//! published crate, so consumers (e.g. `chd-rs`) get a dependency-free library.

#![forbid(unsafe_code)]
// Scaffolding while the encoder is built up milestone by milestone; removed at F4
// once `encoder` wires everything into the public API.
#![allow(dead_code)]

// Modules are added milestone by milestone:
//   F0  bitwriter + crc            (bit packing, CRC-8/CRC-16)
//   F1  fixed + subframe + frame   (CONSTANT/VERBATIM/FIXED, header/footer)
//   F2  window + lpc/*             (the float-parity gate)
//   F3  rice + mid-side            (partition search, channel assignment)
//   F4  encoder                    (public API, level-8 preset wiring)

mod bitwriter;
mod crc;
mod encoder;
mod fixed;
mod format;
mod frame;
mod rice;
mod subframe;

/// Internals exposed **only** for the differential tests (`--features cref`). Not
/// part of the public API and carries no stability guarantee; absent from the
/// published crate (the feature is dev-only).
#[cfg(feature = "cref")]
#[doc(hidden)]
pub mod testing {
    pub use crate::bitwriter::BitWriter;
    pub use crate::crc::{crc8, crc16};
    pub use crate::encoder::encode_frames;
}
