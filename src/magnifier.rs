// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Ialto

//! Geometry and pixel rendering of the magnifier panel. Pure code (no Win32 calls),
//! so it can be unit-tested.
//!
//! Panel layout, in physical pixels:
//!
//! ```text
//! +-------------------+   1 px border
//! |                   |
//! |     magnifier     |   `mag` × `mag`: `cells` × `cells` source pixels of `cell` px each
//! |                   |
//! +-------------------+   1 px separator
//! | [■] #RRGGBB       |
//! |     R, G, B       |   info box, `info` px high
//! |     x, y · ×zoom  |
//! +-------------------+   1 px border
//! ```

use crate::color::Rgb;
use crate::win::scale;

/// Size, in physical pixels at 100 % scaling, of one source pixel in the magnifier.
pub const ZOOM_LEVELS: [i32; 10] = [3, 4, 5, 6, 8, 10, 12, 15, 20, 30];
pub const DEFAULT_ZOOM: usize = 5;

/// Minimum magnifier side at 100 % scaling.
const TARGET_SIZE: i32 = 180;
const INFO_HEIGHT: i32 = 62;
pub const BORDER: i32 = 1;
/// Grid lines are only drawn when cells are at least this large (physical pixels).
const GRID_MIN_CELL: i32 = 6;

const OUTSIDE_DARK: u32 = 0x2A2A2A;
const OUTSIDE_LIGHT: u32 = 0x3C3C3C;
const RING_INNER: u32 = 0xFFFFFF;
const RING_OUTER: u32 = 0x000000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub dpi: u32,
    /// Zoom factor at 100 % scaling (one of `ZOOM_LEVELS`).
    pub zoom: i32,
    /// Physical pixels per source pixel.
    pub cell: i32,
    /// Source pixels per side (always odd, so that the target is the exact center).
    pub cells: i32,
    /// Magnifier side, `cell * cells`.
    pub mag: i32,
    pub info: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Layout {
    pub fn new(zoom_index: usize, dpi: u32) -> Self {
        let zoom = ZOOM_LEVELS[zoom_index.min(ZOOM_LEVELS.len() - 1)];
        let cell = scale(zoom, dpi).max(1);
        let target = scale(TARGET_SIZE, dpi);
        let mut cells = (target + cell - 1) / cell;
        if cells % 2 == 0 {
            cells += 1;
        }
        let cells = cells.max(3);
        let mag = cells * cell;
        let info = scale(INFO_HEIGHT, dpi);
        Self {
            dpi,
            zoom,
            cell,
            cells,
            mag,
            info,
            width: mag + 2 * BORDER,
            height: mag + info + 3 * BORDER,
        }
    }

    pub fn magnifier_area(&self) -> Area {
        Area {
            x: BORDER,
            y: BORDER,
            w: self.mag,
            h: self.mag,
        }
    }

    pub fn info_area(&self) -> Area {
        Area {
            x: BORDER,
            y: 2 * BORDER + self.mag,
            w: self.mag,
            h: self.info,
        }
    }
}

/// Position of the panel's top-left corner: below-right of the cursor, flipped to the
/// other side of the cursor when it would leave `bounds` (left, top, right, bottom).
pub fn place(
    cursor: (i32, i32),
    size: (i32, i32),
    bounds: (i32, i32, i32, i32),
    offset: i32,
) -> (i32, i32) {
    let axis = |c: i32, len: i32, lo: i32, hi: i32| {
        let mut v = c + offset;
        if v + len > hi {
            v = c - offset - len;
        }
        v.clamp(lo, (hi - len).max(lo))
    };
    (
        axis(cursor.0, size.0, bounds.0, bounds.2),
        axis(cursor.1, size.1, bounds.1, bounds.3),
    )
}

/// Source image, 0xAARRGGBB pixels, rows of `width` pixels.
pub struct Source<'a> {
    pub pixels: &'a [u32],
    pub width: i32,
    pub height: i32,
}

/// Target image: `pixels` with rows of `stride` pixels.
pub struct Target<'a> {
    pub pixels: &'a mut [u32],
    pub stride: i32,
}

impl Target<'_> {
    /// Fills `area`, clipped to `clip` and to the buffer.
    pub fn fill(&mut self, area: Area, clip: Area, color: u32) {
        let rows = self.pixels.len() as i32 / self.stride;
        let x0 = area.x.max(clip.x).max(0);
        let y0 = area.y.max(clip.y).max(0);
        let x1 = (area.x + area.w).min(clip.x + clip.w).min(self.stride);
        let y1 = (area.y + area.h).min(clip.y + clip.h).min(rows);
        for y in y0..y1 {
            let row = (y * self.stride) as usize;
            self.pixels[row + x0 as usize..row + x1.max(x0) as usize].fill(color);
        }
    }

    /// Draws a square outline `t` pixels thick whose outer side is `size`.
    fn ring(&mut self, x: i32, y: i32, size: i32, t: i32, clip: Area, color: u32) {
        self.fill(
            Area {
                x,
                y,
                w: size,
                h: t,
            },
            clip,
            color,
        );
        self.fill(
            Area {
                x,
                y: y + size - t,
                w: size,
                h: t,
            },
            clip,
            color,
        );
        self.fill(
            Area {
                x,
                y: y + t,
                w: t,
                h: size - 2 * t,
            },
            clip,
            color,
        );
        self.fill(
            Area {
                x: x + size - t,
                y: y + t,
                w: t,
                h: size - 2 * t,
            },
            clip,
            color,
        );
    }
}

/// Renders the neighbourhood of `center` (source coordinates) magnified into `area` of
/// `dst`, with an optional pixel grid and a black-and-white ring around the target pixel.
pub fn render(
    src: &Source,
    center: (i32, i32),
    layout: &Layout,
    grid: bool,
    dst: &mut Target,
    area: Area,
) {
    let half = layout.cells / 2;
    let cell = layout.cell;
    let grid = grid && cell >= GRID_MIN_CELL;

    for oy in 0..layout.mag {
        let cy = oy / cell;
        let sy = center.1 - half + cy;
        let row = ((area.y + oy) * dst.stride + area.x) as usize;
        let inside_row = sy >= 0 && sy < src.height;
        for ox in 0..layout.mag {
            let cx = ox / cell;
            let sx = center.0 - half + cx;
            let mut color = if inside_row && sx >= 0 && sx < src.width {
                src.pixels[(sy * src.width + sx) as usize] & 0x00FF_FFFF
            } else if (cx + cy) % 2 == 0 {
                OUTSIDE_DARK
            } else {
                OUTSIDE_LIGHT
            };
            if grid && (ox % cell == 0 || oy % cell == 0) {
                color = grid_tint(color);
            }
            dst.pixels[row + ox as usize] = color;
        }
    }

    // Target ring, drawn just outside the center cell so that the cell stays fully visible.
    let t = scale(1, layout.dpi).max(1);
    let x0 = area.x + half * cell;
    let y0 = area.y + half * cell;
    dst.ring(x0 - t, y0 - t, cell + 2 * t, t, area, RING_INNER);
    dst.ring(x0 - 2 * t, y0 - 2 * t, cell + 4 * t, t, area, RING_OUTER);
}

/// Grid line color: the pixel color pushed 30 % toward black (light pixels) or white (dark ones).
fn grid_tint(color: u32) -> u32 {
    let c = Rgb::from_xrgb(color);
    let target: u32 = if c.luma() > 140 { 0 } else { 255 };
    let mix = |v: u8| ((u32::from(v) * 7 + target * 3) / 10) as u8;
    Rgb::new(mix(c.r), mix(c.g), mix(c.b)).to_xrgb()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_has_odd_cell_count_and_minimum_size() {
        for dpi in [96, 120, 144, 192] {
            for zoom in 0..ZOOM_LEVELS.len() {
                let l = Layout::new(zoom, dpi);
                assert_eq!(l.cells % 2, 1, "zoom {zoom} dpi {dpi}");
                assert!(l.mag >= scale(TARGET_SIZE, dpi));
                assert_eq!(l.mag, l.cells * l.cell);
                assert_eq!(l.height, l.info_area().y + l.info + BORDER);
            }
        }
        let l = Layout::new(DEFAULT_ZOOM, 96);
        assert_eq!((l.cell, l.cells, l.mag), (10, 19, 190));
    }

    #[test]
    fn panel_is_flipped_near_monitor_edges() {
        let bounds = (0, 0, 1920, 1080);
        assert_eq!(place((100, 100), (200, 260), bounds, 20), (120, 120));
        assert_eq!(place((1900, 100), (200, 260), bounds, 20), (1680, 120));
        assert_eq!(place((100, 1000), (200, 260), bounds, 20), (120, 720));
        assert_eq!(place((1900, 1000), (200, 260), bounds, 20), (1680, 720));
        // Secondary monitor on the left, with negative coordinates.
        assert_eq!(
            place((-10, 5), (200, 260), (-1280, 0, 0, 1024), 20),
            (-230, 25)
        );
    }

    fn sample_source() -> Vec<u32> {
        (0..25).map(|i| 0xFF00_0000 | (i * 0x0A0A0A)).collect()
    }

    #[test]
    fn center_cell_shows_the_target_pixel() {
        let pixels = sample_source();
        let src = Source {
            pixels: &pixels,
            width: 5,
            height: 5,
        };
        let layout = Layout::new(ZOOM_LEVELS.len() - 1, 96); // 30 px cells, 7 cells
        assert_eq!((layout.cell, layout.cells), (30, 7));
        let mut buffer = vec![0u32; (layout.width * layout.height) as usize];
        let mut dst = Target {
            pixels: &mut buffer,
            stride: layout.width,
        };
        let area = layout.magnifier_area();
        render(&src, (2, 2), &layout, true, &mut dst, area);

        let at = |x: i32, y: i32| buffer[((area.y + y) * layout.width + area.x + x) as usize];
        // Middle of the center cell = source pixel (2, 2), alpha stripped.
        assert_eq!(at(105, 105), 12 * 0x0A0A0A);
        // Middle of the cell right of the center = source pixel (3, 2).
        assert_eq!(at(135, 105), 13 * 0x0A0A0A);
        // Top-left cell maps to (-1, -1): outside the source, checkerboard.
        assert_eq!(at(15, 15), OUTSIDE_DARK);
        // Ring around the center cell: white just outside it, black one pixel further.
        assert_eq!(at(89, 105), RING_INNER);
        assert_eq!(at(88, 105), RING_OUTER);
    }

    #[test]
    fn fill_is_clipped() {
        let mut buffer = vec![0u32; 16];
        let mut dst = Target {
            pixels: &mut buffer,
            stride: 4,
        };
        dst.fill(
            Area {
                x: -2,
                y: 2,
                w: 10,
                h: 10,
            },
            Area {
                x: 0,
                y: 0,
                w: 3,
                h: 4,
            },
            7,
        );
        assert_eq!(buffer, [0, 0, 0, 0, 0, 0, 0, 0, 7, 7, 7, 0, 7, 7, 7, 0]);
    }
}
