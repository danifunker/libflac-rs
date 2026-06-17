# ROADMAP — `libflac-rs`: a bit-exact libFLAC 1.4.3 encoder in Rust

> **Status:** the original goal is **met and exceeded.** The CHD/MAME configuration
> (level 8, 2 ch / 16-bit / 44.1 kHz, subset off, fixed block size) is byte-identical
> to the C reference **end-to-end**, and the port has since generalized to **all
> compression levels (0–8)**, **all common bit depths (8/12/16/20/24)**, and **complete
> `.flac` files** (STREAMINFO + MD5 + VORBIS_COMMENT + PADDING) that are byte-identical
> to libFLAC's default output. Remaining: 32-bit, extra metadata blocks, a standalone
> decoder, the public API, and publish. See [§9 Milestones](#9-milestones).

This document is modeled on the original project handoff and updated to mark what is
done. The companion [CLAUDE.md](CLAUDE.md) holds the working-session detail; this file
is the authoritative status + forward plan. **Every "done" below means byte-identical to
the compiled C oracle** (the `cref` feature) across the test corpus — not merely "valid
FLAC."

Legend: ✅ done & differentially verified · 🟡 partially done · ⬜ not started.

---

## 1. Goal & success criterion — ✅ ACHIEVED

A pure-Rust port of the **libFLAC 1.4.3 encoder** whose frame output is **byte-identical**
to libFLAC. This was the trickiest of the planned ports because the encoder makes its
decisions in **floating point** (windows, autocorrelation, LPC quantization) where
rounding must match the C reference exactly — [§6](#6-bit-exactness-hazards--all-resolved)
is the record of how each float hazard was pinned down.

**Done =** for every PCM input in the corpus, the encoded FLAC frames (and, now, full
streams with metadata) are byte-identical to libFLAC's output for the same settings.

- ✅ **Frames** byte-identical at the CHD config (level 8, 2 ch / 16-bit / 44.1 kHz).
- ✅ **Generalized**: byte-identical across **levels 0–8**, **mono + stereo**, and bit
  depths **8 / 12 / 16 / 20 / 24**.
- ✅ **Full files**: marker + STREAMINFO + VORBIS_COMMENT (+ optional PADDING) + frames
  byte-identical to libFLAC's *default* output; streams round-trip through the real
  libFLAC **decoder** back to the original PCM.
- 🟡 **32-bit** input (the 33-bit side channel / wide-residual paths) is the one
  remaining depth.

---

## 2. Provenance & license

- Upstream: libFLAC **1.4.3** (Xiph.Org). Vendored self-contained at
  [`cref/vendor/flac`](cref/vendor/flac) (so CI and a fresh checkout build the oracle
  with no external tree); the sibling MAME checkout at `../mame/3rdparty/flac` is the
  fallback, overridable via `FLAC_C_DIR`.
- License: **BSD-3-Clause** — every `src/libFLAC/*.c` carries the Xiph BSD-3 header
  (© 2000–2009 Josh Coalson, © 2011–2023 Xiph.Org). The `COPYING.GPL`/`COPYING.LGPL`
  files cover only the **CLI tools**, not the library; nothing was read or ported from
  them. The port is **BSD-3-Clause**, retaining the Xiph copyright (see [LICENSE](LICENSE)).
- The vendored C and `build.rs`/`cref/` are **dev-only** (the `cref` feature) and are
  excluded from the published crate, so consumers get a pure-Rust, zero-dependency library.

---

## 3. Source files ported (encoder, scalar reference path only)

| C file | Rust module(s) | Role | Status |
| --- | --- | --- | --- |
| `bitwriter.c`, `crc.c` | `bitwriter` | bit packing, CRC-8 / CRC-16 | ✅ |
| `fixed.c` | `fixed` | fixed predictors (orders 0–4) + best-order | ✅ |
| `stream_encoder_framing.c` | `frame`, `subframe` | frame/subframe headers + footer | ✅ |
| `window.c` | `window` | apodization (`tukey`, `subdivide_tukey`) | ✅ |
| `lpc.c` | `lpc/{autocorr,levinson,quantize,residual}` | window→autocorrelation→Levinson→quantize→residual | ✅ |
| `bitmath.c` | `bitmath` | `ilog2` / `silog2` | ✅ |
| `stream_encoder.c` | `encoder`, `rice` | orchestration, partition search, mid-side, presets | ✅ |
| `format.c` | `format` (constants) | header / validation constants | ✅ |
| `md5.c` | `md5` | RFC 1321 + little-endian sample serialization | ✅ |
| `metadata` (STREAMINFO/VORBIS_COMMENT/PADDING) | `metadata` | metadata blocks | ✅ |
| `memory.c` | — | replaced by Rust ownership | ✅ |

**Intentionally omitted:** every `*_intrin_*` / `*_asm_*` translation unit and the
`deduplication/*_intrin_*` autocorrelation variants — only the **scalar** reference path
is ported (see the SIMD note in [§6](#6-bit-exactness-hazards--all-resolved)). The
libFLAC **decoder** is currently linked only inside the oracle (its verify / round-trip
path); a standalone Rust decoder is [D1](#9-milestones), not yet started.

---

## 4. Exact settings CHD uses — ✅ verified

`flac.cpp:77` / `chdcodec.cpp`: `set_verify(false)` (MD5 off), `set_compression_level(8)`,
`set_channels(2)`, `set_bits_per_sample(16)`, `set_sample_rate(44100)`,
`set_total_samples_estimate(0)`, `set_streamable_subset(false)`,
`set_blocksize(block_size)` (typically **2048**).

**Level-8 preset** (`stream_encoder.c:132`, last row): `do_mid_side=true`,
`loose_mid_side=false`, `max_lpc_order=12`, `qlp_coeff_precision=0` (auto → **11** at
blocksize 2048 / 16-bit), `do_qlp_coeff_prec_search=false`, `do_escape_coding=false`,
`do_exhaustive_model_search=false`, `min_residual_partition_order=0`,
`max_residual_partition_order=6`, `rice_parameter_search_dist=0`, apodization
`"subdivide_tukey(3)"`. All reproduced exactly in `encoder::preset(8)` and verified.

CHD's outer wrapper — the `'L'`/`'B'` endian-flag byte, the both-endian trial, and the CD
subcode→deflate split — is **out of scope** for the library (it belongs in `chd-rs`); see
[F4](#9-milestones).

---

## 5. Rust module layout — actual

```
libflac-rs/
  src/
    lib.rs          // crate root; gates the dev-only `testing` module under `cref`
    bitwriter.rs    // ✅ bit packing + CRC-8/CRC-16 (bitwriter.c, crc.c)
    crc.rs          // ✅ CRC-8 (header) / CRC-16 (footer) tables
    bitmath.rs      // ✅ ilog2 / silog2
    format.rs       // ✅ bitstream field lengths + type masks
    window.rs       // ✅ tukey + subdivide_tukey's Tukey window (exact cosf path)
    lpc/
      mod.rs        // ✅ windowing (window_data / _partial) + re-exports
      autocorr.rs   // ✅ double-precision autocorrelation (scalar template, no FMA)
      levinson.rs   // ✅ Levinson-Durbin + best-order + expected-bits (double)
      quantize.rs   // ✅ qlp quantization (lround-equivalent + error feedback)
      residual.rs   // ✅ FIR residual (narrow + wide/limit paths)
    fixed.rs        // ✅ fixed predictors 0..4, scalar
    rice.rs         // ✅ partition-order search + RICE/RICE2 parameter estimation
    subframe.rs     // ✅ CONSTANT/VERBATIM/FIXED/LPC writers + bit estimates
    frame.rs        // ✅ frame header/footer, channel-assignment codes
    md5.rs          // ✅ audio MD5 (RFC 1321 + format_input_ serialization)
    metadata.rs     // ✅ STREAMINFO + VORBIS_COMMENT + PADDING
    encoder.rs      // ✅ orchestration: presets, apodization state machine,
                    //    mid-side decision, frame/stream assembly
  cref/             // dev-only C oracle (vendored libFLAC 1.4.3 + FFI shim)
  tests/cref.rs     // ✅ byte-for-byte differential tests vs the oracle
```

---

## 6. Bit-exactness hazards — ALL RESOLVED

Each item below is where a careless port silently diverges. Every one is now matched
**bit-for-bit in IEEE-754**, verified against an exposed C leaf function (`cref/shim.c`).

1. ✅ **Apodization windows** (`window.c`): coefficients via `cosf`/`fabsf` (f32).
   Reproduced the exact float expressions and evaluation order (`M_PI * n / Np` in
   `double`, cast to `f32`, then `cosf`). **Confirmed `f32::cos` == glibc `cosf`**
   per-coefficient under WSL/glibc — no embedded tables needed.
2. ✅ **Autocorrelation** (`lpc.c:133` template): accumulated in **`double`**, operands
   promoted from `f32`, plain `*` then `+` (**no `mul_add`/FMA**), exact loop order.
3. ✅ **Levinson-Durbin** (`lpc.c:176`): reflection coefficient `r /= err` in `double`,
   the early `if(err == 0.0)` exit, round-to-nearest-even.
4. ✅ **LPC quantization** (`lpc.c:~265`): `q = lround(error); error -= q;` with error
   feedback. **Correction to the original handoff:** the oracle is built `HAVE_LROUND=1`,
   so it uses glibc `lround`, which per C99 rounds **half away from zero** (*not*
   half-to-even — that's `rint`/`nearbyint`). Rust's `f64::round` is also half-away, so
   `error.round() as i32` matches exactly. Verified against the oracle.
5. ✅ **Auto qlp precision** (`lpc.c` / `stream_encoder.c:704`): the blocksize/bps table
   (→ **11** for 2048/16-bit) plus the `min(.., 32 - subframe_bps - ilog2(order))` clamp,
   with `subframe_bps = 17` on the side channel. `ilog2`/`silog2` ported in `bitmath`.
6. ✅ **Rice partition search** (`stream_encoder.c:~4089`): integer; tie-breaks (higher
   order evaluated first, strict-improvement replacement) match. `do_escape_coding` is
   false at level 8. **RICE2** (5-bit params) is selected per the C rule — type upgraded
   to RICE2 iff any partition parameter reaches the escape value 15 — driving the
   `bps>16 ? 31 : 15` parameter limit; the bit *estimate* keeps the 4-bit width as the C
   does ("err on side of 16bps").
7. ✅ **Mid-side decision** (`stream_encoder.c:~3265`, `loose_mid_side=false`): per-frame
   L/R vs L/S vs R/S vs M/S by summed estimated bits (side at +1 bps), independent
   preferred on ties; plus the `loose_mid_side` periodic-redecision mode (levels 1 & 4).
8. ✅ **`f32` (`FLAC__real`) vs `f64`**: each variable's type matched precisely — coeffs
   stored `f32`, autocorrelation accumulated `f64`.

> **SIMD vs scalar — RESOLVED: scalar is the correct target.** MAME compiles libFLAC
> **without `FLAC__USE_AVX`**, so the FMA/AVX2 autocorrelation is compiled out; chdman's
> dispatch lands on the SSE2 `double` path, which is bit-identical to the scalar template
> (verified 60/60 across randomized signals). The oracle is built `FLAC__NO_ASM` (scalar,
> deterministic, platform-independent) and matches x86 chdman. The original handoff's
> worry that SIMD "uses 24-bit intermediates" does not apply to this build. *Caveat:*
> CHDs created on ARM (NEON has FMA) could differ; the x86 scalar path is canonical.

---

## 7. Differential testing — ✅ the rig

`build.rs` + [`cref/shim.c`](cref/shim.c) compile the vendored libFLAC under the `cref`
feature and expose it to [`tests/cref.rs`](tests/cref.rs), which diffs Rust output against
the C **byte-for-byte**.

- **Full-encoder entry points** with staging knobs: `libflac_rs_cref_encode`
  (`max_lpc_order`/`do_mid_side` `< 0` keep the preset, `>= 0` override — e.g.
  `max_lpc_order = 0` forces fixed/constant/verbatim only); `..._encode_cfg`
  (compression level); `..._encode_full` (seekable full stream); `..._decode`
  (round-trip through libFLAC's decoder); `..._vendor_string`; `..._md5`.
- **Leaf-function wrappers** for stage-wise localization of float drift: `window_tukey`,
  `lpc_window_data[_partial]`, `compute_autocorrelation`, `compute_lp_coefficients`,
  `compute_best_order`, `expected_bits`, `quantize_coefficients`, `compute_residual` —
  so a divergence is pinned to a single stage rather than the final bytes.
- **Corpus:** silence (→ CONSTANT), full-scale ±max, sine sweeps (low → near-Nyquist),
  decorrelated noise (exercises mid-side), short final frames, every wasted-bits count,
  multi-blocksize, and per-bit-depth multi-partial+noise signals — compared in stages
  (frame header → subframe header → qlp_coeff → residual/rice → CRC-16).
- **Linkage:** the shim archive is passed whole (`--whole-archive`) to the *test* linker
  via `cargo:rustc-link-arg-tests`, since build-script `rustc-link-lib` doesn't reach
  integration-test binaries.
- Build/run in **WSL/glibc** so Rust's `f32`/`f64` transcendentals and the oracle's libm
  resolve identically; CI runs the pure-Rust suite on Linux/Windows/macOS plus the
  Linux/glibc differential + lint.

---

## 8. Public API — ⬜ F4 (pending)

Today the encoder is reachable only through the dev-only `testing` module
(`encode`, `encode_frames`, `preset`, `Config`, `Apodization`) behind the `cref` feature;
there is **no stable public API yet**. F4 introduces one — a small config-struct-based
surface rather than long argument lists — roughly:

```rust
pub struct FlacEncoder { /* config (level, channels, bps, sample_rate, blocksize, …) */ }
impl FlacEncoder {
    pub fn new_chd(block_size: u32) -> Self;          // 2ch/16-bit/44.1k, level 8, subset off
    pub fn with_config(cfg: Config) -> Self;          // general
    pub fn encode_interleaved(&mut self, samples: &[i32]) -> Vec<u8>; // raw frames
    pub fn finish(self) -> Vec<u8>;                   // + STREAMINFO/metadata for full files
}
```

`chd-rs` wraps this for the raw `flac` codec (the `'L'`/`'B'` endian byte + both-endian
trial) and the `cdfl` codec (audio→FLAC, subcode→deflate split).

---

## 9. Milestones

### Core — the CHD/MAME target

- [x] **F0** — Bitwriter + CRC-8/16; frame skeleton bytes. ✅
- [x] **F1** — CONSTANT/VERBATIM/FIXED subframes + framing (header/footer, UTF-8 frame
  number, block-size hints) + integer Rice partition search; byte-exact fixed-only. ✅
- [x] **F2** — Windows + autocorrelation + Levinson + quantization + residual — **the
  float-parity gate**; byte-exact LPC subframes on sine/noise. ✅
- [x] **F3** — Per-frame mid-side decision; **full corpus byte-exact at the CHD level-8
  config → the first target is complete end-to-end.** ✅
- [ ] **F4** — Public API ([§8](#8-public-api---f4-pending)), docs, publish. (CD
  subcode split + endian-trial wrapper optional / left to `chd-rs`.) ⬜

### Generalization — beyond the CHD slice

- [x] **G1** — All compression levels **0–8** (`Config`/`preset`, `tukey(p)` +
  `subdivide_tukey(parts)`, per-level order/partition caps, `loose_mid_side`). ✅
- [x] **G2** — Metadata + MD5: STREAMINFO, audio MD5, full streams that round-trip
  through libFLAC's decoder. ✅
- [x] **G2+** — VORBIS_COMMENT (vendor) + PADDING: **entire stream byte-identical to
  libFLAC's default output.** ✅
- [🟡] **G3** — Wider bit depths:
  - [x] **8 / 12 / 16 / 20 / 24-bit** — RICE2 above 16 bps; frames + full streams
    byte-exact across levels. ✅
  - [ ] **32-bit** — the 33-bit side channel (`side = L−R` / `mid = (L+R)>>1` overflow
    `i32`) and wide-residual paths (`integer_signal_33bit_side`,
    `_wide`/`_limit_residual`, `window_data_wide`). ⬜
- [ ] **G4** — Remaining metadata blocks: SEEKTABLE, APPLICATION, CUESHEET, PICTURE; a
  public way to pass user metadata. ⬜
- [ ] **D1** — A standalone **bit-exact decoder** (currently libFLAC's decoder is linked
  only as the oracle's verify/round-trip path). ⬜
- [ ] **Ogg FLAC** — optional. ⬜

---

## 10. Risk / effort — retrospective

The original assessment rated this **High** risk, with **floating-point parity** as the
real danger (windows and `lround` tie-breaking diverging from the reference libm). That
risk is now **retired**: under WSL/glibc, `f32::cos == cosf`, `f64::round` matches glibc
`lround` (half-away-from-zero), the scalar `double` autocorrelation matches MAME's
compiled-out-of-AVX SSE2 path, and every float stage is differentially verified leaf by
leaf. The fallback contingencies (embedded window tables, a fixed `lround` policy) proved
**unnecessary**.

What remains is comparatively low-risk, well-scoped engineering:

- **32-bit** (finish G3) — invasive but mechanical: thread an `i64` side channel and the
  wide/limit residual paths through `fixed`, `lpc::residual`, verbatim, and constant
  detection. The differential rig already covers it the moment the depth is enabled.
- **G4 metadata / D1 decoder / Ogg** — additive; the decoder is the largest single piece
  but is verifiable against the same vendored libFLAC.
- **F4 public API + publish** — packaging, docs, and a stable surface for `chd-rs`.

**Net:** the hard part (bit-exact float parity for the complete CHD encoder, then all
common configs and full files) is **done and on `main`**. The codebase is
`#![forbid(unsafe_code)]`, edition 2024, MSRV 1.85, zero runtime dependencies.
