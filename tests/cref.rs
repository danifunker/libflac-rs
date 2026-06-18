//! Differential tests against the real libFLAC 1.4.3 encoder, compiled by
//! `build.rs` under the `cref` feature. Run with:
//!
//! ```text
//! cargo test --features cref
//! ```
//!
//! (Inside WSL/glibc for the float-parity work: the C oracle and Rust's `f32`
//! transcendentals then resolve to the same libm.) The whole file compiles to
//! nothing without the feature.
#![cfg(feature = "cref")]

use libflac_rs::testing::MetadataBlock;
use std::os::raw::c_int;

unsafe extern "C" {
    fn libflac_rs_cref_encode(
        interleaved: *const i32,
        nsamples: u32,
        channels: u32,
        bps: u32,
        sample_rate: u32,
        blocksize: u32,
        max_lpc_order: i32,
        do_mid_side: i32,
        out: *mut u8,
        out_len: *mut usize,
    ) -> c_int;
    fn libflac_rs_cref_encode_cfg(
        interleaved: *const i32,
        nsamples: u32,
        channels: u32,
        bps: u32,
        sample_rate: u32,
        blocksize: u32,
        compression_level: i32,
        max_lpc_order: i32,
        do_mid_side: i32,
        out: *mut u8,
        out_len: *mut usize,
    ) -> c_int;
    fn libflac_rs_cref_encode_full(
        interleaved: *const i32,
        nsamples: u32,
        channels: u32,
        bps: u32,
        sample_rate: u32,
        blocksize: u32,
        compression_level: i32,
        do_md5: i32,
        out: *mut u8,
        out_len: *mut usize,
    ) -> c_int;
    #[allow(clippy::too_many_arguments)]
    fn libflac_rs_cref_encode_ogg(
        interleaved: *const i32,
        nsamples: u32,
        channels: u32,
        bps: u32,
        sample_rate: u32,
        blocksize: u32,
        compression_level: i32,
        do_md5: i32,
        serial_number: i32,
        out: *mut u8,
        out_len: *mut usize,
    ) -> c_int;
    fn libflac_rs_cref_decode(
        data: *const u8,
        len: usize,
        out: *mut i32,
        out_len: *mut usize,
    ) -> c_int;
    #[allow(clippy::too_many_arguments)]
    fn libflac_rs_cref_encode_full_app(
        interleaved: *const i32,
        nsamples: u32,
        channels: u32,
        bps: u32,
        sample_rate: u32,
        blocksize: u32,
        compression_level: i32,
        do_md5: i32,
        app_id: *const u8,
        app_data: *const u8,
        app_data_len: u32,
        out: *mut u8,
        out_len: *mut usize,
    ) -> c_int;
    #[allow(clippy::too_many_arguments)]
    fn libflac_rs_cref_encode_full_picture(
        interleaved: *const i32,
        nsamples: u32,
        channels: u32,
        bps: u32,
        sample_rate: u32,
        blocksize: u32,
        compression_level: i32,
        do_md5: i32,
        picture_type: u32,
        mime: *const u8,
        desc: *const u8,
        width: u32,
        height: u32,
        depth: u32,
        colors: u32,
        pic_data: *const u8,
        pic_data_len: u32,
        out: *mut u8,
        out_len: *mut usize,
    ) -> c_int;
    #[allow(clippy::too_many_arguments)]
    fn libflac_rs_cref_encode_full_seektable(
        interleaved: *const i32,
        nsamples: u32,
        channels: u32,
        bps: u32,
        sample_rate: u32,
        blocksize: u32,
        compression_level: i32,
        do_md5: i32,
        sample_numbers: *const u64,
        num_points: u32,
        out: *mut u8,
        out_len: *mut usize,
    ) -> c_int;
    #[allow(clippy::too_many_arguments)]
    fn libflac_rs_cref_encode_full_cuesheet(
        interleaved: *const i32,
        nsamples: u32,
        channels: u32,
        bps: u32,
        sample_rate: u32,
        blocksize: u32,
        compression_level: i32,
        do_md5: i32,
        media_catalog_number: *const u8,
        lead_in: u64,
        is_cd: i32,
        num_tracks: u32,
        track_offsets: *const u64,
        track_numbers: *const u8,
        track_isrcs: *const u8,
        track_types: *const u8,
        track_pre_emphasis: *const u8,
        track_num_indices: *const u8,
        index_offsets: *const u64,
        index_numbers: *const u8,
        out: *mut u8,
        out_len: *mut usize,
    ) -> c_int;
    fn libflac_rs_cref_vendor_string(out: *mut u8, cap: usize) -> usize;
    fn libflac_rs_cref_crc8(data: *const u8, len: u32) -> u8;
    fn libflac_rs_cref_crc16(data: *const u8, len: u32) -> u16;
    fn libflac_rs_cref_md5(
        interleaved: *const i32,
        nsamples: u32,
        channels: u32,
        bytes_per_sample: u32,
        out16: *mut u8,
    );

    // F2 leaf-function wrappers (see cref/shim.c).
    fn libflac_rs_cref_window_tukey(p: f32, l: i32, out: *mut f32);
    fn libflac_rs_cref_lpc_window_data(
        input: *const i32,
        window: *const f32,
        out: *mut f32,
        data_len: u32,
    );
    fn libflac_rs_cref_lpc_window_data_partial(
        input: *const i32,
        window: *const f32,
        out: *mut f32,
        data_len: u32,
        part_size: u32,
        data_shift: u32,
    );
    fn libflac_rs_cref_compute_autocorrelation(
        data: *const f32,
        data_len: u32,
        lag: u32,
        autoc: *mut f64,
    );
    fn libflac_rs_cref_compute_lp_coefficients(
        autoc: *const f64,
        max_order: u32,
        lp_coeff_flat: *mut f32,
        error: *mut f64,
    ) -> u32;
    fn libflac_rs_cref_expected_bits(lpc_error: f64, total_samples: u32) -> f64;
    fn libflac_rs_cref_compute_best_order(
        lpc_error: *const f64,
        max_order: u32,
        total_samples: u32,
        overhead_bits_per_order: u32,
    ) -> u32;
    fn libflac_rs_cref_quantize_coefficients(
        lp_coeff: *const f32,
        order: u32,
        precision: u32,
        qlp_coeff: *mut i32,
        shift: *mut i32,
    ) -> i32;
    fn libflac_rs_cref_compute_residual(
        signal: *const i32,
        blocksize: u32,
        qlp_coeff: *const i32,
        order: u32,
        lp_quantization: i32,
        residual: *mut i32,
    );
}

/// Encode interleaved PCM via the C libFLAC reference, returning the raw audio
/// frames (metadata stripped, as CHD does). `max_lpc_order`/`do_mid_side` < 0
/// keep the level-8 preset; >= 0 override for staged testing.
fn c_encode(
    interleaved: &[i32],
    channels: u32,
    bps: u32,
    blocksize: u32,
    max_lpc_order: i32,
    do_mid_side: i32,
) -> Vec<u8> {
    assert_eq!(
        interleaved.len() % channels as usize,
        0,
        "ragged interleave"
    );
    let nsamples = (interleaved.len() / channels as usize) as u32;
    // Generous capacity: VERBATIM frames are ~bps/8 per sample plus headers.
    let mut out = vec![0u8; interleaved.len() * 4 + 8192];
    let mut out_len = out.len();
    let rc = unsafe {
        libflac_rs_cref_encode(
            interleaved.as_ptr(),
            nsamples,
            channels,
            bps,
            44_100,
            blocksize,
            max_lpc_order,
            do_mid_side,
            out.as_mut_ptr(),
            &mut out_len,
        )
    };
    assert_eq!(rc, 0, "C encode returned {rc}");
    out.truncate(out_len);
    out
}

/// Encode via the C reference at an explicit compression level (no LPC/mid-side
/// overrides), for the all-levels differential test.
fn c_encode_level(
    interleaved: &[i32],
    channels: u32,
    bps: u32,
    blocksize: u32,
    level: i32,
) -> Vec<u8> {
    let nsamples = (interleaved.len() / channels as usize) as u32;
    let mut out = vec![0u8; interleaved.len() * 4 + 8192];
    let mut out_len = out.len();
    let rc = unsafe {
        libflac_rs_cref_encode_cfg(
            interleaved.as_ptr(),
            nsamples,
            channels,
            bps,
            44_100,
            blocksize,
            level,
            -1,
            -1,
            out.as_mut_ptr(),
            &mut out_len,
        )
    };
    assert_eq!(rc, 0, "C encode_cfg returned {rc}");
    out.truncate(out_len);
    out
}

/// Encode a complete FLAC stream (marker + metadata + frames) via the C reference.
fn c_encode_full(
    interleaved: &[i32],
    channels: u32,
    bps: u32,
    blocksize: u32,
    level: i32,
    do_md5: bool,
) -> Vec<u8> {
    let nsamples = (interleaved.len() / channels as usize) as u32;
    let mut out = vec![0u8; interleaved.len() * 4 + 8192];
    let mut out_len = out.len();
    let rc = unsafe {
        libflac_rs_cref_encode_full(
            interleaved.as_ptr(),
            nsamples,
            channels,
            bps,
            44_100,
            blocksize,
            level,
            do_md5 as i32,
            out.as_mut_ptr(),
            &mut out_len,
        )
    };
    assert_eq!(rc, 0, "C encode_full returned {rc}");
    out.truncate(out_len);
    out
}

/// Encode a complete FLAC stream with a SEEKTABLE (the given target sample numbers)
/// via the C reference; libFLAC generates/fills/sorts it during encoding.
fn c_encode_full_seektable(
    interleaved: &[i32],
    channels: u32,
    bps: u32,
    blocksize: u32,
    level: i32,
    do_md5: bool,
    sample_numbers: &[u64],
) -> Vec<u8> {
    let nsamples = (interleaved.len() / channels as usize) as u32;
    let mut out = vec![0u8; interleaved.len() * 4 + 8192];
    let mut out_len = out.len();
    let rc = unsafe {
        libflac_rs_cref_encode_full_seektable(
            interleaved.as_ptr(),
            nsamples,
            channels,
            bps,
            44_100,
            blocksize,
            level,
            do_md5 as i32,
            sample_numbers.as_ptr(),
            sample_numbers.len() as u32,
            out.as_mut_ptr(),
            &mut out_len,
        )
    };
    assert_eq!(rc, 0, "C encode_full_seektable returned {rc}");
    out.truncate(out_len);
    out
}

/// Widen i32 samples to the encoder's unified i64 channel type (the leaf
/// LPC/fixed stages take i64; the C oracle leaves take i32 — same values).
fn to_i64(s: &[i32]) -> Vec<i64> {
    s.iter().map(|&x| x as i64).collect()
}

/// Stereo-interleave two channels of i16 (as i32, sign-extended) PCM.
fn interleave(left: &[i16], right: &[i16]) -> Vec<i32> {
    assert_eq!(left.len(), right.len());
    let mut v = Vec::with_capacity(left.len() * 2);
    for (&l, &r) in left.iter().zip(right) {
        v.push(l as i32);
        v.push(r as i32);
    }
    v
}

#[test]
fn c_reference_links_and_emits_frames() {
    // Silence -> CONSTANT subframes. Proves the oracle compiles, links, runs, and
    // that metadata stripping leaves valid FLAC audio frames: each frame begins
    // with the 14-bit sync 0b11111111_111110, reserved 0, blocking-strategy 0
    // (fixed block size) -> bytes 0xFF 0xF8.
    let bs = 2048usize;
    let silence = vec![0i32; bs * 2 * 2]; // two stereo blocks
    let frames = c_encode(&silence, 2, 16, bs as u32, 0, -1);
    assert!(
        frames.len() >= 2,
        "expected frame bytes, got {}",
        frames.len()
    );
    assert_eq!(frames[0], 0xFF, "frame sync byte 0");
    assert_eq!(frames[1] & 0xFE, 0xF8, "frame sync byte 1 (sync+reserved)");
    assert_eq!(
        frames[1], 0xF8,
        "fixed block size -> blocking-strategy bit 0"
    );
}

#[test]
fn c_reference_lpc_knob_changes_encoding() {
    // A sine is well-modeled by LPC but poorly by the fixed predictors, so the
    // fixed-only (max_lpc_order = 0) and full-LPC encodings must differ — this
    // confirms the staged-testing knob actually steers the encoder. Also uses a
    // non-block-multiple length to exercise a short final frame across frames.
    // (A linear ramp would NOT work here: a fixed order-2 predictor nulls it, so
    // LPC can't improve on it and the two encodings come out identical.)
    let bs = 2048u32;
    let n = bs as usize * 3 + 777;
    // High-frequency (near-Nyquist) sines: the fixed predictors badly mispredict
    // these (their residual is larger than the signal), while LPC models a single
    // sinusoid almost exactly, so LPC is decisively chosen over fixed.
    let sine = |f: f64, amp: f64| -> Vec<i16> {
        (0..n)
            .map(|i| (amp * (i as f64 * f).sin()).round() as i16)
            .collect()
    };
    let pcm = interleave(&sine(2.4, 9000.0), &sine(1.9, 7000.0));

    let fixed_only = c_encode(&pcm, 2, 16, bs, 0, -1);
    let full_lpc = c_encode(&pcm, 2, 16, bs, -1, -1);
    assert_eq!(fixed_only[0], 0xFF, "fixed-only frame sync");
    assert_eq!(full_lpc[0], 0xFF, "full-LPC frame sync");
    assert_ne!(fixed_only, full_lpc, "LPC should beat fixed on a sine");
}

// --- F0: CRC-8 / CRC-16 vs the C reference (FLAC__crc8 / FLAC__crc16) ----------

/// Byte buffers covering lengths around the CRC loop's word boundaries plus
/// structured and pseudo-random content.
fn crc_corpus() -> Vec<Vec<u8>> {
    let mut v: Vec<Vec<u8>> = vec![
        Vec::new(),
        vec![0x00],
        vec![0xFF],
        b"123456789".to_vec(),
        (0..=255u8).collect(),
    ];
    // Lengths 0..40 to straddle the C's 8-byte-at-a-time unrolling.
    for n in 0..40usize {
        v.push((0..n).map(|i| (i as u32 * 37 + 11) as u8).collect());
    }
    // A few pseudo-random blocks of CHD-frame-ish sizes.
    for &(len, seed) in &[(2048usize, 1u32), (4096, 7), (8192, 99)] {
        let mut x = seed | 1;
        v.push(
            (0..len)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 17;
                    x ^= x << 5;
                    (x >> 24) as u8
                })
                .collect(),
        );
    }
    v
}

#[test]
fn crc8_matches_c_reference() {
    for data in crc_corpus() {
        let rust = libflac_rs::testing::crc8(&data);
        let c = unsafe { libflac_rs_cref_crc8(data.as_ptr(), data.len() as u32) };
        assert_eq!(rust, c, "crc8 mismatch on {}-byte input", data.len());
    }
}

#[test]
fn crc16_matches_c_reference() {
    for data in crc_corpus() {
        let rust = libflac_rs::testing::crc16(&data);
        let c = unsafe { libflac_rs_cref_crc16(data.as_ptr(), data.len() as u32) };
        assert_eq!(rust, c, "crc16 mismatch on {}-byte input", data.len());
    }
}

// --- Metadata: MD5 of the decoded audio (STREAMINFO checksum) ------------------

/// `audio_md5` must equal libFLAC's `FLAC__MD5Accumulate`/`Final` over the same
/// interleaved PCM, for mono/stereo and 16-bit (2 bytes/sample), across lengths
/// that straddle the 64-byte MD5 block boundary.
#[test]
fn audio_md5_matches_c() {
    for channels in [1u32, 2] {
        for &n in &[1usize, 7, 31, 32, 33, 64, 65, 100, 2048, 4096, 5000] {
            let stereo = gen_pcm(0xABCD ^ n as u32, n);
            let pcm: Vec<i32> = if channels == 2 {
                stereo[..n * 2].to_vec()
            } else {
                stereo.iter().step_by(2).take(n).copied().collect()
            };
            let rust = libflac_rs::testing::audio_md5(&pcm, 2);
            let mut c = [0u8; 16];
            unsafe {
                libflac_rs_cref_md5(pcm.as_ptr(), n as u32, channels, 2, c.as_mut_ptr());
            }
            assert_eq!(rust, c, "md5 channels={channels} n={n}");
        }
    }
}

/// The STREAMINFO block (`fLaC` marker + the 34-byte body) must be byte-identical
/// to libFLAC's, with the final min/max framesize, total samples, and (with
/// `do_md5`) the audio checksum. libFLAC follows STREAMINFO with an auto
/// VORBIS_COMMENT, so only the body (bytes 8..42, before that block) is compared.
#[test]
fn streaminfo_matches_c() {
    let bs = 2048u32;
    for &do_md5 in &[true, false] {
        for level in [0u32, 5, 8] {
            for seed in 1..=5u32 {
                let samples = bs as usize + (seed as usize * 257) % 3000;
                let pcm = gen_pcm(seed, samples);
                let rust = libflac_rs::testing::encode(
                    &pcm,
                    2,
                    16,
                    44_100,
                    bs,
                    &libflac_rs::testing::preset(level),
                    do_md5,
                    &[],
                );
                let c = c_encode_full(&pcm, 2, 16, bs, level as i32, do_md5);
                assert_eq!(&rust[0..4], b"fLaC", "rust marker");
                assert_eq!(&c[0..4], b"fLaC", "c marker");
                assert_eq!(
                    &rust[8..42],
                    &c[8..42],
                    "[level {level} seed {seed} md5 {do_md5}] STREAMINFO body differs"
                );
            }
        }
    }
}

/// The full Rust stream (marker + STREAMINFO + frames) must decode back to the
/// exact original PCM through the real libFLAC decoder — proof the complete file
/// is well-formed (including the `is_last` STREAMINFO with no VORBIS_COMMENT).
#[test]
fn full_stream_round_trips() {
    let bs = 2048u32;
    for level in [0u32, 4, 8] {
        for seed in 1..=5u32 {
            let samples = bs as usize + (seed as usize * 333) % 2500;
            let pcm = gen_pcm(seed, samples);
            // Also exercise a VORBIS_COMMENT and a PADDING block, so those decode.
            let stream = libflac_rs::testing::encode(
                &pcm,
                2,
                16,
                44_100,
                bs,
                &libflac_rs::testing::preset(level),
                true,
                &[
                    MetadataBlock::VorbisComment("libflac-rs round-trip"),
                    MetadataBlock::Padding(100),
                ],
            );
            let mut decoded = vec![0i32; pcm.len()];
            let mut dlen = decoded.len();
            let rc = unsafe {
                libflac_rs_cref_decode(
                    stream.as_ptr(),
                    stream.len(),
                    decoded.as_mut_ptr(),
                    &mut dlen,
                )
            };
            assert_eq!(rc, 0, "[level {level} seed {seed}] decode failed: {rc}");
            decoded.truncate(dlen);
            assert_eq!(
                decoded, pcm,
                "[level {level} seed {seed}] round-trip mismatch"
            );
        }
    }
}

/// The hardcoded vendor constant must match the compiled libFLAC's
/// `FLAC__VENDOR_STRING`.
#[test]
fn vendor_string_matches_c() {
    let mut buf = [0u8; 128];
    let len = unsafe { libflac_rs_cref_vendor_string(buf.as_mut_ptr(), buf.len()) };
    let c_vendor = std::str::from_utf8(&buf[..len]).unwrap();
    assert_eq!(libflac_rs::testing::LIBFLAC_VENDOR_STRING, c_vendor);
}

/// With the libFLAC vendor string and no padding, the **entire** Rust stream
/// (marker + STREAMINFO + auto VORBIS_COMMENT + frames) must be byte-identical to
/// libFLAC's default full output — not just the STREAMINFO body.
#[test]
fn full_stream_matches_c_default() {
    let bs = 2048u32;
    let mut buf = [0u8; 128];
    let len = unsafe { libflac_rs_cref_vendor_string(buf.as_mut_ptr(), buf.len()) };
    let vendor = std::str::from_utf8(&buf[..len]).unwrap();
    for level in [0u32, 4, 8] {
        for seed in 1..=5u32 {
            let samples = bs as usize + (seed as usize * 281) % 3000;
            let pcm = gen_pcm(seed, samples);
            let rust = libflac_rs::testing::encode(
                &pcm,
                2,
                16,
                44_100,
                bs,
                &libflac_rs::testing::preset(level),
                true,
                &[MetadataBlock::VorbisComment(vendor)],
            );
            let c = c_encode_full(&pcm, 2, 16, bs, level as i32, true);
            assert_eq!(
                rust, c,
                "[level {level} seed {seed}] full stream differs from libFLAC"
            );
        }
    }
}

// --- F2: apodization windows (subdivide_tukey deps: tukey/hann/rectangle) ------

/// Per-element bit-exact compare of the Rust Tukey window against the C
/// `FLAC__window_tukey`, across the level-8 parameter (`0.5/3`), the taper
/// extremes that fall back to rectangle (`p<=0`) and Hann (`p>=1`), and assorted
/// `p`/length combinations (odd lengths, tiny `Np`).
#[test]
fn window_tukey_matches_c() {
    let level8_p = 0.5f32 / 3.0; // subdivide_tukey(3): p (=0.5 default) / parts (=3)
    let ps = [
        level8_p, 0.5f32, 0.25f32, 0.1f32, 0.99f32, 1.0f32, 1.5f32, 0.0f32, -0.3f32,
    ];
    let lengths = [2048i32, 4096, 1024, 682, 333, 64, 17, 5, 2];
    for &l in &lengths {
        for &p in &ps {
            let mut rust = vec![0f32; l as usize];
            libflac_rs::testing::window::tukey(&mut rust, p);
            let mut c = vec![0f32; l as usize];
            unsafe { libflac_rs_cref_window_tukey(p, l, c.as_mut_ptr()) };
            for (i, (&r, &cv)) in rust.iter().zip(&c).enumerate() {
                assert_eq!(
                    r.to_bits(),
                    cv.to_bits(),
                    "tukey[{i}] p={p} L={l}: rust={r} c={cv}"
                );
            }
        }
    }
}

/// One channel of diverse 16-bit-ish PCM for the float-pipeline stage tests.
fn mono_signal(seed: u32, n: usize) -> Vec<i32> {
    gen_pcm(seed, n).iter().step_by(2).copied().collect()
}

/// Windowing (`FLAC__lpc_window_data`) must be bit-exact: `out[i] = in[i]*win[i]`
/// in `f32`. Feed the verified Rust Tukey window through both the Rust and C
/// windowers over real-ish signals and compare every `f32` bit pattern.
#[test]
fn lpc_window_data_matches_c() {
    let level8_p = 0.5f32 / 3.0;
    for &bs in &[2048usize, 4096, 1024] {
        for seed in 1..=8u32 {
            let sig = mono_signal(seed, bs);
            let sig64 = to_i64(&sig);
            let mut win = vec![0f32; bs];
            libflac_rs::testing::window::tukey(&mut win, level8_p);

            let mut rust = vec![0f32; bs];
            libflac_rs::testing::lpc::window_data(&sig64, &win, &mut rust, bs);
            let mut c = vec![0f32; bs];
            unsafe {
                libflac_rs_cref_lpc_window_data(
                    sig.as_ptr(),
                    win.as_ptr(),
                    c.as_mut_ptr(),
                    bs as u32,
                )
            };
            for (i, (&r, &cv)) in rust.iter().zip(&c).enumerate() {
                assert_eq!(
                    r.to_bits(),
                    cv.to_bits(),
                    "window_data[{i}] bs={bs} seed={seed}"
                );
            }
        }
    }
}

/// Partial windowing (`FLAC__lpc_window_data_partial`) for the exact
/// `(part_size, data_shift)` combinations the `subdivide_tukey(3)` a/b/c state
/// machine produces at blocksize 2048 (b=2 and b=3 sub-blocks). Compares the
/// region the autocorrelation subsequently reads (`[0, blocksize/b)`).
#[test]
fn lpc_window_data_partial_matches_c() {
    let level8_p = 0.5f32 / 3.0;
    let bs = 2048usize;
    let mut win = vec![0f32; bs];
    libflac_rs::testing::window::tukey(&mut win, level8_p);
    // (b, data_shift) pairs; part_size = bs/b/2, autocorr reads bs/b samples.
    let configs = [(2usize, 0usize), (2, 1024), (3, 0), (3, 682), (3, 1365)];
    for seed in 1..=6u32 {
        let sig = mono_signal(seed, bs);
        let sig64 = to_i64(&sig);
        for &(b, shift) in &configs {
            let part_size = bs / b / 2;
            let read_len = bs / b;
            let mut rust = vec![0f32; bs];
            libflac_rs::testing::lpc::window_data_partial(
                &sig64, &win, &mut rust, bs, part_size, shift,
            );
            let mut c = vec![0f32; bs];
            unsafe {
                libflac_rs_cref_lpc_window_data_partial(
                    sig.as_ptr(),
                    win.as_ptr(),
                    c.as_mut_ptr(),
                    bs as u32,
                    part_size as u32,
                    shift as u32,
                )
            };
            for i in 0..read_len {
                assert_eq!(
                    rust[i].to_bits(),
                    c[i].to_bits(),
                    "partial[{i}] b={b} shift={shift} seed={seed}"
                );
            }
        }
    }
}

/// Autocorrelation (`FLAC__lpc_compute_autocorrelation`) in `f64` must be
/// bit-exact. Feed identical windowed `f32` signals to the Rust and C routines and
/// compare every `autoc[j]` bit pattern, at the level-8 lag (13) and the
/// neighbouring lag buckets (8/12/16) plus a tiny `data_len` (generic C path).
#[test]
fn compute_autocorrelation_matches_c() {
    let level8_p = 0.5f32 / 3.0;
    for &(bs, lag) in &[
        (2048usize, 13u32),
        (2048, 8),
        (2048, 12),
        (2048, 16),
        (682, 13),
        (20, 13),
    ] {
        for seed in 1..=8u32 {
            let sig = mono_signal(seed, bs);
            let sig64 = to_i64(&sig);
            let mut win = vec![0f32; bs];
            libflac_rs::testing::window::tukey(&mut win, level8_p);
            let mut windowed = vec![0f32; bs];
            libflac_rs::testing::lpc::window_data(&sig64, &win, &mut windowed, bs);

            // The C routine writes MAX_LAG (lag rounded up to the 8/12/16 bucket)
            // values, not `lag`; the encoder sizes this buffer at MAX_LPC_ORDER+1.
            let mut rust = vec![0f64; 33];
            libflac_rs::testing::lpc::compute_autocorrelation(&windowed, lag as usize, &mut rust);
            let mut c = vec![0f64; 33];
            unsafe {
                libflac_rs_cref_compute_autocorrelation(
                    windowed.as_ptr(),
                    bs as u32,
                    lag,
                    c.as_mut_ptr(),
                )
            };
            for j in 0..lag as usize {
                assert_eq!(
                    rust[j].to_bits(),
                    c[j].to_bits(),
                    "autoc[{j}] bs={bs} lag={lag} seed={seed}"
                );
            }
        }
    }
}

/// Compute the autocorrelation for a real-ish signal via the verified Rust
/// window+autocorr stages, for feeding the Levinson tests.
fn autoc_for(seed: u32, bs: usize, lag: usize) -> Vec<f64> {
    let sig = to_i64(&mono_signal(seed, bs));
    let mut win = vec![0f32; bs];
    libflac_rs::testing::window::tukey(&mut win, 0.5f32 / 3.0);
    let mut windowed = vec![0f32; bs];
    libflac_rs::testing::lpc::window_data(&sig, &win, &mut windowed, bs);
    let mut autoc = vec![0f64; 33];
    libflac_rs::testing::lpc::compute_autocorrelation(&windowed, lag, &mut autoc);
    autoc
}

/// Levinson-Durbin (`FLAC__lpc_compute_lp_coefficients`): the `f64` recursion and
/// the `f32`-narrowed coefficient rows must be bit-exact across all orders, as
/// must the per-order `f64` error and the (possibly reduced) max order.
#[test]
fn compute_lp_coefficients_matches_c() {
    const MLO: usize = 32;
    for &max_order in &[12usize, 8, 1] {
        for seed in 1..=12u32 {
            let autoc = autoc_for(seed, 2048, max_order + 1);
            let rust = libflac_rs::testing::lpc::compute_lp_coefficients(&autoc, max_order);

            let mut c_flat = vec![0f32; MLO * MLO];
            let mut c_err = vec![0f64; MLO];
            let c_max = unsafe {
                libflac_rs_cref_compute_lp_coefficients(
                    autoc.as_ptr(),
                    max_order as u32,
                    c_flat.as_mut_ptr(),
                    c_err.as_mut_ptr(),
                )
            } as usize;
            assert_eq!(
                rust.max_order, c_max,
                "max_order seed={seed} mo={max_order}"
            );
            for order in 1..=rust.max_order {
                let c_row = &c_flat[(order - 1) * MLO..(order - 1) * MLO + order];
                for (j, (&r, &cv)) in rust.row(order).iter().zip(c_row).enumerate() {
                    assert_eq!(
                        r.to_bits(),
                        cv.to_bits(),
                        "lp_coeff[{}][{j}] seed={seed} mo={max_order}",
                        order - 1
                    );
                }
                assert_eq!(
                    rust.error[order - 1].to_bits(),
                    c_err[order - 1].to_bits(),
                    "error[{}] seed={seed} mo={max_order}",
                    order - 1
                );
            }
        }
    }
}

/// Order selection + the expected-bits estimator (`log`/`M_LN2` in `f64`).
#[test]
fn compute_best_order_matches_c() {
    for &max_order in &[12usize, 8] {
        for seed in 1..=12u32 {
            let autoc = autoc_for(seed, 2048, max_order + 1);
            let rust = libflac_rs::testing::lpc::compute_lp_coefficients(&autoc, max_order);
            let mut c_flat = vec![0f32; 32 * 32];
            let mut c_err = vec![0f64; 32];
            let c_max = unsafe {
                libflac_rs_cref_compute_lp_coefficients(
                    autoc.as_ptr(),
                    max_order as u32,
                    c_flat.as_mut_ptr(),
                    c_err.as_mut_ptr(),
                )
            } as usize;
            assert_eq!(rust.max_order, c_max);

            // Per-order expected-bits estimate.
            for order in 1..=rust.max_order {
                let r = libflac_rs::testing::lpc::expected_bits(
                    rust.error[order - 1],
                    2048 - order as u32,
                );
                let c =
                    unsafe { libflac_rs_cref_expected_bits(c_err[order - 1], 2048 - order as u32) };
                assert_eq!(
                    r.to_bits(),
                    c.to_bits(),
                    "expected_bits order={order} seed={seed}"
                );
            }

            // Best order, for both main (bps 16) and side (bps 17) overhead.
            for overhead in [16u32 + 11, 17 + 11] {
                let r = libflac_rs::testing::lpc::compute_best_order(
                    &rust.error,
                    rust.max_order,
                    2048,
                    overhead,
                );
                let c = unsafe {
                    libflac_rs_cref_compute_best_order(c_err.as_ptr(), c_max as u32, 2048, overhead)
                } as usize;
                assert_eq!(
                    r, c,
                    "best_order overhead={overhead} seed={seed} mo={max_order}"
                );
            }
        }
    }
}

/// Quantization (`FLAC__lpc_quantize_coefficients`): the integer `qlp_coeff[]`,
/// the shift, and the return code must all match, driven by realistic
/// coefficients from the full window→autocorr→Levinson pipeline at precision 11
/// (the level-8/16-bit auto precision) across every order.
#[test]
fn quantize_coefficients_matches_c() {
    let precision = 11u32;
    for seed in 1..=16u32 {
        let autoc = autoc_for(seed, 2048, 13);
        let lp = libflac_rs::testing::lpc::compute_lp_coefficients(&autoc, 12);
        for order in 1..=lp.max_order {
            let row = lp.row(order);
            let rust = libflac_rs::testing::lpc::quantize_coefficients(row, order, precision);

            let mut c_qlp = vec![0i32; order];
            let mut c_shift = 0i32;
            let c_ret = unsafe {
                libflac_rs_cref_quantize_coefficients(
                    row.as_ptr(),
                    order as u32,
                    precision,
                    c_qlp.as_mut_ptr(),
                    &mut c_shift,
                )
            };
            match rust {
                Ok(q) => {
                    assert_eq!(c_ret, 0, "ret order={order} seed={seed}");
                    assert_eq!(q.shift, c_shift, "shift order={order} seed={seed}");
                    assert_eq!(q.qlp_coeff, c_qlp, "qlp order={order} seed={seed}");
                }
                Err(code) => assert_eq!(code, c_ret, "err code order={order} seed={seed}"),
            }
        }
    }
}

/// LPC residual (`FLAC__lpc_compute_residual_from_qlp_coefficients`): with
/// realistic quantized predictors the prediction sum never overflows i32, so the
/// `i64` accumulation must reproduce the C residual exactly, every sample.
#[test]
fn compute_residual_matches_c() {
    let precision = 11u32;
    let bs = 2048usize;
    for seed in 1..=16u32 {
        let sig = mono_signal(seed, bs);
        let sig64 = to_i64(&sig);
        let mut win = vec![0f32; bs];
        libflac_rs::testing::window::tukey(&mut win, 0.5f32 / 3.0);
        let mut windowed = vec![0f32; bs];
        libflac_rs::testing::lpc::window_data(&sig64, &win, &mut windowed, bs);
        let mut autoc = vec![0f64; 33];
        libflac_rs::testing::lpc::compute_autocorrelation(&windowed, 13, &mut autoc);
        let lp = libflac_rs::testing::lpc::compute_lp_coefficients(&autoc, 12);

        for order in 1..=lp.max_order {
            let q = match libflac_rs::testing::lpc::quantize_coefficients(
                lp.row(order),
                order,
                precision,
            ) {
                Ok(q) => q,
                Err(_) => continue,
            };
            let rust =
                libflac_rs::testing::lpc::compute_residual(&sig64, order, &q.qlp_coeff, q.shift);
            let mut c = vec![0i32; bs - order];
            unsafe {
                libflac_rs_cref_compute_residual(
                    sig.as_ptr(),
                    bs as u32,
                    q.qlp_coeff.as_ptr(),
                    order as u32,
                    q.shift,
                    c.as_mut_ptr(),
                )
            };
            assert_eq!(rust, c, "residual order={order} seed={seed}");
        }
    }
}

// --- F1: CONSTANT / VERBATIM subframes + framing (fixed-only, independent) -----

/// Rust frame bytes for interleaved 16-bit PCM, CHD's audio format. `max_lpc_order`
/// mirrors the C staging knob (0 = fixed-only, 12 = level-8 LPC); independent
/// channels, otherwise the level-8 settings (subdivide_tukey(3), max_po 6).
fn rust_frames_lpc(
    interleaved: &[i32],
    channels: u32,
    blocksize: u32,
    max_lpc_order: u32,
) -> Vec<u8> {
    let mut cfg = libflac_rs::testing::preset(8);
    cfg.max_lpc_order = max_lpc_order;
    cfg.do_mid_side = false;
    cfg.loose_mid_side = false;
    libflac_rs::testing::encode_frames(interleaved, channels, 16, 44_100, blocksize, &cfg)
}

/// Fixed-only Rust frame bytes (F1).
fn rust_frames(interleaved: &[i32], channels: u32, blocksize: u32) -> Vec<u8> {
    rust_frames_lpc(interleaved, channels, blocksize, 0)
}

/// Full CHD level-8 config: LPC order 12 + the mid-side channel decision (F3).
fn rust_frames_full(interleaved: &[i32], channels: u32, blocksize: u32) -> Vec<u8> {
    libflac_rs::testing::encode_frames(
        interleaved,
        channels,
        16,
        44_100,
        blocksize,
        &libflac_rs::testing::preset(8),
    )
}

/// Rust frame bytes at an explicit compression level (the all-levels test).
fn rust_frames_level(interleaved: &[i32], channels: u32, blocksize: u32, level: u32) -> Vec<u8> {
    let cfg = libflac_rs::testing::preset(level);
    libflac_rs::testing::encode_frames(interleaved, channels, 16, 44_100, blocksize, &cfg)
}

#[test]
fn constant_and_short_frames_match_c() {
    let bs = 2048u32;
    // Constant signals exercise CONSTANT subframes across a range of wasted bits;
    // the short final-frame lengths (<= MAX_FIXED_ORDER and a 500-sample tail)
    // exercise VERBATIM plus both block-size-hint encodings (8- and 16-bit) and
    // the UTF-8 frame-number increment across frames.
    let cases: &[(&str, i16, i16)] = &[
        ("silence", 0, 0),            // wasted 0
        ("full_pos", 32767, 32767),   // wasted 0
        ("full_neg", -32768, -32768), // -32768 -> 15 wasted bits
        ("mixed_const", 1000, -2048), // R: 11 wasted bits, L: 3
        ("dc_offset", 256, 256),      // 8 wasted bits
    ];
    let lengths = [
        bs as usize * 2,       // two full frames
        bs as usize * 2 + 500, // + a 500-sample tail (16-bit block-size hint)
        bs as usize + 3,       // + a 3-sample tail (VERBATIM, 8-bit hint)
        3,                     // a single 3-sample VERBATIM frame
    ];
    for &(name, lv, rv) in cases {
        for &n in &lengths {
            let pcm = interleave(&vec![lv; n], &vec![rv; n]);
            let rust = rust_frames(&pcm, 2, bs);
            let c = c_encode(&pcm, 2, 16, bs, 0, 0); // fixed-only, independent stereo
            assert_eq!(rust, c, "[{name} n={n}] frame bytes differ from C");
        }
    }
}

/// Full LPC subframes (window → autocorr → Levinson → quantize → residual →
/// subdivide_tukey order selection) must be byte-identical at the level-8 config
/// (max_lpc_order 12, independent stereo). Block-multiple lengths first, to
/// isolate the LPC path from the short-final-frame window recompute.
#[test]
fn lpc_subframes_match_c_block_multiple() {
    let bs = 2048u32;
    for seed in 1..=40u32 {
        let blocks = 1 + (seed as usize % 3);
        let samples = bs as usize * blocks;
        let pcm = gen_pcm(seed, samples);
        let rust = rust_frames_lpc(&pcm, 2, bs, 12);
        let c = c_encode(&pcm, 2, 16, bs, 12, 0); // LPC order 12, independent stereo
        assert_eq!(
            rust, c,
            "[seed {seed}, {samples} samples] LPC frames differ from C"
        );
    }
}

/// LPC with non-block-multiple lengths: the short final frame recomputes the
/// window at its own block size and uses the init-time qlp precision. Covers a
/// range of tail lengths including very short tails.
#[test]
fn lpc_subframes_match_c_short_final_frame() {
    let bs = 2048u32;
    for seed in 1..=40u32 {
        let tail = (seed as usize * 277) % 4000;
        let samples = bs as usize + 7 + tail;
        let pcm = gen_pcm(seed, samples);
        let rust = rust_frames_lpc(&pcm, 2, bs, 12);
        let c = c_encode(&pcm, 2, 16, bs, 12, 0);
        assert_eq!(
            rust, c,
            "[seed {seed}, {samples} samples] LPC short-frame bytes differ"
        );
    }
}

/// LPC across block sizes that select different auto qlp-coeff precisions
/// (192→7, 576→9, 1024→10, 4096→12).
#[test]
fn lpc_various_blocksizes_match_c() {
    for &bs in &[192u32, 576, 1024, 4096] {
        for seed in 1..=12u32 {
            let samples = bs as usize * 2 + (seed as usize * 37) % bs as usize;
            let pcm = gen_pcm(seed, samples);
            let rust = rust_frames_lpc(&pcm, 2, bs, 12);
            let c = c_encode(&pcm, 2, 16, bs, 12, 0);
            assert_eq!(rust, c, "[bs {bs}, seed {seed}] LPC frames differ");
        }
    }
}

/// Wasted-bits interaction with LPC: a signal whose samples are all multiples of
/// 2^k windows/predicts on the shifted signal at reduced subframe bps.
#[test]
fn lpc_wasted_bits_match_c() {
    let bs = 2048u32;
    for shift in [2u32, 4, 8] {
        for seed in 1..=8u32 {
            let base = gen_pcm(seed, bs as usize * 2 + 333);
            // Force `shift` wasted bits by zeroing the low bits (clamped to i16).
            let pcm: Vec<i32> = base
                .iter()
                .map(|&s| ((s >> shift) << shift).clamp(-32768, 32767))
                .collect();
            let rust = rust_frames_lpc(&pcm, 2, bs, 12);
            let c = c_encode(&pcm, 2, 16, bs, 12, 0);
            assert_eq!(
                rust, c,
                "[shift {shift}, seed {seed}] LPC wasted-bits bytes differ"
            );
        }
    }
}

/// Pure sinusoids from low to near-Nyquist: LPC models a single sine almost
/// exactly, exercising high guess orders and tiny residuals.
#[test]
fn lpc_pure_sine_match_c() {
    let bs = 2048u32;
    let n = bs as usize * 2 + 1234;
    for &freq in &[0.01f64, 0.05, 0.2, 0.5, 1.0, 1.7, 2.5, 3.0] {
        let l: Vec<i16> = (0..n)
            .map(|i| (12000.0 * (i as f64 * freq).sin()).round() as i16)
            .collect();
        let r: Vec<i16> = (0..n)
            .map(|i| (9000.0 * (i as f64 * freq * 0.7 + 0.3).sin()).round() as i16)
            .collect();
        let pcm = interleave(&l, &r);
        let rust = rust_frames_lpc(&pcm, 2, bs, 12);
        let c = c_encode(&pcm, 2, 16, bs, 12, 0);
        assert_eq!(rust, c, "[freq {freq}] LPC pure-sine bytes differ");
    }
}

/// F3: the full CHD level-8 config — LPC order 12 **plus** the per-frame mid-side
/// channel decision — byte-identical to the oracle's preset (`max_lpc_order=-1`,
/// `do_mid_side=-1`). The decorrelated-noise corpus drives a mix of channel
/// assignments across frames.
#[test]
fn full_config_mid_side_match_c() {
    let bs = 2048u32;
    for seed in 1..=60u32 {
        let samples = bs as usize + (seed as usize * 263) % 3500;
        let pcm = gen_pcm(seed, samples);
        let rust = rust_frames_full(&pcm, 2, bs);
        let c = c_encode(&pcm, 2, 16, bs, -1, -1); // real CHD level-8 preset
        assert_eq!(
            rust, c,
            "[seed {seed}, {samples} samples] full-config bytes differ"
        );
    }
}

/// Crafted L/R relationships to force each of the four channel assignments
/// (identical → mid/side, anti-correlated, scaled, fully independent).
#[test]
fn full_config_channel_assignments_match_c() {
    let bs = 2048u32;
    let n = bs as usize * 2 + 555;
    let sine = |f: f64, amp: f64, phase: f64| -> Vec<i16> {
        (0..n)
            .map(|i| (amp * (i as f64 * f + phase).sin()).round() as i16)
            .collect()
    };
    let l = sine(0.05, 10000.0, 0.0);
    let cases: Vec<(&str, Vec<i16>, Vec<i16>)> = vec![
        ("identical", l.clone(), l.clone()),
        (
            "anti",
            l.clone(),
            l.iter().map(|&x| x.saturating_neg()).collect(),
        ),
        (
            "scaled",
            l.clone(),
            l.iter().map(|&x| (x as i32 * 3 / 4) as i16).collect(),
        ),
        (
            "independent",
            sine(0.05, 10000.0, 0.0),
            sine(0.17, 8000.0, 1.1),
        ),
    ];
    for (name, lc, rc) in cases {
        let pcm = interleave(&lc, &rc);
        let rust = rust_frames_full(&pcm, 2, bs);
        let c = c_encode(&pcm, 2, 16, bs, -1, -1);
        assert_eq!(rust, c, "[{name}] full-config bytes differ");
    }
}

/// Generalization: every compression level 0–8 byte-identical to the oracle's
/// preset. Exercises the apodization variety (tukey(0.5) for 0–5,
/// subdivide_tukey(2) for 6–7, subdivide_tukey(3) for 8), the loose mid-side mode
/// (levels 1 and 4), fixed-only levels (0–2), and the differing LPC orders and
/// partition-order caps. Non-block-multiple lengths cover short final frames.
#[test]
fn all_compression_levels_match_c() {
    let bs = 2048u32;
    for level in 0..=8u32 {
        for seed in 1..=12u32 {
            let samples = bs as usize + (seed as usize * 211) % 3500;
            let pcm = gen_pcm(seed, samples);
            let rust = rust_frames_level(&pcm, 2, bs, level);
            let c = c_encode_level(&pcm, 2, 16, bs, level as i32);
            assert_eq!(
                rust, c,
                "[level {level}, seed {seed}, {samples} samples] bytes differ"
            );
        }
    }
}

/// Loose mid-side (levels 1, 4) over a long signal that crosses the ~9-frame
/// re-decision boundary several times, so both the reuse and the periodic
/// re-evaluation paths are exercised (plus a short final frame).
#[test]
fn loose_mid_side_long_signal_match_c() {
    let bs = 2048u32;
    let samples = bs as usize * 13 + 1000;
    for level in [1u32, 4] {
        for seed in 1..=4u32 {
            let pcm = gen_pcm(seed, samples);
            let rust = rust_frames_level(&pcm, 2, bs, level);
            let c = c_encode_level(&pcm, 2, 16, bs, level as i32);
            assert_eq!(rust, c, "[loose level {level}, seed {seed}] bytes differ");
        }
    }
}

/// Mono at assorted levels (the oracle disables mid-side for non-stereo at init,
/// `stream_encoder.c:675`; the Rust path does too): exercises the independent path
/// with each level's LPC order and apodization.
#[test]
fn mono_levels_match_c() {
    let bs = 2048u32;
    for level in [0u32, 2, 3, 5, 8] {
        for seed in 1..=6u32 {
            let stereo = gen_pcm(seed, bs as usize * 2 + 600);
            let mono: Vec<i32> = stereo.iter().step_by(2).copied().collect();
            let rust = rust_frames_level(&mono, 1, bs, level);
            let c = c_encode_level(&mono, 1, 16, bs, level as i32);
            assert_eq!(rust, c, "[mono level {level}, seed {seed}] bytes differ");
        }
    }
}

/// Rust frame bytes at an explicit compression level and bit depth.
fn rust_frames_level_bps(
    interleaved: &[i32],
    channels: u32,
    blocksize: u32,
    level: u32,
    bps: u32,
) -> Vec<u8> {
    let cfg = libflac_rs::testing::preset(level);
    libflac_rs::testing::encode_frames(interleaved, channels, bps, 44_100, blocksize, &cfg)
}

/// Diverse multi-partial + noise PCM scaled to fill a `bps`-bit range, for the
/// wider-bit-depth tests (RICE for 8/12, RICE2 for 20/24/32; 33-bit side at 32).
fn gen_pcm_bps(seed: u32, samples_per_channel: usize, bps: u32) -> Vec<i32> {
    let mut st = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    let urand = |st: &mut u32| (lcg(st) >> 8) as f64 / 16_777_216.0;
    let maxv = ((1i64 << (bps - 1)) - 1) as f64;
    let minv = -(1i64 << (bps - 1)) as f64;
    let scale = (1u64 << (bps - 1)) as f64 / 32768.0;
    let np = 1 + (urand(&mut st) * 4.0) as usize;
    let partials: Vec<(f64, f64, f64)> = (0..np)
        .map(|_| {
            (
                urand(&mut st) * 3.1,
                (200.0 + urand(&mut st) * 7000.0) * scale,
                urand(&mut st) * std::f64::consts::TAU,
            )
        })
        .collect();
    let noise = urand(&mut st) * urand(&mut st) * 2000.0 * scale;
    let mut out = Vec::with_capacity(samples_per_channel * 2);
    for i in 0..samples_per_channel {
        for ch in 0..2u32 {
            let mut v = 0.0f64;
            for &(f, a, p) in &partials {
                v += a * (f * i as f64 + p + ch as f64 * 0.4).sin();
            }
            v += noise * ((lcg(&mut st) >> 16) as u16 as i16 as f64 / 32768.0);
            out.push(v.round().clamp(minv, maxv) as i32);
        }
    }
    out
}

/// G3: wider bit depths. 8/12-bit use RICE; 20/24/32-bit use RICE2; 32-bit also
/// exercises the 33-bit side channel + wide residual. Byte-exact across levels.
#[test]
fn bit_depths_match_c() {
    let bs = 2048u32;
    for &bps in &[8u32, 12, 20, 24, 32] {
        for level in [0u32, 5, 8] {
            for seed in 1..=4u32 {
                let samples = bs as usize + (seed as usize * 191) % 2000;
                let pcm = gen_pcm_bps(seed, samples, bps);
                let rust = rust_frames_level_bps(&pcm, 2, bs, level, bps);
                let c = c_encode_level(&pcm, 2, bps, bs, level as i32);
                assert_eq!(
                    rust, c,
                    "[bps {bps} level {level} seed {seed}] frames differ"
                );
            }
        }
    }
}

/// Crafted 32-bit stereo cases that force each channel assignment and drive the
/// 33-bit side past `i32` (anti-correlated / independent full-range), exercising
/// the wide fixed-validity and LPC-bail paths. Byte-exact vs the oracle.
#[test]
fn wide_32bit_channel_cases_match_c() {
    let bs = 2048u32;
    let n = bs as usize * 2 + 300;
    let mut x = 0x9e37_79b9u32;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        x as i32 // full-range i32 from the PRNG bits
    };
    for case in 0..4 {
        let mut pcm = Vec::with_capacity(n * 2);
        for _ in 0..n {
            let l = next();
            let r = match case {
                0 => l,                            // identical -> side 0 (constant)
                1 => l.wrapping_neg(),             // anti -> mid 0, side ~33-bit
                2 => next(),                       // independent full-range
                _ => l.wrapping_add(next() >> 12), // near-correlated -> small side
            };
            pcm.push(l);
            pcm.push(r);
        }
        for level in [0u32, 5, 8] {
            let rust = rust_frames_level_bps(&pcm, 2, bs, level, 32);
            let c = c_encode_level(&pcm, 2, 32, bs, level as i32);
            assert_eq!(rust, c, "[32-bit case {case} level {level}] frames differ");
        }
    }
}

/// Wider bit depths through the **full** pipeline: complete stream (STREAMINFO
/// with the wider bps + 3-byte/etc. MD5, VORBIS_COMMENT, frames) byte-identical to
/// libFLAC's default output for 8/12/20/24-bit.
#[test]
fn wider_depth_full_stream_matches_c() {
    let bs = 2048u32;
    let mut buf = [0u8; 128];
    let len = unsafe { libflac_rs_cref_vendor_string(buf.as_mut_ptr(), buf.len()) };
    let vendor = std::str::from_utf8(&buf[..len]).unwrap();
    for &bps in &[8u32, 12, 20, 24, 32] {
        for level in [0u32, 5, 8] {
            for seed in 1..=3u32 {
                let pcm = gen_pcm_bps(seed, bs as usize + 700, bps);
                let rust = libflac_rs::testing::encode(
                    &pcm,
                    2,
                    bps,
                    44_100,
                    bs,
                    &libflac_rs::testing::preset(level),
                    true,
                    &[MetadataBlock::VorbisComment(vendor)],
                );
                let c = c_encode_full(&pcm, 2, bps, bs, level as i32, true);
                assert_eq!(
                    rust, c,
                    "[bps {bps} level {level} seed {seed}] full stream differs"
                );
            }
        }
    }
}

/// Phase 8: an APPLICATION metadata block must serialize byte-identically to
/// libFLAC. The C side sets a manually-filled APPLICATION block; the full stream
/// (STREAMINFO + APPLICATION + frames, no auto VORBIS_COMMENT once metadata is
/// set) must match the Rust `MetadataBlock::Application` output.
#[test]
fn application_metadata_matches_c() {
    let bs = 2048u32;
    let pcm = gen_pcm_bps(3, bs as usize + 200, 16);
    let id = *b"riff";
    let data: Vec<u8> = (0..53u8).map(|i| i.wrapping_mul(7)).collect();
    // libFLAC auto-inserts its default VORBIS_COMMENT (vendor) ahead of any
    // user metadata, so include it explicitly to compare like-for-like.
    let vendor = libflac_rs::testing::LIBFLAC_VENDOR_STRING;
    for level in [0u32, 8] {
        let rust = libflac_rs::testing::encode(
            &pcm,
            2,
            16,
            44_100,
            bs,
            &libflac_rs::testing::preset(level),
            true,
            &[
                MetadataBlock::VorbisComment(vendor),
                MetadataBlock::Application { id, data: &data },
            ],
        );
        let mut out = vec![0u8; pcm.len() * 4 + 8192];
        let mut out_len = out.len();
        let rc = unsafe {
            libflac_rs_cref_encode_full_app(
                pcm.as_ptr(),
                (pcm.len() / 2) as u32,
                2,
                16,
                44_100,
                bs,
                level as i32,
                1,
                id.as_ptr(),
                data.as_ptr(),
                data.len() as u32,
                out.as_mut_ptr(),
                &mut out_len,
            )
        };
        assert_eq!(rc, 0, "C encode_full_app returned {rc}");
        out.truncate(out_len);
        assert_eq!(
            rust, out,
            "[level {level}] APPLICATION stream differs from C"
        );
        // And it round-trips through our own decoder.
        let dec = libflac_rs::testing::decode(&rust).expect("decode");
        assert_eq!(
            dec.interleaved, pcm,
            "[level {level}] APPLICATION round-trip"
        );
    }
}

/// Phase 8: a PICTURE metadata block (cover art) must serialize byte-identically
/// to libFLAC. As with APPLICATION, libFLAC prepends its default VORBIS_COMMENT.
#[test]
fn picture_metadata_matches_c() {
    let bs = 2048u32;
    let pcm = gen_pcm_bps(4, bs as usize + 150, 16);
    let vendor = libflac_rs::testing::LIBFLAC_VENDOR_STRING;
    let mime = "image/png";
    let desc = "front cover";
    let pic_data: Vec<u8> = (0..200u16).map(|i| (i * 3) as u8).collect();
    let (ptype, w, h, depth, colors) = (3u32, 16u32, 16u32, 24u32, 0u32); // 3 = front cover
    let rust = libflac_rs::testing::encode(
        &pcm,
        2,
        16,
        44_100,
        bs,
        &libflac_rs::testing::preset(8),
        true,
        &[
            MetadataBlock::VorbisComment(vendor),
            MetadataBlock::Picture {
                picture_type: ptype,
                mime_type: mime,
                description: desc,
                width: w,
                height: h,
                depth,
                colors,
                data: &pic_data,
            },
        ],
    );
    // NUL-terminate mime/desc for the C strlen path.
    let mime_c = std::ffi::CString::new(mime).unwrap();
    let desc_c = std::ffi::CString::new(desc).unwrap();
    let mut out = vec![0u8; pcm.len() * 4 + 8192];
    let mut out_len = out.len();
    let rc = unsafe {
        libflac_rs_cref_encode_full_picture(
            pcm.as_ptr(),
            (pcm.len() / 2) as u32,
            2,
            16,
            44_100,
            bs,
            8,
            1,
            ptype,
            mime_c.as_ptr() as *const u8,
            desc_c.as_ptr() as *const u8,
            w,
            h,
            depth,
            colors,
            pic_data.as_ptr(),
            pic_data.len() as u32,
            out.as_mut_ptr(),
            &mut out_len,
        )
    };
    assert_eq!(rc, 0, "C encode_full_picture returned {rc}");
    out.truncate(out_len);
    assert_eq!(rust, out, "PICTURE stream differs from C");
    let dec = libflac_rs::testing::decode(&rust).expect("decode");
    assert_eq!(dec.interleaved, pcm, "PICTURE round-trip");
}

/// Target sample numbers for `num` evenly-spaced seek points (the formula in
/// `metadata::spaced_seek_points` / libFLAC's `append_spaced_points`), used to drive
/// both the Rust template and the C shim identically.
fn spaced_targets(num: u32, total: u64) -> Vec<u64> {
    libflac_rs::testing::spaced_seek_points(num, total)
        .iter()
        .map(|p| p.sample_number)
        .collect()
}

/// Phase 8: a SEEKTABLE block. Unlike APPLICATION/PICTURE (which the caller fully
/// supplies), libFLAC *generates* the seektable during encoding: each placeholder
/// point is filled with the frame holding its target sample (rewriting the sample to
/// the frame's first sample, recording the frame's byte offset and the *configured*
/// blocksize), then the table is sorted + uniquified at finish — collapsing multiple
/// targets that land in one frame and padding the freed tail with placeholders. The
/// Rust encoder must reproduce the filled+sorted table and the whole stream byte-for-
/// byte. As with the other metadata blocks, libFLAC prepends its default
/// VORBIS_COMMENT, so the block list starts with it.
#[test]
fn seektable_metadata_matches_c() {
    let vendor = libflac_rs::testing::LIBFLAC_VENDOR_STRING;
    let bs = 2048u32;
    let n_sparse = bs as usize * 4 + 500; // ~5 frames
    let n_dense = bs as usize * 2 + 700; // ~3 frames
    let n_explicit = bs as usize * 3 + 100; // ~4 frames (short final)
    let n_unclaimed = bs as usize * 2 + 10; // ~3 frames
    // Cases exercising: one point per frame (no dedup); many targets per frame
    // (heavy dedup -> trailing placeholders); explicit targets on/near frame
    // boundaries; a target past the end (never claimed -> kept as written); and a
    // single point.
    let cases: &[(usize, Vec<u64>)] = &[
        (n_sparse, spaced_targets(4, n_sparse as u64)),
        (n_dense, spaced_targets(32, n_dense as u64)),
        (
            n_explicit,
            vec![
                0,
                bs as u64,
                bs as u64 + 5,
                bs as u64 * 2,
                bs as u64 * 3 + 50,
            ],
        ),
        (n_unclaimed, vec![0, bs as u64, bs as u64 * 5]),
        (bs as usize + 10, vec![0]),
    ];
    for (n, targets) in cases {
        for &bps in &[16u32, 24] {
            for level in [0u32, 8] {
                let pcm = gen_pcm_bps(7, *n, bps);
                let template: Vec<libflac_rs::testing::SeekPoint> = targets
                    .iter()
                    .map(|&s| libflac_rs::testing::SeekPoint {
                        sample_number: s,
                        stream_offset: 0,
                        frame_samples: 0,
                    })
                    .collect();
                let rust = libflac_rs::testing::encode(
                    &pcm,
                    2,
                    bps,
                    44_100,
                    bs,
                    &libflac_rs::testing::preset(level),
                    true,
                    &[
                        MetadataBlock::VorbisComment(vendor),
                        MetadataBlock::Seektable(&template),
                    ],
                );
                let c = c_encode_full_seektable(&pcm, 2, bps, bs, level as i32, true, targets);
                assert_eq!(
                    rust,
                    c,
                    "[bps {bps} level {level} npts {} n {n}] SEEKTABLE stream differs",
                    targets.len()
                );
                // Round-trips, and our decoder recovers the (preserved) point count
                // in sorted order with placeholders last.
                let dec = libflac_rs::testing::decode(&rust).expect("decode");
                assert_eq!(dec.interleaved, pcm, "SEEKTABLE round-trip PCM");
                assert_eq!(
                    dec.seek_points.len(),
                    targets.len(),
                    "decoded seek point count"
                );
                let mut prev = 0u64;
                for p in &dec.seek_points {
                    assert!(p.sample_number >= prev, "decoded seek points sorted");
                    prev = p.sample_number;
                }
            }
        }
    }
}

/// One CUESHEET track for the test (owns its index list, flattened for the C FFI).
struct CueTrack {
    offset: u64,
    number: u8,
    isrc: [u8; 12],
    non_audio: bool,
    pre_emphasis: bool,
    indices: Vec<libflac_rs::testing::CueSheetIndex>,
}

/// Phase 8: a CUESHEET block. Fully caller-supplied (no encoder generation), so the
/// Rust serialization must match libFLAC byte-for-byte across the 396-byte fixed
/// header, the non-byte-aligned reserved runs, and the nested track/index lists.
/// Covers a non-CD cuesheet (a track with two indices + flag bits set, and a track
/// with no indices) and a legal CD-DA cuesheet (`is_cd` triggers the stricter
/// legality libFLAC validates at init). libFLAC prepends its default
/// VORBIS_COMMENT, so the block list starts with it.
#[test]
fn cuesheet_metadata_matches_c() {
    use libflac_rs::testing::{CueSheetIndex, CueSheetTrack};
    let vendor = libflac_rs::testing::LIBFLAC_VENDOR_STRING;
    let bs = 2048u32;
    let pcm = gen_pcm_bps(11, bs as usize + 400, 16);
    let mut mcn = [0u8; 128];
    mcn[..13].copy_from_slice(b"CATALOG012345");

    let cases: Vec<(&str, bool, u64, Vec<CueTrack>)> = vec![
        (
            "non_cd",
            false,
            0,
            vec![
                CueTrack {
                    offset: 0,
                    number: 1,
                    isrc: *b"ABCDE1234567",
                    non_audio: false,
                    pre_emphasis: false,
                    indices: vec![
                        CueSheetIndex {
                            offset: 0,
                            number: 0,
                        },
                        CueSheetIndex {
                            offset: 2000,
                            number: 1,
                        },
                    ],
                },
                CueTrack {
                    offset: 10000,
                    number: 2,
                    isrc: [0u8; 12],
                    non_audio: true,
                    pre_emphasis: true,
                    indices: vec![],
                },
            ],
        ),
        (
            "cd_da",
            true,
            88200, // 2s @ 44.1k, divisible by 588
            vec![
                CueTrack {
                    offset: 0,
                    number: 1,
                    isrc: *b"US1234567890",
                    non_audio: false,
                    pre_emphasis: false,
                    indices: vec![CueSheetIndex {
                        offset: 0,
                        number: 1,
                    }],
                },
                CueTrack {
                    offset: 176400, // divisible by 588
                    number: 170,    // lead-out
                    isrc: [0u8; 12],
                    non_audio: false,
                    pre_emphasis: false,
                    indices: vec![],
                },
            ],
        ),
    ];

    for (label, is_cd, lead_in, tracks) in &cases {
        let rust_tracks: Vec<CueSheetTrack> = tracks
            .iter()
            .map(|t| CueSheetTrack {
                offset: t.offset,
                number: t.number,
                isrc: t.isrc,
                non_audio: t.non_audio,
                pre_emphasis: t.pre_emphasis,
                indices: &t.indices,
            })
            .collect();
        // Flatten tracks + (cross-track, in order) indices for the C FFI.
        let track_offsets: Vec<u64> = tracks.iter().map(|t| t.offset).collect();
        let track_numbers: Vec<u8> = tracks.iter().map(|t| t.number).collect();
        let track_isrcs: Vec<u8> = tracks.iter().flat_map(|t| t.isrc).collect();
        let track_types: Vec<u8> = tracks.iter().map(|t| t.non_audio as u8).collect();
        let track_pre: Vec<u8> = tracks.iter().map(|t| t.pre_emphasis as u8).collect();
        let track_nidx: Vec<u8> = tracks.iter().map(|t| t.indices.len() as u8).collect();
        let idx_offsets: Vec<u64> = tracks
            .iter()
            .flat_map(|t| t.indices.iter().map(|ix| ix.offset))
            .collect();
        let idx_numbers: Vec<u8> = tracks
            .iter()
            .flat_map(|t| t.indices.iter().map(|ix| ix.number))
            .collect();

        for level in [0u32, 8] {
            let rust = libflac_rs::testing::encode(
                &pcm,
                2,
                16,
                44_100,
                bs,
                &libflac_rs::testing::preset(level),
                true,
                &[
                    MetadataBlock::VorbisComment(vendor),
                    MetadataBlock::CueSheet {
                        media_catalog_number: &mcn,
                        lead_in: *lead_in,
                        is_cd: *is_cd,
                        tracks: &rust_tracks,
                    },
                ],
            );
            let mut out = vec![0u8; pcm.len() * 4 + 8192];
            let mut out_len = out.len();
            let rc = unsafe {
                libflac_rs_cref_encode_full_cuesheet(
                    pcm.as_ptr(),
                    (pcm.len() / 2) as u32,
                    2,
                    16,
                    44_100,
                    bs,
                    level as i32,
                    1,
                    mcn.as_ptr(),
                    *lead_in,
                    *is_cd as i32,
                    tracks.len() as u32,
                    track_offsets.as_ptr(),
                    track_numbers.as_ptr(),
                    track_isrcs.as_ptr(),
                    track_types.as_ptr(),
                    track_pre.as_ptr(),
                    track_nidx.as_ptr(),
                    idx_offsets.as_ptr(),
                    idx_numbers.as_ptr(),
                    out.as_mut_ptr(),
                    &mut out_len,
                )
            };
            assert_eq!(
                rc, 0,
                "[{label} level {level}] C encode_full_cuesheet returned {rc}"
            );
            out.truncate(out_len);
            assert_eq!(rust, out, "[{label} level {level}] CUESHEET stream differs");
            let dec = libflac_rs::testing::decode(&rust).expect("decode");
            assert_eq!(
                dec.interleaved, pcm,
                "[{label} level {level}] CUESHEET round-trip"
            );
        }
    }
}

fn c_encode_ogg(interleaved: &[i32], bps: u32, blocksize: u32, level: i32, serial: i32) -> Vec<u8> {
    let mut out = vec![0u8; interleaved.len() * 4 + 16384];
    let mut out_len = out.len();
    let rc = unsafe {
        libflac_rs_cref_encode_ogg(
            interleaved.as_ptr(),
            (interleaved.len() / 2) as u32,
            2,
            bps,
            44_100,
            blocksize,
            level,
            1,
            serial,
            out.as_mut_ptr(),
            &mut out_len,
        )
    };
    assert_eq!(rc, 0, "C encode_ogg returned {rc}");
    out.truncate(out_len);
    out
}

/// Phase 10: Ogg FLAC. The Rust `encode_ogg` output must be byte-identical to
/// libFLAC+libogg (the oracle is built with FLAC__HAS_OGG=1). Covers small (single
/// EOS-flushed audio page) and large (multiple nominal audio pages) signals at a few
/// levels; libFLAC auto-inserts its default VORBIS_COMMENT, so the Rust block list is
/// just that. A fixed serial number makes both sides deterministic.
#[test]
fn ogg_stream_matches_c() {
    let vendor = libflac_rs::testing::LIBFLAC_VENDOR_STRING;
    let serial = 0x1234_5678i32;
    // Sizes spanning a single EOS-flushed audio page and many nominal pages; depths
    // covering RICE (8-bit) and RICE2 (24-bit) frames.
    for &(n, bs) in &[(5000usize, 4096u32), (60_000, 4096), (33_333, 2048)] {
        for &bps in &[8u32, 16, 24] {
            for level in [0u32, 5, 8] {
                let pcm = gen_pcm_bps(1, n, bps);
                let rust = libflac_rs::testing::encode_ogg(
                    &pcm,
                    2,
                    bps,
                    44_100,
                    bs,
                    &libflac_rs::testing::preset(level),
                    true,
                    &[MetadataBlock::VorbisComment(vendor)],
                    serial,
                );
                let c = c_encode_ogg(&pcm, bps, bs, level as i32, serial);
                if rust != c {
                    let first = (0..rust.len().min(c.len()))
                        .find(|&i| rust[i] != c[i])
                        .unwrap_or(rust.len().min(c.len()));
                    let lo = first.saturating_sub(6);
                    panic!(
                        "[n {n} bs {bs} bps {bps} level {level}] Ogg differs: rust.len={} c.len={} first@{first}\n  rust={:02x?}\n  c   ={:02x?}",
                        rust.len(),
                        c.len(),
                        &rust[lo..(first + 20).min(rust.len())],
                        &c[lo..(first + 20).min(c.len())],
                    );
                }
            }
        }
    }
}

/// Decode **real libFLAC Ogg output** back to PCM: encode via the oracle, decode with
/// our `decode_ogg`, and confirm the samples and embedded MD5 round-trip — across bit
/// depths. Proves Ogg page demux + FLAC demap + frame decode against the C reference.
#[test]
fn decode_ogg_libflac_streams() {
    for &bps in &[8u32, 16, 20, 24] {
        for level in [0u32, 8] {
            let pcm = gen_pcm_bps(2, 40_000, bps);
            let c = c_encode_ogg(&pcm, bps, 4096, level as i32, 0x0BAD_F00D_u32 as i32);
            let dec = libflac_rs::testing::decode_ogg(&c).expect("decode_ogg");
            assert_eq!(dec.channels, 2, "[bps {bps} level {level}]");
            assert_eq!(dec.bits_per_sample, bps);
            assert_eq!(dec.total_samples, (pcm.len() / 2) as u64);
            assert!(dec.md5_ok, "[bps {bps} level {level}] Ogg MD5");
            assert_eq!(dec.interleaved, pcm, "[bps {bps} level {level}] Ogg PCM");
        }
    }
}

/// The decoder against **real libFLAC output**: decode complete streams the C
/// reference produced (marker + metadata + frames, MD5 on) and confirm the PCM is
/// reproduced exactly and the embedded MD5 verifies — across every bit depth.
#[test]
fn decode_libflac_streams() {
    let bs = 2048u32;
    for &bps in &[8u32, 12, 16, 20, 24, 32] {
        for level in [0u32, 5, 8] {
            for seed in 1..=2u32 {
                let pcm = gen_pcm_bps(seed, bs as usize + 600, bps);
                let c = c_encode_full(&pcm, 2, bps, bs, level as i32, true);
                let dec = libflac_rs::testing::decode(&c).expect("decode libFLAC stream");
                assert_eq!(dec.channels, 2);
                assert_eq!(dec.bits_per_sample, bps);
                assert!(
                    dec.md5_ok,
                    "[bps {bps} level {level} seed {seed}] MD5 verify"
                );
                assert_eq!(
                    dec.interleaved, pcm,
                    "[bps {bps} level {level} seed {seed}] decoded PCM differs"
                );
            }
        }
    }
}

fn lcg(state: &mut u32) -> u32 {
    *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    *state
}

/// Diverse stereo 16-bit PCM (a few random sine partials + noise, clamped),
/// seeded — exercises every fixed order and the Rice partition search. Returns
/// interleaved i32. The float generation is test-only (not on the bit-exact path).
fn gen_pcm(seed: u32, samples_per_channel: usize) -> Vec<i32> {
    let mut st = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    let urand = |st: &mut u32| (lcg(st) >> 8) as f64 / 16_777_216.0;
    let np = 1 + (urand(&mut st) * 4.0) as usize;
    let partials: Vec<(f64, f64, f64)> = (0..np)
        .map(|_| {
            (
                urand(&mut st) * 3.1,
                200.0 + urand(&mut st) * 7000.0,
                urand(&mut st) * std::f64::consts::TAU,
            )
        })
        .collect();
    let noise = urand(&mut st) * urand(&mut st) * 2000.0;

    let mut out = Vec::with_capacity(samples_per_channel * 2);
    for i in 0..samples_per_channel {
        for ch in 0..2u32 {
            let mut v = 0.0f64;
            for &(f, a, p) in &partials {
                v += a * (f * i as f64 + p + ch as f64 * 0.4).sin();
            }
            v += noise * ((lcg(&mut st) >> 16) as u16 as i16 as f64 / 32768.0);
            out.push(v.round().clamp(-32768.0, 32767.0) as i32);
        }
    }
    out
}

#[test]
fn fixed_subframes_match_c() {
    let bs = 2048u32;
    for seed in 1..=40u32 {
        // Non-block-multiple lengths exercise the short final frame too.
        let samples = bs as usize + (seed as usize * 311) % 3000;
        let pcm = gen_pcm(seed, samples);
        let rust = rust_frames(&pcm, 2, bs);
        let c = c_encode(&pcm, 2, 16, bs, 0, 0); // fixed-only, independent stereo
        assert_eq!(
            rust, c,
            "[seed {seed}, {samples} samples] fixed frames differ from C"
        );
    }
}
