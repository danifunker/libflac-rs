# libflac-rs

A pure-Rust, **bit-exact** port of [libFLAC](https://xiph.org/flac/) 1.4.3 — a
complete FLAC **encoder and decoder** whose output is **byte-identical** to the C
reference. Unlike a "produces valid FLAC" library, the goal is to *recreate the
exact bytes* libFLAC/MAME produce, so tools like `chd-rs` can reproduce and verify
CHD files losslessly. (`flacenc`, the existing pure-Rust encoder, is **not**
byte-identical to libFLAC — which is precisely why this exists.)

## Status — complete and byte-exact

Every byte was verified **against the real libFLAC** (and libogg for Ogg), compiled
from source as a differential oracle. That oracle is no longer vendored — the repo is
100% pure Rust — but the process to re-establish and re-run it is documented in
[`ORACLE.md`](ORACLE.md):

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

The crate is `#![forbid(unsafe_code)]`, edition 2024, with **no dependencies at all**
— not even a build script. There is no C in the repository.

```sh
cargo test   # pure-Rust unit + lossless round-trip tests (any platform)
```

Byte-exactness against the C reference is re-checked **on demand** by restoring the
differential oracle (vendored libFLAC + libogg, built under glibc/gcc — the libm
parity target MAME/chdman use). See [`ORACLE.md`](ORACLE.md) for the exact process.

## License

BSD-3-Clause, retaining the Xiph.Org copyrights on the ported libFLAC and libogg
sources. See [`LICENSE`](LICENSE).
