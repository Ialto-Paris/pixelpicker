// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Ialto

//! Pixel Picker icon artwork: a 3×3 grid of colored pixels whose center one is
//! highlighted, like the target in the magnifier.
//!
//! Pure code, shared by the application (window icons, drawn at the exact system size)
//! and by `build.rs` (icon resource of the executable), so both always match.

const PALETTE: [u32; 9] = [
    0xE74C3C, 0xF1C40F, 0x2ECC71, //
    0x9B59B6, 0x3498DB, 0x1ABC9C, //
    0xE67E22, 0xECF0F1, 0x34495E,
];
const OPAQUE: u32 = 0xFF00_0000;
const BLACK: u32 = OPAQUE;
const WHITE: u32 = OPAQUE | 0xFF_FFFF;

/// Straight-alpha 0xAARRGGBB pixels of the icon at `size` × `size`, rows top to bottom.
pub fn draw(size: usize) -> Vec<u32> {
    let mut canvas = Canvas {
        size,
        pixels: vec![0; size * size],
    };
    let cell = size / 3;
    let offset = (size - cell * 3) / 2;
    let gap = if size >= 20 { (size / 24).max(1) } else { 0 };
    let ring = (size / 32).max(1);

    let cell_area = |row: usize, col: usize| Area {
        x0: offset + col * cell + gap,
        y0: offset + row * cell + gap,
        x1: offset + (col + 1) * cell - gap,
        y1: offset + (row + 1) * cell - gap,
    };
    for (index, &color) in PALETTE.iter().enumerate() {
        canvas.fill(cell_area(index / 3, index % 3), OPAQUE | color);
    }

    // The target ring: white on the edge of the center cell, black just outside it.
    let center = cell_area(1, 1);
    canvas.frame(center.grown(ring), ring, BLACK);
    canvas.frame(center, ring, WHITE);
    canvas.pixels
}

/// Pixels `x0..x1` × `y0..y1`.
#[derive(Clone, Copy)]
struct Area {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

impl Area {
    fn grown(self, n: usize) -> Self {
        Self {
            x0: self.x0.saturating_sub(n),
            y0: self.y0.saturating_sub(n),
            x1: self.x1 + n,
            y1: self.y1 + n,
        }
    }
}

struct Canvas {
    size: usize,
    pixels: Vec<u32>,
}

impl Canvas {
    fn fill(&mut self, a: Area, color: u32) {
        for y in a.y0..a.y1.min(self.size) {
            for x in a.x0..a.x1.min(self.size) {
                self.pixels[y * self.size + x] = color;
            }
        }
    }

    /// Outline `t` pixels thick along the inside of `a`.
    fn frame(&mut self, a: Area, t: usize, color: u32) {
        self.fill(Area { y1: a.y0 + t, ..a }, color);
        self.fill(Area { y0: a.y1 - t, ..a }, color);
        self.fill(Area { x1: a.x0 + t, ..a }, color);
        self.fill(Area { x0: a.x1 - t, ..a }, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn art_has_opaque_center_and_transparent_corners_when_padded() {
        for size in [16, 20, 24, 32, 40, 48, 64, 256] {
            let px = draw(size);
            assert_eq!(px.len(), size * size);
            let center = px[(size / 2) * size + size / 2];
            assert_eq!(center >> 24, 0xFF, "size {size}");
        }
        let px = draw(32);
        assert_eq!(px[0], 0, "corner gap is transparent");
    }

    #[test]
    fn alpha_is_binary() {
        // The executable's icon resource relies on this to build its transparency mask.
        for size in [16, 32, 256] {
            assert!(draw(size).iter().all(|p| matches!(p >> 24, 0 | 0xFF)));
        }
    }

    #[test]
    fn target_ring_is_white_inside_black_outside() {
        let size = 32;
        let px = draw(size);
        // Center cell spans 12..20 at 32 px (cell 10, offset 1, gap 1).
        assert_eq!(px[16 * size + 12], WHITE);
        assert_eq!(px[16 * size + 11], BLACK);
        assert_eq!(px[16 * size + 16], OPAQUE | PALETTE[4]);
    }
}
