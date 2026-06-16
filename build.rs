//! Build script. Under the `cref` (C reference) feature, compiles the real
//! libFLAC 1.4.3 encoder (scalar reference path only) plus a small FFI shim so
//! tests can diff the Rust frame output against the C encoder byte-for-byte.
//! Does nothing for normal builds — the published crate excludes both this script
//! and `cref/`, so consumers get a pure-Rust, dependency-free library.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=cref/shim.c");
    println!("cargo:rerun-if-env-changed=FLAC_C_DIR");

    // Only touch the C toolchain when the differential rig is requested.
    if std::env::var_os("CARGO_FEATURE_CREF").is_none() {
        return;
    }

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    // Prefer the vendored libFLAC 1.4.3 (`cref/vendor/flac`) so CI and a fresh
    // checkout are self-contained; fall back to the sibling MAME tree if the
    // vendor dir is absent. Override either with FLAC_C_DIR (a libFLAC source tree
    // containing include/FLAC/ and src/libFLAC/).
    let flac = std::env::var_os("FLAC_C_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let vendored = manifest.join("cref").join("vendor").join("flac");
            if vendored
                .join("src")
                .join("libFLAC")
                .join("stream_encoder.c")
                .exists()
            {
                vendored
            } else {
                manifest
                    .join("..")
                    .join("mame")
                    .join("3rdparty")
                    .join("flac")
            }
        });

    let libflac = flac.join("src").join("libFLAC");
    if !libflac.join("stream_encoder.c").exists() {
        panic!(
            "feature `cref` is enabled but the libFLAC sources were not found at {}.\n\
             Point FLAC_C_DIR at a libFLAC 1.4.3 checkout containing src/libFLAC/.",
            libflac.display()
        );
    }

    // Expose the C tree to the tests (used as a source of real PCM-like data).
    println!("cargo:rustc-env=FLAC_C_DIR={}", flac.display());

    let mut build = cc::Build::new();
    build
        .include(flac.join("include")) // FLAC/*.h, share/*.h
        .include(libflac.join("include")) // private/*.h, protected/*.h, config.h
        // Mirror MAME's libFLAC define set (scripts/src/3rdparty.lua) so the oracle
        // matches chdman's macro environment exactly, then force the *scalar*
        // reference path with FLAC__NO_ASM (the `#ifndef FLAC__NO_ASM` dispatch in
        // stream_encoder.c is skipped, so no intrin symbols are referenced even
        // though config.h sets FLAC__HAS_X86INTRIN=1). config.h supplies the CPU
        // macros + PACKAGE_VERSION; release semantics via NDEBUG.
        .define("HAVE_CONFIG_H", None)
        .define("FLAC__NO_ASM", None)
        .define("FLAC__HAS_OGG", "0")
        .define("OGG_FOUND", "0")
        .define("NDEBUG", None)
        .define("HAVE_LROUND", "1")
        .define("ENABLE_64_BIT_WORDS", "1")
        .define("HAVE_INTTYPES_H", None)
        .define("HAVE_STDBOOL_H", None)
        .define("HAVE_STDINT_H", None)
        .define("HAVE_STDIO_H", None)
        .define("HAVE_STDLIB_H", None)
        .define("HAVE_STRING_H", None)
        .warnings(false);

    // libFLAC encoder + the verify decoder it links (verify is off at runtime,
    // but the symbols are referenced). All *_intrin_* / *_asm_* and ogg/metadata
    // translation units are intentionally omitted.
    for f in [
        "stream_encoder.c",
        "stream_encoder_framing.c",
        "lpc.c",
        "fixed.c",
        "window.c",
        "bitwriter.c",
        "bitmath.c",
        "crc.c",
        "format.c",
        "memory.c",
        "float.c",
        "md5.c",
        "cpu.c",
        "stream_decoder.c",
        "bitreader.c",
    ] {
        build.file(libflac.join(f));
    }
    build.file(manifest.join("cref").join("shim.c"));

    // The shim (and the libFLAC it wraps) is referenced only by the
    // integration-test crate, not by the library. Build-script `rustc-link-lib`
    // directives don't reliably reach integration-test link lines, and plain
    // static linkage drops shim.o to archive order/gc anyway (undefined
    // `libflac_rs_cref_encode` at test link time). So pass the archive straight to
    // the *test* linker, whole, so every object is pulled in unconditionally.
    build.cargo_metadata(false);
    build.compile("flac_cref");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    println!("cargo:rustc-link-arg-tests=-Wl,--whole-archive");
    println!("cargo:rustc-link-arg-tests={out_dir}/libflac_cref.a");
    println!("cargo:rustc-link-arg-tests=-Wl,--no-whole-archive");
}
