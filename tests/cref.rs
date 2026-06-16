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
    fn libflac_rs_cref_crc8(data: *const u8, len: u32) -> u8;
    fn libflac_rs_cref_crc16(data: *const u8, len: u32) -> u16;
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

// --- F1: CONSTANT / VERBATIM subframes + framing (fixed-only, independent) -----

/// Rust frame bytes for interleaved 16-bit PCM, CHD's audio format.
fn rust_frames(interleaved: &[i32], channels: u32, blocksize: u32) -> Vec<u8> {
    libflac_rs::testing::encode_frames(interleaved, channels, 16, 44_100, blocksize)
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
