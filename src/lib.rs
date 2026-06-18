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
//   F2  window + lpc/* + bitmath   (the LPC float-parity gate)
//   F3  mid-side                   (channel assignment by estimated bits)
//   F4  encoder                    (public API, level-8 preset wiring)

mod bitmath;
mod bitreader;
mod bitwriter;
mod crc;
mod decoder;
mod encoder;
mod fixed;
mod format;
mod frame;
mod lpc;
mod md5;
mod metadata;
mod ogg;
mod rice;
mod subframe;
mod window;

/// Internals exposed **only** for the differential tests (`--features cref`). Not
/// part of the public API and carries no stability guarantee; absent from the
/// published crate (the feature is dev-only).
#[cfg(feature = "cref")]
#[doc(hidden)]
pub mod testing {
    pub use crate::bitwriter::BitWriter;
    pub use crate::crc::{crc8, crc16};
    pub use crate::decoder::{
        DecodedFrames, DecodedStream, SeekResult, decode, decode_frames, decode_ogg, decode_seek,
    };
    pub use crate::encoder::{Apodization, Config, encode, encode_frames, encode_ogg, preset};
    pub use crate::md5::audio_md5;
    pub use crate::metadata::{
        CueSheetIndex, CueSheetTrack, LIBFLAC_VENDOR_STRING, MetadataBlock, SeekPoint,
        seektable_sort, spaced_seek_points,
    };

    /// Apodization windows, re-exported for per-element differential testing.
    pub mod window {
        pub use crate::window::tukey;
    }

    /// LPC float-pipeline stages, re-exported for stage-wise differential testing.
    pub mod lpc {
        pub use crate::lpc::{
            LpCoefficients, Quantized, compute_autocorrelation, compute_best_order,
            compute_lp_coefficients, compute_residual, expected_bits, quantize_coefficients,
            window_data, window_data_partial,
        };
    }
}
