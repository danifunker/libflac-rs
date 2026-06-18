//! `libflac-rs` — a pure-Rust, **bit-exact** port of **libFLAC 1.4.3**: a complete
//! FLAC **encoder and decoder** whose output is byte-identical to the C reference.
//!
//! It exists for consumers that must *recreate* the exact bytes libFLAC/MAME produce
//! (e.g. `chd-rs` reproducing MAME CHD audio), not merely emit valid FLAC — every
//! encoded byte is continuously differential-tested against the real libFLAC (and
//! libogg, for Ogg), compiled from source as a dev-only oracle.
//!
//! # What's supported
//! - **Encoder**, byte-identical to libFLAC: all compression levels 0–8, all bit
//!   depths (8/12/16/20/24/32), mono / stereo (with mid-side decorrelation) /
//!   multichannel, every metadata block (STREAMINFO, VORBIS_COMMENT, PADDING,
//!   APPLICATION, SEEKTABLE, PICTURE, CUESHEET), and the audio MD5.
//! - **Decoder**: lossless and MD5-verified, with [`decode_seek`] and
//!   variable-block-size support.
//! - **Ogg FLAC** ([`Encoder::encode_ogg`] / [`decode_ogg`]), byte-identical to
//!   libFLAC + libogg.
//! - Pure Rust, `#![forbid(unsafe_code)]`, **zero runtime dependencies**.
//!
//! # Encoding
//! ```
//! use libflac_rs::{Encoder, EncoderConfig};
//!
//! // 2-channel, 16-bit, 44.1 kHz, compression level 8 (libFLAC's defaults).
//! let enc = Encoder::new(EncoderConfig::new(2, 16, 44_100));
//! let pcm: Vec<i32> = vec![0; 4096 * 2]; // interleaved: L R L R …
//! let flac: Vec<u8> = enc.encode(&pcm);  // a complete .flac file
//! assert_eq!(&flac[..4], b"fLaC");
//! ```
//! For the raw frame stream MAME/CHD embeds, use [`EncoderConfig::chd`] with
//! [`Encoder::encode_frames`]; for Ogg FLAC, [`Encoder::encode_ogg`].
//!
//! # Decoding
//! ```
//! # use libflac_rs::{Encoder, EncoderConfig};
//! # let enc = Encoder::new(EncoderConfig::new(2, 16, 44_100));
//! # let pcm: Vec<i32> = vec![7; 4096 * 2];
//! # let flac = enc.encode(&pcm);
//! let decoded = libflac_rs::decode(&flac).expect("valid FLAC");
//! assert_eq!(decoded.interleaved, pcm);
//! assert!(decoded.md5_ok);
//! ```
//!
//! See `ROADMAP.md` for the bit-exactness methodology and milestone history.

#![forbid(unsafe_code)]

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

pub use decoder::{
    DecodedFrames, DecodedStream, SeekResult, decode, decode_frames, decode_ogg, decode_seek,
};
pub use encoder::{Encoder, EncoderConfig};
pub use metadata::{
    CueSheetIndex, CueSheetTrack, LIBFLAC_VENDOR_STRING, MetadataBlock, SeekPoint,
    spaced_seek_points,
};

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
