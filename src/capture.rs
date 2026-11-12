// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Ialto

//! Off-screen 32-bit bitmaps and capture of the whole virtual screen.

use std::ffi::c_void;
use std::ptr::null_mut;

use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, CreateCompatibleDC, CreateDIBSection,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GdiFlush, GetDC, HBITMAP, HDC, HGDIOBJ, ReleaseDC,
    SRCCOPY, SelectObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

use crate::color::Rgb;

/// A top-down 32-bit DIB section selected into its own memory DC.
/// Pixels are `u32` values laid out as 0xAARRGGBB.
pub struct Dib {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    bits: *mut u32,
    width: i32,
    height: i32,
}

impl Dib {
    pub fn new(width: i32, height: i32) -> Option<Self> {
        if width <= 0 || height <= 0 {
            return None;
        }
        unsafe {
            let dc = CreateCompatibleDC(null_mut());
            if dc.is_null() {
                return None;
            }
            let info = bitmap_info(width, height);
            let mut bits: *mut c_void = null_mut();
            let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
            if bitmap.is_null() || bits.is_null() {
                DeleteDC(dc);
                return None;
            }
            let previous = SelectObject(dc, bitmap);
            Some(Self {
                dc,
                bitmap,
                previous,
                bits: bits.cast(),
                width,
                height,
            })
        }
    }

    pub fn dc(&self) -> HDC {
        self.dc
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    fn len(&self) -> usize {
        self.width as usize * self.height as usize
    }

    /// Pixel buffer, after any pending GDI drawing on the bitmap has completed.
    pub fn pixels(&self) -> &[u32] {
        unsafe {
            GdiFlush();
            std::slice::from_raw_parts(self.bits, self.len())
        }
    }

    pub fn pixels_mut(&mut self) -> &mut [u32] {
        unsafe {
            GdiFlush();
            std::slice::from_raw_parts_mut(self.bits, self.len())
        }
    }
}

impl Drop for Dib {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}

/// Header of a top-down 32-bit BI_RGB bitmap.
pub fn bitmap_info(width: i32, height: i32) -> BITMAPINFO {
    // SAFETY: BITMAPINFO is plain old data; all-zero is a valid value.
    let mut info: BITMAPINFO = unsafe { std::mem::zeroed() };
    info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = width;
    info.bmiHeader.biHeight = -height; // negative: rows stored top to bottom
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB as _;
    info
}

/// A frozen copy of every monitor, in physical pixels.
pub struct Screenshot {
    /// Screen coordinates of the top-left corner of the virtual screen (may be negative).
    pub left: i32,
    pub top: i32,
    dib: Dib,
}

impl Screenshot {
    pub fn capture() -> Option<Self> {
        unsafe {
            let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            let dib = Dib::new(width, height)?;

            let screen = GetDC(null_mut());
            if screen.is_null() {
                return None;
            }
            // CAPTUREBLT also captures layered windows (tooltips, translucent windows...).
            let ok = BitBlt(
                dib.dc(),
                0,
                0,
                width,
                height,
                screen,
                left,
                top,
                SRCCOPY | CAPTUREBLT,
            );
            ReleaseDC(null_mut(), screen);
            (ok != 0).then_some(Self { left, top, dib })
        }
    }

    pub fn width(&self) -> i32 {
        self.dib.width()
    }

    pub fn height(&self) -> i32 {
        self.dib.height()
    }

    pub fn dc(&self) -> HDC {
        self.dib.dc()
    }

    pub fn pixels(&self) -> &[u32] {
        self.dib.pixels()
    }

    /// Color of the pixel at screen coordinates (`x`, `y`).
    pub fn pixel_at(&self, x: i32, y: i32) -> Option<Rgb> {
        let (ix, iy) = (x - self.left, y - self.top);
        if ix < 0 || iy < 0 || ix >= self.width() || iy >= self.height() {
            return None;
        }
        let index = iy as usize * self.width() as usize + ix as usize;
        Some(Rgb::from_xrgb(self.pixels()[index]))
    }
}
