// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Ialto

//! Color value type and its text representations.

/// An opaque sRGB color, 8 bits per channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// HSL components, rounded: hue in degrees (0..360), saturation and lightness in percent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hsl {
    pub h: u16,
    pub s: u8,
    pub l: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Reads a 32-bit DIB pixel (0xAARRGGBB, i.e. B, G, R, A bytes in memory). Alpha is ignored.
    pub const fn from_xrgb(v: u32) -> Self {
        Self::new((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    /// Packs the color as a 32-bit DIB pixel (0x00RRGGBB).
    pub const fn to_xrgb(self) -> u32 {
        (self.r as u32) << 16 | (self.g as u32) << 8 | self.b as u32
    }

    /// Packs the color as a GDI COLORREF (0x00BBGGRR).
    pub const fn to_colorref(self) -> u32 {
        (self.b as u32) << 16 | (self.g as u32) << 8 | self.r as u32
    }

    /// `#RRGGBB`, uppercase.
    pub fn hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// `R, G, B` in decimal.
    pub fn rgb(self) -> String {
        format!("{}, {}, {}", self.r, self.g, self.b)
    }

    /// CSS notation, e.g. `hsl(30, 100%, 50%)`.
    pub fn hsl_css(self) -> String {
        let Hsl { h, s, l } = self.hsl();
        format!("hsl({h}, {s}%, {l}%)")
    }

    pub fn hsl(self) -> Hsl {
        let r = f64::from(self.r) / 255.0;
        let g = f64::from(self.g) / 255.0;
        let b = f64::from(self.b) / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        let d = max - min;

        let (h, s) = if d == 0.0 {
            (0.0, 0.0)
        } else {
            let s = if l > 0.5 {
                d / (2.0 - max - min)
            } else {
                d / (max + min)
            };
            let h = if max == r {
                (g - b) / d + if g < b { 6.0 } else { 0.0 }
            } else if max == g {
                (b - r) / d + 2.0
            } else {
                (r - g) / d + 4.0
            };
            (h * 60.0, s)
        };

        Hsl {
            h: (h.round() as u16) % 360,
            s: (s * 100.0).round() as u8,
            l: (l * 100.0).round() as u8,
        }
    }

    /// Perceived brightness (Rec. 601 weights), 0..=255.
    pub fn luma(self) -> u8 {
        ((299 * u32::from(self.r) + 587 * u32::from(self.g) + 114 * u32::from(self.b)) / 1000) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_is_uppercase_and_padded() {
        assert_eq!(Rgb::new(255, 128, 0).hex(), "#FF8000");
        assert_eq!(Rgb::new(0, 10, 1).hex(), "#000A01");
    }

    #[test]
    fn rgb_text() {
        assert_eq!(Rgb::new(255, 128, 0).rgb(), "255, 128, 0");
    }

    #[test]
    fn hsl_conversion() {
        assert_eq!(Rgb::new(255, 128, 0).hsl_css(), "hsl(30, 100%, 50%)");
        assert_eq!(Rgb::new(0, 0, 255).hsl_css(), "hsl(240, 100%, 50%)");
        assert_eq!(Rgb::new(51, 102, 153).hsl_css(), "hsl(210, 50%, 40%)");
        assert_eq!(Rgb::new(255, 0, 128).hsl().h, 330);
        assert_eq!(Rgb::new(255, 255, 255).hsl_css(), "hsl(0, 0%, 100%)");
        assert_eq!(Rgb::new(0, 0, 0).hsl_css(), "hsl(0, 0%, 0%)");
        assert_eq!(Rgb::new(128, 128, 128).hsl_css(), "hsl(0, 0%, 50%)");
    }

    #[test]
    fn pixel_packing_round_trips() {
        let c = Rgb::new(0x12, 0x34, 0x56);
        assert_eq!(c.to_xrgb(), 0x0012_3456);
        assert_eq!(Rgb::from_xrgb(0xFF12_3456), c);
        assert_eq!(c.to_colorref(), 0x0056_3412);
    }

    #[test]
    fn luma_extremes() {
        assert_eq!(Rgb::new(0, 0, 0).luma(), 0);
        assert_eq!(Rgb::new(255, 255, 255).luma(), 255);
    }
}
