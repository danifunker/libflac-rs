# libflac-rs

A pure-Rust, **bit-exact** port of [libFLAC](https://xiph.org/flac/) 1.4.3 — a FLAC
encoder (and, in progress, decoder) whose output is **byte-identical** to the C
reference. Unlike a "produces valid FLAC" encoder, the goal is to *recreate the
exact bytes* libFLAC/MAME produce, so tools like `chd-rs` can reproduce and verify
CHD files losslessly. (`flacenc`, the existing pure-Rust encoder, is **not**
byte-identical to libFLAC — which is precisely why this exists.)

## Status

Built milestone by milestone, each verified **byte-for-byte against the real
libFLAC** compiled from source (the dev-only `cref` feature):

- ✅ Bitwriter + CRC-8/16
- ✅ CONSTANT / VERBATIM / FIXED subframes, framing, the integer Rice partition search
- 🚧 LPC: windows → autocorrelation → Levinson → quantization — the float-parity gate
- ⬜ Mid-side; all bit depths / compression levels / apodization windows; metadata;
  the decoder; Ogg FLAC

The first target is the configuration MAME's CHD codec uses (level 8, 16-bit,
44.1 kHz, stereo); the port then generalizes to the full codec. See
[`CLAUDE.md`](CLAUDE.md) for the architecture and bit-exactness notes.

## Pure Rust, zero dependencies

The library is `#![forbid(unsafe_code)]` with **no runtime dependencies**. The C
libFLAC is compiled **only** as a test oracle under the `cref` feature and is
excluded from the published crate, so consumers get a dependency-free library.

```sh
cargo test                  # pure-Rust unit tests (any platform)
cargo test --features cref  # differential vs the compiled C oracle (Linux/glibc)
```

Bit-exactness is verified on **glibc**: its libm (`cosf`, `lround`) is the parity
target — it is what MAME/chdman use — so the differential tests run there, while
the pure-Rust build is checked on Linux, Windows, and macOS.

## License

BSD-3-Clause, retaining the Xiph.Org copyright on the ported libFLAC sources. See
[`LICENSE`](LICENSE).
