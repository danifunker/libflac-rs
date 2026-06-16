# ROADMAP

The ordered plan for finishing the bit-exact libFLAC 1.4.3 port. Detailed
per-milestone notes and the bit-exactness hazards live in [CLAUDE.md](CLAUDE.md);
this file is the forward-looking checklist. Every item is verified **byte-for-byte
against the compiled C oracle** (the `cref` feature) before it's considered done.

## Done

- **F0** bitwriter + CRC-8/16
- **F1** CONSTANT/VERBATIM/FIXED subframes, framing, integer Rice partition search
- **F2** LPC float pipeline (window → autocorrelation → Levinson → quantize → residual)
- **F3** mid-side channel decision — **the CHD/MAME level-8 target, end-to-end**
- **G1** all compression levels 0–8 (`Config`/`preset`, tukey + loose mid-side)
- **G2** metadata + MD5: STREAMINFO, VORBIS_COMMENT, PADDING — full `.flac` files
  byte-identical to libFLAC's default, round-trip-decodable

All of the above is **16-bit** (8/12-bit may already work; see G3).

## Next

- **G3 — wider bit depths (in progress).**
  - **Done: 8 / 12 / 16 / 20 / 24-bit.** RICE2 entropy coding (5-bit parameters,
    escape 31) above 16 bps via `RicePartition::is_rice2`, and the
    `bps > 16 ? 31 : 15` rice-parameter limit. Frames + full streams byte-exact.
  - **Remaining: 32-bit.** Needs the 33-bit side channel and wide residual paths
    (`integer_signal_33bit_side`, the `_wide` / `_limit_residual` LPC + fixed
    routines, `FLAC__lpc_window_data_wide`); `side = L-R` / `mid = (L+R)>>1`
    overflow `i32`, so the side channel must be `i64`.
- **G4 — remaining metadata.** SEEKTABLE and the other block types (APPLICATION,
  CUESHEET, PICTURE) as needed; a public way to pass user metadata.
- **D1 — decoder.** A standalone bit-exact FLAC decoder (currently libFLAC's
  decoder is only linked as the oracle's verify/round-trip path).
- **Ogg FLAC** (optional).
- **F4 — public API + publish.** A clean public API (a config struct rather than
  long argument lists), docs, CI polish, and publishing the crate.

## How to verify (always)

Build/test in WSL/glibc (`cargo test --features cref`); keep `cargo fmt` and
`cargo clippy --all-targets --features cref -- -D warnings` clean. Commit per
milestone via Windows git with explicit path staging (the vendored `cref/vendor`
tree is CRLF noise under WSL git — never `git add -A`).
