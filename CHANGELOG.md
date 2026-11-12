# Changelog

Every release of Pixel Picker, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- A Microsoft Store package (MSIX), made by every Build and Release run as the `store-package`
  artifact, with logos drawn from the icon's artwork.
- A privacy statement, `PRIVACY.md`: Pixel Picker collects no data and sends nothing anywhere.

## [1.0.1] - 2026-11-12

### Added

- The foot of the window gives the copyright, a link to ialto.com and a link to the Apache
  License 2.0; both open in the browser.

### Changed

- The GitHub Actions run on Node.js 24: `actions/checkout` v7, `actions/upload-artifact` v7,
  `actions/download-artifact` v8.

## [1.0.0] - 2026-11-12

First release.

### Added

- Picking from a frozen screenshot of every monitor, so the value is exactly the one
  displayed and the click never reaches the application underneath.
- Magnifier following the cursor: zoom ×3 to ×30 (mouse wheel, `+`, `−`), pixel grid (`G`),
  target pixel ringed in black and white, and a box with its color, HEX and RGB values,
  position and zoom.
- Pixel-precise moves with the arrow keys (`Shift`: 10 pixels); pick with a click, `Enter` or
  `Space`; cancel with `Esc` or a right click.
- HEX, RGB and HSL values with their Copy buttons; the HEX code is copied after each pick.
- Global shortcut Ctrl+Alt+P, and a history of the last 12 colors.
- Per-Monitor V2 DPI awareness and multi-monitor support.
- Single executable with its icon and version details; release workflow on `vX.Y.Z` tags.

[Unreleased]: https://github.com/ialto-paris/pixelpicker/compare/v1.0.1...HEAD
[1.0.1]: https://github.com/ialto-paris/pixelpicker/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/ialto-paris/pixelpicker/releases/tag/v1.0.0
