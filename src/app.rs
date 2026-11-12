// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Ialto

//! Main window: current color in HEX / RGB / HSL, copy buttons, capture button, history.

use std::cell::{Cell, RefCell};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    ERROR_ALREADY_EXISTS, GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, COLOR_GRAYTEXT, COLOR_WINDOW, COLOR_WINDOWTEXT, CreateSolidBrush, DeleteObject,
    EndPaint, FW_NORMAL, FillRect, FrameRect, GetSysColor, GetSysColorBrush, HDC, HFONT,
    InvalidateRect, PAINTSTRUCT, ScreenToClient, SetBkColor, SetTextColor, UpdateWindow,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemServices::{
    SS_CENTERIMAGE, SS_ENDELLIPSIS, SS_LEFT, SS_LEFTNOWORDWRAP, SS_NOPREFIX,
};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey, SetFocus,
    UnregisterHotKey,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW,
    DispatchMessageW, ES_AUTOHSCROLL, ES_READONLY, FindWindowW, GetCursorPos, GetDlgCtrlID,
    GetDlgItem, GetMessageW, GetSystemMetrics, HMENU, HTCLIENT, IDC_ARROW, IDC_HAND, IsChild,
    IsDialogMessageW, IsIconic, IsWindowVisible, KillTimer, LoadCursorW, MB_ICONERROR, MB_OK, MSG,
    MessageBoxW, MoveWindow, PostQuitMessage, RegisterClassExW, SM_CXICON, SM_CXSMICON, SW_HIDE,
    SW_RESTORE, SW_SHOW, SW_SHOWDEFAULT, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SendMessageW,
    SetCursor, SetForegroundWindow, SetTimer, SetWindowPos, SetWindowTextW, ShowWindow,
    TranslateMessage, WA_INACTIVE, WINDOW_STYLE, WM_ACTIVATE, WM_COMMAND, WM_CTLCOLORSTATIC,
    WM_DESTROY, WM_DPICHANGED, WM_HOTKEY, WM_LBUTTONDOWN, WM_PAINT, WM_SETCURSOR, WM_SETFONT,
    WM_TIMER, WNDCLASSEXW, WS_CAPTION, WS_CHILD, WS_CLIPCHILDREN, WS_EX_CLIENTEDGE, WS_MINIMIZEBOX,
    WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};

use crate::color::Rgb;
use crate::picker::{self, Outcome};
use crate::{clipboard, icon, win};

const CLASS_NAME: &str = "PixelPicker.Main";
const APP_NAME: &str = "Pixel Picker";
/// The version in Cargo.toml, shown in the window title.
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAIN_STYLE: WINDOW_STYLE =
    WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN;

/// Global shortcut that starts picking: Ctrl+Alt+P.
const HOTKEY_ID: i32 = 1;
const HOTKEY_MODIFIERS: u32 = MOD_CONTROL | MOD_ALT | MOD_NOREPEAT;
const HOTKEY_KEY: u8 = b'P';
const HOTKEY_LABEL: &str = "Ctrl+Alt+P";
const TIMER_CAPTURE: usize = 1;
/// Time left to the compositor to remove the main window from the screen before capture.
const HIDE_DELAY_MS: u32 = 150;
const HISTORY_LEN: usize = 12;

// Control identifiers. The capture button is IDOK (1), so Enter starts a capture.
const ID_CAPTURE: u16 = 1;
const ID_POSITION: u16 = 130;
const ID_HISTORY_LABEL: u16 = 131;
const ID_HINT: u16 = 132;
const ID_STATUS: u16 = 133;

// Layout, in 96-DPI units.
const CLIENT_SIZE: (i32, i32) = (412, 300);
const SWATCH: (i32, i32, i32, i32) = (12, 12, 106, 106);
const HISTORY_ORIGIN: (i32, i32) = (12, 198);
const HISTORY_SWATCH: i32 = 26;
const HISTORY_STEP: i32 = 32;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Hex,
    Rgb,
    Hsl,
}

impl Format {
    fn text(self, c: Rgb) -> String {
        match self {
            Self::Hex => c.hex(),
            Self::Rgb => c.rgb(),
            Self::Hsl => c.hsl_css(),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Hex => "HEX",
            Self::Rgb => "RGB",
            Self::Hsl => "HSL",
        }
    }
}

/// One row of the value table: label, read-only field and copy button.
struct Field {
    format: Format,
    label: u16,
    edit: u16,
    copy: u16,
    y: i32,
}

const FIELDS: [Field; 3] = [
    Field {
        format: Format::Hex,
        label: 100,
        edit: 110,
        copy: 120,
        y: 12,
    },
    Field {
        format: Format::Rgb,
        label: 101,
        edit: 111,
        copy: 121,
        y: 42,
    },
    Field {
        format: Format::Hsl,
        label: 102,
        edit: 112,
        copy: 122,
        y: 72,
    },
];

#[derive(Clone, Copy)]
enum Kind {
    Label,
    Value,
    Button,
    DefaultButton,
    Text,
    Hint,
}

/// A child control: identifier, kind, initial text and position (96-DPI units: x, y, w, h).
struct Control {
    id: u16,
    kind: Kind,
    text: String,
    rect: (i32, i32, i32, i32),
}

impl Control {
    fn new(id: u16, kind: Kind, text: impl Into<String>, rect: (i32, i32, i32, i32)) -> Self {
        Self {
            id,
            kind,
            text: text.into(),
            rect,
        }
    }
}

/// Every child control, in tab order.
fn controls() -> Vec<Control> {
    let mut list = Vec::new();
    for f in &FIELDS {
        list.push(Control::new(
            f.label,
            Kind::Label,
            f.format.name(),
            (128, f.y, 36, 24),
        ));
        list.push(Control::new(f.edit, Kind::Value, "", (166, f.y, 142, 24)));
        list.push(Control::new(
            f.copy,
            Kind::Button,
            "Copy",
            (316, f.y - 1, 84, 26),
        ));
    }
    // Non-breaking spaces (U+00A0) keep each "key = action" item of the hint on one line.
    let hint = "Magnifier: click\u{a0}=\u{a0}pick · Esc\u{a0}=\u{a0}cancel · \
                wheel\u{a0}or\u{a0}+/−\u{a0}=\u{a0}zoom · \
                arrows\u{a0}=\u{a0}1\u{a0}pixel\u{a0}(Shift:\u{a0}10) · G\u{a0}=\u{a0}grid";
    list.extend([
        Control::new(ID_POSITION, Kind::Text, "", (128, 100, 272, 18)),
        Control::new(
            ID_CAPTURE,
            Kind::DefaultButton,
            format!("Pick a pixel   ({HOTKEY_LABEL})"),
            (12, 132, 388, 34),
        ),
        Control::new(
            ID_HISTORY_LABEL,
            Kind::Text,
            "History: click a color to recall it",
            (12, 178, 388, 18),
        ),
        Control::new(ID_HINT, Kind::Hint, hint, (12, 234, 388, 34)),
        Control::new(ID_STATUS, Kind::Text, "", (12, 274, 388, 18)),
    ]);
    list
}

#[derive(Clone, Copy)]
struct Sample {
    color: Rgb,
    x: i32,
    y: i32,
}

struct Model {
    hinstance: HINSTANCE,
    dpi: u32,
    font_ui: HFONT,
    font_mono: HFONT,
    current: Option<Sample>,
    /// Most recent first, one entry per color.
    history: Vec<Sample>,
    /// A capture is scheduled or in progress.
    busy: bool,
    /// The window was hidden for the capture and must be shown again afterwards.
    restore: bool,
}

thread_local! {
    static MODEL: RefCell<Option<Model>> = const { RefCell::new(None) };
    /// Child control that had the keyboard focus when the window was deactivated.
    static FOCUS: Cell<HWND> = const { Cell::new(null_mut()) };
}

/// Runs `f` on the model. Never panics on re-entrant calls (Win32 often re-enters the
/// window procedure), it returns `None` instead.
fn with_model<R>(f: impl FnOnce(&mut Model) -> R) -> Option<R> {
    MODEL.with(|m| m.try_borrow_mut().ok()?.as_mut().map(f))
}

pub fn run() -> i32 {
    unsafe {
        let hinstance = GetModuleHandleW(null());
        if !claim_single_instance() {
            return 0;
        }
        if !register_class(hinstance) || !picker::register_class(hinstance) {
            error_box("Could not register the window classes.");
            return 1;
        }
        MODEL.with(|m| {
            *m.borrow_mut() = Some(Model {
                hinstance,
                dpi: 96,
                font_ui: null_mut(),
                font_mono: null_mut(),
                current: None,
                history: Vec::new(),
                busy: false,
                restore: false,
            })
        });
        let Some(hwnd) = create_main_window(hinstance) else {
            error_box("Could not create the main window.");
            return 1;
        };

        let mut msg: MSG = std::mem::zeroed();
        loop {
            let r = GetMessageW(&mut msg, null_mut(), 0, 0);
            if r == 0 || r == -1 {
                break;
            }
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        msg.wParam as i32
    }
}

/// Returns `false` (after bringing the existing window forward) if another instance runs.
fn claim_single_instance() -> bool {
    unsafe {
        let name = win::wide("Local\\PixelPicker.SingleInstance");
        // The mutex is deliberately never closed: it lives as long as the process.
        let mutex = CreateMutexW(null(), 0, name.as_ptr());
        if !mutex.is_null() && GetLastError() == ERROR_ALREADY_EXISTS {
            let class = win::wide(CLASS_NAME);
            let other = FindWindowW(class.as_ptr(), null());
            if !other.is_null() {
                if IsIconic(other) != 0 {
                    ShowWindow(other, SW_RESTORE);
                }
                SetForegroundWindow(other);
            }
            return false;
        }
        true
    }
}

fn error_box(text: &str) {
    let text = win::wide(text);
    let title = win::wide(APP_NAME);
    unsafe {
        MessageBoxW(
            null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn register_class(hinstance: HINSTANCE) -> bool {
    let name = win::wide(CLASS_NAME);
    unsafe {
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: icon::create(GetSystemMetrics(SM_CXICON)),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: GetSysColorBrush(COLOR_WINDOW),
            lpszMenuName: null(),
            lpszClassName: name.as_ptr(),
            hIconSm: icon::create(GetSystemMetrics(SM_CXSMICON)),
        };
        RegisterClassExW(&class) != 0
    }
}

fn create_main_window(hinstance: HINSTANCE) -> Option<HWND> {
    let class = win::wide(CLASS_NAME);
    let title = win::wide(APP_NAME);
    unsafe {
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            MAIN_STYLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            null_mut(),
            null_mut(),
            hinstance,
            null(),
        );
        if hwnd.is_null() {
            return None;
        }

        for c in controls() {
            create_control(hwnd, hinstance, &c);
        }
        let dpi = GetDpiForWindow(hwnd).max(96);
        apply_dpi(hwnd, dpi, None);

        let hotkey = RegisterHotKey(hwnd, HOTKEY_ID, HOTKEY_MODIFIERS, u32::from(HOTKEY_KEY)) != 0;
        if hotkey {
            set_status(
                hwnd,
                &format!("Ready. {HOTKEY_LABEL} starts picking from any application."),
            );
        } else {
            set_text(GetDlgItem(hwnd, i32::from(ID_CAPTURE)), "Pick a pixel");
            set_status(
                hwnd,
                &format!("Shortcut {HOTKEY_LABEL} is already used by another application."),
            );
        }
        update_fields(hwnd);

        ShowWindow(hwnd, SW_SHOWDEFAULT);
        UpdateWindow(hwnd);
        let capture = GetDlgItem(hwnd, i32::from(ID_CAPTURE));
        SetFocus(capture);
        FOCUS.set(capture);
        Some(hwnd)
    }
}

fn create_control(parent: HWND, hinstance: HINSTANCE, c: &Control) {
    let base = WS_CHILD | WS_VISIBLE;
    let (class, style, ex_style) = match c.kind {
        Kind::Label => ("STATIC", base | SS_LEFT | SS_CENTERIMAGE | SS_NOPREFIX, 0),
        Kind::Value => (
            "EDIT",
            base | WS_TABSTOP | ES_READONLY as WINDOW_STYLE | ES_AUTOHSCROLL as WINDOW_STYLE,
            WS_EX_CLIENTEDGE,
        ),
        Kind::Button => (
            "BUTTON",
            base | WS_TABSTOP | BS_PUSHBUTTON as WINDOW_STYLE,
            0,
        ),
        Kind::DefaultButton => (
            "BUTTON",
            base | WS_TABSTOP | BS_DEFPUSHBUTTON as WINDOW_STYLE,
            0,
        ),
        Kind::Text => (
            "STATIC",
            base | SS_LEFTNOWORDWRAP | SS_ENDELLIPSIS | SS_NOPREFIX,
            0,
        ),
        Kind::Hint => ("STATIC", base | SS_LEFT | SS_NOPREFIX, 0),
    };
    let class = win::wide(class);
    let text = win::wide(&c.text);
    unsafe {
        CreateWindowExW(
            ex_style,
            class.as_ptr(),
            text.as_ptr(),
            style,
            0,
            0,
            0,
            0,
            parent,
            usize::from(c.id) as HMENU,
            hinstance,
            null(),
        );
    }
}

/// Creates fonts for `dpi`, lays the controls out and sizes the window. `origin` is the
/// new top-left corner of the window, if it must move.
fn apply_dpi(hwnd: HWND, dpi: u32, origin: Option<(i32, i32)>) {
    let s = |v: i32| win::scale(v, dpi);
    let font_ui = win::create_font("Segoe UI", (9 * dpi as i32 + 36) / 72, FW_NORMAL as i32);
    let font_mono = win::create_font("Consolas", (10 * dpi as i32 + 36) / 72, FW_NORMAL as i32);
    let old = with_model(|m| {
        m.dpi = dpi;
        (
            std::mem::replace(&mut m.font_ui, font_ui),
            std::mem::replace(&mut m.font_mono, font_mono),
        )
    });

    unsafe {
        for c in controls() {
            let child = GetDlgItem(hwnd, i32::from(c.id));
            let (x, y, w, h) = c.rect;
            MoveWindow(child, s(x), s(y), s(w), s(h), 1);
            let font = if matches!(c.kind, Kind::Value) {
                font_mono
            } else {
                font_ui
            };
            SendMessageW(child, WM_SETFONT, font as usize, 1);
        }
        if let Some((a, b)) = old {
            if !a.is_null() {
                DeleteObject(a);
            }
            if !b.is_null() {
                DeleteObject(b);
            }
        }

        let mut frame = RECT {
            left: 0,
            top: 0,
            right: s(CLIENT_SIZE.0),
            bottom: s(CLIENT_SIZE.1),
        };
        AdjustWindowRectExForDpi(&mut frame, MAIN_STYLE, 0, 0, dpi);
        let (w, h) = (frame.right - frame.left, frame.bottom - frame.top);
        match origin {
            Some((x, y)) => {
                SetWindowPos(hwnd, null_mut(), x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE)
            }
            None => SetWindowPos(
                hwnd,
                null_mut(),
                0,
                0,
                w,
                h,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            ),
        };
        InvalidateRect(hwnd, null(), 1);
    }
}

fn set_text(hwnd: HWND, text: &str) {
    let text = win::wide(text);
    unsafe {
        SetWindowTextW(hwnd, text.as_ptr());
    }
}

fn set_status(hwnd: HWND, text: &str) {
    set_text(unsafe { GetDlgItem(hwnd, i32::from(ID_STATUS)) }, text);
}

/// Refreshes every widget showing the current color.
fn update_fields(hwnd: HWND) {
    let current = with_model(|m| m.current).flatten();
    unsafe {
        for f in &FIELDS {
            let text = current.map(|s| f.format.text(s.color)).unwrap_or_default();
            set_text(GetDlgItem(hwnd, i32::from(f.edit)), &text);
            EnableWindow(
                GetDlgItem(hwnd, i32::from(f.copy)),
                current.is_some().into(),
            );
        }
        let position = match current {
            Some(s) => format!("Screen position: {}, {}", s.x, s.y),
            None => "No color picked yet.".to_string(),
        };
        set_text(GetDlgItem(hwnd, i32::from(ID_POSITION)), &position);
        let title = match current {
            Some(s) => format!("{APP_NAME} {APP_VERSION} – {}", s.color.hex()),
            None => format!("{APP_NAME} {APP_VERSION}"),
        };
        set_text(hwnd, &title);
        InvalidateRect(hwnd, null(), 1);
    }
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_COMMAND => {
                on_command(hwnd, win::loword(wparam), win::hiword(wparam));
                0
            }
            WM_HOTKEY => {
                if wparam == HOTKEY_ID as usize {
                    start_capture(hwnd);
                }
                0
            }
            WM_TIMER => {
                if wparam == TIMER_CAPTURE {
                    KillTimer(hwnd, TIMER_CAPTURE);
                    open_picker(hwnd);
                }
                0
            }
            picker::WM_PICK_DONE => {
                finish_capture(hwnd);
                0
            }
            WM_PAINT => {
                paint(hwnd);
                0
            }
            WM_LBUTTONDOWN => {
                let (x, y) = win::point_from_lparam(lparam);
                if let Some(index) = history_hit(x, y) {
                    recall(hwnd, index);
                }
                0
            }
            WM_SETCURSOR => {
                if u32::from(win::loword(lparam as usize)) == HTCLIENT && wparam as HWND == hwnd {
                    let mut p = POINT { x: 0, y: 0 };
                    GetCursorPos(&mut p);
                    ScreenToClient(hwnd, &mut p);
                    if history_hit(p.x, p.y).is_some() {
                        SetCursor(LoadCursorW(null_mut(), IDC_HAND));
                        return 1;
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CTLCOLORSTATIC => {
                // White background for labels and read-only fields, gray for the hint.
                let hdc = wparam as HDC;
                let control = lparam as HWND;
                let text = if GetDlgCtrlID(control) == i32::from(ID_HINT) {
                    COLOR_GRAYTEXT
                } else {
                    COLOR_WINDOWTEXT
                };
                SetTextColor(hdc, GetSysColor(text));
                SetBkColor(hdc, GetSysColor(COLOR_WINDOW));
                GetSysColorBrush(COLOR_WINDOW) as LRESULT
            }
            WM_ACTIVATE => {
                // Keep the keyboard focus on the same control across deactivation.
                if u32::from(win::loword(wparam)) == WA_INACTIVE {
                    FOCUS.set(GetFocus());
                } else {
                    let focus = FOCUS.get();
                    if !focus.is_null() && IsChild(hwnd, focus) != 0 {
                        SetFocus(focus);
                        return 0;
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_DPICHANGED => {
                let suggested = &*(lparam as *const RECT);
                apply_dpi(
                    hwnd,
                    u32::from(win::hiword(wparam)),
                    Some((suggested.left, suggested.top)),
                );
                0
            }
            WM_DESTROY => {
                UnregisterHotKey(hwnd, HOTKEY_ID);
                if let Some((a, b)) = with_model(|m| (m.font_ui, m.font_mono)) {
                    DeleteObject(a);
                    DeleteObject(b);
                }
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn on_command(hwnd: HWND, id: u16, code: u16) {
    const BN_CLICKED: u16 = 0;
    if code != BN_CLICKED {
        return;
    }
    if id == ID_CAPTURE {
        start_capture(hwnd);
    } else if let Some(f) = FIELDS.iter().find(|f| f.copy == id) {
        copy(hwnd, f.format);
    }
}

fn copy(hwnd: HWND, format: Format) {
    let Some(sample) = with_model(|m| m.current).flatten() else {
        return;
    };
    let text = format.text(sample.color);
    if clipboard::set_text(hwnd, &text) {
        set_status(
            hwnd,
            &format!("{} copied to the clipboard: {text}", format.name()),
        );
    } else {
        set_status(hwnd, "The clipboard is unavailable, please try again.");
    }
}

/// Hides the main window (so it does not appear on the frozen screen), then opens the
/// picker once the compositor has removed it.
fn start_capture(hwnd: HWND) {
    let started = with_model(|m| !std::mem::replace(&mut m.busy, true)).unwrap_or(false);
    if !started {
        return;
    }
    unsafe {
        let visible = IsWindowVisible(hwnd) != 0 && IsIconic(hwnd) == 0;
        with_model(|m| m.restore = visible);
        if visible {
            win::set_transitions_disabled(hwnd, true);
            ShowWindow(hwnd, SW_HIDE);
        }
        SetTimer(
            hwnd,
            TIMER_CAPTURE,
            if visible { HIDE_DELAY_MS } else { 10 },
            None,
        );
    }
}

fn open_picker(hwnd: HWND) {
    let Some(hinstance) = with_model(|m| m.hinstance) else {
        return;
    };
    if let Err(message) = picker::begin(hinstance, hwnd) {
        with_model(|m| m.busy = false);
        restore_window(hwnd);
        set_status(hwnd, message);
    }
}

fn finish_capture(hwnd: HWND) {
    with_model(|m| m.busy = false);
    match picker::take_outcome() {
        Some(Outcome::Picked { color, x, y }) => {
            select(hwnd, Sample { color, x, y });
            copy_hex_after_selection(hwnd, color, "picked and copied to the clipboard.");
        }
        _ => set_status(hwnd, "Picking cancelled."),
    }
    restore_window(hwnd);
}

fn restore_window(hwnd: HWND) {
    if with_model(|m| std::mem::take(&mut m.restore)) == Some(true) {
        unsafe {
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
        }
        win::set_transitions_disabled(hwnd, false);
    }
}

/// Makes `sample` the current color and moves it to the front of the history.
fn select(hwnd: HWND, sample: Sample) {
    with_model(|m| {
        m.current = Some(sample);
        m.history.retain(|h| h.color != sample.color);
        m.history.insert(0, sample);
        m.history.truncate(HISTORY_LEN);
    });
    update_fields(hwnd);
}

fn copy_hex_after_selection(hwnd: HWND, color: Rgb, done: &str) {
    let hex = color.hex();
    if clipboard::set_text(hwnd, &hex) {
        set_status(hwnd, &format!("{hex} {done}"));
    } else {
        set_status(
            hwnd,
            &format!("{hex} selected, but the clipboard is unavailable."),
        );
    }
}

fn recall(hwnd: HWND, index: usize) {
    let Some(sample) = with_model(|m| m.history.get(index).copied()).flatten() else {
        return;
    };
    select(hwnd, sample);
    copy_hex_after_selection(hwnd, sample.color, "recalled and copied to the clipboard.");
}

fn swatch_rect(dpi: u32) -> RECT {
    let s = |v: i32| win::scale(v, dpi);
    win::rect(s(SWATCH.0), s(SWATCH.1), s(SWATCH.2), s(SWATCH.3))
}

fn history_rect(dpi: u32, index: usize) -> RECT {
    let s = |v: i32| win::scale(v, dpi);
    let x = HISTORY_ORIGIN.0 + HISTORY_STEP * index as i32;
    win::rect(
        s(x),
        s(HISTORY_ORIGIN.1),
        s(HISTORY_SWATCH),
        s(HISTORY_SWATCH),
    )
}

/// Index of the history entry under client point (`x`, `y`).
fn history_hit(x: i32, y: i32) -> Option<usize> {
    let (dpi, len) = with_model(|m| (m.dpi, m.history.len()))?;
    (0..len).find(|&i| win::contains(&history_rect(dpi, i), x, y))
}

fn paint(hwnd: HWND) {
    let (dpi, current, history) = with_model(|m| {
        (
            m.dpi,
            m.current.map(|s| s.color),
            m.history.iter().map(|s| s.color).collect::<Vec<_>>(),
        )
    })
    .unwrap_or((96, None, Vec::new()));
    unsafe {
        let mut ps: PAINTSTRUCT = std::mem::zeroed();
        let hdc = BeginPaint(hwnd, &mut ps);
        draw_swatch(hdc, &swatch_rect(dpi), current);
        for i in 0..HISTORY_LEN {
            draw_swatch(hdc, &history_rect(dpi, i), history.get(i).copied());
        }
        EndPaint(hwnd, &ps);
    }
}

/// Filled square with a thin frame; `None` draws an empty slot.
fn draw_swatch(hdc: HDC, r: &RECT, color: Option<Rgb>) {
    let (fill, frame) = match color {
        Some(c) => (c.to_colorref(), 0x80_80_80),
        None => (0xF3_F3_F3, 0xD0_D0_D0),
    };
    unsafe {
        let brush = CreateSolidBrush(fill);
        FillRect(hdc, r, brush);
        DeleteObject(brush);
        let brush = CreateSolidBrush(frame);
        FrameRect(hdc, r, brush);
        DeleteObject(brush);
    }
}
