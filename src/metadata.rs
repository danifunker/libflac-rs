//! FLAC metadata blocks. Only STREAMINFO (the mandatory first block) is written;
//! the field layout is from `FLAC/format.h` and the metadata framing in
//! `stream_encoder.c` (`streaminfo` setup at `:1200`, framesize tracking at
//! `:2703`). All fields are big-endian, MSB-first, and the body is exactly 34
//! bytes (byte-aligned throughout).

use crate::bitwriter::BitWriter;

/// The STREAMINFO fields. `min_blocksize`/`max_blocksize` are the configured
/// block size (libFLAC reports the same value for both even with a short final
/// frame); `min_framesize`/`max_framesize` are the min/max frame size in bytes.
pub struct StreamInfo {
    pub min_blocksize: u32,
    pub max_blocksize: u32,
    pub min_framesize: u32,
    pub max_framesize: u32,
    pub sample_rate: u32,
    pub channels: u32,
    pub bits_per_sample: u32,
    pub total_samples: u64,
    pub md5: [u8; 16],
}

/// STREAMINFO block type code.
const METADATA_TYPE_STREAMINFO: u32 = 0;
/// STREAMINFO body length in bytes.
const STREAMINFO_LENGTH: u32 = 34;

/// Write the STREAMINFO metadata block (4-byte block header + 34-byte body).
/// `is_last` sets the last-metadata-block flag.
pub fn write_streaminfo(bw: &mut BitWriter, si: &StreamInfo, is_last: bool) {
    // Metadata block header: 1-bit last flag, 7-bit type, 24-bit length.
    bw.write_raw_u32(is_last as u32, 1);
    bw.write_raw_u32(METADATA_TYPE_STREAMINFO, 7);
    bw.write_raw_u32(STREAMINFO_LENGTH, 24);

    bw.write_raw_u32(si.min_blocksize, 16);
    bw.write_raw_u32(si.max_blocksize, 16);
    bw.write_raw_u32(si.min_framesize, 24);
    bw.write_raw_u32(si.max_framesize, 24);
    bw.write_raw_u32(si.sample_rate, 20);
    bw.write_raw_u32(si.channels - 1, 3);
    bw.write_raw_u32(si.bits_per_sample - 1, 5);
    bw.write_raw_u64(si.total_samples, 36);
    bw.write_byte_block(&si.md5);
}
