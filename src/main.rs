#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(unsafe_op_in_unsafe_fn)]

use anyhow::{Context, Result, anyhow};
use chrono::Local;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{Receiver, Sender, bounded};
use once_cell::sync::OnceCell;
use parakeet_rs::{Nemotron, NemotronMode};
use core::ffi::c_void;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use windows_sys::Win32::Foundation::{
    GlobalFree, HGLOBAL, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateBitmap,
    CreateFontW, CreateRoundRectRgn, CreateSolidBrush, DEFAULT_CHARSET,
    DEFAULT_PITCH, DT_CENTER, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_TOP, DT_VCENTER,
    DT_WORDBREAK, DeleteObject, DrawTextW, EndPaint, FF_DONTCARE, FW_NORMAL, FW_SEMIBOLD,
    HDC, InvalidateRect, OUT_DEFAULT_PRECIS, PAINTSTRUCT, SelectObject, SetBkColor,
    SetBkMode, SetTextColor, SetWindowRgn, TRANSPARENT,
};
use windows_sys::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, BitBlt,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, GdiFlush, GetDC, HBITMAP,
    ReleaseDC, SRCCOPY,
};
use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME, SND_NODEFAULT};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows_sys::Win32::UI::Controls::SetWindowTheme;
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock,
};
use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyNameTextW, GetKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
    MapVirtualKeyW, RegisterHotKey, ReleaseCapture, SendInput, SetCapture, SetFocus,
    UnregisterHotKey, VK_CONTROL, VK_ESCAPE, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VK_SPACE,
};
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreatePopupMenu, CreateWindowExW,
    CreateIconIndirect, DefWindowProcW, DestroyIcon, DestroyMenu, DispatchMessageW, ES_AUTOVSCROLL,
    ES_MULTILINE, ES_NOHIDESEL, ES_READONLY, GetCursorPos, GetForegroundWindow, GetMessageW,
    GetSystemMetrics, GetWindowRect, HICON, HWND_TOPMOST, ICONINFO, IDC_ARROW, IDI_APPLICATION,
    IsWindow, IsWindowVisible, KillTimer, LoadCursorW, LoadIconW, LWA_ALPHA, MF_SEPARATOR, MF_STRING, MSG,
    PostMessageW, PostQuitMessage, RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_SHOW,
    SW_SHOWNA, SWP_NOACTIVATE, SWP_NOSIZE, SendMessageW, SetForegroundWindow,
    SetLayeredWindowAttributes, SetTimer, SetWindowPos, SetWindowTextW, ShowWindow, TPM_RIGHTBUTTON,
    TrackPopupMenu, TranslateMessage, ULW_ALPHA, UpdateLayeredWindow, WM_APP, WM_COMMAND, WM_CREATE,
    WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC, WM_DESTROY, WM_ERASEBKGND, WM_HOTKEY, WM_LBUTTONDOWN,
    WM_KEYDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONUP, WM_SETFONT, WM_SYSKEYDOWN,
    WM_TIMER, WM_USER, WNDCLASSW,
    WS_CHILD, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    WM_THEMECHANGED, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};
use winreg::RegKey;
use winreg::enums::HKEY_CURRENT_USER;

const CLASS_NAME: &str = "NemotronBubbleWindow";
const TRAY_ID: u32 = 1;
const HOTKEY_ID: i32 = 42;
const WM_TRAYICON: u32 = WM_USER + 1;
const WM_UI_UPDATE: u32 = WM_APP + 1;

// Floating bubble: a soft-shadowed pill rendered with per-pixel alpha. The window
// is larger than the pill so the drop shadow has room to fall off smoothly.
const BUBBLE_MARGIN: i32 = 16;
const PILL_W: i32 = 120;
const PILL_H: i32 = 54;
const WIDTH: i32 = PILL_W + BUBBLE_MARGIN * 2;
const HEIGHT: i32 = PILL_H + BUBBLE_MARGIN * 2;
const WAVE_BARS: usize = 18;
const ANIM_TIMER_ID: usize = 77;
const ANIM_INTERVAL_MS: u32 = 33; // ~30 fps

const SETTINGS_WIDTH: i32 = 700;
const SETTINGS_HEIGHT: i32 = 712;
const HISTORY_EDIT_ID: usize = 3001;
const NEMOTRON_SAMPLE_RATE: f64 = 16_000.0;
const NEMOTRON_CHUNK_SIZE: usize = 8_960; // 560 ms at 16 kHz.

const IDM_START_STOP: usize = 1001;
const IDM_SHOW_HIDE: usize = 1002;
const IDM_CLEAR: usize = 1003;
const IDM_QUIT: usize = 1004;
const SETTINGS_TOGGLE_STARTUP: usize = 2001;
const SETTINGS_TOGGLE_PRELOAD: usize = 2002;
const SETTINGS_TOGGLE_LIVE_TYPE: usize = 2003;
const SETTINGS_TOGGLE_FINAL_CLIPBOARD: usize = 2004;
const SETTINGS_TOGGLE_FINAL_PASTE: usize = 2005;
const SETTINGS_TOGGLE_SOUNDS: usize = 2006;
const SETTINGS_TOGGLE_WAVEFORM: usize = 2007;
const SETTINGS_TOGGLE_BUBBLE_CLICK_SETTINGS: usize = 2008;
const SETTINGS_TOGGLE_FLOATING_BUBBLE: usize = 2009;
const SETTINGS_TOGGLE_TRAY_WAVEFORM: usize = 2010;

static APP: OnceCell<Arc<AppState>> = OnceCell::new();
static UI_FONT: OnceCell<isize> = OnceCell::new();
static TITLE_FONT: OnceCell<isize> = OnceCell::new();
static SMALL_FONT: OnceCell<isize> = OnceCell::new();
static EDIT_BG_BRUSH: OnceCell<isize> = OnceCell::new();
// The currently-installed tray HICON. Animated icons are swapped in via NIM_MODIFY
// and the previous one destroyed, so this tracks what needs cleanup.
static CURRENT_TRAY_ICON: AtomicIsize = AtomicIsize::new(0);

#[derive(Clone, Debug)]
enum EngineCommand {
    Preload,
    Start,
    Audio(Vec<f32>),
    Stop,
    Shutdown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
struct AppSettings {
    start_with_windows: bool,
    preload_model: bool,
    live_type_into_cursor: bool,
    copy_final_to_clipboard: bool,
    paste_final_on_stop: bool,
    sounds_enabled: bool,
    waveform_enabled: bool,
    bubble_click_opens_settings: bool,
    show_floating_bubble: bool,
    tray_waveform_enabled: bool,
    hotkey_mods: u32,
    hotkey_vk: u32,
    paste_delay_ms: u64,
    history_limit: usize,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            start_with_windows: true,
            preload_model: true,
            live_type_into_cursor: true,
            copy_final_to_clipboard: true,
            paste_final_on_stop: false,
            sounds_enabled: true,
            waveform_enabled: true,
            bubble_click_opens_settings: true,
            show_floating_bubble: true,
            tray_waveform_enabled: true,
            hotkey_mods: MOD_CONTROL,
            hotkey_vk: VK_SPACE as u32,
            paste_delay_ms: 60,
            history_limit: 20,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct HistoryItem {
    timestamp: String,
    text: String,
}

#[derive(Debug)]
struct UiState {
    recording: bool,
    visible: bool,
    status: String,
    transcript: String,
    model_dir: String,
    waveform: Vec<f32>,
    current_level: f32,
}

struct AppState {
    engine_tx: Sender<EngineCommand>,
    recording: Arc<AtomicBool>,
    hwnd: AtomicIsize,
    target_hwnd: AtomicIsize,
    settings_hwnd: AtomicIsize,
    history_edit_hwnd: AtomicIsize,
    capturing_hotkey: AtomicBool,
    drag: Mutex<Option<DragState>>,
    settings: Mutex<AppSettings>,
    history: Mutex<Vec<HistoryItem>>,
    ui: Mutex<UiState>,
}

struct DragState {
    hwnd: isize,
    start_cursor: POINT,
    start_rect: RECT,
    moved: bool,
}

#[derive(Copy, Clone)]
struct RectI {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl RectI {
    fn contains(self, x: i32, y: i32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }
}

fn main() -> Result<()> {
    if env::args().any(|arg| arg == "--self-test") {
        return self_test();
    }

    #[cfg(feature = "assets")]
    {
        let args: Vec<String> = env::args().collect();
        if let Some(pos) = args.iter().position(|a| a == "--render-assets") {
            let out = args
                .get(pos + 1)
                .cloned()
                .unwrap_or_else(|| "docs".to_string());
            return render_assets(&out);
        }
    }

    let settings = load_settings();
    save_settings(&settings);
    apply_startup_setting(settings.start_with_windows);
    ensure_sound_assets();
    let history = load_history(settings.history_limit);

    let (engine_tx, engine_rx) = bounded::<EngineCommand>(256);
    let app = Arc::new(AppState {
        engine_tx,
        recording: Arc::new(AtomicBool::new(false)),
        hwnd: AtomicIsize::new(0),
        target_hwnd: AtomicIsize::new(0),
        settings_hwnd: AtomicIsize::new(0),
        history_edit_hwnd: AtomicIsize::new(0),
        capturing_hotkey: AtomicBool::new(false),
        drag: Mutex::new(None),
        settings: Mutex::new(settings),
        history: Mutex::new(history),
        ui: Mutex::new(UiState {
            recording: false,
            visible: true,
            status: "Ready. Press Ctrl+Space to start.".to_string(),
            transcript: "Tap Ctrl+Space to dictate.".to_string(),
            model_dir: String::new(),
            waveform: vec![0.0; WAVE_BARS],
            current_level: 0.0,
        }),
    });

    APP.set(app.clone()).ok();

    thread::spawn({
        let app = app.clone();
        move || run_engine(engine_rx, app)
    });

    let _audio_stream = start_audio_capture(app.clone()).context("failed to start microphone")?;
    if settings_snapshot(&app).preload_model {
        let _ = app.engine_tx.send(EngineCommand::Preload);
    }

    unsafe {
        run_window(app).context("failed to run window")?;
    }

    Ok(())
}

fn self_test() -> Result<()> {
    let (mut model, model_dir) = load_nemotron()?;
    model.reset();
    let mode = match model.mode() {
        NemotronMode::EnglishOnly => "English-only",
        NemotronMode::Multilingual => "Multilingual",
    };
    let _ = model.transcribe_chunk(&vec![0.0; NEMOTRON_CHUNK_SIZE])?;
    println!("Nemotron loaded: {mode}");
    println!("Model dir: {}", model_dir.display());
    Ok(())
}

fn apply_startup_setting(enabled: bool) {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok((run_key, _)) =
        hkcu.create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
    else {
        return;
    };

    if enabled {
        let Ok(exe) = env::current_exe() else {
            return;
        };
        let value = format!("\"{}\"", exe.display());
        let _ = run_key.set_value("NemotronBubble", &value);
    } else {
        let _ = run_key.delete_value("NemotronBubble");
    }
}

fn settings_snapshot(app: &Arc<AppState>) -> AppSettings {
    app.settings
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn history_snapshot(app: &Arc<AppState>) -> Vec<HistoryItem> {
    app.history
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn toggle_setting(app: &Arc<AppState>, id: usize) {
    let settings = {
        let mut settings = app
            .settings
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match id {
            SETTINGS_TOGGLE_STARTUP => settings.start_with_windows = !settings.start_with_windows,
            SETTINGS_TOGGLE_PRELOAD => settings.preload_model = !settings.preload_model,
            SETTINGS_TOGGLE_LIVE_TYPE => {
                settings.live_type_into_cursor = !settings.live_type_into_cursor
            }
            SETTINGS_TOGGLE_FINAL_CLIPBOARD => {
                settings.copy_final_to_clipboard = !settings.copy_final_to_clipboard
            }
            SETTINGS_TOGGLE_FINAL_PASTE => {
                settings.paste_final_on_stop = !settings.paste_final_on_stop
            }
            SETTINGS_TOGGLE_SOUNDS => settings.sounds_enabled = !settings.sounds_enabled,
            SETTINGS_TOGGLE_WAVEFORM => settings.waveform_enabled = !settings.waveform_enabled,
            SETTINGS_TOGGLE_BUBBLE_CLICK_SETTINGS => {
                settings.bubble_click_opens_settings = !settings.bubble_click_opens_settings
            }
            SETTINGS_TOGGLE_FLOATING_BUBBLE => {
                settings.show_floating_bubble = !settings.show_floating_bubble
            }
            SETTINGS_TOGGLE_TRAY_WAVEFORM => {
                settings.tray_waveform_enabled = !settings.tray_waveform_enabled
            }
            _ => {}
        }
        settings.clone()
    };

    save_settings(&settings);
    apply_startup_setting(settings.start_with_windows);
    if settings.preload_model {
        let _ = app.engine_tx.send(EngineCommand::Preload);
    }
    unsafe {
        let hwnd = hwnd_from_app(app);
        if hwnd != null_mut() {
            if settings.show_floating_bubble {
                render_bubble(hwnd);
                ShowWindow(hwnd, SW_SHOWNA);
            } else {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
    }
    refresh_tray_idle(app);
    update_ui(app, |ui| {
        ui.status = "Settings saved.".to_string();
    });
}

fn load_settings() -> AppSettings {
    let path = settings_path();
    fs::read_to_string(path)
        .ok()
        .and_then(|json| serde_json::from_str::<AppSettings>(&json).ok())
        .unwrap_or_default()
}

fn save_settings(settings: &AppSettings) {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = fs::write(path, json);
    }
}

fn load_history(limit: usize) -> Vec<HistoryItem> {
    let path = history_path();
    let mut history = fs::read_to_string(path)
        .ok()
        .and_then(|json| serde_json::from_str::<Vec<HistoryItem>>(&json).ok())
        .unwrap_or_default();
    history.truncate(limit);
    history
}

fn save_history(history: &[HistoryItem]) {
    let path = history_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(history) {
        let _ = fs::write(path, json);
    }
}

fn add_history(app: &Arc<AppState>, text: String) {
    let clean = text.trim().to_string();
    if clean.is_empty() {
        return;
    }
    let limit = settings_snapshot(app).history_limit;
    let mut history = app
        .history
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    history.insert(
        0,
        HistoryItem {
            timestamp: Local::now().format("%b %-d, %-I:%M %p").to_string(),
            text: clean,
        },
    );
    history.truncate(limit);
    save_history(&history);
    drop(history);
    update_history_edit(app);
}

fn clear_history(app: &Arc<AppState>) {
    {
        let mut history = app
            .history
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        history.clear();
        save_history(&history);
    }
    update_ui(app, |ui| {
        ui.status = "History cleared.".to_string();
    });
    update_history_edit(app);
}

unsafe fn create_history_edit(parent: HWND, app: &Arc<AppState>) {
    let class = wide_null("EDIT");
    let edit = CreateWindowExW(
        0,
        class.as_ptr(),
        wide_null("").as_ptr(),
        WS_CHILD
            | WS_VISIBLE
            | WS_TABSTOP
            | WS_VSCROLL
            | (ES_MULTILINE as u32)
            | (ES_READONLY as u32)
            | (ES_AUTOVSCROLL as u32)
            | (ES_NOHIDESEL as u32),
        400,
        182,
        SETTINGS_WIDTH - 436,
        SETTINGS_HEIGHT - 206,
        parent,
        HISTORY_EDIT_ID as _,
        GetModuleHandleW(null()),
        null_mut(),
    );
    if edit != null_mut() {
        app.history_edit_hwnd.store(edit as isize, Ordering::SeqCst);
        SendMessageW(edit, WM_SETFONT, ui_font() as usize, 1);
        apply_dark_scrollbar(edit);
        update_history_edit(app);
    }
}

/// Give a control the modern flat dark scrollbar (the thin overlay used by dark-mode
/// Explorer) instead of the chunky classic Win32 scrollbar with arrow buttons.
/// Uses the documented SetWindowTheme plus the undocumented (but stable since Win10
/// 1903) uxtheme dark-mode ordinals; every call is best-effort.
unsafe fn apply_dark_scrollbar(hwnd: HWND) {
    let uxtheme = LoadLibraryW(wide_null("uxtheme.dll").as_ptr());
    if uxtheme != null_mut() {
        // SetPreferredAppMode(AllowDark = 1) — ordinal 135
        if let Some(proc) = GetProcAddress(uxtheme, 135 as *const u8) {
            let set_preferred_app_mode: unsafe extern "system" fn(i32) -> i32 =
                std::mem::transmute(proc);
            set_preferred_app_mode(1);
        }
        // AllowDarkModeForWindow(hwnd, true) — ordinal 133
        if let Some(proc) = GetProcAddress(uxtheme, 133 as *const u8) {
            let allow_dark_mode_for_window: unsafe extern "system" fn(HWND, i32) -> i32 =
                std::mem::transmute(proc);
            allow_dark_mode_for_window(hwnd, 1);
        }
    }
    SetWindowTheme(hwnd, wide_null("DarkMode_Explorer").as_ptr(), null());
    SendMessageW(hwnd, WM_THEMECHANGED, 0, 0);
}

fn update_history_edit(app: &Arc<AppState>) {
    let hwnd = history_edit_hwnd_from_app(app);
    if hwnd == null_mut() {
        return;
    }
    let text = history_snapshot(app)
        .iter()
        .map(|item| format!("{}\r\n{}\r\n", item.timestamp, item.text.trim()))
        .collect::<Vec<_>>()
        .join("\r\n");
    unsafe {
        let wide = wide_null(if text.is_empty() {
            "No dictations yet."
        } else {
            &text
        });
        SetWindowTextW(hwnd, wide.as_ptr());
    }
}

fn settings_path() -> PathBuf {
    app_data_dir().join("settings.json")
}

fn history_path() -> PathBuf {
    app_data_dir().join("history.json")
}

fn app_data_dir() -> PathBuf {
    env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("NemotronBubble")
}

fn ensure_sound_assets() {
    let dir = app_data_dir().join("sounds");
    let _ = fs::create_dir_all(&dir);
    let start = dir.join("start.wav");
    let stop = dir.join("stop.wav");
    let _ = write_chime(&start, &[(329.63, 0.08), (392.00, 0.12)]);
    let _ = write_chime(&stop, &[(392.00, 0.07), (293.66, 0.11)]);
}

fn sound_path(name: &str) -> PathBuf {
    app_data_dir().join("sounds").join(name)
}

fn write_chime(path: &PathBuf, notes: &[(f32, f32)]) -> Result<()> {
    const SAMPLE_RATE: u32 = 22_050;
    let mut samples = Vec::<i16>::new();
    for (freq, seconds) in notes {
        let count = (SAMPLE_RATE as f32 * seconds) as usize;
        for i in 0..count {
            let t = i as f32 / SAMPLE_RATE as f32;
            let progress = i as f32 / count.max(1) as f32;
            let attack = (progress / 0.18).min(1.0);
            let release = ((1.0 - progress) / 0.35).min(1.0);
            let envelope = attack.min(release).powf(1.4);
            let wave = (2.0 * std::f32::consts::PI * freq * t).sin();
            samples.push((wave * envelope * 6_500.0) as i16);
        }
    }

    let mut file = fs::File::create(path)?;
    let data_len = samples.len() as u32 * 2;
    file.write_all(b"RIFF")?;
    file.write_all(&(36 + data_len).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&SAMPLE_RATE.to_le_bytes())?;
    file.write_all(&(SAMPLE_RATE * 2).to_le_bytes())?;
    file.write_all(&2u16.to_le_bytes())?;
    file.write_all(&16u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&data_len.to_le_bytes())?;
    for sample in samples {
        file.write_all(&sample.to_le_bytes())?;
    }
    Ok(())
}

unsafe fn run_window(app: Arc<AppState>) -> Result<()> {
    let hinstance = GetModuleHandleW(null());
    if hinstance == null_mut() {
        return Err(anyhow!("GetModuleHandleW failed"));
    }

    let class_name = wide_null(CLASS_NAME);
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinstance,
        hCursor: LoadCursorW(null_mut(), IDC_ARROW),
        hbrBackground: null_mut(),
        lpszClassName: class_name.as_ptr(),
        ..std::mem::zeroed()
    };

    if RegisterClassW(&wc) == 0 {
        return Err(anyhow!("RegisterClassW failed"));
    }

    let title = wide_null("Nemotron Bubble");
    // No WS_VISIBLE: per-pixel-alpha layered windows are made visible only after the
    // first UpdateLayeredWindow so there is never a blank flash.
    let hwnd = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_POPUP,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        WIDTH,
        HEIGHT,
        null_mut(),
        null_mut(),
        hinstance,
        null_mut(),
    );

    if hwnd == null_mut() {
        return Err(anyhow!("CreateWindowExW failed"));
    }
    // NOTE: a per-pixel-alpha window driven by UpdateLayeredWindow must NOT also call
    // SetLayeredWindowAttributes, and needs no window region — the alpha gives the shape.

    app.hwnd.store(hwnd as isize, Ordering::SeqCst);
    place_window(hwnd);
    render_bubble(hwnd);
    add_tray_icon(hwnd);

    let settings_title = wide_null("Nemotron Bubble Settings");
    let settings_hwnd = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
        class_name.as_ptr(),
        settings_title.as_ptr(),
        WS_POPUP,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        SETTINGS_WIDTH,
        SETTINGS_HEIGHT,
        null_mut(),
        null_mut(),
        hinstance,
        null_mut(),
    );
    if settings_hwnd != null_mut() {
        SetLayeredWindowAttributes(settings_hwnd, 0, 246, LWA_ALPHA);
        app.settings_hwnd
            .store(settings_hwnd as isize, Ordering::SeqCst);
        place_settings_window(settings_hwnd);
        apply_settings_rounding(settings_hwnd);
        create_history_edit(settings_hwnd, &app);
        ShowWindow(settings_hwnd, SW_HIDE);
    }

    let snapshot = settings_snapshot(&app);
    if !register_global_hotkey(hwnd, snapshot.hotkey_mods, snapshot.hotkey_vk) {
        update_ui(&app, |ui| {
            ui.status = "Shortcut unavailable — open settings to choose another.".to_string();
        });
    }

    if settings_snapshot(&app).show_floating_bubble {
        ShowWindow(hwnd, SW_SHOWNA);
    } else {
        ShowWindow(hwnd, SW_HIDE);
    }

    // Drive waveform + tray animation from a single UI-thread timer.
    SetTimer(hwnd, ANIM_TIMER_ID, ANIM_INTERVAL_MS, None);

    let mut msg: MSG = std::mem::zeroed();
    while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    Ok(())
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => 0,
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            if is_settings_window(hwnd) {
                paint_settings(hwnd);
            } else {
                paint(hwnd);
            }
            0
        }
        WM_HOTKEY => {
            if wparam as i32 == HOTKEY_ID {
                toggle_recording();
            }
            0
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            if is_settings_window(hwnd) {
                if let Some(app) = APP.get() {
                    if app.capturing_hotkey.load(Ordering::SeqCst) {
                        handle_capture_key(app, wparam as u32);
                        return 0;
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_LBUTTONDOWN => {
            let (x, y) = lparam_point(lparam);
            if should_start_drag(hwnd, x, y) {
                begin_drag(hwnd);
            }
            0
        }
        WM_MOUSEMOVE => {
            continue_drag();
            0
        }
        WM_LBUTTONUP => {
            let (x, y) = lparam_point(lparam);
            let was_click = end_drag();
            if was_click {
                if is_settings_window(hwnd) {
                    handle_settings_click(x, y);
                } else {
                    handle_click(x, y);
                }
            }
            0
        }
        WM_TRAYICON => {
            match lparam as u32 {
                WM_LBUTTONUP => toggle_settings_window(),
                WM_RBUTTONUP => show_tray_menu(hwnd),
                _ => {}
            }
            0
        }
        WM_COMMAND => {
            handle_command(loword(wparam as usize) as usize);
            0
        }
        WM_TIMER => {
            if wparam == ANIM_TIMER_ID {
                on_anim_tick(hwnd);
            }
            0
        }
        WM_UI_UPDATE => {
            if is_settings_window(hwnd) {
                InvalidateRect(hwnd, null(), 0);
            } else {
                render_bubble(hwnd);
            }
            0
        }
        WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC => {
            let hdc = wparam as HDC;
            SetTextColor(hdc, rgb(225, 230, 238));
            SetBkColor(hdc, rgb(33, 37, 46));
            edit_bg_brush() as isize
        }
        WM_DESTROY => {
            if let Some(app) = APP.get() {
                if hwnd == hwnd_from_app(app) {
                    let _ = app.engine_tx.send(EngineCommand::Shutdown);
                    KillTimer(hwnd, ANIM_TIMER_ID);
                    UnregisterHotKey(hwnd, HOTKEY_ID);
                    remove_tray_icon(hwnd);
                    PostQuitMessage(0);
                }
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn run_engine(rx: Receiver<EngineCommand>, app: Arc<AppState>) {
    let mut model: Option<Nemotron> = None;
    let mut model_path = PathBuf::new();
    let mut active = false;
    let mut audio_buffer = Vec::<f32>::with_capacity(NEMOTRON_CHUNK_SIZE * 2);

    loop {
        match rx.recv() {
            Ok(EngineCommand::Preload) => {
                if model.is_some() {
                    continue;
                }
                update_ui(&app, |ui| {
                    ui.status = "Loading Nemotron...".to_string();
                });
                match load_nemotron() {
                    Ok((loaded, path)) => {
                        model = Some(loaded);
                        model_path = path;
                        update_ui(&app, |ui| {
                            ui.status = "Ready. Nemotron loaded. Ctrl+Space toggles.".to_string();
                            ui.model_dir = model_path.display().to_string();
                            if ui.transcript.trim().is_empty() {
                                ui.transcript = "Tap Ctrl+Space to dictate.".to_string();
                            }
                        });
                    }
                    Err(err) => {
                        update_ui(&app, |ui| {
                            ui.status = "Nemotron model missing or failed to load.".to_string();
                            ui.transcript = format!("{err:#}");
                            ui.model_dir = expected_model_hint();
                        });
                    }
                }
            }
            Ok(EngineCommand::Start) => {
                audio_buffer.clear();
                if model.is_none() {
                    update_ui(&app, |ui| {
                        ui.status = "Loading Nemotron model...".to_string();
                    });
                    match load_nemotron() {
                        Ok((loaded, path)) => {
                            model = Some(loaded);
                            model_path = path;
                        }
                        Err(err) => {
                            app.recording.store(false, Ordering::SeqCst);
                            update_ui(&app, |ui| {
                                ui.recording = false;
                                ui.status = "Nemotron model missing or failed to load.".to_string();
                                ui.transcript = format!("{err:#}");
                                ui.model_dir = expected_model_hint();
                            });
                            continue;
                        }
                    }
                }

                if let Some(model) = model.as_mut() {
                    model.reset();
                    active = true;
                    update_ui(&app, |ui| {
                        ui.recording = true;
                        ui.status = "Listening...".to_string();
                        ui.transcript.clear();
                        ui.model_dir = model_path.display().to_string();
                    });
                    play_start_sound(&app);
                }
            }
            Ok(EngineCommand::Audio(samples)) => {
                if !active {
                    continue;
                }
                let Some(model) = model.as_mut() else {
                    continue;
                };

                audio_buffer.extend(samples);
                while audio_buffer.len() >= NEMOTRON_CHUNK_SIZE {
                    let chunk: Vec<f32> = audio_buffer.drain(..NEMOTRON_CHUNK_SIZE).collect();
                    match model.transcribe_chunk(&chunk) {
                        Ok(text) if !text.is_empty() => {
                            let settings = settings_snapshot(&app);
                            let live_type = settings.live_type_into_cursor;
                            // Type the new words directly via Unicode keystrokes —
                            // the clipboard stays untouched until the final stop.
                            let type_result = if live_type {
                                type_unicode_text(
                                    &text,
                                    hwnd_from_app(&app),
                                    target_hwnd_from_app(&app),
                                    settings.paste_delay_ms,
                                )
                            } else {
                                Ok(())
                            };
                            update_ui(&app, |ui| {
                                ui.status = if let Err(err) = type_result {
                                    format!("Live typing failed: {err}")
                                } else if live_type {
                                    "Listening... typing live.".to_string()
                                } else {
                                    "Listening...".to_string()
                                };
                                ui.transcript.push_str(&text);
                            });
                        }
                        Ok(_) => {}
                        Err(err) => {
                            active = false;
                            app.recording.store(false, Ordering::SeqCst);
                            update_ui(&app, |ui| {
                                ui.recording = false;
                                ui.status = "ASR error.".to_string();
                                ui.transcript = format!("{err:#}");
                            });
                        }
                    }
                }
            }
            Ok(EngineCommand::Stop) => {
                app.recording.store(false, Ordering::SeqCst);
                if active {
                    active = false;
                    if let Some(model) = model.as_mut() {
                        if !audio_buffer.is_empty() {
                            audio_buffer.resize(NEMOTRON_CHUNK_SIZE, 0.0);
                            if let Ok(text) = model.transcribe_chunk(&audio_buffer) {
                                paste_live_tail(&app, &text);
                            }
                            audio_buffer.clear();
                        }
                        for _ in 0..3 {
                            if let Ok(text) = model.transcribe_chunk(&vec![0.0; NEMOTRON_CHUNK_SIZE])
                            {
                                paste_live_tail(&app, &text);
                            }
                        }

                        let final_text = model.get_transcript();
                        let has_final = !final_text.trim().is_empty();
                        let settings = settings_snapshot(&app);
                        let mut final_status = if has_final {
                            "Stopped. Text was typed live.".to_string()
                        } else {
                            "Ready. Press Ctrl+Space to start.".to_string()
                        };

                        if has_final {
                            add_history(&app, final_text.clone());
                            // Paste-on-stop already writes the clipboard via Ctrl+V, so
                            // only do a standalone copy when not pasting — never both.
                            if settings.paste_final_on_stop {
                                final_status = match paste_text_to_target(
                                    &final_text,
                                    hwnd_from_app(&app),
                                    target_hwnd_from_app(&app),
                                    settings.paste_delay_ms,
                                ) {
                                    Ok(()) => "Stopped. Final transcript pasted.".to_string(),
                                    Err(err) => format!("Final paste failed: {err}"),
                                };
                            } else if settings.copy_final_to_clipboard {
                                final_status =
                                    match set_clipboard_text(&final_text, hwnd_from_app(&app)) {
                                        Ok(()) => {
                                            "Stopped. Final transcript copied to clipboard."
                                                .to_string()
                                        }
                                        Err(err) => format!("Clipboard failed: {err}"),
                                    };
                            }
                        }

                        update_ui(&app, |ui| {
                            ui.recording = false;
                            ui.status = final_status;
                            ui.transcript = if !has_final {
                                "No speech detected.".to_string()
                            } else {
                                final_text
                            };
                        });
                    }
                } else {
                    update_ui(&app, |ui| {
                        ui.recording = false;
                        ui.status = "Ready. Press Ctrl+Space to start.".to_string();
                    });
                }
            }
            Ok(EngineCommand::Shutdown) | Err(_) => break,
        }
    }
}

fn load_nemotron() -> Result<(Nemotron, PathBuf)> {
    let model_dir = find_model_dir()?;
    let mut model = Nemotron::from_pretrained(&model_dir, None)
        .map_err(|err| anyhow!("failed to load {}: {err}", model_dir.display()))?;

    if model.mode() == NemotronMode::Multilingual {
        model
            .set_target_lang("auto")
            .map_err(|err| anyhow!("failed to set multilingual auto language: {err}"))?;
    }

    Ok((model, model_dir))
}

fn find_model_dir() -> Result<PathBuf> {
    let mut candidates = Vec::<PathBuf>::new();

    if let Ok(path) = env::var("NEMOTRON_MODEL_DIR") {
        candidates.push(PathBuf::from(path));
    }

    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join("models").join("nemotron"));
        candidates.push(cwd.join("models").join("nemotron_multi"));
    }

    // Walk up from the executable so the model is found no matter the working
    // directory. This matters for "Start with Windows", where Windows launches us
    // with the working directory set to System32 — and for the cargo layout, where
    // the exe sits in target\release\ but the model is in the project root.
    if let Ok(exe) = env::current_exe() {
        let mut dir = exe.parent();
        for _ in 0..5 {
            let Some(d) = dir else { break };
            candidates.push(d.join("models").join("nemotron"));
            candidates.push(d.join("models").join("nemotron_multi"));
            dir = d.parent();
        }
    }

    for candidate in &candidates {
        if candidate.join("encoder.onnx").exists()
            && candidate.join("decoder_joint.onnx").exists()
            && candidate.join("tokenizer.model").exists()
        {
            return Ok(candidate.clone());
        }
    }

    Err(anyhow!(
        "Expected Nemotron ONNX files in one of these folders:\n{}",
        candidates
            .iter()
            .map(|p| format!("  - {}", p.display()))
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

fn expected_model_hint() -> String {
    env::current_dir()
        .map(|cwd| cwd.join("models").join("nemotron").display().to_string())
        .unwrap_or_else(|_| "models\\nemotron".to_string())
}

fn start_audio_capture(app: Arc<AppState>) -> Result<cpal::Stream> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("no default microphone found"))?;
    let input_config = device.default_input_config()?;
    let sample_rate = input_config.sample_rate().0 as f64;
    let channels = input_config.channels() as usize;
    let config = input_config.config();

    update_ui(&app, |ui| {
        ui.status = format!(
            "Ready. Mic: {} Hz, {} channel(s). Ctrl+Space toggles.",
            sample_rate as u32, channels
        );
    });

    let tx = app.engine_tx.clone();
    let recording = app.recording.clone();
    let err_app = app.clone();
    let err_fn = move |err: cpal::StreamError| {
        update_ui(&err_app, |ui| {
            ui.status = "Microphone error.".to_string();
            ui.transcript = err.to_string();
        });
    };

    let stream = match input_config.sample_format() {
        cpal::SampleFormat::F32 => {
            let mut callback = InputCallback::new(channels, sample_rate, recording, tx, app.clone());
            device.build_input_stream(
                &config,
                move |data: &[f32], _| callback.push_f32(data),
                err_fn,
                None,
            )?
        }
        cpal::SampleFormat::I16 => {
            let mut callback = InputCallback::new(channels, sample_rate, recording, tx, app.clone());
            device.build_input_stream(
                &config,
                move |data: &[i16], _| callback.push_i16(data),
                err_fn,
                None,
            )?
        }
        cpal::SampleFormat::U16 => {
            let mut callback = InputCallback::new(channels, sample_rate, recording, tx, app.clone());
            device.build_input_stream(
                &config,
                move |data: &[u16], _| callback.push_u16(data),
                err_fn,
                None,
            )?
        }
        other => return Err(anyhow!("unsupported microphone sample format: {other:?}")),
    };

    stream.play()?;
    Ok(stream)
}

struct InputCallback {
    channels: usize,
    tx: Sender<EngineCommand>,
    recording: Arc<AtomicBool>,
    app: Arc<AppState>,
    was_recording: bool,
    resampler: LinearResampler,
}

impl InputCallback {
    fn new(
        channels: usize,
        source_rate: f64,
        recording: Arc<AtomicBool>,
        tx: Sender<EngineCommand>,
        app: Arc<AppState>,
    ) -> Self {
        Self {
            channels,
            tx,
            recording,
            app,
            was_recording: false,
            resampler: LinearResampler::new(source_rate, NEMOTRON_SAMPLE_RATE),
        }
    }

    fn push_f32(&mut self, data: &[f32]) {
        self.process(data.iter().copied());
    }

    fn push_i16(&mut self, data: &[i16]) {
        self.process(data.iter().map(|s| *s as f32 / 32768.0));
    }

    fn push_u16(&mut self, data: &[u16]) {
        self.process(data.iter().map(|s| (*s as f32 / 65535.0) * 2.0 - 1.0));
    }

    fn process<I>(&mut self, samples: I)
    where
        I: Iterator<Item = f32>,
    {
        let now_recording = self.recording.load(Ordering::Relaxed);
        if !now_recording {
            self.was_recording = false;
            return;
        }

        if !self.was_recording {
            self.resampler.reset();
            self.was_recording = true;
        }

        let mut mono = Vec::new();
        let mut frame = Vec::with_capacity(self.channels);
        for sample in samples {
            frame.push(sample);
            if frame.len() == self.channels {
                let sum = frame.iter().sum::<f32>();
                mono.push(sum / self.channels as f32);
                frame.clear();
            }
        }

        if !mono.is_empty() {
            // Loudness -> bar height. NOISE_FLOOR gates hiss; GAIN sets sensitivity so
            // ordinary speech reaches most of the height; the sub-1.0 exponent lifts
            // quiet syllables so they read as tall bumps instead of tiny specks.
            const NOISE_FLOOR: f32 = 0.006;
            const GAIN: f32 = 14.0;
            const SHAPE: f32 = 0.6;
            let rms = (mono.iter().map(|v| v * v).sum::<f32>() / mono.len() as f32).sqrt();
            let norm = ((rms - NOISE_FLOOR) * GAIN).clamp(0.0, 1.0);
            let level = norm.powf(SHAPE);
            report_level(&self.app, level);
        }

        let mut out = Vec::new();
        self.resampler.push(&mono, &mut out);
        if !out.is_empty() {
            let _ = self.tx.try_send(EngineCommand::Audio(out));
        }
    }
}

struct LinearResampler {
    source_rate: f64,
    target_rate: f64,
    position: f64,
    last_sample: Option<f32>,
}

impl LinearResampler {
    fn new(source_rate: f64, target_rate: f64) -> Self {
        Self {
            source_rate,
            target_rate,
            position: 0.0,
            last_sample: None,
        }
    }

    fn reset(&mut self) {
        self.position = 0.0;
        self.last_sample = None;
    }

    fn push(&mut self, input: &[f32], output: &mut Vec<f32>) {
        if input.is_empty() {
            return;
        }

        let mut data = Vec::with_capacity(input.len() + usize::from(self.last_sample.is_some()));
        if let Some(last) = self.last_sample {
            data.push(last);
        }
        data.extend_from_slice(input);

        let step = self.source_rate / self.target_rate;
        while self.position + 1.0 < data.len() as f64 {
            let index = self.position.floor() as usize;
            let frac = (self.position - index as f64) as f32;
            let a = data[index];
            let b = data[index + 1];
            output.push(a + (b - a) * frac);
            self.position += step;
        }

        self.last_sample = data.last().copied();
        self.position -= (data.len() - 1) as f64;
    }
}

fn toggle_recording() {
    let Some(app) = APP.get() else {
        return;
    };

    if app.recording.load(Ordering::SeqCst) {
        stop_recording(app);
    } else {
        start_recording(app);
    }
}

fn start_recording(app: &Arc<AppState>) {
    remember_target_window(app);
    app.recording.store(true, Ordering::SeqCst);
    let _ = app.engine_tx.send(EngineCommand::Start);
    update_ui(app, |ui| {
        ui.recording = true;
        ui.visible = true;
        ui.status = "Starting...".to_string();
        ui.transcript.clear();
    });
    unsafe {
        let hwnd = hwnd_from_app(app);
        if hwnd != null_mut() && settings_snapshot(app).show_floating_bubble {
            ShowWindow(hwnd, SW_SHOWNA);
        }
    }
}

fn remember_target_window(app: &Arc<AppState>) {
    unsafe {
        let foreground = GetForegroundWindow();
        let own = hwnd_from_app(app);
        if foreground != null_mut() && foreground != own {
            app.target_hwnd
                .store(foreground as isize, Ordering::SeqCst);
        }
    }
}

fn stop_recording(app: &Arc<AppState>) {
    app.recording.store(false, Ordering::SeqCst);
    play_stop_sound(app);
    let _ = app.engine_tx.send(EngineCommand::Stop);
    update_ui(app, |ui| {
        ui.recording = false;
        ui.status = "Finalizing...".to_string();
    });
}

fn play_start_sound(app: &Arc<AppState>) {
    if !settings_snapshot(app).sounds_enabled {
        return;
    }
    play_sound_file(sound_path("start.wav"));
}

fn play_stop_sound(app: &Arc<AppState>) {
    if !settings_snapshot(app).sounds_enabled {
        return;
    }
    play_sound_file(sound_path("stop.wav"));
}

fn play_sound_file(path: PathBuf) {
    thread::spawn(move || unsafe {
        let wide = wide_null(&path.display().to_string());
        PlaySoundW(wide.as_ptr(), null_mut(), SND_FILENAME | SND_ASYNC | SND_NODEFAULT);
    });
}

fn paste_live_tail(app: &Arc<AppState>, text: &str) {
    if text.is_empty() || !settings_snapshot(app).live_type_into_cursor {
        return;
    }

    let delay_ms = settings_snapshot(app).paste_delay_ms;
    if let Err(err) =
        type_unicode_text(text, hwnd_from_app(app), target_hwnd_from_app(app), delay_ms)
    {
        update_ui(app, |ui| {
            ui.status = format!("Live typing failed: {err}");
        });
    }
}

fn paste_text_to_target(
    text: &str,
    owner_hwnd: HWND,
    target_hwnd: HWND,
    delay_ms: u64,
) -> Result<()> {
    if text.trim().is_empty() {
        return Ok(());
    }

    set_clipboard_text(text, owner_hwnd)?;

    unsafe {
        let target = if target_hwnd == null_mut() {
            GetForegroundWindow()
        } else {
            target_hwnd
        };

        if target != null_mut()
            && target != owner_hwnd
            && IsWindow(target) != 0
            && SetForegroundWindow(target) != 0
        {
            thread::sleep(Duration::from_millis(delay_ms));
        }

        send_ctrl_v()
    }
}

fn set_clipboard_text(text: &str, owner_hwnd: HWND) -> Result<()> {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let byte_len = wide.len() * std::mem::size_of::<u16>();

    unsafe {
        if OpenClipboard(owner_hwnd) == 0 {
            return Err(anyhow!("clipboard is busy"));
        }

        EmptyClipboard();
        let handle = GlobalAlloc(GMEM_MOVEABLE, byte_len);
        if handle == null_mut() {
            CloseClipboard();
            return Err(anyhow!("clipboard allocation failed"));
        }

        let locked = GlobalLock(handle) as *mut u16;
        if locked == null_mut() {
            GlobalFree(handle as HGLOBAL);
            CloseClipboard();
            return Err(anyhow!("clipboard lock failed"));
        }

        std::ptr::copy_nonoverlapping(wide.as_ptr(), locked, wide.len());
        GlobalUnlock(handle);

        if SetClipboardData(CF_UNICODETEXT as u32, handle) == null_mut() {
            GlobalFree(handle as HGLOBAL);
            CloseClipboard();
            return Err(anyhow!("clipboard set failed"));
        }

        CloseClipboard();
    }

    Ok(())
}

unsafe fn send_ctrl_v() -> Result<()> {
    let inputs = [
        keyboard_input(VK_CONTROL, 0),
        keyboard_input('V' as u16, 0),
        keyboard_input('V' as u16, KEYEVENTF_KEYUP),
        keyboard_input(VK_CONTROL, KEYEVENTF_KEYUP),
    ];

    let sent = SendInput(
        inputs.len() as u32,
        inputs.as_ptr(),
        std::mem::size_of::<INPUT>() as i32,
    );

    if sent != inputs.len() as u32 {
        return Err(anyhow!("Ctrl+V injection failed"));
    }

    Ok(())
}

fn keyboard_input(vk: u16, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn unicode_input(unit: u16, extra_flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: 0,
                wScan: unit,
                dwFlags: KEYEVENTF_UNICODE | extra_flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Type text directly as synthesized Unicode keystrokes. Unlike the Ctrl+V path
/// this never touches the clipboard, so live dictation can stream into the cursor
/// without clobbering whatever the user has copied. UTF-16 (incl. surrogate pairs)
/// is sent verbatim.
unsafe fn send_unicode(text: &str) -> Result<()> {
    let mut inputs: Vec<INPUT> = Vec::with_capacity(text.len() * 2);
    for unit in text.encode_utf16() {
        inputs.push(unicode_input(unit, 0));
        inputs.push(unicode_input(unit, KEYEVENTF_KEYUP));
    }
    if inputs.is_empty() {
        return Ok(());
    }
    let sent = SendInput(
        inputs.len() as u32,
        inputs.as_ptr(),
        std::mem::size_of::<INPUT>() as i32,
    );
    if sent != inputs.len() as u32 {
        return Err(anyhow!("unicode injection failed"));
    }
    Ok(())
}

/// Live-type a chunk into the remembered target window without using the clipboard.
/// Only steals focus back to the target if our own window is currently foreground
/// (e.g. recording was started from the tray menu); otherwise it types into whatever
/// already has focus, keeping things snappy with no per-chunk focus flicker.
fn type_unicode_text(
    text: &str,
    owner_hwnd: HWND,
    target_hwnd: HWND,
    delay_ms: u64,
) -> Result<()> {
    if text.trim().is_empty() {
        return Ok(());
    }
    unsafe {
        let foreground = GetForegroundWindow();
        // Only grab focus back if our own window currently holds it (e.g. recording
        // was started from the tray). Validate the target still exists first so a
        // stale handle can't redirect keystrokes into the wrong window.
        if (foreground == null_mut() || foreground == owner_hwnd)
            && target_hwnd != null_mut()
            && target_hwnd != owner_hwnd
            && IsWindow(target_hwnd) != 0
            && SetForegroundWindow(target_hwnd) != 0
        {
            thread::sleep(Duration::from_millis(delay_ms));
        }
        send_unicode(text)
    }
}

fn clear_transcript() {
    if let Some(app) = APP.get() {
        update_ui(app, |ui| {
            ui.transcript.clear();
            ui.status = "Cleared. Press Ctrl+Space to start.".to_string();
        });
    }
}

fn toggle_window_visibility() {
    let Some(app) = APP.get() else {
        return;
    };
    unsafe {
        let hwnd = hwnd_from_app(app);
        if hwnd == null_mut() {
            return;
        }
        let visible = {
            let mut ui = lock_ui(app);
            ui.visible = !ui.visible;
            ui.visible
        };
        ShowWindow(hwnd, if visible { SW_SHOWNA } else { SW_HIDE });
    }
}

// --- Global shortcut (customizable hotkey) ---------------------------------

/// (Re)register the global hotkey on `hwnd`. MOD_NOREPEAT stops held keys from
/// firing repeatedly. Returns false if the combo is reserved / already taken.
fn register_global_hotkey(hwnd: HWND, mods: u32, vk: u32) -> bool {
    if hwnd == null_mut() {
        return false;
    }
    unsafe {
        UnregisterHotKey(hwnd, HOTKEY_ID);
        RegisterHotKey(hwnd, HOTKEY_ID, mods | MOD_NOREPEAT, vk) != 0
    }
}

fn key_is_down(vk: u16) -> bool {
    unsafe { (GetKeyState(vk as i32) as u16 & 0x8000) != 0 }
}

/// Localised name of a single key ("Space", "F8", "A", ...).
fn key_name(vk: u32) -> String {
    unsafe {
        let scan = MapVirtualKeyW(vk, MAPVK_VK_TO_VSC);
        if scan != 0 {
            let lparam = (scan << 16) as i32;
            let mut buf = [0u16; 64];
            let len = GetKeyNameTextW(lparam, buf.as_mut_ptr(), buf.len() as i32);
            if len > 0 {
                return String::from_utf16_lossy(&buf[..len as usize]);
            }
        }
    }
    format!("Key {vk}")
}

/// Human-readable shortcut label like "Ctrl + Shift + Space".
fn hotkey_label(mods: u32, vk: u32) -> String {
    let mut parts: Vec<String> = Vec::new();
    if mods & MOD_CONTROL != 0 {
        parts.push("Ctrl".to_string());
    }
    if mods & MOD_ALT != 0 {
        parts.push("Alt".to_string());
    }
    if mods & MOD_SHIFT != 0 {
        parts.push("Shift".to_string());
    }
    if mods & MOD_WIN != 0 {
        parts.push("Win".to_string());
    }
    parts.push(key_name(vk));
    parts.join(" + ")
}

/// Enter capture mode: free the current hotkey so the next chord arrives as
/// WM_KEYDOWN, and pull keyboard focus to the settings window.
fn start_hotkey_capture(app: &Arc<AppState>) {
    app.capturing_hotkey.store(true, Ordering::SeqCst);
    unsafe {
        let bubble = hwnd_from_app(app);
        if bubble != null_mut() {
            UnregisterHotKey(bubble, HOTKEY_ID);
        }
        let settings = settings_hwnd_from_app(app);
        if settings != null_mut() {
            SetFocus(settings);
        }
    }
    update_ui(app, |ui| {
        ui.status = "Press your shortcut...  (Esc to cancel)".to_string();
    });
}

fn cancel_hotkey_capture(app: &Arc<AppState>) {
    if !app.capturing_hotkey.swap(false, Ordering::SeqCst) {
        return;
    }
    let s = settings_snapshot(app);
    register_global_hotkey(hwnd_from_app(app), s.hotkey_mods, s.hotkey_vk);
    update_ui(app, |ui| {
        ui.status = "Shortcut unchanged.".to_string();
    });
}

fn apply_captured_hotkey(app: &Arc<AppState>, mods: u32, vk: u32) {
    app.capturing_hotkey.store(false, Ordering::SeqCst);
    let bubble = hwnd_from_app(app);
    if register_global_hotkey(bubble, mods, vk) {
        {
            let mut s = app
                .settings
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            s.hotkey_mods = mods;
            s.hotkey_vk = vk;
        }
        save_settings(&settings_snapshot(app));
        let label = hotkey_label(mods, vk);
        update_ui(app, |ui| {
            ui.status = format!("Shortcut set to {label}.");
        });
    } else {
        // Roll back to the previous (still-working) binding.
        let s = settings_snapshot(app);
        register_global_hotkey(bubble, s.hotkey_mods, s.hotkey_vk);
        update_ui(app, |ui| {
            ui.status = "That combo is unavailable - try another.".to_string();
        });
    }
}

/// Handle a key press while capturing a new shortcut.
fn handle_capture_key(app: &Arc<AppState>, vk: u32) {
    let v = vk as u16;
    if v == VK_CONTROL || v == VK_SHIFT || v == VK_MENU || v == VK_LWIN || v == VK_RWIN {
        return; // a bare modifier — keep waiting for the main key
    }
    if v == VK_ESCAPE {
        cancel_hotkey_capture(app);
        return;
    }
    let mut mods = 0u32;
    if key_is_down(VK_CONTROL) {
        mods |= MOD_CONTROL;
    }
    if key_is_down(VK_MENU) {
        mods |= MOD_ALT;
    }
    if key_is_down(VK_SHIFT) {
        mods |= MOD_SHIFT;
    }
    if key_is_down(VK_LWIN) || key_is_down(VK_RWIN) {
        mods |= MOD_WIN;
    }
    // Letters and digits would hijack normal typing without a modifier.
    let needs_mod = (0x30..=0x5A).contains(&vk);
    if mods == 0 && needs_mod {
        update_ui(app, |ui| {
            ui.status = "Add Ctrl, Alt, or Shift to that key.".to_string();
        });
        return; // stay in capture mode
    }
    apply_captured_hotkey(app, mods, vk);
}

fn handle_command(command: usize) {
    match command {
        IDM_START_STOP => toggle_recording(),
        IDM_SHOW_HIDE => toggle_window_visibility(),
        IDM_CLEAR => clear_transcript(),
        IDM_QUIT => unsafe {
            let Some(app) = APP.get() else {
                return;
            };
            let hwnd = hwnd_from_app(app);
            if hwnd != null_mut() {
                windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
            }
        },
        _ => {}
    }
}

fn handle_click(x: i32, y: i32) {
    let _ = (x, y);
    let Some(app) = APP.get() else {
        return;
    };
    if settings_snapshot(app).bubble_click_opens_settings {
        toggle_settings_window();
    } else {
        toggle_recording();
    }
}

fn should_start_drag(hwnd: HWND, x: i32, y: i32) -> bool {
    if is_settings_window(hwnd) {
        y < 84 && !settings_close_rect().contains(x, y)
    } else {
        true
    }
}

fn begin_drag(hwnd: HWND) {
    let Some(app) = APP.get() else {
        return;
    };
    unsafe {
        let mut point: POINT = std::mem::zeroed();
        let mut rect: RECT = std::mem::zeroed();
        if GetCursorPos(&mut point) == 0 || GetWindowRect(hwnd, &mut rect) == 0 {
            return;
        }
        SetCapture(hwnd);
        let mut drag = app
            .drag
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *drag = Some(DragState {
            hwnd: hwnd as isize,
            start_cursor: point,
            start_rect: rect,
            moved: false,
        });
    }
}

fn continue_drag() {
    let Some(app) = APP.get() else {
        return;
    };
    let mut drag = app
        .drag
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(state) = drag.as_mut() else {
        return;
    };

    unsafe {
        let mut point: POINT = std::mem::zeroed();
        if GetCursorPos(&mut point) == 0 {
            return;
        }
        let dx = point.x - state.start_cursor.x;
        let dy = point.y - state.start_cursor.y;
        if dx.abs() > 3 || dy.abs() > 3 {
            state.moved = true;
        }
        SetWindowPos(
            state.hwnd as HWND,
            HWND_TOPMOST,
            state.start_rect.left + dx,
            state.start_rect.top + dy,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOSIZE,
        );
    }
}

fn end_drag() -> bool {
    let Some(app) = APP.get() else {
        return true;
    };
    unsafe {
        ReleaseCapture();
    }
    let mut drag = app
        .drag
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let was_click = drag.as_ref().map(|state| !state.moved).unwrap_or(true);
    *drag = None;
    was_click
}

fn handle_settings_click(x: i32, y: i32) {
    let Some(app) = APP.get() else {
        return;
    };

    // Shortcut rebind button: click to begin capture, click again to cancel.
    if settings_shortcut_rect().contains(x, y) {
        if app.capturing_hotkey.load(Ordering::SeqCst) {
            cancel_hotkey_capture(app);
        } else {
            start_hotkey_capture(app);
        }
        return;
    }
    // Any other click ends an in-progress capture before doing its thing.
    if app.capturing_hotkey.load(Ordering::SeqCst) {
        cancel_hotkey_capture(app);
    }

    if settings_close_rect().contains(x, y) {
        unsafe {
            let hwnd = settings_hwnd_from_app(app);
            if hwnd != null_mut() {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
        return;
    }

    if settings_copy_latest_rect().contains(x, y) {
        if let Some(item) = history_snapshot(app).first() {
            let _ = set_clipboard_text(&item.text, settings_hwnd_from_app(app));
            update_ui(app, |ui| {
                ui.status = "Latest history item copied.".to_string();
            });
        }
        return;
    }

    if settings_clear_history_rect().contains(x, y) {
        clear_history(app);
        return;
    }

    for id in [
        SETTINGS_TOGGLE_STARTUP,
        SETTINGS_TOGGLE_PRELOAD,
        SETTINGS_TOGGLE_LIVE_TYPE,
        SETTINGS_TOGGLE_FINAL_CLIPBOARD,
        SETTINGS_TOGGLE_FINAL_PASTE,
        SETTINGS_TOGGLE_SOUNDS,
        SETTINGS_TOGGLE_WAVEFORM,
        SETTINGS_TOGGLE_BUBBLE_CLICK_SETTINGS,
        SETTINGS_TOGGLE_FLOATING_BUBBLE,
        SETTINGS_TOGGLE_TRAY_WAVEFORM,
    ] {
        if settings_toggle_rect(id).contains(x, y) {
            toggle_setting(app, id);
            return;
        }
    }
}

fn update_ui<F>(app: &Arc<AppState>, update: F)
where
    F: FnOnce(&mut UiState),
{
    {
        let mut ui = lock_ui(app);
        update(&mut ui);
    }
    unsafe {
        let hwnd = hwnd_from_app(app);
        if hwnd != null_mut() {
            PostMessageW(hwnd, WM_UI_UPDATE, 0, 0);
        }
        let settings_hwnd = settings_hwnd_from_app(app);
        if settings_hwnd != null_mut() {
            PostMessageW(settings_hwnd, WM_UI_UPDATE, 0, 0);
        }
    }
}

/// Called from the audio thread with the latest instantaneous bar height (0..1).
/// Smooths it with a fast attack / slow release so the meter feels responsive but
/// never jittery, then stashes it for the UI-thread animation timer to sample.
/// Does no GDI work — drawing only happens on the UI thread.
fn report_level(app: &Arc<AppState>, target: f32) {
    let target = target.clamp(0.0, 1.0);
    let mut ui = lock_ui(app);
    let prev = ui.current_level;
    let smoothed = if target > prev {
        prev * 0.4 + target * 0.6
    } else {
        prev * 0.78 + target * 0.22
    };
    ui.current_level = smoothed.clamp(0.0, 1.0);
}

/// UI-thread tick: scroll the rolling waveform buffer by one sample of the current
/// level. When idle the level decays smoothly to zero so the bars settle instead of
/// freezing. Returns the peak bar height so callers can decide whether to keep
/// animating. Runs at the timer rate, which decouples scroll speed from the mic
/// callback rate for a steady, even flow.
fn advance_waveform(app: &Arc<AppState>) -> f32 {
    let mut ui = lock_ui(app);
    if !ui.recording {
        ui.current_level *= 0.80;
        if ui.current_level < 0.001 {
            ui.current_level = 0.0;
        }
    }
    let level = ui.current_level;
    ui.waveform.push(level);
    while ui.waveform.len() > WAVE_BARS {
        ui.waveform.remove(0);
    }
    ui.waveform.iter().copied().fold(level, f32::max)
}

fn lock_ui(app: &Arc<AppState>) -> std::sync::MutexGuard<'_, UiState> {
    app.ui.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

// ===========================================================================
// Software canvas
//
// A top-down 32-bit DIB section we can both poke pixels into directly (for
// anti-aliased vector shapes computed with signed-distance fields) and hand to
// GDI (for ClearType text). Pixels are stored premultiplied as 0xAARRGGBB so the
// buffer can be fed straight to UpdateLayeredWindow for a smooth, shaped, shadowed
// floating bubble — no jagged window region, no GDI blockiness.
// ===========================================================================

struct Canvas {
    w: i32,
    h: i32,
    hdc: HDC,
    bitmap: HBITMAP,
    old_bitmap: isize,
    bits: *mut u32,
}

impl Canvas {
    unsafe fn new(w: i32, h: i32) -> Option<Canvas> {
        if w <= 0 || h <= 0 {
            return None;
        }
        let screen = GetDC(null_mut());
        let hdc = CreateCompatibleDC(screen);
        ReleaseDC(null_mut(), screen);
        if hdc == null_mut() {
            return None;
        }
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = w;
        bmi.bmiHeader.biHeight = -h; // negative => top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB as u32;
        let mut bits: *mut c_void = null_mut();
        let bitmap = CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if bitmap == null_mut() || bits == null_mut() {
            DeleteDC(hdc);
            return None;
        }
        let old_bitmap = SelectObject(hdc, bitmap as _) as isize;
        Some(Canvas {
            w,
            h,
            hdc,
            bitmap,
            old_bitmap,
            bits: bits as *mut u32,
        })
    }

    fn pixels(&mut self) -> &mut [u32] {
        unsafe { std::slice::from_raw_parts_mut(self.bits, (self.w * self.h) as usize) }
    }

    fn clear_transparent(&mut self) {
        for p in self.pixels() {
            *p = 0;
        }
    }

    /// Push the buffer to a per-pixel-alpha layered window. Position is left
    /// unchanged (so dragging via SetWindowPos keeps working).
    unsafe fn present_layered(&self, hwnd: HWND) {
        GdiFlush();
        let screen = GetDC(null_mut());
        let size = SIZE {
            cx: self.w,
            cy: self.h,
        };
        let src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        UpdateLayeredWindow(
            hwnd,
            screen,
            null(),
            &size,
            self.hdc,
            &src,
            0,
            &blend,
            ULW_ALPHA,
        );
        if screen != null_mut() {
            ReleaseDC(null_mut(), screen);
        }
    }

    /// Copy the buffer to a destination DC (opaque double-buffer blit).
    unsafe fn blit_to(&self, dst: HDC) {
        GdiFlush();
        BitBlt(dst, 0, 0, self.w, self.h, self.hdc, 0, 0, SRCCOPY);
    }
}

impl Drop for Canvas {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.hdc, self.old_bitmap as _);
            DeleteObject(self.bitmap as _);
            DeleteDC(self.hdc);
        }
    }
}

type Rgb = (u8, u8, u8);

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Premultiplied source-over blend of a straight-alpha colour onto one pixel.
#[inline]
fn blend_px(buf: &mut [u32], idx: usize, r: f32, g: f32, b: f32, a: f32) {
    if a <= 0.0 {
        return;
    }
    let a = a.min(1.0);
    let d = buf[idx];
    let da = ((d >> 24) & 0xff) as f32;
    let dr = ((d >> 16) & 0xff) as f32;
    let dg = ((d >> 8) & 0xff) as f32;
    let db = (d & 0xff) as f32;
    let inv = 1.0 - a;
    let or = r * a + dr * inv;
    let og = g * a + dg * inv;
    let ob = b * a + db * inv;
    let oa = a * 255.0 + da * inv;
    buf[idx] = ((oa as u32) << 24) | ((or as u32) << 16) | ((og as u32) << 8) | (ob as u32);
}

/// Signed distance from a point to a rounded rectangle (negative inside).
#[inline]
fn sd_round_rect(px: f32, py: f32, cx: f32, cy: f32, hx: f32, hy: f32, r: f32) -> f32 {
    let qx = (px - cx).abs() - (hx - r);
    let qy = (py - cy).abs() - (hy - r);
    let ax = qx.max(0.0);
    let ay = qy.max(0.0);
    (ax * ax + ay * ay).sqrt() + qx.max(qy).min(0.0) - r
}

/// Anti-aliased rounded rectangle with an optional vertical colour gradient.
#[allow(clippy::too_many_arguments)]
fn fill_round_rect(
    buf: &mut [u32],
    cw: i32,
    ch: i32,
    x: f32,
    y: f32,
    rw: f32,
    rh: f32,
    radius: f32,
    top: Rgb,
    bot: Rgb,
    alpha: f32,
) {
    if rw <= 0.0 || rh <= 0.0 {
        return;
    }
    let cx = x + rw / 2.0;
    let cy = y + rh / 2.0;
    let hx = rw / 2.0;
    let hy = rh / 2.0;
    let r = radius.min(hx).min(hy).max(0.0);
    let x0 = (x.floor() as i32 - 1).max(0);
    let y0 = (y.floor() as i32 - 1).max(0);
    let x1 = ((x + rw).ceil() as i32 + 1).min(cw);
    let y1 = ((y + rh).ceil() as i32 + 1).min(ch);
    for py in y0..y1 {
        let fy = py as f32 + 0.5;
        let t = ((fy - y) / rh).clamp(0.0, 1.0);
        let cr = lerp(top.0 as f32, bot.0 as f32, t);
        let cg = lerp(top.1 as f32, bot.1 as f32, t);
        let cb = lerp(top.2 as f32, bot.2 as f32, t);
        let row = (py * cw) as usize;
        for px in x0..x1 {
            let fx = px as f32 + 0.5;
            let d = sd_round_rect(fx, fy, cx, cy, hx, hy, r);
            let cov = (0.5 - d).clamp(0.0, 1.0);
            if cov > 0.0 {
                blend_px(buf, row + px as usize, cr, cg, cb, cov * alpha);
            }
        }
    }
}

/// Soft drop shadow: a rounded-rect silhouette whose alpha falls off over `blur`.
#[allow(clippy::too_many_arguments)]
fn fill_shadow(
    buf: &mut [u32],
    cw: i32,
    ch: i32,
    x: f32,
    y: f32,
    rw: f32,
    rh: f32,
    radius: f32,
    blur: f32,
    max_alpha: f32,
) {
    let cx = x + rw / 2.0;
    let cy = y + rh / 2.0;
    let hx = rw / 2.0;
    let hy = rh / 2.0;
    let r = radius.min(hx).min(hy).max(0.0);
    let x0 = ((x - blur).floor() as i32).max(0);
    let y0 = ((y - blur).floor() as i32).max(0);
    let x1 = ((x + rw + blur).ceil() as i32).min(cw);
    let y1 = ((y + rh + blur).ceil() as i32).min(ch);
    for py in y0..y1 {
        let fy = py as f32 + 0.5;
        let row = (py * cw) as usize;
        for px in x0..x1 {
            let fx = px as f32 + 0.5;
            let d = sd_round_rect(fx, fy, cx, cy, hx, hy, r).max(0.0);
            let cov = (1.0 - d / blur).clamp(0.0, 1.0);
            if cov > 0.0 {
                // ease the falloff so the shadow looks soft, not linear
                let a = cov * cov * max_alpha;
                blend_px(buf, row + px as usize, 0.0, 0.0, 0.0, a);
            }
        }
    }
}

/// Render the floating bubble into a layered window: drop shadow, glassy pill,
/// status dot, and the mirrored anti-aliased waveform.
unsafe fn render_bubble(hwnd: HWND) {
    if hwnd == null_mut() {
        return;
    }
    let Some(app) = APP.get() else {
        return;
    };
    let (waveform, recording, wave_enabled) = {
        let ui = lock_ui(app);
        let settings = settings_snapshot(app);
        (ui.waveform.clone(), ui.recording, settings.waveform_enabled)
    };

    let Some(mut canvas) = Canvas::new(WIDTH, HEIGHT) else {
        return;
    };
    canvas.clear_transparent();
    {
        let buf = canvas.pixels();
        paint_bubble_into(buf, WIDTH, HEIGHT, 0.0, 0.0, &waveform, recording, wave_enabled);
    }
    canvas.present_layered(hwnd);
}

/// Draw the bubble (shadow, glassy pill, status dot, waveform) into a premultiplied
/// pixel buffer. The bubble's margin box is placed at (ox, oy) so it can be composited
/// anywhere — used both by the live layered window and the asset exporter.
#[allow(clippy::too_many_arguments)]
fn paint_bubble_into(
    buf: &mut [u32],
    cw: i32,
    ch: i32,
    ox: f32,
    oy: f32,
    waveform: &[f32],
    recording: bool,
    wave_enabled: bool,
) {
    let px = ox + BUBBLE_MARGIN as f32;
    let py = oy + BUBBLE_MARGIN as f32;
    let pw = PILL_W as f32;
    let ph = PILL_H as f32;
    let radius = ph / 2.0;

    // soft drop shadow, nudged down a touch
    fill_shadow(buf, cw, ch, px - 1.0, py + 4.0, pw + 2.0, ph + 2.0, radius, 13.0, 0.45);

    // glassy pill body with a subtle top-to-bottom gradient
    fill_round_rect(buf, cw, ch, px, py, pw, ph, radius, (46, 51, 62), (26, 29, 36), 0.98);
    // faint inner highlight along the top edge
    fill_round_rect(
        buf,
        cw,
        ch,
        px + 2.0,
        py + 1.5,
        pw - 4.0,
        ph * 0.5,
        radius,
        (255, 255, 255),
        (255, 255, 255),
        0.05,
    );

    // status dot on the left
    let dot_r = 4.5;
    let dot_cx = px + 15.0;
    let dot_cy = py + ph / 2.0;
    let (dot_col, dot_a): (Rgb, f32) = if recording {
        ((255, 92, 92), 0.95)
    } else {
        ((104, 112, 126), 0.7)
    };
    fill_round_rect(
        buf,
        cw,
        ch,
        dot_cx - dot_r,
        dot_cy - dot_r,
        dot_r * 2.0,
        dot_r * 2.0,
        dot_r,
        dot_col,
        dot_col,
        dot_a,
    );

    // waveform fills the remainder of the pill
    let active = recording && wave_enabled;
    draw_wave_bars(buf, cw, ch, waveform, active, dot_cx + dot_r, px + pw, py, ph);
}

/// Mirrored, rounded, anti-aliased waveform bars centred in the pill.
#[allow(clippy::too_many_arguments)]
fn draw_wave_bars(
    buf: &mut [u32],
    cw: i32,
    ch: i32,
    wave: &[f32],
    active: bool,
    left_edge: f32,
    pill_right: f32,
    py: f32,
    ph: f32,
) {
    let n = wave.len().max(1);
    let area_left = left_edge + 6.0;
    let area_right = pill_right - 12.0;
    let area_w = (area_right - area_left).max(1.0);
    let pitch = area_w / n as f32;
    let bw = (pitch * 0.55).clamp(2.0, 4.0);
    let cy = py + ph / 2.0;
    let max_half = ph / 2.0 - 8.0;
    let min_half = 1.3;
    let (top, bot): (Rgb, Rgb) = if active {
        ((150, 192, 255), (86, 148, 250))
    } else {
        ((78, 86, 100), (60, 66, 78))
    };
    let alpha = if active { 0.97 } else { 0.5 };
    for (i, &lvl) in wave.iter().enumerate() {
        let v = if active { lvl.clamp(0.0, 1.0) } else { 0.0 };
        let half = (v * max_half).max(min_half);
        let cx = area_left + pitch * (i as f32 + 0.5);
        let x = cx - bw / 2.0;
        fill_round_rect(buf, cw, ch, x, cy - half, bw, half * 2.0, bw / 2.0, top, bot, alpha);
    }
}

unsafe fn paint(hwnd: HWND) {
    // Bubble content is delivered by UpdateLayeredWindow, not GDI. Validate the
    // paint cycle and push a fresh frame so it stays correct after restores.
    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    BeginPaint(hwnd, &mut ps);
    EndPaint(hwnd, &ps);
    render_bubble(hwnd);
}

static ANIM_TICK: AtomicUsize = AtomicUsize::new(0);
static TRAY_ANIMATING: AtomicBool = AtomicBool::new(false);

/// 30 fps UI-thread heartbeat: scroll the waveform, repaint the bubble when there
/// is motion, and pulse the tray icon while recording. Keeps all GDI on the UI
/// thread; the audio thread only feeds levels.
unsafe fn on_anim_tick(hwnd: HWND) {
    let Some(app) = APP.get() else {
        return;
    };
    let peak = advance_waveform(app);
    let settings = settings_snapshot(app);
    let recording = lock_ui(app).recording;

    if settings.show_floating_bubble && IsWindowVisible(hwnd) != 0 && (recording || peak > 0.004) {
        render_bubble(hwnd);
    }

    let tick = ANIM_TICK.fetch_add(1, Ordering::Relaxed);
    if recording && settings.tray_waveform_enabled {
        if tick % 3 == 0 {
            let level = lock_ui(app).current_level;
            update_tray_icon(hwnd, make_tray_icon(level, true));
        }
        TRAY_ANIMATING.store(true, Ordering::Relaxed);
    } else if TRAY_ANIMATING.swap(false, Ordering::Relaxed) {
        // Recording just ended (or animation was switched off): settle to idle.
        update_tray_icon(hwnd, make_tray_icon(0.0, false));
    }
}

struct SettingRow {
    id: usize,
    title: &'static str,
    detail: &'static str,
    enabled: bool,
}

fn setting_rows(settings: &AppSettings) -> Vec<SettingRow> {
    vec![
        SettingRow {
            id: SETTINGS_TOGGLE_STARTUP,
            title: "Start with Windows",
            detail: "Launch automatically after sign-in.",
            enabled: settings.start_with_windows,
        },
        SettingRow {
            id: SETTINGS_TOGGLE_PRELOAD,
            title: "Preload Nemotron",
            detail: "Use RAM so the first dictation starts faster.",
            enabled: settings.preload_model,
        },
        SettingRow {
            id: SETTINGS_TOGGLE_LIVE_TYPE,
            title: "Type live into cursor",
            detail: "Type words as you speak (no clipboard).",
            enabled: settings.live_type_into_cursor,
        },
        SettingRow {
            id: SETTINGS_TOGGLE_FINAL_CLIPBOARD,
            title: "Copy final to clipboard",
            detail: "Copy the full transcript once on stop.",
            enabled: settings.copy_final_to_clipboard,
        },
        SettingRow {
            id: SETTINGS_TOGGLE_FINAL_PASTE,
            title: "Paste final on stop",
            detail: "Off by default to avoid duplicates.",
            enabled: settings.paste_final_on_stop,
        },
        SettingRow {
            id: SETTINGS_TOGGLE_SOUNDS,
            title: "Soft sound cues",
            detail: "Short low-volume start and stop chimes.",
            enabled: settings.sounds_enabled,
        },
        SettingRow {
            id: SETTINGS_TOGGLE_WAVEFORM,
            title: "Waveform animation",
            detail: "Show live mic levels while recording.",
            enabled: settings.waveform_enabled,
        },
        SettingRow {
            id: SETTINGS_TOGGLE_BUBBLE_CLICK_SETTINGS,
            title: "Bubble click opens settings",
            detail: "Off makes a bubble click toggle recording.",
            enabled: settings.bubble_click_opens_settings,
        },
        SettingRow {
            id: SETTINGS_TOGGLE_FLOATING_BUBBLE,
            title: "Show floating bubble",
            detail: "Off keeps control in the tray only.",
            enabled: settings.show_floating_bubble,
        },
        SettingRow {
            id: SETTINGS_TOGGLE_TRAY_WAVEFORM,
            title: "Animate the tray icon",
            detail: "Pulse the taskbar tray icon while speaking.",
            enabled: settings.tray_waveform_enabled,
        },
    ]
}

unsafe fn paint_settings(hwnd: HWND) {
    let Some(app) = APP.get() else {
        return;
    };
    let settings = settings_snapshot(app);

    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    if hdc == null_mut() {
        return;
    }
    let capturing = app.capturing_hotkey.load(Ordering::SeqCst);
    if let Some(mut canvas) = Canvas::new(SETTINGS_WIDTH, SETTINGS_HEIGHT) {
        // history = None: the live EDIT control paints the recent-dictations panel.
        render_settings_canvas(&mut canvas, &settings, None, capturing);
        canvas.blit_to(hdc);
    }
    EndPaint(hwnd, &ps);
}

/// Render the entire settings surface into a canvas: background, cards, toggle
/// switches, buttons and ClearType text. When `history` is Some, the recent-dictations
/// panel (normally the live EDIT control) is drawn too — used by the asset exporter.
unsafe fn render_settings_canvas(
    canvas: &mut Canvas,
    settings: &AppSettings,
    history: Option<&[HistoryItem]>,
    capturing: bool,
) {
    let rows = setting_rows(settings);
    let cw = canvas.w;
    let ch = canvas.h;
    let card_top = 102.0;
    let card_h = ch as f32 - card_top - 20.0;
    let right_card_x = 384.0;

    // history panel region (matches create_history_edit)
    let ex = 400;
    let ey = 182;
    let ew = SETTINGS_WIDTH - 436;
    let eh = SETTINGS_HEIGHT - 206;

    // -------- pass 1: anti-aliased shapes straight into the pixel buffer --------
    {
        let buf = canvas.pixels();
        fill_round_rect(buf, cw, ch, 0.0, 0.0, cw as f32, ch as f32, 0.0, (25, 27, 33), (16, 17, 21), 1.0);
        fill_round_rect(buf, cw, ch, 28.0, 26.0, 4.0, 32.0, 2.0, (110, 162, 255), (84, 138, 250), 1.0);
        fill_round_rect(buf, cw, ch, 20.0, card_top, 352.0, card_h, 16.0, (30, 33, 41), (25, 28, 35), 1.0);
        fill_round_rect(
            buf,
            cw,
            ch,
            right_card_x,
            card_top,
            cw as f32 - right_card_x - 20.0,
            card_h,
            16.0,
            (30, 33, 41),
            (25, 28, 35),
            1.0,
        );

        draw_button_bg(buf, cw, ch, settings_close_rect(), false);
        draw_button_bg(buf, cw, ch, settings_copy_latest_rect(), true);
        draw_button_bg(buf, cw, ch, settings_clear_history_rect(), false);
        // shortcut rebind button (accent while capturing)
        draw_button_bg(buf, cw, ch, settings_shortcut_rect(), capturing);

        for row in &rows {
            draw_toggle(buf, cw, ch, settings_toggle_rect(row.id), row.enabled);
        }

        if history.is_some() {
            // history panel background + a thin modern scrollbar (mirrors the live look)
            fill_round_rect(buf, cw, ch, ex as f32, ey as f32, ew as f32, eh as f32, 8.0, (33, 37, 46), (30, 34, 42), 1.0);
            let sb_x = (ex + ew - 8) as f32;
            fill_round_rect(buf, cw, ch, sb_x, (ey + 8) as f32, 4.0, (eh - 16) as f32, 2.0, (45, 50, 61), (45, 50, 61), 1.0);
            fill_round_rect(buf, cw, ch, sb_x, (ey + 12) as f32, 4.0, eh as f32 * 0.4, 2.0, (92, 100, 116), (92, 100, 116), 1.0);
        }
    }

    // -------- pass 2: ClearType text on top via GDI --------
    let dc = canvas.hdc;
    SetBkMode(dc, TRANSPARENT as i32);

    SelectObject(dc, title_font() as _);
    SetTextColor(dc, rgb(237, 241, 247));
    text_left(dc, "Nemotron Bubble", 42, 22, 320, 34);

    SelectObject(dc, small_font() as _);
    SetTextColor(dc, rgb(140, 150, 164));
    text_left(dc, "Settings & recent dictations", 42, 54, 320, 20);

    SelectObject(dc, ui_font() as _);
    SetTextColor(dc, rgb(210, 216, 226));
    text_center(dc, "Close", settings_close_rect());

    SelectObject(dc, small_font() as _);
    SetTextColor(dc, rgb(122, 132, 147));
    text_left(dc, "BEHAVIOR", 34, 82, 200, 18);
    text_left(dc, "HISTORY", right_card_x as i32 + 16, 82, 200, 18);

    // shortcut row
    SelectObject(dc, ui_font() as _);
    SetTextColor(dc, rgb(233, 238, 244));
    text_left(dc, "Global shortcut", 40, 115, 168, 20);
    SelectObject(dc, small_font() as _);
    SetTextColor(dc, rgb(124, 134, 148));
    text_left(dc, "Click, then press your keys.", 40, 135, 168, 18);
    SelectObject(dc, ui_font() as _);
    SetTextColor(dc, rgb(245, 248, 252));
    let shortcut_label = if capturing {
        "Press keys...".to_string()
    } else {
        hotkey_label(settings.hotkey_mods, settings.hotkey_vk)
    };
    text_center(dc, &shortcut_label, settings_shortcut_rect());

    SetTextColor(dc, rgb(245, 248, 252));
    text_center(dc, "Copy Latest", settings_copy_latest_rect());
    SetTextColor(dc, rgb(214, 220, 230));
    text_center(dc, "Clear", settings_clear_history_rect());

    SetTextColor(dc, rgb(126, 136, 150));
    text_left(
        dc,
        "Select a line and copy with Ctrl+C.",
        right_card_x as i32 + 16,
        158,
        cw - right_card_x as i32 - 36,
        18,
    );

    for row in &rows {
        let rect = settings_toggle_rect(row.id);
        SelectObject(dc, ui_font() as _);
        SetTextColor(dc, rgb(233, 238, 244));
        text_left(dc, row.title, rect.left + 8, rect.top + 1, 232, 20);
        SelectObject(dc, small_font() as _);
        SetTextColor(dc, rgb(124, 134, 148));
        text_left(dc, row.detail, rect.left + 8, rect.top + 21, 248, 18);
    }

    if let Some(items) = history {
        let mut y = ey + 12;
        let bottom = ey + eh - 12;
        let text_w = ew - 30;
        for item in items {
            if y > bottom - 16 {
                break;
            }
            SelectObject(dc, small_font() as _);
            SetTextColor(dc, rgb(140, 152, 170));
            text_left(dc, &item.timestamp, ex + 14, y, text_w, 16);
            y += 18;
            SelectObject(dc, ui_font() as _);
            SetTextColor(dc, rgb(216, 223, 233));
            let lines = (item.text.chars().count() / 32 + 1).min(3) as i32;
            let h = lines * 19;
            let mut r = RECT {
                left: ex + 14,
                top: y,
                right: ex + 14 + text_w,
                bottom: (y + h).min(bottom),
            };
            let wide = wide_null(&item.text);
            DrawTextW(dc, wide.as_ptr(), -1, &mut r, DT_LEFT | DT_TOP | DT_WORDBREAK | DT_NOPREFIX);
            y += h + 12;
        }
    }
}

/// Anti-aliased toggle switch (track + knob) inside a settings row.
fn draw_toggle(buf: &mut [u32], cw: i32, ch: i32, row: RectI, enabled: bool) {
    let w = 46.0;
    let h = 24.0;
    let x = (row.right - 50) as f32;
    let y = (row.top + 10) as f32;
    let (track_top, track_bot): (Rgb, Rgb) = if enabled {
        ((92, 152, 252), (58, 118, 240))
    } else {
        ((66, 72, 84), (52, 57, 67))
    };
    fill_round_rect(buf, cw, ch, x, y, w, h, h / 2.0, track_top, track_bot, 1.0);

    let kr = 9.0;
    let ky = y + h / 2.0;
    let kx = if enabled { x + w - kr - 3.0 } else { x + kr + 3.0 };
    // subtle knob shadow then the knob
    fill_round_rect(buf, cw, ch, kx - kr, ky - kr + 1.0, kr * 2.0, kr * 2.0, kr, (0, 0, 0), (0, 0, 0), 0.20);
    fill_round_rect(buf, cw, ch, kx - kr, ky - kr, kr * 2.0, kr * 2.0, kr, (255, 255, 255), (236, 240, 247), 1.0);
}

/// Anti-aliased button background. Primary buttons get the accent gradient.
fn draw_button_bg(buf: &mut [u32], cw: i32, ch: i32, rect: RectI, primary: bool) {
    let x = rect.left as f32;
    let y = rect.top as f32;
    let w = (rect.right - rect.left) as f32;
    let h = (rect.bottom - rect.top) as f32;
    let (top, bot): (Rgb, Rgb) = if primary {
        ((98, 152, 250), (62, 116, 238))
    } else {
        ((60, 66, 78), (47, 52, 62))
    };
    fill_round_rect(buf, cw, ch, x, y, w, h, 13.0, top, bot, 1.0);
}

unsafe fn text_left(dc: HDC, text: &str, x: i32, y: i32, w: i32, h: i32) {
    let mut rect = RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    };
    let wide = wide_null(text);
    DrawTextW(
        dc,
        wide.as_ptr(),
        -1,
        &mut rect,
        DT_LEFT | DT_TOP | DT_WORDBREAK | DT_NOPREFIX,
    );
}

unsafe fn text_center(dc: HDC, text: &str, rect: RectI) {
    let mut r = RECT {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    };
    let wide = wide_null(text);
    DrawTextW(
        dc,
        wide.as_ptr(),
        -1,
        &mut r,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
    );
}

unsafe fn place_window(hwnd: HWND) {
    let screen_w = GetSystemMetrics(SM_CXSCREEN);
    let screen_h = GetSystemMetrics(SM_CYSCREEN);
    let x = (screen_w - WIDTH - 24).max(0);
    let y = (screen_h - HEIGHT - 88).max(0);
    // Position only — visibility is controlled separately so the first frame is
    // rendered (via UpdateLayeredWindow) before the window is shown.
    SetWindowPos(hwnd, HWND_TOPMOST, x, y, WIDTH, HEIGHT, SWP_NOACTIVATE);
}

unsafe fn place_settings_window(hwnd: HWND) {
    let screen_w = GetSystemMetrics(SM_CXSCREEN);
    let screen_h = GetSystemMetrics(SM_CYSCREEN);
    let x = (screen_w - SETTINGS_WIDTH - 32).max(0);
    let y = ((screen_h - SETTINGS_HEIGHT) / 2).max(0);
    SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        x,
        y,
        SETTINGS_WIDTH,
        SETTINGS_HEIGHT,
        SWP_NOACTIVATE,
    );
}

unsafe fn apply_settings_rounding(hwnd: HWND) {
    let region = CreateRoundRectRgn(0, 0, SETTINGS_WIDTH + 1, SETTINGS_HEIGHT + 1, 18, 18);
    if region != null_mut() {
        SetWindowRgn(hwnd, region, 1);
    }
}

/// Open the settings window if hidden, close it if already showing.
fn toggle_settings_window() {
    let Some(app) = APP.get() else {
        return;
    };
    unsafe {
        let hwnd = settings_hwnd_from_app(app);
        if hwnd == null_mut() {
            return;
        }
        if IsWindowVisible(hwnd) != 0 {
            cancel_hotkey_capture(app);
            ShowWindow(hwnd, SW_HIDE);
        } else {
            place_settings_window(hwnd);
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
        }
    }
}

fn is_settings_window(hwnd: HWND) -> bool {
    APP.get()
        .map(|app| hwnd == settings_hwnd_from_app(app))
        .unwrap_or(false)
}

unsafe fn add_tray_icon(hwnd: HWND) {
    // Track only the icon we own. If creation fails, fall back to the shared system
    // icon for display but store 0 so it is never passed to DestroyIcon.
    let icon = make_tray_icon(0.0, false);
    let shown = if icon == null_mut() {
        LoadIconW(null_mut(), IDI_APPLICATION)
    } else {
        icon
    };
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ID;
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_TRAYICON;
    nid.hIcon = shown;
    fill_wide_fixed(&mut nid.szTip, "Nemotron Bubble");
    Shell_NotifyIconW(NIM_ADD, &mut nid);
    CURRENT_TRAY_ICON.store(icon as isize, Ordering::SeqCst);
}

unsafe fn remove_tray_icon(hwnd: HWND) {
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ID;
    Shell_NotifyIconW(NIM_DELETE, &mut nid);
    let prev = CURRENT_TRAY_ICON.swap(0, Ordering::SeqCst);
    if prev != 0 {
        DestroyIcon(prev as HICON);
    }
}

unsafe fn show_tray_menu(hwnd: HWND) {
    let menu = CreatePopupMenu();
    if menu == null_mut() {
        return;
    }

    let recording = APP
        .get()
        .map(|app| app.recording.load(Ordering::SeqCst))
        .unwrap_or(false);

    let start_text = wide_null(if recording { "Stop" } else { "Start" });
    let show_text = wide_null("Show / Hide Bubble");
    let clear_text = wide_null("Clear");
    let quit_text = wide_null("Quit");

    AppendMenuW(menu, MF_STRING, IDM_START_STOP, start_text.as_ptr());
    AppendMenuW(menu, MF_STRING, IDM_SHOW_HIDE, show_text.as_ptr());
    AppendMenuW(menu, MF_STRING, IDM_CLEAR, clear_text.as_ptr());
    AppendMenuW(menu, MF_SEPARATOR, 0, null());
    AppendMenuW(menu, MF_STRING, IDM_QUIT, quit_text.as_ptr());

    let mut point: POINT = std::mem::zeroed();
    GetCursorPos(&mut point);
    SetForegroundWindow(hwnd);
    TrackPopupMenu(menu, TPM_RIGHTBUTTON, point.x, point.y, 0, hwnd, null());
    DestroyMenu(menu);
}

fn settings_close_rect() -> RectI {
    RectI {
        left: SETTINGS_WIDTH - 96,
        top: 24,
        right: SETTINGS_WIDTH - 24,
        bottom: 56,
    }
}

fn settings_copy_latest_rect() -> RectI {
    RectI {
        left: 400,
        top: 114,
        right: 512,
        bottom: 146,
    }
}

fn settings_clear_history_rect() -> RectI {
    RectI {
        left: 524,
        top: 114,
        right: 600,
        bottom: 146,
    }
}

/// The clickable shortcut-rebind button at the top of the Behavior card.
fn settings_shortcut_rect() -> RectI {
    RectI {
        left: 208,
        top: 116,
        right: 360,
        bottom: 150,
    }
}

fn settings_toggle_rect(id: usize) -> RectI {
    let index = match id {
        SETTINGS_TOGGLE_STARTUP => 0,
        SETTINGS_TOGGLE_PRELOAD => 1,
        SETTINGS_TOGGLE_LIVE_TYPE => 2,
        SETTINGS_TOGGLE_FINAL_CLIPBOARD => 3,
        SETTINGS_TOGGLE_FINAL_PASTE => 4,
        SETTINGS_TOGGLE_SOUNDS => 5,
        SETTINGS_TOGGLE_WAVEFORM => 6,
        SETTINGS_TOGGLE_BUBBLE_CLICK_SETTINGS => 7,
        SETTINGS_TOGGLE_FLOATING_BUBBLE => 8,
        SETTINGS_TOGGLE_TRAY_WAVEFORM => 9,
        _ => 0,
    };
    // Toggles begin below the shortcut row.
    let top = 170 + index as i32 * 49;
    RectI {
        left: 32,
        top,
        right: 360,
        bottom: top + 44,
    }
}

fn wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn create_font(height: i32, weight: i32) -> isize {
    let face = wide_null("Segoe UI");
    CreateFontW(
        height,
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        OUT_DEFAULT_PRECIS as u32,
        CLIP_DEFAULT_PRECIS as u32,
        CLEARTYPE_QUALITY as u32,
        (DEFAULT_PITCH | FF_DONTCARE) as u32,
        face.as_ptr(),
    ) as isize
}

fn ui_font() -> isize {
    *UI_FONT.get_or_init(|| unsafe { create_font(-15, FW_NORMAL as i32) })
}

fn title_font() -> isize {
    *TITLE_FONT.get_or_init(|| unsafe { create_font(-25, FW_SEMIBOLD as i32) })
}

fn small_font() -> isize {
    *SMALL_FONT.get_or_init(|| unsafe { create_font(-12, FW_NORMAL as i32) })
}

fn edit_bg_brush() -> isize {
    *EDIT_BG_BRUSH.get_or_init(|| unsafe { CreateSolidBrush(rgb(33, 37, 46)) as isize })
}

/// Build a 32x32 tray icon. While recording the equalizer bars scale with `level`
/// and a small red dot appears; idle shows a static dim glyph. The caller owns the
/// returned HICON and must DestroyIcon it when replaced.
unsafe fn make_tray_icon(level: f32, recording: bool) -> HICON {
    const SIZE: i32 = 32;
    let mut pixels = vec![0u32; (SIZE * SIZE) as usize];
    let center = (SIZE as f32 - 1.0) / 2.0;
    let bg = if recording { 0xFF20242E } else { 0xFF1A1D21 };
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            if (dx * dx + dy * dy).sqrt() <= 14.5 {
                pixels[(y * SIZE + x) as usize] = bg;
            }
        }
    }

    // Equalizer bars. The shape array gives a pleasant resting silhouette; while
    // recording it is modulated by the live level so the icon "breathes".
    let shape = [0.30f32, 0.58, 0.86, 1.0, 0.72, 0.46, 0.66, 0.9, 0.5];
    let lvl = level.clamp(0.0, 1.0);
    let accent = if recording { 0xFF7AB8FF } else { 0xFF59616F };
    for (i, &s) in shape.iter().enumerate() {
        let frac = if recording {
            (0.18 + s * lvl * 0.95).min(1.0)
        } else {
            s * 0.5
        };
        let bar_h = (frac * 22.0) as i32;
        let x = 7 + i as i32 * 2;
        let top = 16 - bar_h / 2;
        let bottom = 16 + bar_h / 2;
        for yy in top..=bottom {
            for xx in x..=x + 1 {
                if (0..SIZE).contains(&xx) && (0..SIZE).contains(&yy) {
                    pixels[(yy * SIZE + xx) as usize] = accent;
                }
            }
        }
    }

    if recording {
        for y in 3..9 {
            for x in 23..29 {
                let dx = x as f32 - 25.5;
                let dy = y as f32 - 5.5;
                if dx * dx + dy * dy <= 6.0 {
                    pixels[(y * SIZE + x) as usize] = 0xFFFF5C5C;
                }
            }
        }
    }

    let color = CreateBitmap(SIZE, SIZE, 1, 32, pixels.as_ptr() as _);
    let mask_bits = vec![0u8; ((SIZE * SIZE) / 8) as usize];
    let mask = CreateBitmap(SIZE, SIZE, 1, 1, mask_bits.as_ptr() as _);
    let info = ICONINFO {
        fIcon: 1,
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: mask,
        hbmColor: color,
    };
    let icon = CreateIconIndirect(&info);
    if color != null_mut() {
        DeleteObject(color as _);
    }
    if mask != null_mut() {
        DeleteObject(mask as _);
    }
    // Return null on failure (never a shared system icon): update_tray_icon only ever
    // DestroyIcon's handles it owns, so a system icon must never enter that path.
    icon
}

/// Swap the live tray icon, destroying the one it replaces.
unsafe fn update_tray_icon(hwnd: HWND, icon: HICON) {
    if hwnd == null_mut() || icon == null_mut() {
        return;
    }
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ID;
    nid.uFlags = NIF_ICON;
    nid.hIcon = icon;
    Shell_NotifyIconW(NIM_MODIFY, &mut nid);
    let prev = CURRENT_TRAY_ICON.swap(icon as isize, Ordering::SeqCst);
    if prev != 0 && prev != icon as isize {
        DestroyIcon(prev as HICON);
    }
}

/// Reset the tray icon to its idle glyph.
fn refresh_tray_idle(app: &Arc<AppState>) {
    unsafe {
        let hwnd = hwnd_from_app(app);
        if hwnd != null_mut() {
            update_tray_icon(hwnd, make_tray_icon(0.0, false));
        }
    }
}

fn rgb(red: u8, green: u8, blue: u8) -> u32 {
    red as u32 | ((green as u32) << 8) | ((blue as u32) << 16)
}

fn fill_wide_fixed<const N: usize>(target: &mut [u16; N], text: &str) {
    target.fill(0);
    for (slot, value) in target
        .iter_mut()
        .take(N.saturating_sub(1))
        .zip(text.encode_utf16())
    {
        *slot = value;
    }
}

fn lparam_point(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam & 0xffff) as i16 as i32;
    let y = ((lparam >> 16) & 0xffff) as i16 as i32;
    (x, y)
}

fn loword(value: usize) -> u16 {
    (value & 0xffff) as u16
}

fn hwnd_from_app(app: &Arc<AppState>) -> HWND {
    app.hwnd.load(Ordering::SeqCst) as HWND
}

fn target_hwnd_from_app(app: &Arc<AppState>) -> HWND {
    app.target_hwnd.load(Ordering::SeqCst) as HWND
}

fn settings_hwnd_from_app(app: &Arc<AppState>) -> HWND {
    app.settings_hwnd.load(Ordering::SeqCst) as HWND
}

fn history_edit_hwnd_from_app(app: &Arc<AppState>) -> HWND {
    app.history_edit_hwnd.load(Ordering::SeqCst) as HWND
}

// ===========================================================================
// Asset exporter (feature = "assets")
//
// Reuses the real renderer to emit README art on a clean, generated background:
//   bubble.png   — the floating bubble on transparency (soft shadow preserved)
//   demo.gif     — the waveform pulsing on a tasteful gradient (seamless loop)
//   settings.png — the settings window with demo data and a modern scrollbar
// Run with:  cargo run --release --features assets -- --render-assets docs
// ===========================================================================

#[cfg(feature = "assets")]
fn render_assets(out_dir: &str) -> Result<()> {
    use std::path::Path;
    let dir = Path::new(out_dir);
    std::fs::create_dir_all(dir)?;

    let bars = WAVE_BARS;
    let pi = std::f32::consts::PI;
    // A lively per-bar pattern. Integer "speed" per bar keeps the loop seamless.
    let frame_wave = |t: f32, frames: f32| -> Vec<f32> {
        (0..bars)
            .map(|i| {
                let phase = i as f32 * 0.55;
                let speed = 1.0 + (i % 3) as f32; // 1,2,3 -> periodic over [0, frames)
                let hump = (i as f32 / (bars - 1) as f32 * pi).sin(); // taller in the middle
                let env = 0.42 + 0.58 * hump;
                let s = (2.0 * pi * (t / frames) * speed + phase).sin();
                (env * (0.30 + 0.70 * (0.5 + 0.5 * s))).clamp(0.06, 1.0)
            })
            .collect()
    };

    // ---- bubble.png : transparent, 2x upscaled ----
    {
        let wave = frame_wave(9.0, 48.0);
        let mut buf = vec![0u32; (WIDTH * HEIGHT) as usize];
        paint_bubble_into(&mut buf, WIDTH, HEIGHT, 0.0, 0.0, &wave, true, true);
        let (big, bw, bh) = upscale_premul(&buf, WIDTH as usize, HEIGHT as usize, 2);
        let rgba: Vec<u8> = big.iter().flat_map(|&p| unpremultiply(p)).collect();
        write_png(&dir.join("bubble.png"), bw as u32, bh as u32, &rgba)?;
    }

    // ---- demo.gif : bubble pulsing on a gradient ----
    {
        const HERO_W: usize = 480;
        const HERO_H: usize = 220;
        const FRAMES: usize = 48;
        let scale = 2usize;
        let bg = gradient_bg(HERO_W, HERO_H);
        let bw = WIDTH as usize * scale;
        let bh = HEIGHT as usize * scale;
        let ox = (HERO_W - bw) / 2;
        let oy = (HERO_H - bh) / 2;
        let mut frames: Vec<Vec<u32>> = Vec::with_capacity(FRAMES);
        for t in 0..FRAMES {
            let wave = frame_wave(t as f32, FRAMES as f32);
            let mut buf = vec![0u32; (WIDTH * HEIGHT) as usize];
            paint_bubble_into(&mut buf, WIDTH, HEIGHT, 0.0, 0.0, &wave, true, true);
            let (big, _, _) = upscale_premul(&buf, WIDTH as usize, HEIGHT as usize, scale);
            let mut frame = bg.clone();
            composite_over(&mut frame, HERO_W, HERO_H, &big, bw, bh, ox, oy);
            frames.push(frame);
        }
        write_gif(&dir.join("demo.gif"), HERO_W, HERO_H, &frames, 3)?;
    }

    // ---- settings.png : settings window with demo data ----
    {
        let settings = AppSettings::default();
        let history = demo_history();
        let cw = SETTINGS_WIDTH;
        let ch = SETTINGS_HEIGHT;
        if let Some(mut canvas) = unsafe { Canvas::new(cw, ch) } {
            unsafe { render_settings_canvas(&mut canvas, &settings, Some(&history), false) };
            let rgba = settings_to_rgba(canvas.pixels(), cw as usize, ch as usize, 16.0);
            write_png(&dir.join("settings.png"), cw as u32, ch as u32, &rgba)?;
        }
    }

    println!("Wrote bubble.png, demo.gif, settings.png to {out_dir}");
    Ok(())
}

#[cfg(feature = "assets")]
fn demo_history() -> Vec<HistoryItem> {
    let make = |timestamp: &str, text: &str| HistoryItem {
        timestamp: timestamp.to_string(),
        text: text.to_string(),
    };
    vec![
        make("Today, 2:14 PM", "Let's ship the new dictation bubble — it looks fantastic."),
        make("Today, 2:11 PM", "Meeting notes: confirm the roadmap and follow up with the design team tomorrow."),
        make("Today, 1:58 PM", "Reminder to refactor the audio capture pipeline next sprint."),
        make("Today, 1:42 PM", "The quick brown fox jumps over the lazy dog."),
        make("Yesterday, 6:30 PM", "Draft the release notes and update the screenshots in the README."),
    ]
}

#[cfg(feature = "assets")]
fn unpremultiply(p: u32) -> [u8; 4] {
    let a = ((p >> 24) & 0xff) as u32;
    if a == 0 {
        return [0, 0, 0, 0];
    }
    let un = |c: u32| (((c & 0xff) * 255 + a / 2) / a).min(255) as u8;
    [un(p >> 16), un(p >> 8), un(p), a as u8]
}

#[cfg(feature = "assets")]
fn lerp_argb(a: u32, b: u32, t: f32) -> u32 {
    let mix = |sh: u32| {
        let ca = ((a >> sh) & 0xff) as f32;
        let cb = ((b >> sh) & 0xff) as f32;
        (((ca + (cb - ca) * t).round() as u32).min(255)) << sh
    };
    mix(24) | mix(16) | mix(8) | mix(0)
}

/// Bilinear upscale of a premultiplied ARGB buffer (interpolating alpha too).
#[cfg(feature = "assets")]
fn upscale_premul(src: &[u32], sw: usize, sh: usize, scale: usize) -> (Vec<u32>, usize, usize) {
    let dw = sw * scale;
    let dh = sh * scale;
    let mut dst = vec![0u32; dw * dh];
    for dy in 0..dh {
        let fy = (dy as f32 + 0.5) / scale as f32 - 0.5;
        let y0 = fy.floor().max(0.0) as usize;
        let y1 = (y0 + 1).min(sh - 1);
        let ty = (fy - y0 as f32).clamp(0.0, 1.0);
        for dx in 0..dw {
            let fx = (dx as f32 + 0.5) / scale as f32 - 0.5;
            let x0 = fx.floor().max(0.0) as usize;
            let x1 = (x0 + 1).min(sw - 1);
            let tx = (fx - x0 as f32).clamp(0.0, 1.0);
            let top = lerp_argb(src[y0 * sw + x0], src[y0 * sw + x1], tx);
            let bot = lerp_argb(src[y1 * sw + x0], src[y1 * sw + x1], tx);
            dst[dy * dw + dx] = lerp_argb(top, bot, ty);
        }
    }
    (dst, dw, dh)
}

/// A tasteful opaque gradient backdrop with a soft blue glow near the top-centre.
#[cfg(feature = "assets")]
fn gradient_bg(w: usize, h: usize) -> Vec<u32> {
    let mut v = vec![0u32; w * h];
    for y in 0..h {
        let t = y as f32 / (h - 1).max(1) as f32;
        for x in 0..w {
            let base_r = lerp(34.0, 14.0, t);
            let base_g = lerp(38.0, 16.0, t);
            let base_b = lerp(56.0, 23.0, t);
            let dx = (x as f32 - w as f32 * 0.5) / (w as f32 * 0.6);
            let dy = (y as f32 - h as f32 * 0.30) / (h as f32 * 0.6);
            let glow = (1.0 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0).powf(2.0) * 24.0;
            let r = (base_r + glow * 0.5).min(255.0) as u32;
            let g = (base_g + glow * 0.8).min(255.0) as u32;
            let b = (base_b + glow * 1.5).min(255.0) as u32;
            v[y * w + x] = 0xFF00_0000 | (r << 16) | (g << 8) | b;
        }
    }
    v
}

/// Composite a premultiplied ARGB sprite over an opaque background buffer.
#[cfg(feature = "assets")]
#[allow(clippy::too_many_arguments)]
fn composite_over(
    bg: &mut [u32],
    bgw: usize,
    bgh: usize,
    fg: &[u32],
    fw: usize,
    fh: usize,
    ox: usize,
    oy: usize,
) {
    for y in 0..fh {
        let by = oy + y;
        if by >= bgh {
            break;
        }
        for x in 0..fw {
            let bx = ox + x;
            if bx >= bgw {
                continue;
            }
            let s = fg[y * fw + x];
            let sa = ((s >> 24) & 0xff) as f32 / 255.0;
            if sa <= 0.0 {
                continue;
            }
            let inv = 1.0 - sa;
            let d = bg[by * bgw + bx];
            let r = (((s >> 16) & 0xff) as f32 + ((d >> 16) & 0xff) as f32 * inv).min(255.0) as u32;
            let g = (((s >> 8) & 0xff) as f32 + ((d >> 8) & 0xff) as f32 * inv).min(255.0) as u32;
            let b = ((s & 0xff) as f32 + (d & 0xff) as f32 * inv).min(255.0) as u32;
            bg[by * bgw + bx] = 0xFF00_0000 | (r << 16) | (g << 8) | b;
        }
    }
}

/// Convert an opaque settings canvas to RGBA: GDI text leaves the alpha byte
/// undefined, so force alpha opaque and apply a rounded-corner mask for polish.
#[cfg(feature = "assets")]
fn settings_to_rgba(buf: &[u32], w: usize, h: usize, radius: f32) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 4];
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
    let hx = w as f32 / 2.0;
    let hy = h as f32 / 2.0;
    for y in 0..h {
        for x in 0..w {
            let p = buf[y * w + x];
            let d = sd_round_rect(x as f32 + 0.5, y as f32 + 0.5, cx, cy, hx, hy, radius);
            let cov = (0.5 - d).clamp(0.0, 1.0);
            let i = (y * w + x) * 4;
            out[i] = ((p >> 16) & 0xff) as u8;
            out[i + 1] = ((p >> 8) & 0xff) as u8;
            out[i + 2] = (p & 0xff) as u8;
            out[i + 3] = (cov * 255.0) as u8;
        }
    }
    out
}

#[cfg(feature = "assets")]
fn write_png(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) -> Result<()> {
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(())
}

#[cfg(feature = "assets")]
fn write_gif(
    path: &std::path::Path,
    w: usize,
    h: usize,
    frames: &[Vec<u32>],
    delay_cs: u16,
) -> Result<()> {
    let mut file = std::fs::File::create(path)?;
    let mut encoder = gif::Encoder::new(&mut file, w as u16, h as u16, &[])?;
    encoder.set_repeat(gif::Repeat::Infinite)?;
    for frame in frames {
        let mut rgba: Vec<u8> = Vec::with_capacity(frame.len() * 4);
        for &p in frame {
            rgba.push(((p >> 16) & 0xff) as u8);
            rgba.push(((p >> 8) & 0xff) as u8);
            rgba.push((p & 0xff) as u8);
            rgba.push(255);
        }
        let mut f = gif::Frame::from_rgba_speed(w as u16, h as u16, &mut rgba, 10);
        f.delay = delay_cs;
        encoder.write_frame(&f)?;
    }
    Ok(())
}
