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

#include "FLAC/stream_decoder.h"
#include "FLAC/stream_encoder.h"
#include "private/crc.h"
#include "private/lpc.h"
#include "private/md5.h"
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

/* A seekable in-memory sink so the encoder rewrites STREAMINFO with the final
 * min/max framesize, total samples, and MD5 at finish. Captures the complete FLAC
 * stream (marker + metadata + frames) for verifying full-file output. */
typedef struct {
    uint8_t *buf;
    size_t cap;
    size_t pos;
    size_t len;
    int overflow;
} mem_sink;

static FLAC__StreamEncoderWriteStatus mem_write(const FLAC__StreamEncoder *encoder,
                                                const FLAC__byte buffer[],
                                                size_t bytes, uint32_t samples,
                                                uint32_t current_frame,
                                                void *client_data) {
    mem_sink *s = (mem_sink *)client_data;
    (void)encoder;
    (void)samples;
    (void)current_frame;
    if (s->pos + bytes > s->cap) {
        s->overflow = 1;
        return FLAC__STREAM_ENCODER_WRITE_STATUS_FATAL_ERROR;
    }
    memcpy(s->buf + s->pos, buffer, bytes);
    s->pos += bytes;
    if (s->pos > s->len) {
        s->len = s->pos;
    }
    return FLAC__STREAM_ENCODER_WRITE_STATUS_OK;
}

static FLAC__StreamEncoderSeekStatus mem_seek(const FLAC__StreamEncoder *encoder,
                                              FLAC__uint64 absolute_byte_offset,
                                              void *client_data) {
    mem_sink *s = (mem_sink *)client_data;
    (void)encoder;
    s->pos = (size_t)absolute_byte_offset;
    return FLAC__STREAM_ENCODER_SEEK_STATUS_OK;
}

static FLAC__StreamEncoderTellStatus mem_tell(const FLAC__StreamEncoder *encoder,
                                              FLAC__uint64 *absolute_byte_offset,
                                              void *client_data) {
    mem_sink *s = (mem_sink *)client_data;
    (void)encoder;
    *absolute_byte_offset = (FLAC__uint64)s->pos;
    return FLAC__STREAM_ENCODER_TELL_STATUS_OK;
}

/* Encode a complete FLAC stream (marker + metadata + frames) to memory at the
 * given compression level, optionally computing the audio MD5. */
int libflac_rs_cref_encode_full(const int32_t *interleaved, uint32_t nsamples,
                                uint32_t channels, uint32_t bps,
                                uint32_t sample_rate, uint32_t blocksize,
                                int32_t compression_level, int32_t do_md5,
                                uint8_t *out, size_t *out_len) {
    mem_sink sink;
    FLAC__StreamEncoder *enc;
    FLAC__StreamEncoderInitStatus init;
    int rc = 0;

    sink.buf = out;
    sink.cap = *out_len;
    sink.pos = 0;
    sink.len = 0;
    sink.overflow = 0;

    enc = FLAC__stream_encoder_new();
    if (!enc) {
        return -1;
    }

    FLAC__stream_encoder_set_verify(enc, false);
    FLAC__stream_encoder_set_compression_level(
        enc, compression_level >= 0 ? (uint32_t)compression_level : 8);
    FLAC__stream_encoder_set_channels(enc, channels);
    FLAC__stream_encoder_set_bits_per_sample(enc, bps);
    FLAC__stream_encoder_set_sample_rate(enc, sample_rate);
    FLAC__stream_encoder_set_total_samples_estimate(enc, 0);
    FLAC__stream_encoder_set_streamable_subset(enc, false);
    FLAC__stream_encoder_set_blocksize(enc, blocksize);
    FLAC__stream_encoder_set_do_md5(enc, do_md5 != 0);

    init = FLAC__stream_encoder_init_stream(enc, mem_write, mem_seek, mem_tell,
                                            NULL, &sink);
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

/* Encode a complete FLAC stream with one APPLICATION metadata block set (via a
 * manually-filled FLAC__StreamMetadata, serialized by FLAC__add_metadata_block).
 * Used to verify the Rust APPLICATION block byte-for-byte. With metadata set
 * explicitly, libFLAC does not auto-insert a VORBIS_COMMENT (HAS_OGG=0, no
 * reorder), so the output is STREAMINFO + APPLICATION + frames. */
int libflac_rs_cref_encode_full_app(const int32_t *interleaved, uint32_t nsamples,
                                    uint32_t channels, uint32_t bps,
                                    uint32_t sample_rate, uint32_t blocksize,
                                    int32_t compression_level, int32_t do_md5,
                                    const uint8_t *app_id, const uint8_t *app_data,
                                    uint32_t app_data_len, uint8_t *out,
                                    size_t *out_len) {
    mem_sink sink;
    FLAC__StreamEncoder *enc;
    FLAC__StreamEncoderInitStatus init;
    FLAC__StreamMetadata app;
    FLAC__StreamMetadata *metas[1];
    int rc = 0;

    sink.buf = out;
    sink.cap = *out_len;
    sink.pos = 0;
    sink.len = 0;
    sink.overflow = 0;

    enc = FLAC__stream_encoder_new();
    if (!enc) {
        return -1;
    }

    FLAC__stream_encoder_set_verify(enc, false);
    FLAC__stream_encoder_set_compression_level(
        enc, compression_level >= 0 ? (uint32_t)compression_level : 8);
    FLAC__stream_encoder_set_channels(enc, channels);
    FLAC__stream_encoder_set_bits_per_sample(enc, bps);
    FLAC__stream_encoder_set_sample_rate(enc, sample_rate);
    FLAC__stream_encoder_set_total_samples_estimate(enc, 0);
    FLAC__stream_encoder_set_streamable_subset(enc, false);
    FLAC__stream_encoder_set_blocksize(enc, blocksize);
    FLAC__stream_encoder_set_do_md5(enc, do_md5 != 0);

    memset(&app, 0, sizeof(app));
    app.type = FLAC__METADATA_TYPE_APPLICATION;
    app.is_last = false; /* the encoder sets is_last on the final block */
    app.length = 4 + app_data_len;
    memcpy(app.data.application.id, app_id, 4);
    app.data.application.data = (FLAC__byte *)app_data;
    metas[0] = &app;
    FLAC__stream_encoder_set_metadata(enc, metas, 1);

    init = FLAC__stream_encoder_init_stream(enc, mem_write, mem_seek, mem_tell,
                                            NULL, &sink);
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

/* Encode a complete FLAC stream with one PICTURE metadata block set, to verify
 * the Rust PICTURE block byte-for-byte. mime/desc are NUL-terminated. */
int libflac_rs_cref_encode_full_picture(
    const int32_t *interleaved, uint32_t nsamples, uint32_t channels, uint32_t bps,
    uint32_t sample_rate, uint32_t blocksize, int32_t compression_level,
    int32_t do_md5, uint32_t picture_type, const char *mime, const char *desc,
    uint32_t width, uint32_t height, uint32_t depth, uint32_t colors,
    const uint8_t *pic_data, uint32_t pic_data_len, uint8_t *out, size_t *out_len) {
    mem_sink sink;
    FLAC__StreamEncoder *enc;
    FLAC__StreamEncoderInitStatus init;
    FLAC__StreamMetadata pic;
    FLAC__StreamMetadata *metas[1];
    int rc = 0;

    sink.buf = out;
    sink.cap = *out_len;
    sink.pos = 0;
    sink.len = 0;
    sink.overflow = 0;

    enc = FLAC__stream_encoder_new();
    if (!enc) {
        return -1;
    }
    FLAC__stream_encoder_set_verify(enc, false);
    FLAC__stream_encoder_set_compression_level(
        enc, compression_level >= 0 ? (uint32_t)compression_level : 8);
    FLAC__stream_encoder_set_channels(enc, channels);
    FLAC__stream_encoder_set_bits_per_sample(enc, bps);
    FLAC__stream_encoder_set_sample_rate(enc, sample_rate);
    FLAC__stream_encoder_set_total_samples_estimate(enc, 0);
    FLAC__stream_encoder_set_streamable_subset(enc, false);
    FLAC__stream_encoder_set_blocksize(enc, blocksize);
    FLAC__stream_encoder_set_do_md5(enc, do_md5 != 0);

    memset(&pic, 0, sizeof(pic));
    pic.type = FLAC__METADATA_TYPE_PICTURE;
    pic.is_last = false;
    pic.length = 32 + (uint32_t)strlen(mime) + (uint32_t)strlen(desc) + pic_data_len;
    pic.data.picture.type = (FLAC__StreamMetadata_Picture_Type)picture_type;
    pic.data.picture.mime_type = (char *)mime;
    pic.data.picture.description = (FLAC__byte *)desc;
    pic.data.picture.width = width;
    pic.data.picture.height = height;
    pic.data.picture.depth = depth;
    pic.data.picture.colors = colors;
    pic.data.picture.data = (FLAC__byte *)pic_data;
    pic.data.picture.data_length = pic_data_len;
    metas[0] = &pic;
    FLAC__stream_encoder_set_metadata(enc, metas, 1);

    init = FLAC__stream_encoder_init_stream(enc, mem_write, mem_seek, mem_tell,
                                            NULL, &sink);
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

/* Encode a complete FLAC stream with one SEEKTABLE metadata block, to verify the
 * Rust SEEKTABLE byte-for-byte. The caller supplies `num_points` target sample
 * numbers; the block is filled as a template (sample_number = target,
 * stream_offset/frame_samples = 0) and libFLAC generates + sorts + rewrites it
 * during encoding (the seekable mem_sink lets it seek back). As with the other
 * metadata shims, with metadata set explicitly libFLAC still prepends its default
 * VORBIS_COMMENT, so the output is STREAMINFO + VORBIS_COMMENT + SEEKTABLE +
 * frames. The targets must be strictly increasing (FLAC__format_seektable_is_legal,
 * checked at init). */
int libflac_rs_cref_encode_full_seektable(
    const int32_t *interleaved, uint32_t nsamples, uint32_t channels, uint32_t bps,
    uint32_t sample_rate, uint32_t blocksize, int32_t compression_level,
    int32_t do_md5, const uint64_t *sample_numbers, uint32_t num_points,
    uint8_t *out, size_t *out_len) {
    mem_sink sink;
    FLAC__StreamEncoder *enc;
    FLAC__StreamEncoderInitStatus init;
    FLAC__StreamMetadata st;
    FLAC__StreamMetadata *metas[1];
    FLAC__StreamMetadata_SeekPoint *pts;
    uint32_t i;
    int rc = 0;

    sink.buf = out;
    sink.cap = *out_len;
    sink.pos = 0;
    sink.len = 0;
    sink.overflow = 0;

    enc = FLAC__stream_encoder_new();
    if (!enc) {
        return -1;
    }
    FLAC__stream_encoder_set_verify(enc, false);
    FLAC__stream_encoder_set_compression_level(
        enc, compression_level >= 0 ? (uint32_t)compression_level : 8);
    FLAC__stream_encoder_set_channels(enc, channels);
    FLAC__stream_encoder_set_bits_per_sample(enc, bps);
    FLAC__stream_encoder_set_sample_rate(enc, sample_rate);
    FLAC__stream_encoder_set_total_samples_estimate(enc, 0);
    FLAC__stream_encoder_set_streamable_subset(enc, false);
    FLAC__stream_encoder_set_blocksize(enc, blocksize);
    FLAC__stream_encoder_set_do_md5(enc, do_md5 != 0);

    pts = (FLAC__StreamMetadata_SeekPoint *)malloc((size_t)num_points * sizeof(*pts));
    if (!pts) {
        FLAC__stream_encoder_delete(enc);
        return -1;
    }
    for (i = 0; i < num_points; i++) {
        pts[i].sample_number = sample_numbers[i];
        pts[i].stream_offset = 0;
        pts[i].frame_samples = 0;
    }
    memset(&st, 0, sizeof(st));
    st.type = FLAC__METADATA_TYPE_SEEKTABLE;
    st.is_last = false;
    st.length = num_points * FLAC__STREAM_METADATA_SEEKPOINT_LENGTH;
    st.data.seek_table.num_points = num_points;
    st.data.seek_table.points = pts;
    metas[0] = &st;
    FLAC__stream_encoder_set_metadata(enc, metas, 1);

    init = FLAC__stream_encoder_init_stream(enc, mem_write, mem_seek, mem_tell,
                                            NULL, &sink);
    if (init != FLAC__STREAM_ENCODER_INIT_STATUS_OK) {
        free(pts);
        FLAC__stream_encoder_delete(enc);
        return -100 - (int)init;
    }
    if (!FLAC__stream_encoder_process_interleaved(enc, interleaved, nsamples)) {
        rc = -200 - (int)FLAC__stream_encoder_get_state(enc);
    } else if (!FLAC__stream_encoder_finish(enc)) {
        rc = -300 - (int)FLAC__stream_encoder_get_state(enc);
    }
    FLAC__stream_encoder_delete(enc);
    free(pts);
    if (rc != 0) {
        return rc;
    }
    if (sink.overflow) {
        return -2;
    }
    *out_len = sink.len;
    return 0;
}

/* Encode a complete FLAC stream with one CUESHEET metadata block, to verify the
 * Rust CUESHEET byte-for-byte. The cuesheet is passed as scalars plus flattened
 * per-track and (across all tracks, in order) per-index arrays, reassembled into a
 * manually-filled FLAC__StreamMetadata_CueSheet that libFLAC serializes via
 * FLAC__add_metadata_block. The cuesheet must be legal for its `is_cd` flag
 * (FLAC__format_cuesheet_is_legal, checked at init). As with the other metadata
 * shims, libFLAC prepends its default VORBIS_COMMENT, so the output is STREAMINFO +
 * VORBIS_COMMENT + CUESHEET + frames. */
int libflac_rs_cref_encode_full_cuesheet(
    const int32_t *interleaved, uint32_t nsamples, uint32_t channels, uint32_t bps,
    uint32_t sample_rate, uint32_t blocksize, int32_t compression_level,
    int32_t do_md5, const uint8_t *media_catalog_number, uint64_t lead_in,
    int32_t is_cd, uint32_t num_tracks, const uint64_t *track_offsets,
    const uint8_t *track_numbers, const uint8_t *track_isrcs,
    const uint8_t *track_types, const uint8_t *track_pre_emphasis,
    const uint8_t *track_num_indices, const uint64_t *index_offsets,
    const uint8_t *index_numbers, uint8_t *out, size_t *out_len) {
    mem_sink sink;
    FLAC__StreamEncoder *enc;
    FLAC__StreamEncoderInitStatus init;
    FLAC__StreamMetadata cs;
    FLAC__StreamMetadata *metas[1];
    FLAC__StreamMetadata_CueSheet_Track *tracks;
    uint32_t i, j, idx_cursor = 0, length = 396;
    int rc = 0;

    sink.buf = out;
    sink.cap = *out_len;
    sink.pos = 0;
    sink.len = 0;
    sink.overflow = 0;

    enc = FLAC__stream_encoder_new();
    if (!enc) {
        return -1;
    }
    FLAC__stream_encoder_set_verify(enc, false);
    FLAC__stream_encoder_set_compression_level(
        enc, compression_level >= 0 ? (uint32_t)compression_level : 8);
    FLAC__stream_encoder_set_channels(enc, channels);
    FLAC__stream_encoder_set_bits_per_sample(enc, bps);
    FLAC__stream_encoder_set_sample_rate(enc, sample_rate);
    FLAC__stream_encoder_set_total_samples_estimate(enc, 0);
    FLAC__stream_encoder_set_streamable_subset(enc, false);
    FLAC__stream_encoder_set_blocksize(enc, blocksize);
    FLAC__stream_encoder_set_do_md5(enc, do_md5 != 0);

    tracks = (FLAC__StreamMetadata_CueSheet_Track *)calloc(
        num_tracks, sizeof(*tracks));
    if (!tracks) {
        FLAC__stream_encoder_delete(enc);
        return -1;
    }
    for (i = 0; i < num_tracks; i++) {
        tracks[i].offset = track_offsets[i];
        tracks[i].number = track_numbers[i];
        memcpy(tracks[i].isrc, track_isrcs + (size_t)i * 12, 12);
        tracks[i].isrc[12] = 0;
        tracks[i].type = track_types[i] & 1;
        tracks[i].pre_emphasis = track_pre_emphasis[i] & 1;
        tracks[i].num_indices = track_num_indices[i];
        length += 36 + (uint32_t)track_num_indices[i] * 12;
        if (track_num_indices[i] > 0) {
            tracks[i].indices = (FLAC__StreamMetadata_CueSheet_Index *)calloc(
                track_num_indices[i], sizeof(*tracks[i].indices));
            for (j = 0; j < track_num_indices[i]; j++) {
                tracks[i].indices[j].offset = index_offsets[idx_cursor];
                tracks[i].indices[j].number = index_numbers[idx_cursor];
                idx_cursor++;
            }
        } else {
            tracks[i].indices = NULL;
        }
    }

    memset(&cs, 0, sizeof(cs));
    cs.type = FLAC__METADATA_TYPE_CUESHEET;
    cs.is_last = false;
    cs.length = length;
    memcpy(cs.data.cue_sheet.media_catalog_number, media_catalog_number, 128);
    cs.data.cue_sheet.media_catalog_number[128] = 0;
    cs.data.cue_sheet.lead_in = lead_in;
    cs.data.cue_sheet.is_cd = is_cd != 0;
    cs.data.cue_sheet.num_tracks = num_tracks;
    cs.data.cue_sheet.tracks = tracks;
    metas[0] = &cs;
    FLAC__stream_encoder_set_metadata(enc, metas, 1);

    init = FLAC__stream_encoder_init_stream(enc, mem_write, mem_seek, mem_tell,
                                            NULL, &sink);
    if (init != FLAC__STREAM_ENCODER_INIT_STATUS_OK) {
        rc = -100 - (int)init;
    } else if (!FLAC__stream_encoder_process_interleaved(enc, interleaved,
                                                         nsamples)) {
        rc = -200 - (int)FLAC__stream_encoder_get_state(enc);
    } else if (!FLAC__stream_encoder_finish(enc)) {
        rc = -300 - (int)FLAC__stream_encoder_get_state(enc);
    }

    FLAC__stream_encoder_delete(enc);
    for (i = 0; i < num_tracks; i++) {
        free(tracks[i].indices);
    }
    free(tracks);

    if (rc != 0) {
        return rc;
    }
    if (sink.overflow) {
        return -2;
    }
    *out_len = sink.len;
    return 0;
}

/* ---- Decoder round-trip --------------------------------------------------
 * Decode a complete in-memory FLAC stream back to interleaved PCM via the real
 * libFLAC decoder, to prove the Rust full-stream output is a valid, decodable
 * file (marker + STREAMINFO + frames). */
typedef struct {
    const uint8_t *data;
    size_t len;
    size_t pos;
    int32_t *out;
    size_t out_cap;
    size_t out_len;
    int error;
} dec_ctx;

static FLAC__StreamDecoderReadStatus dec_read(const FLAC__StreamDecoder *decoder,
                                              FLAC__byte buffer[], size_t *bytes,
                                              void *client_data) {
    dec_ctx *c = (dec_ctx *)client_data;
    size_t avail = c->len - c->pos;
    size_t want = *bytes;
    (void)decoder;
    if (want > avail) {
        want = avail;
    }
    if (want == 0) {
        *bytes = 0;
        return FLAC__STREAM_DECODER_READ_STATUS_END_OF_STREAM;
    }
    memcpy(buffer, c->data + c->pos, want);
    c->pos += want;
    *bytes = want;
    return FLAC__STREAM_DECODER_READ_STATUS_CONTINUE;
}

static FLAC__StreamDecoderWriteStatus dec_write(const FLAC__StreamDecoder *decoder,
                                                const FLAC__Frame *frame,
                                                const FLAC__int32 *const buffer[],
                                                void *client_data) {
    dec_ctx *c = (dec_ctx *)client_data;
    uint32_t blocksize = frame->header.blocksize;
    uint32_t channels = frame->header.channels;
    uint32_t i, j;
    (void)decoder;
    for (i = 0; i < blocksize; i++)
        for (j = 0; j < channels; j++)
            if (c->out_len < c->out_cap)
                c->out[c->out_len++] = buffer[j][i];
    return FLAC__STREAM_DECODER_WRITE_STATUS_CONTINUE;
}

static void dec_error(const FLAC__StreamDecoder *decoder,
                      FLAC__StreamDecoderErrorStatus status, void *client_data) {
    dec_ctx *c = (dec_ctx *)client_data;
    (void)decoder;
    c->error = (int)status + 1;
}

int libflac_rs_cref_decode(const uint8_t *data, size_t len, int32_t *out,
                           size_t *out_len) {
    dec_ctx c;
    FLAC__StreamDecoder *dec;

    c.data = data;
    c.len = len;
    c.pos = 0;
    c.out = out;
    c.out_cap = *out_len;
    c.out_len = 0;
    c.error = 0;

    dec = FLAC__stream_decoder_new();
    if (!dec) {
        return -1;
    }
    if (FLAC__stream_decoder_init_stream(dec, dec_read, NULL, NULL, NULL, NULL,
                                         dec_write, NULL, dec_error,
                                         &c) != FLAC__STREAM_DECODER_INIT_STATUS_OK) {
        FLAC__stream_decoder_delete(dec);
        return -2;
    }
    if (!FLAC__stream_decoder_process_until_end_of_stream(dec)) {
        FLAC__stream_decoder_delete(dec);
        return -3;
    }
    FLAC__stream_decoder_finish(dec);
    FLAC__stream_decoder_delete(dec);

    if (c.error) {
        return -100 - c.error;
    }
    *out_len = c.out_len;
    return 0;
}

/* MD5 of the decoded audio (`FLAC__MD5Accumulate`/`Final`), de-interleaving the
 * input into per-channel arrays as the encoder holds it. Verifies the STREAMINFO
 * audio checksum. */
void libflac_rs_cref_md5(const int32_t *interleaved, uint32_t nsamples,
                         uint32_t channels, uint32_t bytes_per_sample,
                         uint8_t *out16) {
    FLAC__MD5Context ctx;
    int32_t *chans[8];
    const FLAC__int32 *signal[8];
    uint32_t c, i;

    for (c = 0; c < channels; c++) {
        chans[c] = (int32_t *)malloc((size_t)nsamples * sizeof(int32_t));
        signal[c] = chans[c];
    }
    for (i = 0; i < nsamples; i++)
        for (c = 0; c < channels; c++)
            chans[c][i] = interleaved[i * channels + c];

    FLAC__MD5Init(&ctx);
    FLAC__MD5Accumulate(&ctx, signal, channels, nsamples, bytes_per_sample);
    FLAC__MD5Final(out16, &ctx);

    for (c = 0; c < channels; c++)
        free(chans[c]);
}

/* The compiled libFLAC's vendor string (written into the default VORBIS_COMMENT),
 * so the full-stream test can match it without hardcoding the version. Returns the
 * length; copies into `out` when `cap` is large enough. */
size_t libflac_rs_cref_vendor_string(char *out, size_t cap) {
    size_t len = strlen(FLAC__VENDOR_STRING);
    if (out && cap >= len) {
        memcpy(out, FLAC__VENDOR_STRING, len);
    }
    return len;
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
