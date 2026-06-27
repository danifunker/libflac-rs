# Differential testing against the C oracle

This crate is a **bit-exact** port of libFLAC 1.4.3: its output is byte-identical to
the C reference. That guarantee was established and continuously checked by a
**differential test harness** — it compiled the real libFLAC (and libogg, for Ogg
FLAC) from source as an *oracle* and diffed the Rust output against the C output
byte-for-byte.

To keep the repository and the published crate **100% pure Rust** — no vendored C, no
build script, no build dependencies — that harness has been **removed from the
working tree**. It is not gone: every file is preserved in git history. This document
is the durable record of what the oracle was, how it was built, and exactly how to
re-establish it to re-verify byte-exactness after changing the encoder/decoder.

> **You only need this if you change code that can affect output bytes** (the LPC
> float pipeline, framing, rice coding, metadata, Ogg paging, …). Pure refactors and
> the lossless round-trip tests in `src/**` do not require the oracle. But any change
> that *could* move a byte should be re-checked against it before release.

---

## 1. What the oracle is

| Component | Upstream | Version / tag | Role |
| --- | --- | --- | --- |
| libFLAC | [xiph/flac](https://github.com/xiph/flac) | `1.4.3` | the encoder/decoder reference (scalar path only) |
| libogg | [xiph/ogg](https://github.com/xiph/ogg) | `v1.3.5` | Ogg page framing (libFLAC delegates all Ogg paging to it) |

Both are BSD-3-Clause (Xiph.Org). The oracle was compiled with the `cc` crate from a
`build.rs`, behind an off-by-default `cref` Cargo feature, and driven through a small
FFI shim (`cref/shim.c`). The integration test `tests/cref.rs` diffed Rust vs C.

### The float-parity environment matters

Build and run the oracle under **glibc/gcc** (on Windows: WSL). glibc's libm is the
parity target — it is what MAME/chdman use — so `f32::cos` resolves to the same
`cosf`, and `f64::round() as i32` to the same `lround` (round-half-away-from-zero,
C99, via `HAVE_LROUND=1`). A different libm can diverge in the windowing /
quantization float math. The oracle is also built `FLAC__NO_ASM`, which is
deterministic and platform-independent and matches x86 chdman (see
[`CLAUDE.md`](CLAUDE.md) → "SIMD vs scalar").

---

## 2. Quick restore (from git — the normal path)

Everything was committed at the tag **`c-oracle`** (the commit just before the
removal). Restore the rig into the working tree:

```sh
git checkout c-oracle -- cref build.rs
```

Re-add the feature wiring to `Cargo.toml` (removed alongside the rig):

```toml
[features]
default = []
cref = ["dep:cc"]

[build-dependencies]
cc = { version = "1.2", optional = true }
```

Re-add the cref-only `testing` module to `src/lib.rs` and the one cref re-export in
`src/lpc/mod.rs` — both are at the `c-oracle` tag, so the simplest way is to restore
them too:

```sh
git show c-oracle:src/lib.rs     # copy the `#[cfg(feature = "cref")] pub mod testing { … }`
git show c-oracle:src/lpc/mod.rs # copy the `#[cfg(feature = "cref")] pub use quantize::Quantized;`
git checkout c-oracle -- tests/cref.rs
```

Then run the differential suite under glibc (WSL on Windows). A native Linux target
dir keeps `/mnt/c` builds fast:

```sh
# from a WSL shell, repo checked out under /mnt/c/…/libflac-rs
export CARGO_TARGET_DIR=$HOME/.cache/libflac-rs-target
cargo test --release --features cref
```

All differential tests passing == the Rust output is still byte-identical to
libFLAC/libogg. When done, `git checkout` / `git stash` the restore away to return to
the pure-Rust tree.

> Tip: to verify a *specific* change, restore the rig, run `cargo test --features
> cref` on `main`, then apply your change and run it again.

---

## 3. From-scratch re-vendor (if git history is unavailable)

If the `c-oracle` snapshot is somehow gone, the rig can be rebuilt from upstream.

### 3.1 Vendor the C sources

```sh
git clone --depth 1 --branch 1.4.3  https://github.com/xiph/flac.git /tmp/flac143
git clone --depth 1 --branch v1.3.5 https://github.com/xiph/ogg.git  /tmp/libogg
```

Copy into `cref/vendor/`:

- **libFLAC** → `cref/vendor/flac/` — keep `include/` (the `FLAC/`, `share/` public
  headers) and `src/libFLAC/` with its `include/` (the `private/`, `protected/`,
  `config.h`). Only these translation units are compiled (scalar path; no
  `*_intrin_*` / `*_asm_*`, no `metadata_object.c`):

  `stream_encoder.c`, `stream_encoder_framing.c`, `lpc.c`, `fixed.c`, `window.c`,
  `bitwriter.c`, `bitmath.c`, `crc.c`, `format.c`, `memory.c`, `float.c`, `md5.c`,
  `cpu.c`, `stream_decoder.c`, `bitreader.c`, and the Ogg layer
  `ogg_encoder_aspect.c`, `ogg_decoder_aspect.c`, `ogg_helper.c`, `ogg_mapping.c`.

- **libogg** → `cref/vendor/ogg/` — `src/framing.c`, `src/bitwise.c`,
  `src/crctable.h`, `include/ogg/ogg.h`, `include/ogg/os_types.h`.

### 3.2 Two generated headers libogg's build normally creates

libogg's `os_types.h` falls through to `#include <ogg/config_types.h>` on generic
platforms, and `framing.c`/`bitwise.c` do `#include "config.h"` under
`HAVE_CONFIG_H`. Neither ships in the tarball — provide them so the build is
self-contained (do **not** rely on a system `libogg-dev`):

`cref/vendor/ogg/include/ogg/config_types.h`:

```c
#ifndef __CONFIG_TYPES_H__
#define __CONFIG_TYPES_H__
#include <stdint.h>
typedef int16_t  ogg_int16_t;
typedef uint16_t ogg_uint16_t;
typedef int32_t  ogg_int32_t;
typedef uint32_t ogg_uint32_t;
typedef int64_t  ogg_int64_t;
typedef uint64_t ogg_uint64_t;
#endif
```

`cref/vendor/ogg/src/config.h` — an empty stub (a comment is enough). It shadows
libFLAC's `config.h` via quote-include resolution (the file's own directory is
searched first), so libogg does not pick up libFLAC's.

### 3.3 The build script (`build.rs`)

Compile everything into one archive with the `cc` crate, gated on the `cref` feature.
The exact macro set mirrors MAME's libFLAC build (`scripts/src/3rdparty.lua`) plus
`FLAC__HAS_OGG=1`:

```rust
let mut build = cc::Build::new();
build
    .include("cref/vendor/flac/include")
    .include("cref/vendor/flac/src/libFLAC/include")
    .include("cref/vendor/ogg/include")
    .define("HAVE_CONFIG_H", None)
    .define("FLAC__NO_ASM", None)      // scalar reference path (matches x86 chdman)
    .define("FLAC__HAS_OGG", "1")      // runtime-gated on is_ogg; native output unchanged
    .define("OGG_FOUND", "1")
    .define("NDEBUG", None)
    .define("HAVE_LROUND", "1")        // glibc lround == f64::round half-away
    .define("ENABLE_64_BIT_WORDS", "1")
    .define("HAVE_INTTYPES_H", None)
    .define("HAVE_STDBOOL_H", None)
    .define("HAVE_STDINT_H", None)
    .define("HAVE_STDIO_H", None)
    .define("HAVE_STDLIB_H", None)
    .define("HAVE_STRING_H", None)
    .warnings(false);
// add the libFLAC + Ogg-layer .c files (list in §3.1), then libogg framing.c/bitwise.c,
// then cref/shim.c.
build.cargo_metadata(false);
build.compile("flac_cref");
```

The shim symbols must reach the **integration-test** binary, which build-script
`rustc-link-lib` does not do reliably; pass the archive straight to the test linker,
whole, so every object is pulled in:

```rust
let out = std::env::var("OUT_DIR").unwrap();
println!("cargo:rustc-link-arg-tests=-Wl,--whole-archive");
println!("cargo:rustc-link-arg-tests={out}/libflac_cref.a");
println!("cargo:rustc-link-arg-tests=-Wl,--no-whole-archive");
```

`FLAC__HAS_OGG=1` is safe for the native (non-Ogg) tests: the Ogg code is fully
runtime-gated on the encoder's `is_ogg` flag, so native output is byte-identical to a
`HAS_OGG=0` build (the native differential tests confirmed this).

### 3.4 The FFI shim (`cref/shim.c`)

A thin C layer that drives the libFLAC encoder/decoder and captures bytes for the
diff. Entry points the Rust tests called:

| Symbol | Purpose |
| --- | --- |
| `…_encode` / `…_encode_cfg` | raw audio frames (metadata stripped, as CHD does); staging knobs `max_lpc_order` / `do_mid_side` (`<0` = preset) and a `compression_level` knob |
| `…_encode_full` | a complete `.flac` stream (seekable sink → STREAMINFO rewritten at finish) |
| `…_encode_full_app` / `…_full_picture` / `…_full_seektable` / `…_full_cuesheet` | full streams with one manually-filled metadata block, to diff each block type |
| `…_encode_ogg` | a complete Ogg FLAC stream (`init_ogg_stream`, fixed serial) |
| `…_decode` | decode a stream back to PCM via the C decoder (round-trip proof) |
| `…_md5` / `…_vendor_string` / `…_crc8` / `…_crc16` | leaf checks |
| F2 leaf fns: `…_window_tukey`, `…_lpc_window_data[_partial]`, `…_compute_autocorrelation`, `…_compute_lp_coefficients`, `…_compute_best_order`, `…_quantize_coefficients`, `…_compute_residual` | stage-by-stage float-pipeline diffs |

The Rust side exposed the internals these tests need under a `#[cfg(feature =
"cref")] #[doc(hidden)] pub mod testing { … }` re-export block in `src/lib.rs`
(`encode`/`encode_frames`/`encode_ogg`/`decode*`/`preset`/`Config`, the metadata
types, and the LPC/window/crc/md5 leaf functions).

---

## 4. How the comparison was staged

Bring up parity field-by-field, diffing at the first mismatching byte to localize
float drift: **frame header → subframe header → `qlp_coeff` integers →
residual/rice → CRC-16**. The corpus: silence (→ CONSTANT), full-scale ±max, sine
sweeps (low → near-Nyquist), decorrelated noise (exercises mid-side), and real 16-bit
CD rips. The decoder is checked differently — not byte-parity but **lossless
round-trip** (`decode(encode(pcm)) == pcm`, MD5 verified) plus **decoding real
libFLAC output**.

The specific bit-exactness hazards the oracle pinned down (scalar `double`
autocorrelation with no FMA, the `lround` half-away rounding, the `frexp`/`ilog2`
derivations, the per-frame mid-side decision, the 32-bit `i64` paths, the libogg
`pageout` flush heuristic + CRC-32, …) are documented in [`CLAUDE.md`](CLAUDE.md)
("Bit-exactness hazards" and the per-phase notes). Keep those accurate when porting.

---

## 5. After re-verifying

Once the differential suite is green for your change, revert the rig so the tree is
pure Rust again:

```sh
git checkout -- cref build.rs Cargo.toml src/lib.rs src/lpc/mod.rs
rm -f tests/cref.rs          # if you restored it
```

(Or just `git stash` the whole restore.) The published crate never contained any of
this — `cref/` and `build.rs` were always excluded from the package — so removing it
from the tree changes nothing a consumer sees; it only takes the *automated*
byte-exactness check offline in favor of this on-demand process.
