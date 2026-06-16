//! Fixed predictors (orders 0–4), ported from `fixed.c`.

use crate::format::MAX_FIXED_ORDER;

/// Pick the fixed predictor order 0..=4 minimizing the sum of `|residual|` over
/// samples `[MAX_FIXED_ORDER, len)`, preferring the lowest order on ties
/// (`FLAC__fixed_compute_best_predictor`, `fixed.c:222`). Integer-only: the
/// residual differences in `i64` with a 64-bit accumulator reproduce both the C's
/// 32-bit and `_wide` paths for <=17-bit signals (neither the differences nor the
/// sums overflow). The float bits-per-sample estimate the C also fills is only
/// used by the LPC path and is omitted here.
pub fn compute_best_predictor_order(signal: &[i32]) -> u32 {
    let mut te = [0u64; 5];
    for i in MAX_FIXED_ORDER as usize..signal.len() {
        let d0 = signal[i] as i64;
        let d1 = signal[i - 1] as i64;
        let d2 = signal[i - 2] as i64;
        let d3 = signal[i - 3] as i64;
        let d4 = signal[i - 4] as i64;
        te[0] += d0.unsigned_abs();
        te[1] += (d0 - d1).unsigned_abs();
        te[2] += (d0 - 2 * d1 + d2).unsigned_abs();
        te[3] += (d0 - 3 * d1 + 3 * d2 - d3).unsigned_abs();
        te[4] += (d0 - 4 * d1 + 6 * d2 - 4 * d3 + d4).unsigned_abs();
    }
    // Prefer the lowest order (the C uses `<=` against the min of the rest).
    if te[0] <= te[1].min(te[2]).min(te[3]).min(te[4]) {
        0
    } else if te[1] <= te[2].min(te[3]).min(te[4]) {
        1
    } else if te[2] <= te[3].min(te[4]) {
        2
    } else if te[3] <= te[4] {
        3
    } else {
        4
    }
}

/// Fixed-predictor residual for `order` over `signal[order..]`
/// (`FLAC__fixed_compute_residual`, `fixed.c:470`). `i32` arithmetic is exact for
/// the <=27-bit residuals produced by 16/17-bit signals.
pub fn compute_residual(signal: &[i32], order: u32) -> Vec<i32> {
    let n = signal.len();
    match order {
        0 => signal.to_vec(),
        1 => (1..n).map(|i| signal[i] - signal[i - 1]).collect(),
        2 => (2..n)
            .map(|i| signal[i] - 2 * signal[i - 1] + signal[i - 2])
            .collect(),
        3 => (3..n)
            .map(|i| signal[i] - 3 * signal[i - 1] + 3 * signal[i - 2] - signal[i - 3])
            .collect(),
        4 => (4..n)
            .map(|i| {
                signal[i] - 4 * signal[i - 1] + 6 * signal[i - 2] - 4 * signal[i - 3]
                    + signal[i - 4]
            })
            .collect(),
        _ => unreachable!("fixed order <= 4"),
    }
}
