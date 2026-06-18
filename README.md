# libflac-rs

A pure-Rust, **bit-exact** port of [libFLAC](https://xiph.org/flac/) 1.4.3 — a
complete FLAC **encoder and decoder** whose output is **byte-identical** to the C
reference. Unlike a "produces valid FLAC" library, the goal is to *recreate the
exact bytes* libFLAC/MAME produce, so tools like `chd-rs` can reproduce and verify
CHD files losslessly. (`flacenc`, the existing pure-Rust encoder, is **not**
byte-identical to libFLAC — which is precisely why this exists.)

## Status — complete and byte-exact

Every byte is continuously verified **against the real libFLAC** (and libogg for
Ogg), compiled from source as a dev-only oracle (the `cref` feature):

- ✅ **Encoder**, byte-identical to libFLAC: compression levels **0–8**, bit depths
  **8/12/16/20/24/32**, mono / stereo (mid-side) / multichannel, the audio MD5, and
  every metadata block (STREAMINFO, VORBIS_COMMENT, PADDING, APPLICATION, SEEKTABLE,
  PICTURE, CUESHEET).
- ✅ **Decoder**: lossless and MD5-verified, with `seek()` and variable-block-size
  support.
- ✅ **Ogg FLAC** (encode + decode), byte-identical to libFLAC + libogg.
- ✅ Pure Rust, `#![forbid(unsafe_code)]`, **zero runtime dependencies**.

## Usage

```rust
use libflac_rs::{Encoder, EncoderConfig};

// 2-channel, 16-bit, 44.1 kHz, compression level 8 (libFLAC's defaults).
let enc = Encoder::new(EncoderConfig::new(2, 16, 44_100));
let pcm: Vec<i32> = vec![0; 4096 * 2]; // interleaved: L R L R …

let flac: Vec<u8> = enc.encode(&pcm);      // a complete .flac file
let frames: Vec<u8> = enc.encode_frames(&pcm); // raw frames (what MAME/CHD embeds)
let ogg: Vec<u8> = enc.encode_ogg(&pcm, 0);    // an Ogg FLAC stream

let decoded = libflac_rs::decode(&flac).unwrap();
assert_eq!(decoded.interleaved, pcm);
assert!(decoded.md5_ok);
```

For the exact configuration MAME's CHD codec uses (level 8, 2ch/16-bit/44.1 kHz,
MD5 off), construct with `EncoderConfig::chd(block_size)` and call `encode_frames`.

## Pure Rust, zero dependencies

The library is `#![forbid(unsafe_code)]` with **no runtime dependencies**. The C
libFLAC (and libogg) are compiled **only** as a test oracle under the `cref` feature
and are excluded from the published crate, so consumers get a dependency-free
library.

```sh
cargo test                  # pure-Rust unit tests (any platform)
cargo test --features cref  # differential vs the compiled C oracle (Linux/glibc)
```

Bit-exactness is verified on **glibc**: its libm (`cosf`, `lround`) is the parity
target — it is what MAME/chdman use — so the differential tests run there, while
the pure-Rust build is checked on Linux, Windows, and macOS.

## License

BSD-3-Clause, retaining the Xiph.Org copyrights on the ported libFLAC and libogg
sources. See [`LICENSE`](LICENSE).
