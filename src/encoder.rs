//! Block/frame orchestration, ported from `process_frame_` /
//! `process_subframes_` / `process_subframe_` (`stream_encoder.c`).
//! CONSTANT/VERBATIM/FIXED/LPC subframes and, for stereo, the per-frame mid-side
//! channel decision (L/R vs L/S vs R/S vs M/S by estimated bits, including the
//! `loose_mid_side` periodic-redecision mode). All compression levels 0–8 are
//! supported via [`Config`]/[`preset`] (the `tukey(0.5)` / `subdivide_tukey(2|3)`
//! apodizations and the per-level LPC-order / partition-order caps). Currently
//! 16-bit; wider bit depths (RICE2, wide residual) come later.

use crate::bitmath;
use crate::bitwriter::BitWriter;
use crate::format::{
    MAX_FIXED_ORDER, MAX_LPC_ORDER, MAX_QLP_COEFF_PRECISION, MIN_QLP_COEFF_PRECISION,
};
use crate::frame::{ChannelAssignment, FrameHeader, write_frame_footer, write_frame_header};
use crate::{fixed, lpc, rice, subframe, window};

// Minimum residual partition order (0 for every compression level), and the Rice
// parameter limit for a 16-bit stream (escape parameter 15; RICE2 is only used
// above 16 bps).
const MIN_RESIDUAL_PARTITION_ORDER: u32 = 0;
const RICE_PARAMETER_LIMIT_16BPS: u32 = 15;

/// The apodization a compression level uses. Both variants ultimately window with
/// a Tukey window; `SubdivideTukey` additionally evaluates the per-subdivision
/// partial/punchout sub-windows via the a/b/c state machine.
#[derive(Clone, Copy)]
pub enum Apodization {
    /// A single Tukey window of the given `p` (e.g. `tukey(0.5)`).
    Tukey(f32),
    /// `subdivide_tukey(parts)`: a Tukey window of `p = 0.5/parts` plus its
    /// `parts`-deep partial/punchout subdivisions.
    SubdivideTukey(i32),
}

impl Apodization {
    /// The `p` of the underlying Tukey window.
    fn window_p(self) -> f32 {
        match self {
            Apodization::Tukey(p) => p,
            Apodization::SubdivideTukey(parts) => 0.5 / parts as f32,
        }
    }
    fn subdivide_parts(self) -> i32 {
        match self {
            Apodization::Tukey(_) => 1,
            Apodization::SubdivideTukey(parts) => parts,
        }
    }
    fn is_subdivide(self) -> bool {
        matches!(self, Apodization::SubdivideTukey(_))
    }
}

/// The compression settings of one libFLAC compression level (the fields of
/// `compression_levels_[]`, `stream_encoder.c:123`, that actually vary). The
/// constant fields — `min_residual_partition_order = 0`, `qlp_coeff_precision = 0`
/// (auto), and the escape/exhaustive/precision-search flags all `false` — are
/// implied.
#[derive(Clone, Copy)]
pub struct Config {
    pub do_mid_side: bool,
    pub loose_mid_side: bool,
    pub max_lpc_order: u32,
    pub max_residual_partition_order: u32,
    pub apodization: Apodization,
}

/// The preset for compression level `0..=8` (`compression_levels_[]`); levels
/// above 8 clamp to 8.
pub fn preset(level: u32) -> Config {
    use Apodization::{SubdivideTukey, Tukey};
    let (do_mid_side, loose_mid_side, max_lpc_order, max_residual_partition_order, apodization) =
        match level {
            0 => (false, false, 0, 3, Tukey(0.5)),
            1 => (true, true, 0, 3, Tukey(0.5)),
            2 => (true, false, 0, 3, Tukey(0.5)),
            3 => (false, false, 6, 4, Tukey(0.5)),
            4 => (true, true, 8, 4, Tukey(0.5)),
            5 => (true, false, 8, 5, Tukey(0.5)),
            6 => (true, false, 8, 6, SubdivideTukey(2)),
            7 => (true, false, 12, 6, SubdivideTukey(2)),
            _ => (true, false, 12, 6, SubdivideTukey(3)),
        };
    Config {
        do_mid_side,
        loose_mid_side,
        max_lpc_order,
        max_residual_partition_order,
        apodization,
    }
}

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

/// Settings the LPC path needs, plus the precomputed window for the current
/// frame's block size.
struct LpcCtx<'a> {
    window: &'a [f32],
    max_lpc_order: u32,
    qlp_coeff_precision: u32,
    apodization: Apodization,
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

/// A channel's chosen subframe plus everything needed to write it once the
/// channel assignment is decided.
struct ChosenChannel {
    signal: Vec<i32>,
    subframe_bps: u32,
    wasted_bits: u32,
    choice: Choice,
    bits: u32,
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
    let parts = ctx.apodization.subdivide_parts();
    let lag = max_order + 1;
    if *b == 1 {
        // Window the full block (the "root" autocorrelation).
        lpc::window_data(signal, window, windowed, blocksize);
        lpc::compute_autocorrelation(&windowed[..blocksize], lag, autoc);
        if ctx.apodization.is_subdivide() {
            autoc_root[..max_order].copy_from_slice(&autoc[..max_order]);
            *b += 1;
        } else {
            // A plain Tukey apodization is a single full-block window.
            *a += 1;
        }
    } else {
        let bb = *b as usize;
        if blocksize / bb <= MAX_LPC_ORDER as usize {
            // Windowing parts <= 32 samples is unsupported; skip and advance.
            set_next_subdivide_tukey(parts, a, b, c);
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
        set_next_subdivide_tukey(parts, a, b, c);
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

/// Choose the smallest-estimate subframe for one (already wasted-bits-shifted)
/// channel block, returning the choice and its estimated bit cost
/// (`process_subframe_`, `stream_encoder.c:3441`). VERBATIM is the baseline;
/// CONSTANT wins for a single repeated value; otherwise FIXED and LPC compete. On
/// ties the earlier candidate is kept (strict `<`). Writing is deferred so the
/// mid-side channel decision can pick among already-evaluated subframes.
#[allow(clippy::too_many_arguments)]
fn choose_subframe(
    signal: &[i32],
    subframe_bps: u32,
    wasted_bits: u32,
    blocksize: u32,
    min_partition_order: u32,
    max_partition_order: u32,
    lpc_ctx: Option<&LpcCtx>,
) -> (Choice, u32) {
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

    (best, best_bits)
}

/// Write a previously-chosen subframe (`add_subframe_`, `stream_encoder.c:3787`).
/// `signal` is the channel block the choice was made on (for warmup/raw samples).
fn write_choice(
    bw: &mut BitWriter,
    choice: &Choice,
    signal: &[i32],
    subframe_bps: u32,
    wasted_bits: u32,
) {
    match choice {
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
            *order,
            &signal[..*order as usize],
            subframe_bps,
            wasted_bits,
            residual,
            rice,
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
}

/// Encode interleaved integer PCM into FLAC audio frames (no metadata) with the
/// given compression `config`. For stereo with `do_mid_side`, each frame picks the
/// channel assignment with the fewest bits (or, with `loose_mid_side`, re-decides
/// only every ~0.4 s and reuses it between). The block size is fixed except for a
/// possibly shorter final frame.
pub fn encode_frames(
    interleaved: &[i32],
    channels: u32,
    bits_per_sample: u32,
    sample_rate: u32,
    blocksize: u32,
    config: &Config,
) -> Vec<u8> {
    let ch = channels as usize;
    assert!(ch > 0 && interleaved.len() % ch == 0, "ragged interleave");
    let total = interleaved.len() / ch;
    let stereo_ms = config.do_mid_side && ch == 2;

    // Auto qlp precision is fixed once from the stream bps and the full block size.
    let qlp_coeff_precision = auto_qlp_coeff_precision(bits_per_sample, blocksize);
    // The apodization window, recomputed only when the frame block size changes
    // (i.e. once for the full frames, again for a short final frame).
    let window_p = config.apodization.window_p();
    let mut window_bs = 0usize;
    let mut window_buf: Vec<f32> = Vec::new();

    // Loose mid-side: re-decide the assignment only every `loose_frames` frames
    // (`stream_encoder.c:894`), reusing it in between.
    let loose_frames = if config.loose_mid_side {
        ((sample_rate as f64 * 0.4 / blocksize as f64 + 0.5) as u32).max(1)
    } else {
        0
    };
    let mut loose_count = 0u32;
    let mut last_assignment = ChannelAssignment::Independent;

    let mut out = Vec::new();
    let mut frame_number = 0u32;
    let mut start = 0usize;
    while start < total {
        let bs = (total - start).min(blocksize as usize);

        // Per-frame Rice partition-order bounds (`process_subframes_:3163`). The
        // C clamps min to max (`flac_min`); min is 0 so that is a no-op, and
        // `find_best_partition_order` re-clamps min against the limited max.
        let max_partition_order = rice::max_partition_order_from_blocksize(bs as u32)
            .min(config.max_residual_partition_order);
        let min_partition_order = MIN_RESIDUAL_PARTITION_ORDER;

        // (Re)compute the window for this block size if LPC is enabled.
        if config.max_lpc_order > 0 && bs > 1 && bs != window_bs {
            window_buf = vec![0f32; bs];
            window::tukey(&mut window_buf, window_p);
            window_bs = bs;
        }
        let lpc_ctx = (config.max_lpc_order > 0 && bs > 1).then(|| LpcCtx {
            window: &window_buf,
            max_lpc_order: config.max_lpc_order,
            qlp_coeff_precision,
            apodization: config.apodization,
        });

        // Wasted-bits-shift a channel signal, then choose its best subframe.
        // `extra_bps` is 1 for the side channel (its values span one extra bit).
        let choose = |signal: Vec<i32>, extra_bps: u32| -> ChosenChannel {
            let mut sig = signal;
            let mut wasted = get_wasted_bits(&mut sig);
            if wasted > bits_per_sample {
                wasted = bits_per_sample;
            }
            let subframe_bps = bits_per_sample - wasted + extra_bps;
            let (choice, bits) = choose_subframe(
                &sig,
                subframe_bps,
                wasted,
                bs as u32,
                min_partition_order,
                max_partition_order,
                lpc_ctx.as_ref(),
            );
            ChosenChannel {
                signal: sig,
                subframe_bps,
                wasted_bits: wasted,
                choice,
                bits,
            }
        };

        let mut frame = BitWriter::new();

        if stereo_ms {
            let left: Vec<i32> = (0..bs).map(|i| interleaved[(start + i) * ch]).collect();
            let right: Vec<i32> = (0..bs).map(|i| interleaved[(start + i) * ch + 1]).collect();
            // Mid/side from the *original* (pre-wasted-shift) L/R
            // (`process_subframes_:3210`): side = L - R, mid = (L + R) >> 1.
            let mid_side = || {
                let mid: Vec<i32> = (0..bs).map(|i| (left[i] + right[i]) >> 1).collect();
                let side: Vec<i32> = (0..bs).map(|i| left[i] - right[i]).collect();
                (mid, side)
            };

            // Between loose decision points, reuse the last assignment and encode
            // only the channels it needs.
            let (assignment, pair): (ChannelAssignment, [ChosenChannel; 2]) =
                if config.loose_mid_side && loose_count != 0 {
                    if last_assignment == ChannelAssignment::Independent {
                        (
                            ChannelAssignment::Independent,
                            [choose(left, 0), choose(right, 0)],
                        )
                    } else {
                        let (mid, side) = mid_side();
                        (
                            ChannelAssignment::MidSide,
                            [choose(mid, 0), choose(side, 1)],
                        )
                    }
                } else {
                    let (mid, side) = mid_side();
                    let cl = choose(left, 0);
                    let cr = choose(right, 0);
                    let cm = choose(mid, 0);
                    let cs = choose(side, 1);
                    // Smallest total wins, independent preferred on ties. Loose
                    // decision frames consider only independent vs mid-side.
                    let mut assignment = ChannelAssignment::Independent;
                    let mut min_bits = cl.bits + cr.bits;
                    let all = !config.loose_mid_side;
                    for (enabled, bits, ca) in [
                        (all, cl.bits + cs.bits, ChannelAssignment::LeftSide),
                        (all, cr.bits + cs.bits, ChannelAssignment::RightSide),
                        (true, cm.bits + cs.bits, ChannelAssignment::MidSide),
                    ] {
                        if enabled && bits < min_bits {
                            min_bits = bits;
                            assignment = ca;
                        }
                    }
                    let pair = match assignment {
                        ChannelAssignment::Independent => [cl, cr],
                        ChannelAssignment::LeftSide => [cl, cs],
                        ChannelAssignment::RightSide => [cs, cr],
                        ChannelAssignment::MidSide => [cm, cs],
                    };
                    (assignment, pair)
                };

            write_frame_header(
                &mut frame,
                &FrameHeader {
                    blocksize: bs as u32,
                    sample_rate,
                    channels,
                    channel_assignment: assignment,
                    bits_per_sample,
                    frame_number,
                },
            );
            for cc in &pair {
                write_choice(
                    &mut frame,
                    &cc.choice,
                    &cc.signal,
                    cc.subframe_bps,
                    cc.wasted_bits,
                );
            }

            last_assignment = assignment;
            if config.loose_mid_side {
                loose_count += 1;
                if loose_count >= loose_frames {
                    loose_count = 0;
                }
            }
        } else {
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
                let signal: Vec<i32> = (0..bs).map(|i| interleaved[(start + i) * ch + c]).collect();
                let cc = choose(signal, 0);
                write_choice(
                    &mut frame,
                    &cc.choice,
                    &cc.signal,
                    cc.subframe_bps,
                    cc.wasted_bits,
                );
            }
        }

        write_frame_footer(&mut frame);
        out.extend_from_slice(frame.as_bytes());

        start += bs;
        frame_number += 1;
    }
    out
}
