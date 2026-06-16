//! Block/frame orchestration, ported from `process_frame_` /
//! `process_subframes_` / `process_subframe_` (`stream_encoder.c`). Independent
//! channels with CONSTANT/VERBATIM/FIXED subframes (the level-8 settings with LPC
//! off). LPC and the mid-side channel decision are added in later milestones.

use crate::bitwriter::BitWriter;
use crate::format::MAX_FIXED_ORDER;
use crate::frame::{ChannelAssignment, FrameHeader, write_frame_footer, write_frame_header};
use crate::{fixed, rice, subframe};

// Level-8 partition-order bounds, and the Rice parameter limit for a 16-bit
// stream (escape parameter 15; RICE2 is only used above 16 bps).
const MIN_RESIDUAL_PARTITION_ORDER: u32 = 0;
const MAX_RESIDUAL_PARTITION_ORDER: u32 = 6;
const RICE_PARAMETER_LIMIT_16BPS: u32 = 15;

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

enum Choice {
    Constant,
    Verbatim,
    Fixed {
        order: u32,
        residual: Vec<i32>,
        rice: rice::RicePartition,
    },
}

/// Choose and write the smallest-estimate subframe for one (already
/// wasted-bits-shifted) channel block; returns its estimated bit cost
/// (`process_subframe_`, `stream_encoder.c:3441`). VERBATIM is the baseline;
/// CONSTANT wins for a single repeated value; otherwise FIXED competes. On ties
/// the baseline/earlier candidate is kept (strict `<`).
fn process_subframe(
    bw: &mut BitWriter,
    signal: &[i32],
    subframe_bps: u32,
    wasted_bits: u32,
    min_partition_order: u32,
    max_partition_order: u32,
) -> u32 {
    let blocksize = signal.len() as u32;
    let mut best_bits = subframe::verbatim_bits(blocksize, subframe_bps, wasted_bits);
    let mut best = Choice::Verbatim;

    if blocksize > MAX_FIXED_ORDER {
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
    }
    best_bits
}

/// Encode interleaved integer PCM into FLAC audio frames (no metadata), each
/// channel independent. The block size is fixed except for a possibly shorter
/// final frame.
pub fn encode_frames(
    interleaved: &[i32],
    channels: u32,
    bits_per_sample: u32,
    sample_rate: u32,
    blocksize: u32,
) -> Vec<u8> {
    let ch = channels as usize;
    assert!(ch > 0 && interleaved.len() % ch == 0, "ragged interleave");
    let total = interleaved.len() / ch;

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
                min_partition_order,
                max_partition_order,
            );
        }

        write_frame_footer(&mut frame);
        out.extend_from_slice(frame.as_bytes());

        start += bs;
        frame_number += 1;
    }
    out
}
