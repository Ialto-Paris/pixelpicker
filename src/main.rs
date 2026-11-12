// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Ialto

//! Pixel Picker: reads the RGB value of any pixel on screen, with a magnifier for precise
//! targeting. Windows only, plain Win32 (no GUI framework).

#![windows_subsystem = "windows"]

mod app;
mod capture;
mod clipboard;
mod color;
mod icon;
mod icon_art;
mod magnifier;
mod picker;
mod win;

// The PNG writer belongs to build.rs and examples/msix_assets.rs; it is compiled here only
// to run its tests.
#[cfg(test)]
mod png;

use windows_sys::Win32::UI::Controls::{
    ICC_LINK_CLASS, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx,
};
use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};

fn main() {
    unsafe {
        // Physical pixels everywhere. The embedded manifest already requests this; the call
        // is a fallback for builds without it (it fails harmlessly otherwise).
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let controls = INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            // Buttons, fields and labels, and the footer's links (SysLink).
            dwICC: ICC_STANDARD_CLASSES | ICC_LINK_CLASS,
        };
        InitCommonControlsEx(&controls);
    }
    std::process::exit(app::run());
}
