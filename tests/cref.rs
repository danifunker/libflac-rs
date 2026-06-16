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
            let mut win = vec![0f32; bs];
            libflac_rs::testing::window::tukey(&mut win, level8_p);

            let mut rust = vec![0f32; bs];
            libflac_rs::testing::lpc::window_data(&sig, &win, &mut rust, bs);
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
        for &(b, shift) in &configs {
            let part_size = bs / b / 2;
            let read_len = bs / b;
            let mut rust = vec![0f32; bs];
            libflac_rs::testing::lpc::window_data_partial(
                &sig, &win, &mut rust, bs, part_size, shift,
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
            let mut win = vec![0f32; bs];
            libflac_rs::testing::window::tukey(&mut win, level8_p);
            let mut windowed = vec![0f32; bs];
            libflac_rs::testing::lpc::window_data(&sig, &win, &mut windowed, bs);

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
    let sig = mono_signal(seed, bs);
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
        let mut win = vec![0f32; bs];
        libflac_rs::testing::window::tukey(&mut win, 0.5f32 / 3.0);
        let mut windowed = vec![0f32; bs];
        libflac_rs::testing::lpc::window_data(&sig, &win, &mut windowed, bs);
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
                libflac_rs::testing::lpc::compute_residual(&sig, order, &q.qlp_coeff, q.shift);
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
/// mirrors the C staging knob (0 = fixed-only, 12 = level-8 LPC).
fn rust_frames_lpc(
    interleaved: &[i32],
    channels: u32,
    blocksize: u32,
    max_lpc_order: u32,
) -> Vec<u8> {
    libflac_rs::testing::encode_frames(interleaved, channels, 16, 44_100, blocksize, max_lpc_order)
}

/// Fixed-only Rust frame bytes (F1).
fn rust_frames(interleaved: &[i32], channels: u32, blocksize: u32) -> Vec<u8> {
    rust_frames_lpc(interleaved, channels, blocksize, 0)
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
