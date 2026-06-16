# Vendored libFLAC 1.4.3 (differential-test oracle)

This is the subset of [libFLAC](https://xiph.org/flac/) **1.4.3**
(© 2000–2009 Josh Coalson, © 2011–2023 Xiph.Org Foundation, BSD-3-Clause) needed to
compile the encoder + decoder as the **differential-test oracle** for `libflac-rs`.

- Built **only** under the dev-only `cref` Cargo feature (see `../../../build.rs`),
  with `FLAC__NO_ASM` (the scalar reference path) and MAME's exact define set, so
  the oracle matches what MAME/chdman produce.
- **Excluded from the published crate** (`[package.exclude]` in `Cargo.toml`) and
  never linked into the shipped library — `libflac-rs` is pure Rust.
- Sources are the same tree CHD/chdman build (`mame/3rdparty/flac`). The
  `*_intrin_*` / `*_asm_*` SIMD variants, Ogg, and metadata-object translation
  units are omitted — not needed for the scalar reference path.

Set `FLAC_C_DIR` to override the oracle location (e.g. to point at an external
libFLAC checkout); otherwise `build.rs` uses this vendored copy.
