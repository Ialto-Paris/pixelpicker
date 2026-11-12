// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Ialto

//! Writes the logos of Pixel Picker's MSIX package (Microsoft Store) into the folder given as
//! argument, drawn by the same code as the program's icon (`src/icon_art.rs`):
//!
//! ```sh
//! cargo run --example msix_assets -- <folder>
//! ```
//!
//! `.github/scripts/package-msix.ps1` runs it, then MakePri indexes the scale- and size-
//! qualified files, so Windows picks the sharpest one for each place and display scaling.

use std::path::{Path, PathBuf};

#[path = "../src/icon_art.rs"]
mod icon_art;
#[path = "../src/png.rs"]
mod png;

/// Display scales, in percent, that Windows asks the logos for.
const SCALES: [usize; 5] = [100, 125, 150, 200, 400];
/// Exact sizes of the app list, taskbar and title bar icon (Square44x44Logo).
const TARGET_SIZES: [usize; 7] = [16, 20, 24, 32, 40, 48, 256];

/// A logo of the manifest: its name, its side at 100 % and the share of that side the
/// artwork covers (tiles keep a margin around it).
struct Logo {
    name: &'static str,
    side: usize,
    share: f64,
}

const LOGOS: [Logo; 3] = [
    Logo {
        name: "Square44x44Logo",
        side: 44,
        share: 1.0,
    },
    Logo {
        name: "Square150x150Logo",
        side: 150,
        share: 0.6,
    },
    Logo {
        name: "StoreLogo",
        side: 50,
        share: 1.0,
    },
];

fn main() {
    let folder = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .expect("usage: msix_assets <folder>"),
    );
    std::fs::create_dir_all(&folder).expect("can't create the logos' folder");

    for logo in &LOGOS {
        for scale in SCALES {
            let side = (logo.side * scale + 50) / 100;
            let name = format!("{}.scale-{scale}.png", logo.name);
            write(&folder, &name, side, logo.share);
        }
    }
    // Without a plate behind it, the icon keeps its transparent margins on the taskbar.
    for size in TARGET_SIZES {
        write(
            &folder,
            &format!("Square44x44Logo.targetsize-{size}.png"),
            size,
            1.0,
        );
        let unplated = format!("Square44x44Logo.targetsize-{size}_altform-unplated.png");
        write(&folder, &unplated, size, 1.0);
    }
}

/// Writes a `side` × `side` PNG with the artwork centered, covering `share` of the side.
fn write(folder: &Path, name: &str, side: usize, share: f64) {
    let art_side = ((side as f64 * share).round() as usize).clamp(1, side);
    let art = icon_art::draw(art_side);
    let offset = (side - art_side) / 2;
    let mut pixels = vec![0u32; side * side];
    for (y, row) in art.chunks(art_side).enumerate() {
        let start = (y + offset) * side + offset;
        pixels[start..start + art_side].copy_from_slice(row);
    }
    let path = folder.join(name);
    std::fs::write(&path, png::encode(side, side, &pixels))
        .unwrap_or_else(|e| panic!("can't write {}: {e}", path.display()));
}
