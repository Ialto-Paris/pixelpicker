// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Ialto

//! Pixel picking mode: a borderless topmost window covering every monitor shows a frozen
//! screenshot, with the magnifier panel following the cursor.
//!
//! Freezing the screen keeps the reading exact (the magnifier never captures itself) and
//! guarantees that the picking click never reaches the application underneath.

use std::cell::{Cell, RefCell};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, DeleteObject, EndPaint, ExcludeClipRect, FW_NORMAL, FW_SEMIBOLD,
    GetMonitorInfoW, HDC, HFONT, InvalidateRect, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MonitorFromPoint, PAINTSTRUCT, RestoreDC, SRCCOPY, SaveDC, SelectObject, SetBkMode,
    SetTextColor, TRANSPARENT,
};
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_DOWN, VK_ESCAPE, VK_LEFT, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_SPACE, VK_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetCursorPos, IDC_CROSS, LoadCursorW,
    PostMessageW, RegisterClassExW, SW_SHOW, SetCursorPos, SetForegroundWindow, ShowWindow,
    WA_INACTIVE, WM_ACTIVATE, WM_APP, WM_CHAR, WM_DPICHANGED, WM_ERASEBKGND, WM_KEYDOWN,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCDESTROY, WM_PAINT,
    WM_RBUTTONDOWN, WM_RBUTTONUP, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::capture::{Dib, Screenshot};
use crate::color::Rgb;
use crate::magnifier::{self, Area, Layout, Source, Target, ZOOM_LEVELS};
use crate::win;

/// Posted to the owner window when picking ends; read the result with [`take_outcome`].
pub const WM_PICK_DONE: u32 = WM_APP + 1;

const CLASS_NAME: &str = "PixelPicker.Overlay";
const WHEEL_DELTA: i32 = 120;

const PANEL_BORDER: u32 = 0x5A5A5A;
const INFO_BACKGROUND: u32 = 0x1E1E1E;
const SWATCH_BORDER: u32 = 0x8A8A8A;

#[derive(Clone, Copy, Debug)]
pub enum Outcome {
    Picked { color: Rgb, x: i32, y: i32 },
    Cancelled,
}

struct Fonts {
    dpi: u32,
    big: HFONT,
    small: HFONT,
}

impl Fonts {
    fn new(dpi: u32) -> Self {
        Self {
            dpi,
            big: win::create_font("Segoe UI", win::scale(16, dpi), FW_SEMIBOLD as i32),
            small: win::create_font("Segoe UI", win::scale(12, dpi), FW_NORMAL as i32),
        }
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.big);
            DeleteObject(self.small);
        }
    }
}

/// Where the panel is drawn, in client coordinates of the overlay, and its geometry.
struct View {
    rect: RECT,
    layout: Layout,
}

struct State {
    shot: Screenshot,
    /// Target pixel, in screen coordinates (physical pixels).
    cursor: POINT,
    zoom: usize,
    grid: bool,
    /// A left click started on the overlay; the pixel is picked when it is released.
    pressed: bool,
    wheel: i32,
    view: View,
    fonts: Option<Fonts>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    static OUTCOME: Cell<Option<Outcome>> = const { Cell::new(None) };
    static OWNER: Cell<HWND> = const { Cell::new(null_mut()) };
    static CLOSING: Cell<bool> = const { Cell::new(false) };
    // Remembered from one capture to the next.
    static ZOOM: Cell<usize> = const { Cell::new(magnifier::DEFAULT_ZOOM) };
    static GRID: Cell<bool> = const { Cell::new(true) };
}

/// Runs `f` on the picking state, if any. Never panics on re-entrant calls.
fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok()?.as_mut().map(f))
}

pub fn register_class(hinstance: HINSTANCE) -> bool {
    let name = win::wide(CLASS_NAME);
    unsafe {
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_CROSS),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: name.as_ptr(),
            hIconSm: null_mut(),
        };
        RegisterClassExW(&class) != 0
    }
}

/// Freezes the screen and starts picking. `owner` receives [`WM_PICK_DONE`] at the end.
pub fn begin(hinstance: HINSTANCE, owner: HWND) -> Result<(), &'static str> {
    let busy = STATE.with(|s| s.try_borrow().map(|s| s.is_some()).unwrap_or(true));
    if busy {
        return Ok(());
    }
    let shot = Screenshot::capture().ok_or("Screen capture failed.")?;
    let mut cursor = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut cursor) };
    let zoom = ZOOM.get();
    let view = compute_view(&shot, cursor, zoom);
    let (left, top, width, height) = (shot.left, shot.top, shot.width(), shot.height());

    STATE.with(|s| {
        *s.borrow_mut() = Some(State {
            shot,
            cursor,
            zoom,
            grid: GRID.get(),
            pressed: false,
            wheel: 0,
            view,
            fonts: None,
        })
    });
    OWNER.set(owner);
    OUTCOME.set(None);
    CLOSING.set(false);

    let class = win::wide(CLASS_NAME);
    let title = win::wide("Pixel Picker");
    unsafe {
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            left,
            top,
            width,
            height,
            null_mut(),
            null_mut(),
            hinstance,
            null(),
        );
        if hwnd.is_null() {
            let state = STATE.with(|s| s.borrow_mut().take());
            drop(state);
            return Err("Could not open the magnifier.");
        }
        // No fade-in: the overlay must look exactly like the screen it covers.
        win::set_transitions_disabled(hwnd, true);
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
    }
    Ok(())
}

/// Result of the last picking session.
pub fn take_outcome() -> Option<Outcome> {
    OUTCOME.take()
}

fn compute_view(shot: &Screenshot, cursor: POINT, zoom: usize) -> View {
    unsafe {
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        let bounds = if GetMonitorInfoW(monitor, &mut info) != 0 {
            info.rcMonitor
        } else {
            win::rect(shot.left, shot.top, shot.width(), shot.height())
        };
        let (mut dpi, mut dpi_y) = (96u32, 96u32);
        if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi_y) < 0 {
            dpi = 96;
        }

        let layout = Layout::new(zoom, dpi);
        let (x, y) = magnifier::place(
            (cursor.x, cursor.y),
            (layout.width, layout.height),
            (bounds.left, bounds.top, bounds.right, bounds.bottom),
            win::scale(22, dpi),
        );
        View {
            rect: win::rect(x - shot.left, y - shot.top, layout.width, layout.height),
            layout,
        }
    }
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_MOUSEMOVE => {
            let (x, y) = win::point_from_lparam(lparam);
            move_to(hwnd, x, y);
            0
        }
        WM_LBUTTONDOWN => {
            with_state(|s| s.pressed = true);
            0
        }
        WM_LBUTTONUP => {
            let (x, y) = win::point_from_lparam(lparam);
            move_to(hwnd, x, y);
            if with_state(|s| std::mem::take(&mut s.pressed)).unwrap_or(false) {
                pick(hwnd);
            }
            0
        }
        // Handled (and swallowed) so that no context menu appears.
        WM_RBUTTONDOWN => 0,
        WM_RBUTTONUP => {
            finish(hwnd, Outcome::Cancelled);
            0
        }
        WM_MOUSEWHEEL => {
            wheel(hwnd, win::hiword(wparam) as i16 as i32);
            0
        }
        WM_KEYDOWN => {
            key_down(hwnd, wparam as u16);
            0
        }
        WM_CHAR => {
            match char::from_u32(wparam as u32) {
                Some('+') => zoom(hwnd, 1),
                Some('-') => zoom(hwnd, -1),
                Some('g' | 'G') => {
                    with_state(|s| s.grid = !s.grid);
                    refresh(hwnd);
                }
                _ => {}
            }
            0
        }
        WM_ACTIVATE => {
            // Switching away (Alt+Tab, Windows key...) ends picking.
            if u32::from(win::loword(wparam)) == WA_INACTIVE {
                finish(hwnd, Outcome::Cancelled);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        // The overlay spans every monitor at physical resolution: never resize it.
        WM_DPICHANGED => 0,
        WM_NCDESTROY => {
            let state = STATE.with(|s| s.borrow_mut().take());
            drop(state);
            CLOSING.set(false);
            unsafe {
                PostMessageW(OWNER.get(), WM_PICK_DONE, 0, 0);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Moves the target to client coordinates (`x`, `y`).
fn move_to(hwnd: HWND, x: i32, y: i32) {
    let dirty = with_state(|s| {
        let cursor = POINT {
            x: x + s.shot.left,
            y: y + s.shot.top,
        };
        if cursor.x == s.cursor.x && cursor.y == s.cursor.y {
            return None;
        }
        s.cursor = cursor;
        Some(s.update_view())
    })
    .flatten();
    invalidate(hwnd, dirty);
}

/// Redraws the panel after a zoom or grid change.
fn refresh(hwnd: HWND) {
    invalidate(hwnd, with_state(|s| s.update_view()));
}

fn invalidate(hwnd: HWND, dirty: Option<(RECT, RECT)>) {
    if let Some((old, new)) = dirty {
        unsafe {
            InvalidateRect(hwnd, &old, 0);
            InvalidateRect(hwnd, &new, 0);
        }
    }
}

fn zoom(hwnd: HWND, steps: i32) {
    let changed = with_state(|s| {
        let z = (s.zoom as i32 + steps).clamp(0, ZOOM_LEVELS.len() as i32 - 1) as usize;
        std::mem::replace(&mut s.zoom, z) != z
    });
    if changed == Some(true) {
        refresh(hwnd);
    }
}

fn wheel(hwnd: HWND, delta: i32) {
    // Accumulate, as touchpads send fractions of a notch.
    let steps = with_state(|s| {
        s.wheel += delta;
        let steps = s.wheel / WHEEL_DELTA;
        s.wheel -= steps * WHEEL_DELTA;
        steps
    });
    if let Some(steps @ (..=-1 | 1..)) = steps {
        zoom(hwnd, steps);
    }
}

fn key_down(hwnd: HWND, vk: u16) {
    let fast = unsafe { GetKeyState(i32::from(VK_SHIFT)) } < 0;
    let step = if fast { 10 } else { 1 };
    match vk {
        VK_ESCAPE => finish(hwnd, Outcome::Cancelled),
        VK_RETURN | VK_SPACE => pick(hwnd),
        VK_LEFT => nudge(-step, 0),
        VK_RIGHT => nudge(step, 0),
        VK_UP => nudge(0, -step),
        VK_DOWN => nudge(0, step),
        _ => {}
    }
}

/// Moves the mouse cursor by a few pixels; the resulting WM_MOUSEMOVE updates the target.
fn nudge(dx: i32, dy: i32) {
    unsafe {
        let mut p = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut p) != 0 {
            SetCursorPos(p.x + dx, p.y + dy);
        }
    }
}

fn pick(hwnd: HWND) {
    let outcome = with_state(|s| {
        let (x, y) = (s.cursor.x, s.cursor.y);
        s.shot
            .pixel_at(x, y)
            .map(|color| Outcome::Picked { color, x, y })
    })
    .flatten()
    .unwrap_or(Outcome::Cancelled);
    finish(hwnd, outcome);
}

fn finish(hwnd: HWND, outcome: Outcome) {
    // DestroyWindow sends WM_ACTIVATE, which calls back here: only the first call counts.
    if CLOSING.replace(true) {
        return;
    }
    OUTCOME.set(Some(outcome));
    with_state(|s| {
        ZOOM.set(s.zoom);
        GRID.set(s.grid);
    });
    unsafe {
        DestroyWindow(hwnd);
    }
}

fn paint(hwnd: HWND) {
    unsafe {
        let mut ps: PAINTSTRUCT = std::mem::zeroed();
        let hdc = BeginPaint(hwnd, &mut ps);
        if !hdc.is_null() {
            with_state(|s| s.paint(hdc, &ps.rcPaint));
        }
        EndPaint(hwnd, &ps);
    }
}

impl State {
    /// Recomputes the panel position; returns the old and new panel rectangles.
    fn update_view(&mut self) -> (RECT, RECT) {
        let old = self.view.rect;
        self.view = compute_view(&self.shot, self.cursor, self.zoom);
        (old, self.view.rect)
    }

    fn paint(&mut self, hdc: HDC, dirty: &RECT) {
        let panel = self.view.rect;
        unsafe {
            // Frozen screen everywhere except under the panel, so nothing is drawn twice
            // (no flicker).
            SaveDC(hdc);
            ExcludeClipRect(hdc, panel.left, panel.top, panel.right, panel.bottom);
            BitBlt(
                hdc,
                dirty.left,
                dirty.top,
                dirty.right - dirty.left,
                dirty.bottom - dirty.top,
                self.shot.dc(),
                dirty.left,
                dirty.top,
                SRCCOPY,
            );
            RestoreDC(hdc, -1);

            if win::intersects(&panel, dirty)
                && let Some(image) = self.render_panel()
            {
                BitBlt(
                    hdc,
                    panel.left,
                    panel.top,
                    image.width(),
                    image.height(),
                    image.dc(),
                    0,
                    0,
                    SRCCOPY,
                );
            }
        }
    }

    fn render_panel(&mut self) -> Option<Dib> {
        let layout = self.view.layout;
        let dpi = layout.dpi;
        let s = |v: i32| win::scale(v, dpi);
        let mut image = Dib::new(layout.width, layout.height)?;
        let color = self
            .shot
            .pixel_at(self.cursor.x, self.cursor.y)
            .unwrap_or(Rgb::new(0, 0, 0));

        let info = layout.info_area();
        let pad = s(8);
        let swatch = Area {
            x: info.x + pad,
            y: info.y + pad,
            w: s(36),
            h: info.h - 2 * pad,
        };
        {
            let whole = Area {
                x: 0,
                y: 0,
                w: layout.width,
                h: layout.height,
            };
            let mut dst = Target {
                pixels: image.pixels_mut(),
                stride: layout.width,
            };
            dst.fill(whole, whole, PANEL_BORDER);
            let src = Source {
                pixels: self.shot.pixels(),
                width: self.shot.width(),
                height: self.shot.height(),
            };
            let center = (
                self.cursor.x - self.shot.left,
                self.cursor.y - self.shot.top,
            );
            magnifier::render(
                &src,
                center,
                &layout,
                self.grid,
                &mut dst,
                layout.magnifier_area(),
            );
            dst.fill(info, whole, INFO_BACKGROUND);
            dst.fill(swatch, whole, SWATCH_BORDER);
            let inner = Area {
                x: swatch.x + 1,
                y: swatch.y + 1,
                w: swatch.w - 2,
                h: swatch.h - 2,
            };
            dst.fill(inner, whole, color.to_xrgb());
        }

        let fonts = self.fonts(dpi);
        let (big, small) = (fonts.big, fonts.small);
        let dc = image.dc();
        let x = swatch.x + swatch.w + s(9);
        unsafe {
            SetBkMode(dc, TRANSPARENT as _);
            SelectObject(dc, big);
            SetTextColor(dc, Rgb::new(240, 240, 240).to_colorref());
            win::text_out(dc, x, info.y + s(4), &color.hex());
            SelectObject(dc, small);
            SetTextColor(dc, Rgb::new(200, 200, 200).to_colorref());
            win::text_out(dc, x, info.y + s(25), &color.rgb());
            SetTextColor(dc, Rgb::new(140, 140, 140).to_colorref());
            let position = format!("{}, {}  ·  ×{}", self.cursor.x, self.cursor.y, layout.zoom);
            win::text_out(dc, x, info.y + s(41), &position);
        }
        Some(image)
    }

    fn fonts(&mut self, dpi: u32) -> &Fonts {
        if self.fonts.as_ref().map(|f| f.dpi) != Some(dpi) {
            self.fonts = Some(Fonts::new(dpi));
        }
        self.fonts.as_ref().expect("fonts were just created")
    }
}
