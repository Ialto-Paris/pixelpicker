// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Ialto

//! Small Win32 helpers shared by the windows of the application.

use std::ffi::c_void;

use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_TRANSITIONS_FORCEDISABLED, DwmSetWindowAttribute};
use windows_sys::Win32::Graphics::Gdi::{
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, DEFAULT_CHARSET, DEFAULT_PITCH,
    FF_DONTCARE, HDC, HFONT, OUT_DEFAULT_PRECIS, TextOutW,
};

/// NUL-terminated UTF-16 copy of `s`.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn loword(v: usize) -> u16 {
    (v & 0xFFFF) as u16
}

pub fn hiword(v: usize) -> u16 {
    ((v >> 16) & 0xFFFF) as u16
}

/// Signed client coordinates packed in a mouse message `LPARAM`.
pub fn point_from_lparam(lp: LPARAM) -> (i32, i32) {
    let v = lp as usize;
    (loword(v) as i16 as i32, hiword(v) as i16 as i32)
}

/// Converts a length in 96-DPI units to physical pixels at `dpi`.
pub fn scale(v: i32, dpi: u32) -> i32 {
    ((i64::from(v) * i64::from(dpi) + 48) / 96) as i32
}

pub fn rect(x: i32, y: i32, w: i32, h: i32) -> RECT {
    RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    }
}

pub fn contains(r: &RECT, x: i32, y: i32) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

pub fn intersects(a: &RECT, b: &RECT) -> bool {
    a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
}

/// Creates a font whose em height is `em_px` physical pixels.
pub fn create_font(face: &str, em_px: i32, weight: i32) -> HFONT {
    let face = wide(face);
    unsafe {
        CreateFontW(
            -em_px,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET as _,
            OUT_DEFAULT_PRECIS as _,
            CLIP_DEFAULT_PRECIS as _,
            CLEARTYPE_QUALITY as _,
            (DEFAULT_PITCH | FF_DONTCARE) as _,
            face.as_ptr(),
        )
    }
}

pub fn text_out(hdc: HDC, x: i32, y: i32, text: &str) {
    let w: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        TextOutW(hdc, x, y, w.as_ptr(), w.len() as i32);
    }
}

/// Turns DWM show/hide animations off (or back on) for `hwnd`, so that hiding a window
/// before a screen capture takes effect immediately.
pub fn set_transitions_disabled(hwnd: HWND, disabled: bool) {
    let value: i32 = disabled.into();
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED as _,
            (&raw const value).cast::<c_void>(),
            size_of::<i32>() as u32,
        );
    }
}
