// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Ialto

//! Window icons, created at run time from the shared artwork (`icon_art`) at the exact
//! size the system asks for, so they stay sharp at every display scaling. The executable
//! embeds the same artwork as its icon resource (see `build.rs`).

use std::ffi::c_void;
use std::ptr::null_mut;

use windows_sys::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, DIB_RGB_COLORS, DeleteObject, GdiFlush,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, HICON, ICONINFO};

use crate::capture::bitmap_info;
use crate::icon_art;

/// Creates a square icon of `size` pixels; returns a null handle on failure.
pub fn create(size: i32) -> HICON {
    if size <= 0 {
        return null_mut();
    }
    let art = icon_art::draw(size as usize);
    unsafe {
        let info = bitmap_info(size, size);
        let mut bits: *mut c_void = null_mut();
        let color = CreateDIBSection(null_mut(), &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if color.is_null() || bits.is_null() {
            return null_mut();
        }
        GdiFlush();
        std::ptr::copy_nonoverlapping(art.as_ptr(), bits.cast::<u32>(), art.len());

        // With a 32-bit color bitmap the alpha channel drives transparency; the mask must
        // still exist and is left all zero. Rows of a monochrome bitmap are WORD aligned.
        let mask_bits = vec![0u8; ((size as usize).div_ceil(16) * 2) * size as usize];
        let mask = CreateBitmap(size, size, 1, 1, mask_bits.as_ptr().cast());
        let icon_info = ICONINFO {
            fIcon: 1,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = CreateIconIndirect(&icon_info);
        DeleteObject(mask);
        DeleteObject(color);
        icon
    }
}
