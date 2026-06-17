//! FLAC frame decoder (`stream_decoder.c`): parse audio frames back into PCM.
//!
//! The decoder's contract is *losslessness* — it must reproduce the exact samples
//! the encoder consumed — not byte-parity with libFLAC's decoder internals, so the
//! code is idiomatic. Correctness is established by round-tripping every encoder
//! corpus (`decode_frames(encode_frames(pcm)) == pcm`) and by decoding real
//! libFLAC output. All prediction/restoration runs in `i64` so the 33-bit side
//! channel and any wide predictor sum are exact; the chosen subframes always have
//! `i32`-fitting residuals, so ≤32-bit channels round-trip exactly.

use crate::bitreader::BitReader;
use crate::crc::{crc8, crc16};
use crate::frame::ChannelAssignment;

/// Decoded audio frames.
pub struct DecodedFrames {
    /// Interleaved PCM (channel-major within each sample).
    pub interleaved: Vec<i32>,
    pub channels: u32,
    pub bits_per_sample: u32,
    pub sample_rate: u32,
}

/// Decode every audio frame in `data` (the raw frame stream produced by
/// [`crate::encoder::encode_frames`], i.e. no metadata) back to interleaved PCM.
/// Returns `None` on any malformed/truncated input or CRC mismatch.
pub fn decode_frames(data: &[u8]) -> Option<DecodedFrames> {
    let mut br = BitReader::new(data);
    let mut interleaved = Vec::new();
    let mut channels = 0u32;
    let mut bits_per_sample = 0u32;
    let mut sample_rate = 0u32;

    while br.byte_pos() < data.len() {
        let frame = decode_frame(&mut br, data)?;
        channels = frame.channels;
        bits_per_sample = frame.bits_per_sample;
        sample_rate = frame.sample_rate;
        for i in 0..frame.blocksize {
            for ch in &frame.samples {
                interleaved.push(ch[i]);
            }
        }
    }

    Some(DecodedFrames {
        interleaved,
        channels,
        bits_per_sample,
        sample_rate,
    })
}

struct Frame {
    blocksize: usize,
    channels: u32,
    bits_per_sample: u32,
    sample_rate: u32,
    /// Per-channel decoded PCM (already un-decorrelated to independent channels).
    samples: Vec<Vec<i32>>,
}

fn decode_frame(br: &mut BitReader, data: &[u8]) -> Option<Frame> {
    let frame_start = br.byte_pos();

    // --- header ---
    if br.read_raw_u32(14)? != 0x3ffe {
        return None; // sync
    }
    let _reserved = br.read_raw_u32(1)?;
    let _blocking_strategy = br.read_raw_u32(1)?; // 0 = fixed block size
    let bs_code = br.read_raw_u32(4)?;
    let sr_code = br.read_raw_u32(4)?;
    let ca_code = br.read_raw_u32(4)?;
    let bps_code = br.read_raw_u32(3)?;
    let _reserved2 = br.read_raw_u32(1)?;
    let _frame_number = br.read_utf8_u32()?; // fixed block size -> frame number

    let blocksize = match bs_code {
        1 => 192,
        2 => 576,
        3 => 1152,
        4 => 2304,
        5 => 4608,
        6 => br.read_raw_u32(8)? as usize + 1,
        7 => br.read_raw_u32(16)? as usize + 1,
        8 => 256,
        9 => 512,
        10 => 1024,
        11 => 2048,
        12 => 4096,
        13 => 8192,
        14 => 16384,
        15 => 32768,
        _ => return None, // 0 reserved
    };
    let sample_rate = match sr_code {
        1 => 88200,
        2 => 176400,
        3 => 192000,
        4 => 8000,
        5 => 16000,
        6 => 22050,
        7 => 24000,
        8 => 32000,
        9 => 44100,
        10 => 48000,
        11 => 96000,
        12 => br.read_raw_u32(8)? * 1000,
        13 => br.read_raw_u32(16)?,
        14 => br.read_raw_u32(16)? * 10,
        _ => 0, // 0 = from STREAMINFO, 15 invalid
    };
    let bits_per_sample = match bps_code {
        1 => 8,
        2 => 12,
        4 => 16,
        5 => 20,
        6 => 24,
        7 => 32,
        _ => return None, // 0 from STREAMINFO, 3 reserved
    };
    let (assignment, channels) = match ca_code {
        0..=7 => (ChannelAssignment::Independent, ca_code + 1),
        8 => (ChannelAssignment::LeftSide, 2),
        9 => (ChannelAssignment::RightSide, 2),
        10 => (ChannelAssignment::MidSide, 2),
        _ => return None,
    };

    // CRC-8 over the header bytes just read (the reader is byte-aligned here).
    let computed_crc8 = crc8(br.bytes_since(frame_start));
    if br.read_raw_u32(8)? as u8 != computed_crc8 {
        return None;
    }

    // --- subframes (i64 channel data) ---
    let mut chans: Vec<Vec<i64>> = Vec::with_capacity(channels as usize);
    for ch in 0..channels {
        let side = matches!(
            (assignment, ch),
            (ChannelAssignment::LeftSide, 1)
                | (ChannelAssignment::RightSide, 0)
                | (ChannelAssignment::MidSide, 1)
        );
        let channel_bps = bits_per_sample + u32::from(side);
        chans.push(decode_subframe(br, blocksize, channel_bps)?);
    }

    // --- footer: CRC-16 over the whole frame so far ---
    br.align_to_byte();
    let computed_crc16 = crc16(br.bytes_since(frame_start));
    if br.read_raw_u32(16)? as u16 != computed_crc16 {
        return None;
    }
    let _ = data; // backing slice is reached through the reader

    Some(Frame {
        blocksize,
        channels,
        bits_per_sample,
        sample_rate,
        samples: undecorrelate(assignment, &chans, blocksize),
    })
}

/// Decode one subframe to its (wasted-bits-restored) `i64` channel signal.
fn decode_subframe(br: &mut BitReader, blocksize: usize, channel_bps: u32) -> Option<Vec<i64>> {
    let header = br.read_raw_u32(8)?;
    let wasted = if header & 1 == 1 {
        br.read_unary()? + 1
    } else {
        0
    };
    let type6 = (header >> 1) & 0x3f;
    let subframe_bps = channel_bps - wasted;

    let mut data = vec![0i64; blocksize];
    if type6 == 0 {
        // CONSTANT
        let v = br.read_signed(subframe_bps)?;
        data.fill(v);
    } else if type6 == 1 {
        // VERBATIM
        for d in data.iter_mut() {
            *d = br.read_signed(subframe_bps)?;
        }
    } else if type6 & 0x20 != 0 {
        // LPC, order = (type6 & 0x1f) + 1
        let order = ((type6 & 0x1f) + 1) as usize;
        for d in data.iter_mut().take(order) {
            *d = br.read_signed(subframe_bps)?;
        }
        let precision = br.read_raw_u32(4)? + 1;
        let shift = sign_extend5(br.read_raw_u32(5)?);
        let mut qlp = vec![0i32; order];
        for c in qlp.iter_mut() {
            *c = br.read_signed(precision)? as i32;
        }
        let residual = decode_residual(br, blocksize, order)?;
        for i in order..blocksize {
            let mut sum = 0i64;
            for (j, &c) in qlp.iter().enumerate() {
                sum += c as i64 * data[i - 1 - j];
            }
            data[i] = residual[i - order] as i64 + (sum >> shift);
        }
    } else if type6 & 0x08 != 0 {
        // FIXED, order = type6 & 0x07
        let order = (type6 & 0x07) as usize;
        for d in data.iter_mut().take(order) {
            *d = br.read_signed(subframe_bps)?;
        }
        let residual = decode_residual(br, blocksize, order)?;
        for i in order..blocksize {
            let r = residual[i - order] as i64;
            data[i] = match order {
                0 => r,
                1 => r + data[i - 1],
                2 => r + 2 * data[i - 1] - data[i - 2],
                3 => r + 3 * data[i - 1] - 3 * data[i - 2] + data[i - 3],
                _ => r + 4 * data[i - 1] - 6 * data[i - 2] + 4 * data[i - 3] - data[i - 4],
            };
        }
    } else {
        return None; // reserved subframe type
    }

    if wasted > 0 {
        for d in data.iter_mut() {
            *d <<= wasted;
        }
    }
    Some(data)
}

/// Decode the partitioned-Rice residual (`blocksize - order` values).
fn decode_residual(br: &mut BitReader, blocksize: usize, order: usize) -> Option<Vec<i32>> {
    let is_rice2 = br.read_raw_u32(2)? == 1;
    let param_len = if is_rice2 { 5 } else { 4 };
    let escape = if is_rice2 { 31 } else { 15 };
    let partition_order = br.read_raw_u32(4)?;
    let partitions = 1usize << partition_order;

    let mut residual = vec![0i32; blocksize - order];
    let mut idx = 0usize;
    for p in 0..partitions {
        let count = if p == 0 {
            (blocksize >> partition_order) - order
        } else {
            blocksize >> partition_order
        };
        let param = br.read_raw_u32(param_len)?;
        if param == escape {
            // Escaped partition: raw `raw_bits`-wide samples (the encoder never
            // emits these, but other encoders may).
            let raw_bits = br.read_raw_u32(5)?;
            for r in &mut residual[idx..idx + count] {
                *r = br.read_signed(raw_bits)? as i32;
            }
        } else {
            br.read_rice_signed_block(&mut residual[idx..idx + count], param)?;
        }
        idx += count;
    }
    Some(residual)
}

/// Reverse the stereo decorrelation back to independent `i32` channels.
fn undecorrelate(
    assignment: ChannelAssignment,
    chans: &[Vec<i64>],
    blocksize: usize,
) -> Vec<Vec<i32>> {
    match assignment {
        ChannelAssignment::Independent => chans
            .iter()
            .map(|c| c.iter().map(|&s| s as i32).collect())
            .collect(),
        ChannelAssignment::LeftSide => {
            // ch0 = left, ch1 = side = left - right -> right = left - side.
            let (l, s) = (&chans[0], &chans[1]);
            let left: Vec<i32> = l.iter().map(|&v| v as i32).collect();
            let right: Vec<i32> = (0..blocksize).map(|i| (l[i] - s[i]) as i32).collect();
            vec![left, right]
        }
        ChannelAssignment::RightSide => {
            // ch0 = side = left - right, ch1 = right -> left = right + side.
            let (s, r) = (&chans[0], &chans[1]);
            let left: Vec<i32> = (0..blocksize).map(|i| (r[i] + s[i]) as i32).collect();
            let right: Vec<i32> = r.iter().map(|&v| v as i32).collect();
            vec![left, right]
        }
        ChannelAssignment::MidSide => {
            // ch0 = mid = (L+R)>>1 (LSB dropped), ch1 = side = L-R. The dropped LSB
            // equals side&1, so mid2 = (mid<<1)|(side&1) = L+R; L=(mid2+side)/2.
            let (m, s) = (&chans[0], &chans[1]);
            let mut left = vec![0i32; blocksize];
            let mut right = vec![0i32; blocksize];
            for i in 0..blocksize {
                let side = s[i];
                let mid2 = (m[i] << 1) | (side & 1);
                left[i] = ((mid2 + side) >> 1) as i32;
                right[i] = ((mid2 - side) >> 1) as i32;
            }
            vec![left, right]
        }
    }
}

/// Sign-extend a 5-bit quantization shift (always non-negative in practice).
fn sign_extend5(v: u32) -> i32 {
    ((v << 27) as i32) >> 27
}

#[cfg(test)]
mod tests {
    use super::decode_frames;
    use crate::encoder::{encode_frames, preset};

    /// Multi-partial + noise PCM scaled to a `bps`-bit range, `channels` wide.
    fn gen_pcm(seed: u32, n: usize, bps: u32, channels: u32) -> Vec<i32> {
        let mut st = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
        let mut rng = || {
            st ^= st << 13;
            st ^= st >> 17;
            st ^= st << 5;
            st
        };
        let maxv = ((1i64 << (bps - 1)) - 1) as f64;
        let minv = -(1i64 << (bps - 1)) as f64;
        let scale = (1u64 << (bps - 1)) as f64 / 32768.0;
        let f1 = 0.008 + (rng() >> 22) as f64 / 2.0e6;
        let f2 = 0.05 + (rng() >> 22) as f64 / 1.0e6;
        let a1 = (1500.0 + (rng() >> 19) as f64 / 8.0e3) * scale;
        let a2 = (400.0 + (rng() >> 20) as f64 / 1.0e4) * scale;
        let noise = 250.0 * scale;
        let mut out = Vec::with_capacity(n * channels as usize);
        for i in 0..n {
            for c in 0..channels {
                let p = c as f64 * 0.35;
                let v = a1 * (f1 * i as f64 + p).sin()
                    + a2 * (f2 * i as f64 + p).sin()
                    + noise * ((rng() >> 16) as u16 as i16 as f64 / 32768.0);
                out.push(v.round().clamp(minv, maxv) as i32);
            }
        }
        out
    }

    #[test]
    fn round_trip_all_depths_levels() {
        let bs = 2048u32;
        for &bps in &[8u32, 12, 16, 20, 24, 32] {
            for &ch in &[1u32, 2] {
                for level in [0u32, 2, 5, 8] {
                    for seed in 1..=3u32 {
                        // Non-block-multiple lengths exercise a short final frame.
                        let n = bs as usize + (seed as usize * 173) % 1500;
                        let pcm = gen_pcm(seed, n, bps, ch);
                        let frames = encode_frames(&pcm, ch, bps, 44_100, bs, &preset(level));
                        let dec = decode_frames(&frames).expect("decode failed (CRC/format)");
                        assert_eq!(dec.channels, ch);
                        assert_eq!(dec.bits_per_sample, bps);
                        assert_eq!(dec.sample_rate, 44_100);
                        assert_eq!(
                            dec.interleaved, pcm,
                            "round-trip mismatch [bps {bps} ch {ch} level {level} seed {seed}]"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn round_trip_edge_signals() {
        let bs = 2048u32;
        for &bps in &[16u32, 24, 32] {
            let maxv = ((1i64 << (bps - 1)) - 1) as i32;
            let minv = -(1i64 << (bps - 1)) as i32;
            let cases: &[(&str, Vec<i32>)] = &[
                ("silence", vec![0i32; bs as usize * 2 + 50]), // CONSTANT
                ("dc", vec![1234 << 3; bs as usize + 10]),     // CONSTANT + wasted bits
                ("full_pos", vec![maxv; bs as usize + 7]),
                ("full_neg", vec![minv; bs as usize + 7]),
                ("tiny", vec![5, -5, 5]), // < MAX_FIXED_ORDER -> VERBATIM, short frame
            ];
            for &(name, ref mono) in cases {
                // Duplicate into stereo so mid-side paths run too.
                let stereo: Vec<i32> = mono.iter().flat_map(|&v| [v, v]).collect();
                for level in [0u32, 8] {
                    let frames = encode_frames(&stereo, 2, bps, 44_100, bs, &preset(level));
                    let dec = decode_frames(&frames).expect("decode failed");
                    assert_eq!(dec.interleaved, stereo, "[{name} bps {bps} level {level}]");
                }
            }
        }
    }
}
