//! Block/frame orchestration, ported from `process_frame_` /
//! `process_subframes_` / `process_subframe_` (`stream_encoder.c`). Independent
//! channels with CONSTANT/VERBATIM/FIXED/LPC subframes (the level-8 settings with
//! mid-side off). The mid-side channel decision is added in a later milestone.

use crate::bitmath;
use crate::bitwriter::BitWriter;
use crate::format::{
    MAX_FIXED_ORDER, MAX_LPC_ORDER, MAX_QLP_COEFF_PRECISION, MIN_QLP_COEFF_PRECISION,
};
use crate::frame::{ChannelAssignment, FrameHeader, write_frame_footer, write_frame_header};
use crate::{fixed, lpc, rice, subframe, window};

// Level-8 partition-order bounds, and the Rice parameter limit for a 16-bit
// stream (escape parameter 15; RICE2 is only used above 16 bps).
const MIN_RESIDUAL_PARTITION_ORDER: u32 = 0;
const MAX_RESIDUAL_PARTITION_ORDER: u32 = 6;
const RICE_PARAMETER_LIMIT_16BPS: u32 = 15;

// Level-8 `subdivide_tukey(3)` apodization: parts = 3, Tukey p = 0.5/parts.
const SUBDIVIDE_TUKEY_PARTS: i32 = 3;

/// Detect and strip wasted (common trailing-zero) bits, mutating `signal` in
/// place and returning the shift (`get_wasted_bits_`, `stream_encoder.c:4469`).
/// All-zero signal -> shift 0 (silence keeps its full bps).
pub(crate) fn get_wasted_bits(signal: &mut [i32]) -> u32 {
    let mut x = 0i32;
    let mut i = 0;
    while i < signal.len() && x & 1 == 0 {
        x |= signal[i];
        i += 1;
    }
    let shift = if x == 0 { 0 } else { x.trailing_zeros() };
    if shift > 0 {
        for s in signal.iter_mut() {
            *s >>= shift;
        }
    }
    shift
}

/// Auto QLP-coefficient precision (`stream_encoder.c:704`), derived once at init
/// from the *stream* bits-per-sample and the (full) block size; the short final
/// frame reuses the same value.
fn auto_qlp_coeff_precision(bits_per_sample: u32, blocksize: u32) -> u32 {
    if bits_per_sample < 16 {
        (2 + bits_per_sample / 2).max(MIN_QLP_COEFF_PRECISION)
    } else if bits_per_sample == 16 {
        match blocksize {
            0..=192 => 7,
            193..=384 => 8,
            385..=576 => 9,
            577..=1152 => 10,
            1153..=2304 => 11,
            2305..=4608 => 12,
            _ => 13,
        }
    } else {
        match blocksize {
            0..=384 => MAX_QLP_COEFF_PRECISION - 2,
            385..=1152 => MAX_QLP_COEFF_PRECISION - 1,
            _ => MAX_QLP_COEFF_PRECISION,
        }
    }
}

/// Settings the LPC path needs (the level-8 subset), plus the precomputed window
/// for the current frame's block size.
struct LpcCtx<'a> {
    window: &'a [f32],
    max_lpc_order: u32,
    qlp_coeff_precision: u32,
    parts: i32,
}

/// One fully-evaluated LPC subframe candidate.
struct LpcChoice {
    order: u32,
    qlp_coeff: Vec<i32>,
    precision: u32,
    shift: i32,
    residual: Vec<i32>,
    rice: rice::RicePartition,
}

enum Choice {
    Constant,
    Verbatim,
    Fixed {
        order: u32,
        residual: Vec<i32>,
        rice: rice::RicePartition,
    },
    Lpc(LpcChoice),
}

/// Advance the `subdivide_tukey` a/b/c state to the next sub-window
/// (`set_next_subdivide_tukey`, `stream_encoder.c:3686`). `a` = apodization index,
/// `depth` = subdivision denominator (b), `part` = interleaved partial/punchout
/// counter (c).
fn set_next_subdivide_tukey(parts: i32, a: &mut u32, depth: &mut u32, part: &mut u32) {
    if *depth == 2 {
        // Depth 2 does only partial, no (near-redundant) punchout.
        if *part == 0 {
            *part = 2;
        } else {
            *part = 0;
            *depth += 1;
        }
    } else if *part < 2 * *depth - 1 {
        *part += 1;
    } else {
        *part = 0;
        *depth += 1;
    }
    if *depth > parts as u32 {
        *a += 1;
        *depth = 1;
        *part = 0;
    }
}

/// One step of the apodization state machine (`apply_apodization_`,
/// `stream_encoder.c:3711`) for a `subdivide_tukey` apodization. Produces the LP
/// coefficients and a guess order for the current sub-window, advancing `a/b/c`,
/// or returns `None` to skip this sub-window (tiny block, or a constant signal).
#[allow(clippy::too_many_arguments)]
fn apply_apodization(
    signal: &[i32],
    window: &[f32],
    windowed: &mut [f32],
    autoc: &mut [f64],
    autoc_root: &mut [f64],
    blocksize: usize,
    max_order: usize,
    subframe_bps: u32,
    ctx: &LpcCtx,
    a: &mut u32,
    b: &mut u32,
    c: &mut u32,
) -> Option<(lpc::LpCoefficients, usize)> {
    let lag = max_order + 1;
    if *b == 1 {
        // Window the full block (the "root" autocorrelation).
        lpc::window_data(signal, window, windowed, blocksize);
        lpc::compute_autocorrelation(&windowed[..blocksize], lag, autoc);
        autoc_root[..max_order].copy_from_slice(&autoc[..max_order]);
        *b += 1;
    } else {
        let bb = *b as usize;
        if blocksize / bb <= MAX_LPC_ORDER as usize {
            // Windowing parts <= 32 samples is unsupported; skip and advance.
            set_next_subdivide_tukey(ctx.parts, a, b, c);
            return None;
        }
        if *c % 2 == 0 {
            // Even c: the (c/2)th partial window of size blocksize/b.
            let part_size = blocksize / bb / 2;
            let data_shift = (*c as usize / 2 * blocksize) / bb;
            lpc::window_data_partial(signal, window, windowed, blocksize, part_size, data_shift);
            lpc::compute_autocorrelation(&windowed[..blocksize / bb], lag, autoc);
        } else {
            // Odd c: the root window minus the previous partial (a punchout). Only
            // the first `max_order` lags are subtracted; autoc[max_order] is left
            // as the previous partial's value, exactly as libFLAC does.
            for i in 0..max_order {
                autoc[i] = autoc_root[i] - autoc[i];
            }
        }
        set_next_subdivide_tukey(ctx.parts, a, b, c);
    }

    if autoc[0] == 0.0 {
        // Signal is constant; can't do LP.
        return None;
    }
    let lp = lpc::compute_lp_coefficients(autoc, max_order);
    // do_qlp_coeff_prec_search is false at level 8, so the order-selection overhead
    // uses the actual qlp precision.
    let overhead = subframe_bps + ctx.qlp_coeff_precision;
    let guess_order = lpc::compute_best_order(&lp.error, lp.max_order, blocksize as u32, overhead);
    Some((lp, guess_order))
}

/// Quantize + residual + rice + estimate for one (order, coefficient row)
/// (`evaluate_lpc_subframe_`, `stream_encoder.c:3954`). Returns `None` when the
/// coefficients can't be quantized or the residual overflows i32 (the C returns 0,
/// meaning "can't LPC at this order").
#[allow(clippy::too_many_arguments)]
fn evaluate_lpc_subframe(
    signal: &[i32],
    subframe_bps: u32,
    wasted_bits: u32,
    lp_coeff_row: &[f32],
    order: u32,
    qlp_coeff_precision: u32,
    rice_parameter_limit: u32,
    min_partition_order: u32,
    max_partition_order: u32,
) -> Option<(LpcChoice, u32)> {
    let order_us = order as usize;

    // Keep qlp precision low enough that decode of <=16bps(+1 for side) needs only
    // 32-bit math.
    let mut precision = qlp_coeff_precision;
    if subframe_bps <= 17 {
        precision = precision.min(32 - subframe_bps - bitmath::ilog2(order));
    }

    let q = lpc::quantize_coefficients(lp_coeff_row, order_us, precision).ok()?;

    let residual = if lpc::max_residual_bps(subframe_bps, &q.qlp_coeff, order_us, q.shift) > 32 {
        lpc::compute_residual_limit(signal, order_us, &q.qlp_coeff, q.shift)?
    } else {
        lpc::compute_residual(signal, order_us, &q.qlp_coeff, q.shift)
    };

    let (rice, residual_bits) = rice::find_best_partition_order(
        &residual,
        order,
        rice_parameter_limit,
        min_partition_order,
        max_partition_order,
    );
    let bits = subframe::lpc_bits(order, precision, subframe_bps, wasted_bits, residual_bits);

    Some((
        LpcChoice {
            order,
            qlp_coeff: q.qlp_coeff,
            precision,
            shift: q.shift,
            residual,
            rice,
        },
        bits,
    ))
}

/// Best LPC subframe over all `subdivide_tukey` sub-windows (the LPC arm of
/// `process_subframe_`, `stream_encoder.c:3594`). Each sub-window is evaluated at
/// its single guess order (no exhaustive/precision search at level 8). Returns the
/// lowest-bit candidate, or `None` if none could be produced.
#[allow(clippy::too_many_arguments)]
fn best_lpc_subframe(
    signal: &[i32],
    subframe_bps: u32,
    wasted_bits: u32,
    blocksize: u32,
    ctx: &LpcCtx,
    rice_parameter_limit: u32,
    min_partition_order: u32,
    max_partition_order: u32,
) -> Option<(LpcChoice, u32)> {
    let max_lpc_order = if ctx.max_lpc_order >= blocksize {
        blocksize - 1
    } else {
        ctx.max_lpc_order
    };
    if max_lpc_order == 0 {
        return None;
    }

    let bs = blocksize as usize;
    let mut windowed = vec![0f32; bs];
    let mut autoc = [0f64; MAX_LPC_ORDER as usize + 1];
    let mut autoc_root = [0f64; MAX_LPC_ORDER as usize + 1];

    let mut best: Option<(LpcChoice, u32)> = None;
    // num_apodizations == 1 (a single subdivide_tukey); the state machine drives
    // `a` to 1 once the apodization is exhausted.
    let (mut a, mut b, mut c) = (0u32, 1u32, 0u32);
    while a < 1 {
        let max_order_this = max_lpc_order as usize;
        if let Some((lp, guess_order)) = apply_apodization(
            signal,
            ctx.window,
            &mut windowed,
            &mut autoc,
            &mut autoc_root,
            bs,
            max_order_this,
            subframe_bps,
            ctx,
            &mut a,
            &mut b,
            &mut c,
        ) {
            // Non-exhaustive: only the guess order is tried.
            let lpc_residual_bps =
                lpc::expected_bits(lp.error[guess_order - 1], blocksize - guess_order as u32);
            if lpc_residual_bps < subframe_bps as f64 {
                if let Some((choice, bits)) = evaluate_lpc_subframe(
                    signal,
                    subframe_bps,
                    wasted_bits,
                    lp.row(guess_order),
                    guess_order as u32,
                    ctx.qlp_coeff_precision,
                    rice_parameter_limit,
                    min_partition_order,
                    max_partition_order,
                ) {
                    if best.as_ref().is_none_or(|(_, bb)| bits < *bb) {
                        best = Some((choice, bits));
                    }
                }
            }
        }
    }
    best
}

/// Choose and write the smallest-estimate subframe for one (already
/// wasted-bits-shifted) channel block; returns its estimated bit cost
/// (`process_subframe_`, `stream_encoder.c:3441`). VERBATIM is the baseline;
/// CONSTANT wins for a single repeated value; otherwise FIXED and LPC compete. On
/// ties the earlier candidate is kept (strict `<`).
#[allow(clippy::too_many_arguments)]
fn process_subframe(
    bw: &mut BitWriter,
    signal: &[i32],
    subframe_bps: u32,
    wasted_bits: u32,
    blocksize: u32,
    min_partition_order: u32,
    max_partition_order: u32,
    lpc_ctx: Option<&LpcCtx>,
) -> u32 {
    let bs = signal.len() as u32;
    let mut best_bits = subframe::verbatim_bits(bs, subframe_bps, wasted_bits);
    let mut best = Choice::Verbatim;

    if bs > MAX_FIXED_ORDER {
        if signal.iter().all(|&s| s == signal[0]) {
            let cb = subframe::constant_bits(subframe_bps, wasted_bits);
            if cb < best_bits {
                best_bits = cb;
                best = Choice::Constant;
            }
        } else {
            let order = fixed::compute_best_predictor_order(signal);
            let residual = fixed::compute_residual(signal, order);
            let (rice_part, residual_bits) = rice::find_best_partition_order(
                &residual,
                order,
                RICE_PARAMETER_LIMIT_16BPS,
                min_partition_order,
                max_partition_order,
            );
            let fb = subframe::fixed_bits(order, subframe_bps, wasted_bits, residual_bits);
            if fb < best_bits {
                best_bits = fb;
                best = Choice::Fixed {
                    order,
                    residual,
                    rice: rice_part,
                };
            }

            if let Some(ctx) = lpc_ctx {
                if let Some((lpc_choice, lpc_bits)) = best_lpc_subframe(
                    signal,
                    subframe_bps,
                    wasted_bits,
                    blocksize,
                    ctx,
                    RICE_PARAMETER_LIMIT_16BPS,
                    min_partition_order,
                    max_partition_order,
                ) {
                    if lpc_bits < best_bits {
                        best_bits = lpc_bits;
                        best = Choice::Lpc(lpc_choice);
                    }
                }
            }
        }
    }

    match best {
        Choice::Constant => {
            subframe::write_constant(bw, signal[0] as i64, subframe_bps, wasted_bits)
        }
        Choice::Verbatim => subframe::write_verbatim(bw, signal, subframe_bps, wasted_bits),
        Choice::Fixed {
            order,
            residual,
            rice,
        } => subframe::write_fixed(
            bw,
            order,
            &signal[..order as usize],
            subframe_bps,
            wasted_bits,
            &residual,
            &rice,
        ),
        Choice::Lpc(c) => subframe::write_lpc(
            bw,
            c.order,
            &signal[..c.order as usize],
            &c.qlp_coeff,
            c.precision,
            c.shift,
            subframe_bps,
            wasted_bits,
            &c.residual,
            &c.rice,
        ),
    }
    best_bits
}

/// Encode interleaved integer PCM into FLAC audio frames (no metadata), each
/// channel independent. `max_lpc_order` mirrors the C staging knob: 0 disables
/// LPC (fixed/constant/verbatim only), otherwise it is the LPC order cap (12 at
/// level 8). The block size is fixed except for a possibly shorter final frame.
pub fn encode_frames(
    interleaved: &[i32],
    channels: u32,
    bits_per_sample: u32,
    sample_rate: u32,
    blocksize: u32,
    max_lpc_order: u32,
) -> Vec<u8> {
    let ch = channels as usize;
    assert!(ch > 0 && interleaved.len() % ch == 0, "ragged interleave");
    let total = interleaved.len() / ch;

    // Auto qlp precision is fixed once from the stream bps and the full block size.
    let qlp_coeff_precision = auto_qlp_coeff_precision(bits_per_sample, blocksize);
    // The apodization window, recomputed only when the frame block size changes
    // (i.e. once for the full frames, again for a short final frame).
    let tukey_p = 0.5f32 / SUBDIVIDE_TUKEY_PARTS as f32;
    let mut window_bs = 0usize;
    let mut window_buf: Vec<f32> = Vec::new();

    let mut out = Vec::new();
    let mut frame_number = 0u32;
    let mut start = 0usize;
    while start < total {
        let bs = (total - start).min(blocksize as usize);

        // Per-frame Rice partition-order bounds (`process_subframes_:3163`). The
        // C clamps min to max (`flac_min`); at level 8 min is 0 so that is a no-op,
        // and `find_best_partition_order` re-clamps min against the limited max.
        let max_partition_order =
            rice::max_partition_order_from_blocksize(bs as u32).min(MAX_RESIDUAL_PARTITION_ORDER);
        let min_partition_order = MIN_RESIDUAL_PARTITION_ORDER;

        // (Re)compute the window for this block size if LPC is enabled.
        if max_lpc_order > 0 && bs > 1 && bs != window_bs {
            window_buf = vec![0f32; bs];
            window::tukey(&mut window_buf, tukey_p);
            window_bs = bs;
        }
        let lpc_ctx = (max_lpc_order > 0 && bs > 1).then(|| LpcCtx {
            window: &window_buf,
            max_lpc_order,
            qlp_coeff_precision,
            parts: SUBDIVIDE_TUKEY_PARTS,
        });

        let mut frame = BitWriter::new();
        write_frame_header(
            &mut frame,
            &FrameHeader {
                blocksize: bs as u32,
                sample_rate,
                channels,
                channel_assignment: ChannelAssignment::Independent,
                bits_per_sample,
                frame_number,
            },
        );

        for c in 0..ch {
            let mut sig: Vec<i32> = (0..bs).map(|i| interleaved[(start + i) * ch + c]).collect();
            let mut wasted = get_wasted_bits(&mut sig);
            if wasted > bits_per_sample {
                wasted = bits_per_sample;
            }
            process_subframe(
                &mut frame,
                &sig,
                bits_per_sample - wasted,
                wasted,
                bs as u32,
                min_partition_order,
                max_partition_order,
                lpc_ctx.as_ref(),
            );
        }

        write_frame_footer(&mut frame);
        out.extend_from_slice(frame.as_bytes());

        start += bs;
        frame_number += 1;
    }
    out
}
