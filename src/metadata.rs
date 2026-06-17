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

/// Metadata block type codes.
const METADATA_TYPE_STREAMINFO: u32 = 0;
const METADATA_TYPE_PADDING: u32 = 1;
const METADATA_TYPE_APPLICATION: u32 = 2;
const METADATA_TYPE_VORBIS_COMMENT: u32 = 4;
/// STREAMINFO body length in bytes.
const STREAMINFO_LENGTH: u32 = 34;

/// A metadata block the caller can place after STREAMINFO (which the encoder
/// always writes first). libFLAC writes blocks in the order given (the OGG
/// reorder is compiled out for native FLAC), so this list maps 1:1 to the output.
pub enum MetadataBlock<'a> {
    /// A VORBIS_COMMENT with the given vendor string and no user comments.
    VorbisComment(&'a str),
    /// A PADDING block of N zero bytes.
    Padding(u32),
    /// An APPLICATION block: a 4-byte registered application id + opaque data.
    Application { id: [u8; 4], data: &'a [u8] },
}

/// Write one [`MetadataBlock`] with its `is_last` flag.
pub fn write_block(bw: &mut BitWriter, block: &MetadataBlock, is_last: bool) {
    match block {
        MetadataBlock::VorbisComment(vendor) => write_vorbis_comment(bw, vendor, is_last),
        MetadataBlock::Padding(len) => write_padding(bw, *len, is_last),
        MetadataBlock::Application { id, data } => write_application(bw, id, data, is_last),
    }
}

/// Write an APPLICATION block: 4-byte id then the application data
/// (`FLAC__metadata_object_application`; body length = 4 + data length).
pub fn write_application(bw: &mut BitWriter, id: &[u8; 4], data: &[u8], is_last: bool) {
    write_block_header(
        bw,
        is_last,
        METADATA_TYPE_APPLICATION,
        4 + data.len() as u32,
    );
    bw.write_byte_block(id);
    bw.write_byte_block(data);
}

/// The vendor string libFLAC 1.4.3 writes into its auto VORBIS_COMMENT
/// (`FLAC__VENDOR_STRING` = `"reference libFLAC " PACKAGE_VERSION " 20230623"`
/// with no git tag/hash defined). Used to byte-match libFLAC's default output.
pub const LIBFLAC_VENDOR_STRING: &str = "reference libFLAC 1.4.3 20230623";

/// Write a metadata block header (1-bit last flag, 7-bit type, 24-bit length).
fn write_block_header(bw: &mut BitWriter, is_last: bool, block_type: u32, length: u32) {
    bw.write_raw_u32(is_last as u32, 1);
    bw.write_raw_u32(block_type, 7);
    bw.write_raw_u32(length, 24);
}

/// Write the STREAMINFO metadata block (4-byte block header + 34-byte body).
/// `is_last` sets the last-metadata-block flag.
pub fn write_streaminfo(bw: &mut BitWriter, si: &StreamInfo, is_last: bool) {
    write_block_header(bw, is_last, METADATA_TYPE_STREAMINFO, STREAMINFO_LENGTH);

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

/// Write a VORBIS_COMMENT block with the given vendor string and no comments
/// (the empty block libFLAC auto-writes; `add_metadata_block`,
/// `stream_encoder_framing.c:132`). The vendor length and comment count are
/// little-endian per the Vorbis comment spec.
pub fn write_vorbis_comment(bw: &mut BitWriter, vendor: &str, is_last: bool) {
    let vendor = vendor.as_bytes();
    let length = 4 + vendor.len() as u32 + 4; // vendor-length + vendor + num-comments
    write_block_header(bw, is_last, METADATA_TYPE_VORBIS_COMMENT, length);
    bw.write_raw_u32_little_endian(vendor.len() as u32);
    bw.write_byte_block(vendor);
    bw.write_raw_u32_little_endian(0); // num_comments
}

/// Write a PADDING block of `length` zero bytes (`add_metadata_block`,
/// `stream_encoder_framing.c:112`).
pub fn write_padding(bw: &mut BitWriter, length: u32, is_last: bool) {
    write_block_header(bw, is_last, METADATA_TYPE_PADDING, length);
    bw.write_zeroes(length * 8);
}
