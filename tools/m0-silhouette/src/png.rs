//! A PNG writer, in the one shape this tool needs: 8-bit indexed colour, no filtering.
//!
//! # Why there is a PNG encoder in here
//!
//! Because the alternative is a dependency. An image crate brings a decoder, a filter bank,
//! and a DEFLATE implementation into `cargo deny`'s report so that a debug tool can write
//! files nobody keeps. The format does not require any of it: a PNG's IDAT is a zlib stream,
//! zlib streams may be built from *stored* — uncompressed — DEFLATE blocks, and a stored
//! block is a length, its complement, and the bytes. That is the whole compressor below.
//!
//! The cost is honest and bounded: an image is about its raw size, so a 1024×1024 indexed
//! frame lands near 1 MB instead of the 40 kB a real compressor would manage. They are
//! written to `target/`, which is a build directory, and they are looked at once.
//!
//! # Indexed, not RGB
//!
//! Two reasons, and the second is the real one. Indexed is a third the bytes. More usefully,
//! it makes the palette a *statement*: 256 entries laid out as four ink families × 64
//! coverage levels, so "what colour is a limb" is one table in [`crate::draw`] rather than a
//! blend computed per pixel. Changing how the debug view reads is then an edit to sixteen
//! bytes, which is the kind of tuning this milestone exists to do.

/// The eight bytes every PNG starts with.
const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// The largest a stored DEFLATE block may be, from RFC 1951 §3.2.4.
const STORED_MAX: usize = 0xFFFF;

/// The modulus Adler-32 is computed under, from RFC 1950 §9.
const ADLER_MODULUS: u32 = 65_521;

/// The standard CRC-32 table, built at compile time rather than shipped as a literal.
const CRC_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut n = 0usize;
    while n < 256 {
        let mut c = n as u32;
        let mut bit = 0;
        while bit < 8 {
            c = if c & 1 == 0 {
                c >> 1
            } else {
                0xEDB8_8320 ^ (c >> 1)
            };
            bit += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
};

/// Encodes an 8-bit indexed image.
///
/// `indices` is row-major, `width * height` bytes, each one an index into `palette`.
///
/// # Panics
///
/// If `indices` is not exactly `width * height` bytes. A caller that gets this wrong has a
/// rasterizer bug, and a silently short image would be a confusing way to find out.
#[must_use]
pub(crate) fn encode(width: u32, height: u32, palette: &[[u8; 3]; 256], indices: &[u8]) -> Vec<u8> {
    let expected = width as usize * height as usize;
    assert_eq!(
        indices.len(),
        expected,
        "png::encode: {} indices for a {width}×{height} image",
        indices.len()
    );

    // Filter type 0 (None) on every row. Filtering exists to make the data compress; this
    // does not compress, so a filter would cost a pass over the image and buy nothing.
    let mut raw = Vec::with_capacity(expected + height as usize);
    for row in indices.chunks(width as usize) {
        raw.push(0u8);
        raw.extend_from_slice(row);
    }

    let mut out = Vec::from(SIGNATURE);

    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[
        8, // bit depth
        3, // colour type 3 — indexed
        0, // compression method 0 — the only one defined
        0, // filter method 0 — the only one defined
        0, // not interlaced
    ]);
    chunk(&mut out, b"IHDR", &header);

    let mut plte = Vec::with_capacity(768);
    for entry in palette {
        plte.extend_from_slice(entry);
    }
    chunk(&mut out, b"PLTE", &plte);

    chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Appends one length-tagged, CRC-checked chunk.
fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    let length = u32::try_from(body.len()).expect("chunk body fits in u32");
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(body);

    // The CRC covers the type and the body, but not the length. RFC 2083 §3.2.
    let mut crc = crc32(kind);
    crc = crc32_continue(crc, body);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Wraps `raw` as a zlib stream of stored DEFLATE blocks.
fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    // 0x78 0x01: deflate, 32 KiB window, no preset dictionary, "fastest" level. The pair is
    // checked as a big-endian u16 divisible by 31 — 0x7801 is 30721, which is 991 × 31.
    let mut out = vec![0x78, 0x01];

    let mut offset = 0usize;
    loop {
        let end = raw.len().min(offset + STORED_MAX);
        let block = &raw[offset..end];
        let last = end == raw.len();

        // Bit 0 is BFINAL; bits 1–2 are BTYPE, and 00 is "stored". The remaining bits of the
        // byte are padding to the next boundary, which stored blocks require anyway.
        out.push(u8::from(last));

        let len = u16::try_from(block.len()).expect("block is at most STORED_MAX");
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(block);

        offset = end;
        if last {
            break;
        }
    }

    out.extend_from_slice(&adler32(raw).to_be_bytes());
    out
}

/// CRC-32 of one buffer.
fn crc32(bytes: &[u8]) -> u32 {
    crc32_continue(0, bytes)
}

/// CRC-32 continued from a previous result, so a chunk's type and body hash as one stream.
fn crc32_continue(previous: u32, bytes: &[u8]) -> u32 {
    let mut c = previous ^ 0xFFFF_FFFF;
    for &byte in bytes {
        c = CRC_TABLE[usize::from((c as u8) ^ byte)] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

/// Adler-32 of the uncompressed data, which closes the zlib stream.
fn adler32(bytes: &[u8]) -> u32 {
    let (mut low, mut high) = (1u32, 0u32);
    // Chunked so the sums cannot overflow between reductions: 5552 is the largest run of
    // 0xFF bytes that keeps `high` inside u32, and is the constant zlib itself uses.
    for run in bytes.chunks(5552) {
        for &byte in run {
            low += u32::from(byte);
            high += low;
        }
        low %= ADLER_MODULUS;
        high %= ADLER_MODULUS;
    }
    (high << 16) | low
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two check values in the format, on the vectors their specifications give.
    ///
    /// A wrong CRC or Adler is the failure mode that produces a file every viewer refuses
    /// with no useful message, so they are worth pinning to something external.
    #[test]
    fn the_check_values_agree_with_their_specifications() {
        // The canonical CRC-32 of "123456789".
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        // RFC 1950 §9's worked example.
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        // An empty stream still has a defined Adler-32.
        assert_eq!(adler32(b""), 1);
    }

    /// A stored block's NLEN must be the ones' complement of its LEN, or a decoder rejects
    /// the stream outright. It is one `!` and exactly the sort of thing that gets dropped.
    #[test]
    fn every_stored_block_carries_its_own_length_twice() {
        for size in [0usize, 1, STORED_MAX, STORED_MAX + 1, STORED_MAX * 2 + 7] {
            let stream = zlib_stored(&vec![0xABu8; size]);
            let mut offset = 2; // past the zlib header
            let mut seen = 0usize;
            loop {
                let last = stream[offset] == 1;
                let len = u16::from_le_bytes([stream[offset + 1], stream[offset + 2]]);
                let nlen = u16::from_le_bytes([stream[offset + 3], stream[offset + 4]]);
                assert_eq!(nlen, !len, "size {size}: NLEN is not ~LEN");
                seen += usize::from(len);
                offset += 5 + usize::from(len);
                if last {
                    break;
                }
            }
            assert_eq!(seen, size, "size {size}: blocks do not cover the input");
            assert_eq!(offset + 4, stream.len(), "size {size}: trailing bytes");
        }
    }

    /// The structural claims a decoder makes before it looks at a single pixel.
    #[test]
    fn the_file_is_a_png_a_decoder_would_recognize() {
        let image = encode(3, 2, &[[0, 0, 0]; 256], &[0, 1, 2, 3, 4, 5]);

        assert_eq!(&image[..8], &SIGNATURE);

        // Walk the chunk list the way a decoder does, by length rather than by search.
        let mut kinds = Vec::new();
        let mut offset = 8;
        while offset < image.len() {
            let length = u32::from_be_bytes(image[offset..offset + 4].try_into().unwrap()) as usize;
            let kind = std::str::from_utf8(&image[offset + 4..offset + 8]).unwrap();

            let body = &image[offset + 8..offset + 8 + length];
            let stated = u32::from_be_bytes(
                image[offset + 8 + length..offset + 12 + length]
                    .try_into()
                    .unwrap(),
            );
            let mut computed = crc32(kind.as_bytes());
            computed = crc32_continue(computed, body);
            assert_eq!(stated, computed, "{kind}: CRC disagrees");

            kinds.push(kind.to_string());
            offset += 12 + length;
        }

        assert_eq!(kinds, ["IHDR", "PLTE", "IDAT", "IEND"]);
        assert_eq!(offset, image.len(), "chunk lengths do not cover the file");
    }
}
