use thiserror::Error;

/// Decompression-bomb ceiling for one payload.
pub const MAX_DECOMPRESSED_SIZE: usize = 32 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum CompressError {
    #[error("decompressed size exceeds limit ({0} B)")]
    LimitExceeded(usize),
    #[error("truncated LZ4 block input")]
    TruncatedInput,
    #[error("LZ4 block: zero match offset")]
    ZeroOffset,
    #[error("LZ4 block: match offset before start of output")]
    OffsetOutOfRange,
    #[error("zstd decompression error: {0}")]
    Zstd(String),
    #[error("LZ4 frame decompression error: {0}")]
    Lz4Frame(String),
}

/// Sniff format by magic number. The header only flags that a payload is
/// compressed, not which of LZ4 block / LZ4 frame / Zstd the server picked.
pub fn decompress(src: &[u8]) -> Result<Vec<u8>, CompressError> {
    // Zstandard magic: 28 B5 2F FD
    if src.len() >= 4 && src[0] == 0x28 && src[1] == 0xB5 && src[2] == 0x2F && src[3] == 0xFD {
        return decompress_zstd(src);
    }
    // LZ4 frame magic: 04 22 4D 18
    if src.len() >= 4 && src[0] == 0x04 && src[1] == 0x22 && src[2] == 0x4D && src[3] == 0x18 {
        return decompress_lz4_frame(src);
    }
    // no magic: LZ4 block
    decompress_lz4_block(src, MAX_DECOMPRESSED_SIZE)
}

fn decompress_zstd(src: &[u8]) -> Result<Vec<u8>, CompressError> {
    zstd::stream::decode_all(src).map_err(|e| CompressError::Zstd(e.to_string()))
}

fn decompress_lz4_frame(src: &[u8]) -> Result<Vec<u8>, CompressError> {
    use std::io::Read;
    let mut reader = lz4_flex::frame::FrameDecoder::new(src);
    let mut out = Vec::new();
    reader
        .read_to_end(&mut out)
        .map_err(|e| CompressError::Lz4Frame(e.to_string()))?;
    Ok(out)
}

/// Raw LZ4 block (no frame header, no size prefix), what the server expects
/// outgoing. Decompressed size travels out-of-band in the header flag byte.
pub fn compress_lz4_block(src: &[u8]) -> Vec<u8> {
    lz4_flex::block::compress(src)
}

/// LZ4 frame format. Kept for interop tests; outgoing traffic uses the block form.
pub fn compress_lz4_frame(src: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut enc = lz4_flex::frame::FrameEncoder::new(Vec::new());
    enc.write_all(src).expect("in-memory write cannot fail");
    enc.finish().expect("in-memory finish cannot fail")
}

/// LZ4 block decompression. Block format has no size prefix, so the output grows
/// dynamically.
pub fn decompress_lz4_block(src: &[u8], max_size: usize) -> Result<Vec<u8>, CompressError> {
    let mut out: Vec<u8> = Vec::with_capacity(1024);
    let mut pos = 0usize;

    while pos < src.len() {
        let token = src[pos];
        pos += 1;

        let mut lit_len = (token >> 4) as usize;
        if lit_len == 15 {
            while pos < src.len() {
                let b = src[pos];
                pos += 1;
                lit_len += b as usize;
                if b != 255 {
                    break;
                }
            }
        }

        if lit_len > 0 {
            if out.len() + lit_len > max_size {
                return Err(CompressError::LimitExceeded(max_size));
            }
            if pos + lit_len > src.len() {
                return Err(CompressError::TruncatedInput);
            }
            out.extend_from_slice(&src[pos..pos + lit_len]);
            pos += lit_len;
        }

        if pos >= src.len() {
            break;
        }

        if pos + 1 >= src.len() {
            return Err(CompressError::TruncatedInput);
        }
        let offset = (src[pos] as usize) | ((src[pos + 1] as usize) << 8);
        pos += 2;
        if offset == 0 {
            return Err(CompressError::ZeroOffset);
        }

        let mut match_len = (token & 0x0F) as usize + 4;
        if (token & 0x0F) == 0x0F {
            while pos < src.len() {
                let b = src[pos];
                pos += 1;
                match_len += b as usize;
                if b != 255 {
                    break;
                }
            }
        }

        if out.len() + match_len > max_size {
            return Err(CompressError::LimitExceeded(max_size));
        }
        if offset > out.len() {
            return Err(CompressError::OffsetOutOfRange);
        }
        let start = out.len() - offset;
        // overlapping copy: offset may be < match_len, so go byte-by-byte
        for i in 0..match_len {
            let b = out[start + i];
            out.push(b);
        }
    }

    Ok(out)
}
