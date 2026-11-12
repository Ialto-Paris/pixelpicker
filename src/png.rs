// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Ialto

//! Minimal PNG writer (8-bit RGBA, no filtering) for the icon pictures made at build time: the
//! executable's 256 px icon (`build.rs`) and the Microsoft Store logos
//! (`examples/msix_assets.rs`). It is not part of the program; `main.rs` includes it only for
//! its tests.

/// PNG file of `width` × `height` straight-alpha 0xAARRGGBB pixels, rows top to bottom.
pub fn encode(width: usize, height: usize, argb: &[u32]) -> Vec<u8> {
    assert_eq!(argb.len(), width * height, "pixel count");
    let mut scanlines = Vec::with_capacity(height * (width * 4 + 1));
    for row in argb.chunks(width) {
        scanlines.push(0); // filter type: none
        for &p in row {
            let [b, g, r, a] = p.to_le_bytes();
            scanlines.extend([r, g, b, a]);
        }
    }

    let mut header = Vec::with_capacity(13);
    header.extend((width as u32).to_be_bytes());
    header.extend((height as u32).to_be_bytes());
    header.extend([8, 6, 0, 0, 0]); // 8 bits, RGBA, deflate, no filter method, no interlace

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &header);
    let data = miniz_oxide::deflate::compress_to_vec_zlib(&scanlines, 9);
    chunk(&mut out, b"IDAT", &data);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend((data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend(kind);
    out.extend(data);
    let crc = crc32(&out[start..]);
    out.extend(crc.to_be_bytes());
}

/// CRC-32 (ISO-HDLC), as required by PNG chunks.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_the_png_specification() {
        // The CRC every PNG file ends with: that of the empty IEND chunk.
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    }

    #[test]
    fn pixels_survive_a_round_trip() {
        let argb = [
            0xFF11_2233,
            0x0000_0000,
            0x80AB_CDEF,
            0xFFFF_FFFF,
            0xFF00_0000,
            0x4012_3456,
        ];
        let file = encode(3, 2, &argb);
        assert_eq!(&file[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&file[file.len() - 12..], b"\0\0\0\0IEND\xAE\x42\x60\x82");

        // The IDAT chunk follows the 8-byte signature and the 25-byte IHDR chunk.
        let idat_len = u32::from_be_bytes(file[33..37].try_into().unwrap()) as usize;
        assert_eq!(&file[37..41], b"IDAT");
        let raw = miniz_oxide::inflate::decompress_to_vec_zlib(&file[41..41 + idat_len]).unwrap();
        let decoded: Vec<u32> = raw
            .chunks(3 * 4 + 1)
            .flat_map(|row| {
                assert_eq!(row[0], 0, "filter type");
                row[1..]
                    .chunks(4)
                    .map(|p| u32::from_le_bytes([p[2], p[1], p[0], p[3]]))
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(decoded, argb);
    }
}
