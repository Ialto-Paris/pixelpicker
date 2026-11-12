// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Ialto

//! Puts into the Windows executable, through the MSVC linker (no resource compiler needed):
//! - the application manifest (Per-Monitor V2 DPI awareness, Common Controls v6);
//! - the icon, drawn by the same code as the window icons (`src/icon_art.rs`), so Explorer,
//!   the taskbar pin and the windows show the same picture;
//! - the version details shown in the file's properties, from Cargo.toml.

use std::path::{Path, PathBuf};

#[path = "src/icon_art.rs"]
mod icon_art;

const PRODUCT: &str = "Pixel Picker";
const INTERNAL_NAME: &str = "PixelPicker";
const EXECUTABLE: &str = "PixelPicker.exe";
const COMPANY: &str = "Ialto";
const COPYRIGHT: &str = "Copyright 2026 Ialto. Apache License 2.0.";

/// Icon sizes in the executable; Windows picks the best one for each view and DPI.
const ICON_SIZES: [usize; 8] = [16, 20, 24, 32, 40, 48, 64, 256];

// Resource types and identifiers, and the memory flags the resource compiler writes.
const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
const RT_VERSION: u16 = 16;
const ICON_GROUP_ID: u16 = 1;
const VERSION_ID: u16 = 1;
const FLAGS_ICON: u16 = 0x1010;
const FLAGS_GROUP_ICON: u16 = 0x1030;
const FLAGS_VERSION: u16 = 0x0030;
const LANG_EN_US: u16 = 0x0409;
/// UTF-16, the code page of the version strings.
const CODE_PAGE_UNICODE: u16 = 1200;

fn main() {
    let root = PathBuf::from(env("CARGO_MANIFEST_DIR"));
    let manifest_template = root.join("pixel-picker.manifest");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/icon_art.rs");
    println!("cargo:rerun-if-changed={}", manifest_template.display());

    if env("CARGO_CFG_TARGET_OS") != "windows" {
        return;
    }
    if env("CARGO_CFG_TARGET_ENV") != "msvc" {
        // Without the manifest and the icon, the program would look broken: stop and say why.
        panic!(
            "Pixel Picker is built with the MSVC toolchain (x86_64-pc-windows-msvc): its linker \
             embeds the manifest and the icon"
        );
    }

    let version = Version::from_cargo();
    let out = PathBuf::from(env("OUT_DIR"));
    let manifest = out.join("pixel-picker.manifest");
    let resources = out.join("pixel-picker.res");
    write(
        &manifest,
        read(&manifest_template)
            .replace("@VERSION@", &version.quad())
            .as_bytes(),
    );
    write(&resources, &resource_file(&version));

    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
        manifest.display()
    );
    // The manifest already carries its own trustInfo section.
    println!("cargo:rustc-link-arg-bins=/MANIFESTUAC:NO");
    // The linker accepts compiled resource files as input.
    println!("cargo:rustc-link-arg-bins={}", resources.display());
}

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("can't read {}: {e}", path.display()))
}

fn write(path: &Path, data: &[u8]) {
    std::fs::write(path, data).unwrap_or_else(|e| panic!("can't write {}: {e}", path.display()));
}

/// The package version from Cargo.toml.
struct Version {
    major: u16,
    minor: u16,
    patch: u16,
    text: String,
}

impl Version {
    fn from_cargo() -> Self {
        let part = |name: &str| env(name).parse().expect("version numbers fit in 16 bits");
        Self {
            major: part("CARGO_PKG_VERSION_MAJOR"),
            minor: part("CARGO_PKG_VERSION_MINOR"),
            patch: part("CARGO_PKG_VERSION_PATCH"),
            text: env("CARGO_PKG_VERSION"),
        }
    }

    /// Four-part form for the manifest: "1.2.3.0".
    fn quad(&self) -> String {
        format!("{}.{}.{}.0", self.major, self.minor, self.patch)
    }
}

/// A 32-bit `.res` file: the icon group (ID 1), one image per `ICON_SIZES` entry, and the
/// version details.
fn resource_file(version: &Version) -> Vec<u8> {
    let mut file = Vec::new();
    // A 32-bit resource file starts with an empty entry.
    push_resource(&mut file, 0, 0, 0, 0, &[]);

    // GRPICONDIR: reserved, type (1 = icon), image count; then one entry per image.
    let mut group = Vec::new();
    for v in [0, 1, ICON_SIZES.len() as u16] {
        group.extend(v.to_le_bytes());
    }
    for (index, &size) in ICON_SIZES.iter().enumerate() {
        let id = index as u16 + 1;
        let pixels = icon_art::draw(size);
        // PNG keeps the 256 px image small; smaller sizes use the classic DIB format.
        let image = if size >= 256 {
            png(size, &pixels)
        } else {
            dib(size, &pixels)
        };
        push_resource(&mut file, RT_ICON, id, FLAGS_ICON, LANG_EN_US, &image);

        // GRPICONDIRENTRY: width, height (0 means 256), colors, reserved, planes,
        // bits per pixel, image size, image resource ID.
        let side = if size >= 256 { 0 } else { size as u8 };
        group.extend([side, side, 0, 0]);
        group.extend(1u16.to_le_bytes());
        group.extend(32u16.to_le_bytes());
        group.extend((image.len() as u32).to_le_bytes());
        group.extend(id.to_le_bytes());
    }
    push_resource(
        &mut file,
        RT_GROUP_ICON,
        ICON_GROUP_ID,
        FLAGS_GROUP_ICON,
        LANG_EN_US,
        &group,
    );

    let details = version_info(version);
    push_resource(
        &mut file,
        RT_VERSION,
        VERSION_ID,
        FLAGS_VERSION,
        LANG_EN_US,
        &details,
    );
    file
}

/// Appends one resource with a numeric type and name, padded to a 4-byte boundary.
fn push_resource(file: &mut Vec<u8>, kind: u16, name: u16, flags: u16, language: u16, data: &[u8]) {
    const HEADER_SIZE: u32 = 32;
    file.extend((data.len() as u32).to_le_bytes());
    file.extend(HEADER_SIZE.to_le_bytes());
    for v in [0xFFFF, kind, 0xFFFF, name] {
        file.extend(v.to_le_bytes());
    }
    file.extend(0u32.to_le_bytes()); // data version
    file.extend(flags.to_le_bytes());
    file.extend(language.to_le_bytes());
    file.extend(0u32.to_le_bytes()); // version
    file.extend(0u32.to_le_bytes()); // characteristics
    file.extend(data);
    align4(file);
}

fn align4(data: &mut Vec<u8>) {
    data.resize(data.len().next_multiple_of(4), 0);
}

/// Icon image in DIB format: header with doubled height, 32-bit BGRA rows bottom-up,
/// then the 1-bit transparency mask (1 = transparent), also bottom-up.
fn dib(size: usize, argb: &[u32]) -> Vec<u8> {
    let mask_stride = size.div_ceil(32) * 4;
    let image_size = size * size * 4 + mask_stride * size;
    let mut out = Vec::with_capacity(40 + image_size);
    out.extend(40u32.to_le_bytes()); // biSize
    out.extend((size as i32).to_le_bytes()); // biWidth
    out.extend((2 * size as i32).to_le_bytes()); // biHeight: color rows + mask rows
    out.extend(1u16.to_le_bytes()); // biPlanes
    out.extend(32u16.to_le_bytes()); // biBitCount
    out.extend(0u32.to_le_bytes()); // biCompression: BI_RGB
    out.extend((image_size as u32).to_le_bytes()); // biSizeImage
    out.extend([0u8; 16]); // resolution, palette

    for row in argb.chunks(size).rev() {
        for &p in row {
            out.extend(p.to_le_bytes()); // B, G, R, A
        }
    }
    for row in argb.chunks(size).rev() {
        let mut mask = vec![0u8; mask_stride];
        for (x, &p) in row.iter().enumerate() {
            if p >> 24 == 0 {
                mask[x / 8] |= 0x80 >> (x % 8);
            }
        }
        out.extend(mask);
    }
    out
}

/// Icon image as a PNG file (RGBA, 8 bits per channel).
fn png(size: usize, argb: &[u32]) -> Vec<u8> {
    let mut scanlines = Vec::with_capacity(size * (size * 4 + 1));
    for row in argb.chunks(size) {
        scanlines.push(0); // filter type: none
        for &p in row {
            let [b, g, r, a] = p.to_le_bytes();
            scanlines.extend([r, g, b, a]);
        }
    }

    let mut header = Vec::with_capacity(13);
    header.extend((size as u32).to_be_bytes());
    header.extend((size as u32).to_be_bytes());
    header.extend([8, 6, 0, 0, 0]); // 8 bits, RGBA, deflate, no filter method, no interlace

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    png_chunk(&mut out, b"IHDR", &header);
    png_chunk(
        &mut out,
        b"IDAT",
        &miniz_oxide::deflate::compress_to_vec_zlib(&scanlines, 9),
    );
    png_chunk(&mut out, b"IEND", &[]);
    out
}

fn png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
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

/// VS_VERSIONINFO: the fixed file information, then the strings Explorer shows in the
/// file's properties (Details tab) and Task Manager uses as the program's name.
fn version_info(version: &Version) -> Vec<u8> {
    let ms = u32::from(version.major) << 16 | u32::from(version.minor);
    let ls = u32::from(version.patch) << 16;
    // VS_FIXEDFILEINFO.
    let words: [u32; 13] = [
        0xFEEF_04BD, // signature
        0x0001_0000, // structure version
        ms,          // file version, high
        ls,          // file version, low
        ms,          // product version, high
        ls,          // product version, low
        0x3F,        // valid flags
        0,           // flags
        0x0004_0004, // VOS_NT_WINDOWS32
        1,           // VFT_APP
        0,           // subtype
        0,           // date, high
        0,           // date, low
    ];
    let fixed: Vec<u8> = words.iter().flat_map(|v| v.to_le_bytes()).collect();

    let strings: Vec<Vec<u8>> = [
        ("CompanyName", COMPANY),
        ("FileDescription", PRODUCT),
        ("FileVersion", &version.text),
        ("InternalName", INTERNAL_NAME),
        ("LegalCopyright", COPYRIGHT),
        ("OriginalFilename", EXECUTABLE),
        ("ProductName", PRODUCT),
        ("ProductVersion", &version.text),
    ]
    .iter()
    .map(|(key, value)| text_node(key, value))
    .collect();
    let language = format!("{LANG_EN_US:04X}{CODE_PAGE_UNICODE:04X}");
    let table = version_node(&language, TEXT, &[], 0, &strings);
    let string_file_info = version_node("StringFileInfo", TEXT, &[], 0, &[table]);

    let translation = [LANG_EN_US.to_le_bytes(), CODE_PAGE_UNICODE.to_le_bytes()].concat();
    let var = version_node("Translation", BINARY, &translation, 4, &[]);
    let var_file_info = version_node("VarFileInfo", TEXT, &[], 0, &[var]);

    let fixed_length = fixed.len() as u16;
    version_node(
        "VS_VERSION_INFO",
        BINARY,
        &fixed,
        fixed_length,
        &[string_file_info, var_file_info],
    )
}

const BINARY: u16 = 0;
const TEXT: u16 = 1;

/// A version string: its value is NUL-terminated UTF-16, its length counted in characters.
fn text_node(key: &str, value: &str) -> Vec<u8> {
    let text = utf16z(value);
    version_node(key, TEXT, &text, (text.len() / 2) as u16, &[])
}

/// One block of a VS_VERSIONINFO tree: total length, value length, type, NUL-terminated
/// UTF-16 key, value, then the children, each starting on a 4-byte boundary.
fn version_node(
    key: &str,
    kind: u16,
    value: &[u8],
    value_length: u16,
    children: &[Vec<u8>],
) -> Vec<u8> {
    let mut out = vec![0, 0]; // total length, filled in at the end
    out.extend(value_length.to_le_bytes());
    out.extend(kind.to_le_bytes());
    out.extend(utf16z(key));
    align4(&mut out);
    out.extend(value);
    for child in children {
        align4(&mut out);
        out.extend(child);
    }
    let length = u16::try_from(out.len()).expect("version details fit in 64 KB");
    out[..2].copy_from_slice(&length.to_le_bytes());
    out
}

fn utf16z(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect()
}
