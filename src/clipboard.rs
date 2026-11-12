// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Ialto

//! Plain-text clipboard access.

use std::time::Duration;

use windows_sys::Win32::Foundation::{GlobalFree, HWND};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};

const CF_UNICODETEXT: u32 = 13;

/// Replaces the clipboard content with `text`. Returns `false` if the clipboard stayed
/// unavailable (another application may hold it open).
pub fn set_text(owner: HWND, text: &str) -> bool {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut opened = false;
        for _ in 0..10 {
            if OpenClipboard(owner) != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        if !opened {
            return false;
        }

        let ok = 'copy: {
            if EmptyClipboard() == 0 {
                break 'copy false;
            }
            let memory = GlobalAlloc(GMEM_MOVEABLE, wide.len() * size_of::<u16>());
            if memory.is_null() {
                break 'copy false;
            }
            let target = GlobalLock(memory).cast::<u16>();
            if target.is_null() {
                GlobalFree(memory);
                break 'copy false;
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), target, wide.len());
            GlobalUnlock(memory);
            // On success the system owns the memory; otherwise it is still ours to free.
            if SetClipboardData(CF_UNICODETEXT, memory).is_null() {
                GlobalFree(memory);
                break 'copy false;
            }
            true
        };
        CloseClipboard();
        ok
    }
}
