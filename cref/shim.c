/* cref/shim.c -- minimal FFI shim over the real libFLAC 1.4.3 encoder, used by
 * the differential tests to compare libflac-rs frame output against the C
 * reference byte-for-byte. Built only under the `cref` Cargo feature (see
 * build.rs); never part of the published crate.
 *
 * The shim captures only the encoder's frame writes (write-callback `samples`
 * > 0), discarding the "fLaC" stream marker and the metadata blocks exactly as
 * CHD does (flac.cpp m_strip_metadata). What remains is the raw audio-frame byte
 * stream the Rust port must reproduce. C89-style declarations for portability. */

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "FLAC/stream_encoder.h"
#include "private/crc.h"
#include "private/lpc.h"
#include "private/window.h"

/* CRC-8 / CRC-16 reference wrappers (FLAC__crc8/16, crc.c) for the F0
 * differential tests. */
FLAC__uint8 libflac_rs_cref_crc8(const FLAC__byte *data, uint32_t len) {
    return FLAC__crc8(data, len);
}
FLAC__uint16 libflac_rs_cref_crc16(const FLAC__byte *data, uint32_t len) {
    return FLAC__crc16(data, len);
}

typedef struct {
    uint8_t *buf;
    size_t cap;
    size_t len;
    int overflow;
} frame_sink;

static FLAC__StreamEncoderWriteStatus capture_frames(
    const FLAC__StreamEncoder *encoder, const FLAC__byte buffer[], size_t bytes,
    uint32_t samples, uint32_t current_frame, void *client_data) {
    frame_sink *s = (frame_sink *)client_data;
    (void)encoder;
    (void)current_frame;
    /* samples == 0 marks the stream marker / metadata; CHD strips those and keeps
     * only the audio frames (samples > 0). */
    if (samples == 0) {
        return FLAC__STREAM_ENCODER_WRITE_STATUS_OK;
    }
    if (s->len + bytes > s->cap) {
        s->overflow = 1;
        return FLAC__STREAM_ENCODER_WRITE_STATUS_FATAL_ERROR;
    }
    memcpy(s->buf + s->len, buffer, bytes);
    s->len += bytes;
    return FLAC__STREAM_ENCODER_WRITE_STATUS_OK;
}

/* Encode interleaved 32-bit PCM (`nsamples` frames of `channels`) with CHD's
 * level-8 configuration, returning only the raw audio frames (metadata stripped).
 *
 * `max_lpc_order` and `do_mid_side` < 0 leave the level-8 preset value; >= 0
 * overrides it, for staged differential testing (max_lpc_order = 0 forces
 * fixed/constant/verbatim subframes only; do_mid_side selects independent vs
 * decorrelated stereo). *out_len is capacity on input, produced length on output.
 * Returns 0 on success, a negative code on failure. */
int libflac_rs_cref_encode_cfg(const int32_t *interleaved, uint32_t nsamples,
                               uint32_t channels, uint32_t bps,
                               uint32_t sample_rate, uint32_t blocksize,
                               int32_t compression_level, int32_t max_lpc_order,
                               int32_t do_mid_side, uint8_t *out,
                               size_t *out_len) {
    frame_sink sink;
    FLAC__StreamEncoder *enc;
    FLAC__StreamEncoderInitStatus init;
    int rc = 0;

    sink.buf = out;
    sink.cap = *out_len;
    sink.len = 0;
    sink.overflow = 0;

    enc = FLAC__stream_encoder_new();
    if (!enc) {
        return -1;
    }

    /* Mirror CHD (flac.cpp:77, chdcodec.cpp): verify off, the requested
     * compression level (8 for CHD), the fixed audio format, no total-samples
     * estimate, streamable subset off, explicit block size, MD5 off (no effect on
     * frame bytes; metadata is stripped). */
    FLAC__stream_encoder_set_verify(enc, false);
    FLAC__stream_encoder_set_compression_level(
        enc, compression_level >= 0 ? (uint32_t)compression_level : 8);
    FLAC__stream_encoder_set_channels(enc, channels);
    FLAC__stream_encoder_set_bits_per_sample(enc, bps);
    FLAC__stream_encoder_set_sample_rate(enc, sample_rate);
    FLAC__stream_encoder_set_total_samples_estimate(enc, 0);
    FLAC__stream_encoder_set_streamable_subset(enc, false);
    FLAC__stream_encoder_set_blocksize(enc, blocksize);
    FLAC__stream_encoder_set_do_md5(enc, false);
    if (max_lpc_order >= 0) {
        FLAC__stream_encoder_set_max_lpc_order(enc, (uint32_t)max_lpc_order);
    }
    if (do_mid_side >= 0) {
        FLAC__stream_encoder_set_do_mid_side_stereo(enc, do_mid_side != 0);
    }

    init = FLAC__stream_encoder_init_stream(enc, capture_frames, NULL, NULL, NULL,
                                            &sink);
    if (init != FLAC__STREAM_ENCODER_INIT_STATUS_OK) {
        FLAC__stream_encoder_delete(enc);
        return -100 - (int)init;
    }

    if (!FLAC__stream_encoder_process_interleaved(enc, interleaved, nsamples)) {
        rc = -200 - (int)FLAC__stream_encoder_get_state(enc);
    } else if (!FLAC__stream_encoder_finish(enc)) {
        rc = -300 - (int)FLAC__stream_encoder_get_state(enc);
    }

    FLAC__stream_encoder_delete(enc);

    if (rc != 0) {
        return rc;
    }
    if (sink.overflow) {
        return -2;
    }
    *out_len = sink.len;
    return 0;
}

/* Backward-compatible entry point: the CHD level-8 config with the staged
 * max_lpc_order / do_mid_side overrides. */
int libflac_rs_cref_encode(const int32_t *interleaved, uint32_t nsamples,
                           uint32_t channels, uint32_t bps, uint32_t sample_rate,
                           uint32_t blocksize, int32_t max_lpc_order,
                           int32_t do_mid_side, uint8_t *out, size_t *out_len) {
    return libflac_rs_cref_encode_cfg(interleaved, nsamples, channels, bps,
                                      sample_rate, blocksize, 8, max_lpc_order,
                                      do_mid_side, out, out_len);
}

/* ---- F2 leaf-function wrappers --------------------------------------------
 * Expose the individual float-pipeline stages so the Rust port can be diffed
 * against the C reference one stage at a time (window -> windowing ->
 * autocorrelation -> Levinson -> best-order -> quantize -> residual), localizing
 * any float drift before the full LPC subframe is wired together. FLAC__real is
 * `float`. The 2-D lp_coeff[order][FLAC__MAX_LPC_ORDER] array is flattened. */

void libflac_rs_cref_window_tukey(float p, int32_t l, float *out) {
    FLAC__window_tukey(out, l, p);
}

void libflac_rs_cref_lpc_window_data(const int32_t *in, const float *window,
                                     float *out, uint32_t data_len) {
    FLAC__lpc_window_data(in, window, out, data_len);
}

void libflac_rs_cref_lpc_window_data_partial(const int32_t *in,
                                             const float *window, float *out,
                                             uint32_t data_len,
                                             uint32_t part_size,
                                             uint32_t data_shift) {
    FLAC__lpc_window_data_partial(in, window, out, data_len, part_size, data_shift);
}

void libflac_rs_cref_compute_autocorrelation(const float *data,
                                             uint32_t data_len, uint32_t lag,
                                             double *autoc) {
    FLAC__lpc_compute_autocorrelation(data, data_len, lag, autoc);
}

/* Runs Levinson-Durbin; writes the order-1..max_order coefficient rows into the
 * flat (FLAC__MAX_LPC_ORDER * FLAC__MAX_LPC_ORDER) buffer and the per-order error
 * into `error`. Returns the (possibly reduced, on err==0) max_order. */
uint32_t libflac_rs_cref_compute_lp_coefficients(const double *autoc,
                                                 uint32_t max_order,
                                                 float *lp_coeff_flat,
                                                 double *error) {
    FLAC__real lp_coeff[FLAC__MAX_LPC_ORDER][FLAC__MAX_LPC_ORDER];
    uint32_t mo = max_order;
    uint32_t i, j;
    FLAC__lpc_compute_lp_coefficients(autoc, &mo, lp_coeff, error);
    for (i = 0; i < FLAC__MAX_LPC_ORDER; i++)
        for (j = 0; j < FLAC__MAX_LPC_ORDER; j++)
            lp_coeff_flat[i * FLAC__MAX_LPC_ORDER + j] = lp_coeff[i][j];
    return mo;
}

double libflac_rs_cref_expected_bits(double lpc_error, uint32_t total_samples) {
    return FLAC__lpc_compute_expected_bits_per_residual_sample(lpc_error, total_samples);
}

uint32_t libflac_rs_cref_compute_best_order(const double *lpc_error,
                                            uint32_t max_order,
                                            uint32_t total_samples,
                                            uint32_t overhead_bits_per_order) {
    return FLAC__lpc_compute_best_order(lpc_error, max_order, total_samples,
                                        overhead_bits_per_order);
}

int libflac_rs_cref_quantize_coefficients(const float *lp_coeff, uint32_t order,
                                          uint32_t precision, int32_t *qlp_coeff,
                                          int32_t *shift) {
    int sh = 0;
    int ret = FLAC__lpc_quantize_coefficients(lp_coeff, order, precision, qlp_coeff, &sh);
    *shift = sh;
    return ret;
}

/* `signal` points at the warmup; reads `order` history samples then produces
 * blocksize-order residuals (mirrors evaluate_lpc_subframe_'s `signal+order`). */
void libflac_rs_cref_compute_residual(const int32_t *signal, uint32_t blocksize,
                                      const int32_t *qlp_coeff, uint32_t order,
                                      int lp_quantization, int32_t *residual) {
    FLAC__lpc_compute_residual_from_qlp_coefficients(
        signal + order, blocksize - order, qlp_coeff, order, lp_quantization, residual);
}
