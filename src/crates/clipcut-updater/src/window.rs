//! The window and the work behind it. Plain Win32 and GDI, drawn by hand in
//! the colours of ClipCut's Midnight look, double-buffered so the moving
//! bar does not flicker.

use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, HWND, LPARAM, LRESULT, RECT, WAIT_OBJECT_0, WPARAM,
};
use windows_sys::Win32::Graphics::Dwm::{
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CLEARTYPE_QUALITY, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW,
    CreateSolidBrush, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DeleteDC, DeleteObject,
    DrawTextW, EndPaint, FillRect, GetStockObject, HDC, InvalidateRect, NULL_PEN, PAINTSTRUCT,
    RoundRect, SRCCOPY, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
};
use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForSystem, SetProcessDpiAwarenessContext,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DI_NORMAL, DefWindowProcW, DestroyWindow,
    DispatchMessageW, DrawIconEx, GetClientRect, GetMessageW, GetSystemMetrics, HTCAPTION,
    IDC_ARROW, IMAGE_ICON, LR_LOADFROMFILE, LoadCursorW, LoadImageW, MSG, PostMessageW,
    PostQuitMessage, RegisterClassExW, SM_CXSCREEN, SM_CYSCREEN, SetForegroundWindow, SetTimer,
    TranslateMessage, WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_NCHITTEST, WM_PAINT, WM_TIMER,
    WNDCLASSEXW, WS_EX_APPWINDOW, WS_POPUP, WS_VISIBLE,
};

/// What the command line asked for.
#[derive(Default)]
struct Job {
    setup: Option<PathBuf>,
    wait: Option<u32>,
    app: Option<PathBuf>,
    version: String,
    demo: bool,
}

fn parse(args: Vec<OsString>) -> Job {
    let mut job = Job::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--setup") => job.setup = args.next().map(PathBuf::from),
            Some("--app") => job.app = args.next().map(PathBuf::from),
            Some("--wait") => {
                job.wait = args.next().and_then(|pid| pid.to_str()?.parse().ok());
            }
            Some("--version") => {
                job.version = args
                    .next()
                    .and_then(|v| v.into_string().ok())
                    .unwrap_or_default();
            }
            Some("--demo") => job.demo = true,
            _ => {}
        }
    }
    job
}

// ── colours: the Midnight look, as COLORREF (0x00BBGGRR) ─────────────────

const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    r as u32 | (g as u32) << 8 | (b as u32) << 16
}
const TOP: (u8, u8, u8) = (0x16, 0x1f, 0x48);
const BOTTOM: (u8, u8, u8) = (0x06, 0x08, 0x12);
const ACCENT: u32 = rgb(0x7a, 0xa2, 0xff);
const TRACK: u32 = rgb(0x1c, 0x24, 0x48);
const TEXT: u32 = rgb(0xf4, 0xf6, 0xff);
const MUTED: u32 = rgb(0x93, 0x9c, 0xc0);

/// The window's size in 96-dpi pixels.
const WIDTH: i32 = 440;
const HEIGHT: i32 = 210;

// ── state the window draws from ──────────────────────────────────────────

/// The heading and the line under it.
static STATUS: Mutex<(String, String)> = Mutex::new((String::new(), String::new()));
/// Whether the bar runs (installing) or sits full (done).
static FULL: AtomicU32 = AtomicU32::new(0);
static SCALE: AtomicU32 = AtomicU32::new(100);
static ICON: AtomicIsize = AtomicIsize::new(0);
static TITLE_FONT: AtomicIsize = AtomicIsize::new(0);
static BODY_FONT: AtomicIsize = AtomicIsize::new(0);
static SMALL_FONT: AtomicIsize = AtomicIsize::new(0);
static STARTED: OnceLock<Instant> = OnceLock::new();

fn say(title: &str, line: &str) {
    *STATUS.lock().unwrap_or_else(|e| e.into_inner()) = (title.to_owned(), line.to_owned());
}

fn px(value: i32) -> i32 {
    value * SCALE.load(Ordering::Relaxed) as i32 / 100
}

fn wide(text: impl AsRef<OsStr>) -> Vec<u16> {
    text.as_ref().encode_wide().chain(Some(0)).collect()
}

pub fn run(args: Vec<OsString>) -> i32 {
    let job = parse(args);
    if job.setup.is_none() && !job.demo {
        return 2;
    }
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        SCALE.store(GetDpiForSystem() * 100 / 96, Ordering::Relaxed);
    }
    let _ = STARTED.set(Instant::now());
    let version = if job.version.is_empty() {
        "the new version".to_owned()
    } else {
        format!("version {}", job.version)
    };
    say("Updating ClipCut", &format!("Installing {version}…"));

    let Some(hwnd) = open_window(job.app.as_deref()) else {
        // No window: install anyway, the old way.
        if let Some(setup) = &job.setup {
            let _ = Command::new(setup).spawn();
        }
        return 1;
    };
    let handle = hwnd as isize;
    std::thread::spawn(move || work(job, version, handle));

    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    0
}

/// Waits for the app, installs, starts the new version, and closes the
/// window. Runs on its own thread; the window keeps animating meanwhile.
fn work(job: Job, version: String, hwnd: isize) {
    let close = || unsafe {
        PostMessageW(hwnd as HWND, WM_CLOSE, 0, 0);
    };
    let wizard = |why: &str| {
        say("Almost there", why);
        std::thread::sleep(Duration::from_millis(1800));
        if let Some(setup) = &job.setup {
            let _ = Command::new(setup).spawn();
        }
    };

    // The app quits once it has saved; its files are in use until then.
    let mut app_gone = true;
    if let Some(pid) = job.wait {
        unsafe {
            let process = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
            if !process.is_null() {
                app_gone = WaitForSingleObject(process, 120_000) == WAIT_OBJECT_0;
                CloseHandle(process);
            }
        }
    }
    if !app_gone {
        wizard("ClipCut is still open. Opening the installer…");
        return close();
    }
    std::thread::sleep(Duration::from_millis(400));

    let installed = match &job.setup {
        None => {
            std::thread::sleep(Duration::from_secs(4));
            true
        }
        Some(setup) => {
            let mut command = Command::new(setup);
            command.args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/SP-"]);
            // An install under Program Files was made for everyone, and the
            // update goes there too; anything else is this user's own.
            if job.app.as_deref().is_some_and(in_program_files) {
                command.arg("/ALLUSERS");
            } else {
                command.arg("/CURRENTUSER");
            }
            command.status().is_ok_and(|status| status.success())
        }
    };
    if !installed {
        wizard("The quiet install did not finish. Opening the installer…");
        return close();
    }

    FULL.store(1, Ordering::Relaxed);
    say(
        "Opening ClipCut",
        &format!("{} is installed.", capitalise(&version)),
    );
    if !job.demo
        && let Some(app) = &job.app
    {
        let _ = Command::new(app).spawn();
    }
    std::thread::sleep(Duration::from_millis(1400));
    close();
}

fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

fn in_program_files(path: &Path) -> bool {
    ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"]
        .into_iter()
        .filter_map(std::env::var_os)
        .any(|root| path.starts_with(root))
}

fn open_window(app: Option<&Path>) -> Option<HWND> {
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        // The app's own icon, which the setup puts beside clipcut.exe.
        let icon_path = app
            .and_then(Path::parent)
            .map(|dir| dir.join("clipcut.ico"));
        let size = px(56);
        let icon = icon_path
            .filter(|path| path.is_file())
            .map(|path| {
                LoadImageW(
                    std::ptr::null_mut(),
                    wide(&path).as_ptr(),
                    IMAGE_ICON,
                    size,
                    size,
                    LR_LOADFROMFILE,
                )
            })
            .unwrap_or(std::ptr::null_mut());
        ICON.store(icon as isize, Ordering::Relaxed);

        let font = |height: i32, weight: i32| {
            CreateFontW(
                -px(height),
                0,
                0,
                0,
                weight,
                0,
                0,
                0,
                1,
                0,
                0,
                CLEARTYPE_QUALITY as u32,
                0,
                wide("Segoe UI").as_ptr(),
            ) as isize
        };
        TITLE_FONT.store(font(22, 600), Ordering::Relaxed);
        BODY_FONT.store(font(15, 400), Ordering::Relaxed);
        SMALL_FONT.store(font(13, 400), Ordering::Relaxed);

        let class = wide("ClipCutUpdater");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: icon,
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
            hIconSm: icon,
        };
        if RegisterClassExW(&wc) == 0 {
            return None;
        }
        let (w, h) = (px(WIDTH), px(HEIGHT));
        let x = (GetSystemMetrics(SM_CXSCREEN) - w) / 2;
        let y = (GetSystemMetrics(SM_CYSCREEN) - h) / 2;
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW,
            class.as_ptr(),
            wide("Updating ClipCut").as_ptr(),
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            w,
            h,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return None;
        }
        // Rounded corners on Windows 11; ignored on 10.
        let round = DWMWCP_ROUND;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            (&round as *const i32).cast(),
            4,
        );
        SetForegroundWindow(hwnd);
        SetTimer(hwnd, 1, 16, None);
        Some(hwnd)
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_TIMER => {
                InvalidateRect(hwnd, std::ptr::null(), 0);
                0
            }
            WM_ERASEBKGND => 1,
            // The whole window drags, as a title bar would.
            WM_NCHITTEST => HTCAPTION as LRESULT,
            WM_PAINT => {
                paint(hwnd);
                0
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe fn paint(hwnd: HWND) {
    unsafe {
        let mut ps: PAINTSTRUCT = std::mem::zeroed();
        let screen = BeginPaint(hwnd, &mut ps);
        let mut client: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut client);
        let (w, h) = (client.right, client.bottom);
        let dc = CreateCompatibleDC(screen);
        let bitmap = CreateCompatibleBitmap(screen, w, h);
        let old_bitmap = SelectObject(dc, bitmap);

        draw(dc, w, h);

        BitBlt(screen, 0, 0, w, h, dc, 0, 0, SRCCOPY);
        SelectObject(dc, old_bitmap);
        DeleteObject(bitmap);
        DeleteDC(dc);
        EndPaint(hwnd, &ps);
    }
}

unsafe fn fill(dc: HDC, left: i32, top: i32, right: i32, bottom: i32, colour: u32) {
    unsafe {
        let brush = CreateSolidBrush(colour);
        FillRect(
            dc,
            &RECT {
                left,
                top,
                right,
                bottom,
            },
            brush,
        );
        DeleteObject(brush);
    }
}

unsafe fn pill(dc: HDC, left: i32, top: i32, right: i32, bottom: i32, colour: u32) {
    unsafe {
        let brush = CreateSolidBrush(colour);
        let old_brush = SelectObject(dc, brush);
        let old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
        let radius = bottom - top;
        RoundRect(dc, left, top, right + 1, bottom + 1, radius, radius);
        SelectObject(dc, old_pen);
        SelectObject(dc, old_brush);
        DeleteObject(brush);
    }
}

unsafe fn text(
    dc: HDC,
    font: &AtomicIsize,
    colour: u32,
    words: &str,
    left: i32,
    top: i32,
    right: i32,
    height: i32,
) {
    unsafe {
        let old = SelectObject(dc, font.load(Ordering::Relaxed) as _);
        SetTextColor(dc, colour);
        let mut rect = RECT {
            left,
            top,
            right,
            bottom: top + height,
        };
        let words: Vec<u16> = words.encode_utf16().collect();
        DrawTextW(
            dc,
            words.as_ptr(),
            words.len() as i32,
            &mut rect,
            DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
        SelectObject(dc, old);
    }
}

unsafe fn draw(dc: HDC, w: i32, h: i32) {
    unsafe {
        // A night-sky fall from deep blue to near black, a few lines a band.
        let band = px(2).max(1);
        let mut y = 0;
        while y < h {
            let t = y as f32 / h as f32;
            let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
            let colour = rgb(
                mix(TOP.0, BOTTOM.0),
                mix(TOP.1, BOTTOM.1),
                mix(TOP.2, BOTTOM.2),
            );
            fill(dc, 0, y, w, y + band, colour);
            y += band;
        }
        SetBkMode(dc, TRANSPARENT as i32);

        let margin = px(30);
        let icon = ICON.load(Ordering::Relaxed);
        let mut text_left = margin;
        if icon != 0 {
            DrawIconEx(
                dc,
                margin,
                px(30),
                icon as _,
                px(56),
                px(56),
                0,
                std::ptr::null_mut(),
                DI_NORMAL,
            );
            text_left = margin + px(56) + px(18);
        }
        let (title, line) = STATUS.lock().unwrap_or_else(|e| e.into_inner()).clone();
        text(
            dc,
            &TITLE_FONT,
            TEXT,
            &title,
            text_left,
            px(32),
            w - margin,
            px(32),
        );
        text(
            dc,
            &BODY_FONT,
            MUTED,
            &line,
            text_left,
            px(64),
            w - margin,
            px(24),
        );

        // The bar: a light sweeping a track while it installs, full at the end.
        let (left, right) = (margin, w - margin);
        let (top, bottom) = (px(132), px(138));
        pill(dc, left, top, right, bottom, TRACK);
        if FULL.load(Ordering::Relaxed) == 1 {
            pill(dc, left, top, right, bottom, ACCENT);
        } else {
            let span = right - left;
            let segment = span / 3;
            let elapsed = STARTED
                .get()
                .map_or(0.0, |start| start.elapsed().as_secs_f32());
            let phase = (elapsed % 1.4) / 1.4;
            // Eased, so the light speeds through the middle.
            let eased = 0.5 - 0.5 * (std::f32::consts::PI * phase).cos();
            let head = left - segment + ((span + segment) as f32 * eased) as i32;
            let (from, to) = (head.max(left), (head + segment).min(right));
            if to - from > px(6) {
                pill(dc, from, top, to, bottom, ACCENT);
            }
        }

        text(
            dc,
            &SMALL_FONT,
            MUTED,
            "ClipCut opens again by itself when it is done.",
            margin,
            px(156),
            w - margin,
            px(22),
        );
    }
}
