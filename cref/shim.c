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
int libflac_rs_cref_encode(const int32_t *interleaved, uint32_t nsamples,
                           uint32_t channels, uint32_t bps, uint32_t sample_rate,
                           uint32_t blocksize, int32_t max_lpc_order,
                           int32_t do_mid_side, uint8_t *out, size_t *out_len) {
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

    /* Mirror CHD (flac.cpp:77, chdcodec.cpp): verify off, level 8, the fixed
     * audio format, no total-samples estimate, streamable subset off, explicit
     * block size, MD5 off (no effect on frame bytes; metadata is stripped). */
    FLAC__stream_encoder_set_verify(enc, false);
    FLAC__stream_encoder_set_compression_level(enc, 8);
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
