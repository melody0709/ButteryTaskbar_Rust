#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#![allow(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};
use std::ffi::c_void;
use std::env;
use std::fs;
use std::mem::{size_of, zeroed};
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Mutex, OnceLock};
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
use winreg::RegKey;
use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, PROPERTYKEY, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTOPRIMARY};
use windows_sys::Win32::System::Com::{CoInitializeEx, CoTaskMemAlloc, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows_sys::Win32::System::Com::StructuredStorage::{PROPVARIANT, PropVariantClear};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
use windows_sys::Win32::System::Variant::VT_LPWSTR;
use windows_sys::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows_sys::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_CONTROL,
    VK_LWIN, VK_RWIN,
};
use windows_sys::Win32::UI::Shell::{
    SetCurrentProcessExplicitAppUserModelID, Shell_NotifyIconW, ShellExecuteW, APPBARDATA, NIF_ICON,
    NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETFOCUS, NIM_SETVERSION, NIN_SELECT,
    NOTIFYICONDATAW, ABM_GETSTATE, ABM_SETSTATE, ABS_AUTOHIDE,
};
use windows_sys::Win32::UI::Shell::PropertiesSystem::{PSGetPropertyKeyFromName, SHGetPropertyStoreForWindow};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CallNextHookEx, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, FindWindowExW, FindWindowW, GetClassNameW, GetCursorPos, GetForegroundWindow, GetMessageW,
    EVENT_SYSTEM_FOREGROUND, HHOOK, ICON_BIG, ICON_SMALL, IDC_ARROW, IDI_APPLICATION, IMAGE_ICON,
    IsWindowVisible,
    KBDLLHOOKSTRUCT, LR_DEFAULTSIZE, LR_SHARED, LoadCursorW, LoadIconW, LoadImageW, MF_CHECKED,
    MF_GRAYED, MF_SEPARATOR, MF_STRING, MF_UNCHECKED, MSLLHOOKSTRUCT, MSG, PostMessageW, PostQuitMessage,
    RegisterClassExW, RegisterWindowMessageW, SendMessageW, SetForegroundWindow, SetWindowsHookExW, ShowWindow,
    TPM_BOTTOMALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTALIGN, TPM_RIGHTBUTTON, TrackPopupMenu,
    TranslateMessage, UnhookWindowsHookEx, WNDCLASSEXW, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_APP, WM_CONTEXTMENU,
    WM_CREATE, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NULL, WM_PAINT, WM_RBUTTONDOWN,
    WM_RBUTTONUP, WM_SETICON, WM_SYSKEYDOWN, WM_SYSKEYUP, SW_HIDE, SW_SHOWNOACTIVATE, SW_SHOWNORMAL,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_HOTKEY,
    DialogBoxIndirectParamW, DLGTEMPLATE, DS_CENTER, DS_MODALFRAME, DS_SETFONT, EndDialog,
    GetDlgItem, MapDialogRect, WS_CAPTION, WS_CHILD, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
    GWLP_USERDATA, MB_ICONWARNING, MB_OK, MessageBoxW, GetClientRect, GetWindowRect,
    MoveWindow, WM_CLOSE, WM_COMMAND, WM_GETFONT, WM_INITDIALOG, WM_NCCREATE, WM_NCDESTROY,
    SetWindowLongPtrW, GetWindowLongPtrW, WM_SETFOCUS, WM_KILLFOCUS,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, MapVirtualKeyW, MAPVK_VK_TO_CHAR,
    VK_BACK, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F11, VK_HOME, VK_INSERT, VK_LCONTROL, VK_LEFT, VK_LMENU,
    VK_LSHIFT, VK_MENU, VK_NEXT, VK_PRIOR, VK_RCONTROL, VK_RIGHT, VK_RMENU, VK_RSHIFT, VK_SHIFT, VK_SPACE,
    VK_TAB, VK_UP, VK_RETURN, VK_F1, VK_F24,
};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect, FrameRect, SetBkColor,
    SetTextColor, COLOR_WINDOW, COLOR_WINDOWFRAME, COLOR_WINDOWTEXT, DT_LEFT, DT_SINGLELINE, DT_VCENTER,
    PAINTSTRUCT, GetSysColor,
};
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey};

const APP_NAME: &str = "Buttery Taskbar";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const APP_USER_MODEL_ID: &str = "melody0709.ButteryTaskbar_Rust";
const RELEASES_URL: &str = "https://github.com/melody0709/ButteryTaskbar_Rust/releases";
const WINDOW_CLASS_NAME: &str = "BUTTERY_TASKBAR_RS";
const TRAY_CALLBACK_MESSAGE: u32 = WM_APP + 1;
const TRAY_ICON_ID: u32 = 1;
const APP_ICON_RESOURCE_ID: u16 = 32512;
const NOTIFYICON_VERSION_4_VALUE: u32 = 4;
const NINF_KEY: u32 = 1;
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;
const IID_PROPERTY_STORE: GUID = GUID::from_u128(0x886d8eeb_8cf2_4446_8d02_cdba1dbdcf99);

const CMD_TOGGLE_ENABLED: usize = 1001;
const CMD_SETTINGS: usize = 1002;
const CMD_TOGGLE_SCROLL: usize = 1003;
const CMD_TOGGLE_AUTOHIDE: usize = 1004;
const CMD_TOGGLE_STARTUP: usize = 1005;
const CMD_OPEN_RELEASES: usize = 1006;
const CMD_QUIT: usize = 1999;

const WM_SETFONT: u32 = 0x0030;
const ERROR_HOTKEY_ALREADY_REGISTERED: u32 = 1409;

const HOTKEY_ID_TOGGLE: i32 = 1;

static APP: OnceLock<AppState> = OnceLock::new();

#[repr(C)]
struct IUnknownVtable {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
}

#[repr(C)]
struct IPropertyStoreVtable {
    base__: IUnknownVtable,
    get_count: unsafe extern "system" fn(*mut c_void, *mut u32) -> i32,
    get_at: unsafe extern "system" fn(*mut c_void, u32, *mut PROPERTYKEY) -> i32,
    get_value: unsafe extern "system" fn(*mut c_void, *const PROPERTYKEY, *mut PROPVARIANT) -> i32,
    set_value: unsafe extern "system" fn(*mut c_void, *const PROPERTYKEY, *const PROPVARIANT) -> i32,
    commit: unsafe extern "system" fn(*mut c_void) -> i32,
}

#[repr(C)]
struct IPropertyStoreRaw {
    lp_vtbl: *const IPropertyStoreVtable,
}

struct ComGuard {
    should_uninitialize: bool,
}

impl ComGuard {
    fn initialize() -> Self {
        let hr = unsafe { CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32) };
        Self {
            should_uninitialize: hr >= 0,
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.should_uninitialize {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

struct WideStringPropVariant(PROPVARIANT);

impl WideStringPropVariant {
    unsafe fn new(value: &str) -> Option<Self> {
        let wide = to_wide(value);
        let bytes = wide.len().checked_mul(size_of::<u16>())?;
        let ptr = CoTaskMemAlloc(bytes) as *mut u16;
        if ptr.is_null() {
            return None;
        }

        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());

        let mut prop_variant: PROPVARIANT = zeroed();
        prop_variant.Anonymous.Anonymous.vt = VT_LPWSTR;
        prop_variant.Anonymous.Anonymous.Anonymous.pwszVal = ptr;
        Some(Self(prop_variant))
    }
}

impl Drop for WideStringPropVariant {
    fn drop(&mut self) {
        unsafe {
            let _ = PropVariantClear(&mut self.0);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HotkeyConfig {
    pub win: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: u32,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            win: false,
            ctrl: false,
            shift: false,
            alt: false,
            key: 0,
        }
    }
}

impl HotkeyConfig {
    pub fn is_empty(&self) -> bool {
        self.key == 0
    }

    pub fn default_toggle() -> Self {
        Self {
            win: true,
            ctrl: true,
            shift: false,
            alt: false,
            key: VK_F11 as u32,
        }
    }

    pub fn modifiers(&self) -> u32 {
        let mut mod_flags = 0;
        if self.win {
            mod_flags |= windows_sys::Win32::UI::Input::KeyboardAndMouse::MOD_WIN;
        }
        if self.ctrl {
            mod_flags |= windows_sys::Win32::UI::Input::KeyboardAndMouse::MOD_CONTROL;
        }
        if self.shift {
            mod_flags |= windows_sys::Win32::UI::Input::KeyboardAndMouse::MOD_SHIFT;
        }
        if self.alt {
            mod_flags |= windows_sys::Win32::UI::Input::KeyboardAndMouse::MOD_ALT;
        }
        // Always append MOD_NOREPEAT to prevent holding down from triggering repeatedly
        mod_flags |= windows_sys::Win32::UI::Input::KeyboardAndMouse::MOD_NOREPEAT;
        mod_flags
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    version: u32,
    enabled: bool,
    scroll_activation_enabled: bool,
    #[serde(default)]
    toggle_shortcut: HotkeyConfig,
    auto_launch_enabled: bool,
    autohide_when_disabled: bool,
}

impl Config {
    fn default_with_system_state() -> Self {
        Self {
            version: 1,
            enabled: true,
            scroll_activation_enabled: true,
            toggle_shortcut: HotkeyConfig::default_toggle(),
            auto_launch_enabled: false,
            autohide_when_disabled: system_taskbar_autohide_enabled(),
        }
    }
}

enum TaskbarSignal {
    Refresh,
    Exit,
}

struct AppState {
    config: Mutex<Config>,
    config_path: PathBuf,
    quoted_exe_path: String,
    main_hwnd: AtomicIsize,
    keyboard_hook: Mutex<isize>,
    mouse_hook: Mutex<isize>,
    foreground_hook: Mutex<isize>,
    tray_uses_version_4: AtomicBool,
    menu_active: AtomicBool,
    is_finalizing: AtomicBool,
    should_show_due_to_focus: AtomicBool,
    should_stay_visible_before: AtomicU64,
    is_win_key_down: AtomicBool,
    win_key_press_requested: AtomicBool,
    taskbar_created_message: u32,
    taskbar_tx: SyncSender<TaskbarSignal>,
}

fn main() {
    let _com_guard = ComGuard::initialize();

    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        SetCurrentProcessExplicitAppUserModelID(to_wide(APP_USER_MODEL_ID).as_ptr());
    }

    let quoted_exe_path = quoted_exe_path();
    let config_path = config_path();
    let mut config = load_config(&config_path);
    config.auto_launch_enabled = query_auto_launch_enabled(&quoted_exe_path);

    let taskbar_created_message = unsafe { RegisterWindowMessageW(to_wide("TaskbarCreated").as_ptr()) };
    let (taskbar_tx, taskbar_rx) = sync_channel(1);

    let _ = APP.set(AppState {
        config: Mutex::new(config),
        config_path,
        quoted_exe_path,
        main_hwnd: AtomicIsize::new(0),
        keyboard_hook: Mutex::new(0),
        mouse_hook: Mutex::new(0),
        foreground_hook: Mutex::new(0),
        tray_uses_version_4: AtomicBool::new(false),
        menu_active: AtomicBool::new(false),
        is_finalizing: AtomicBool::new(false),
        should_show_due_to_focus: AtomicBool::new(true),
        should_stay_visible_before: AtomicU64::new(0),
        is_win_key_down: AtomicBool::new(false),
        win_key_press_requested: AtomicBool::new(false),
        taskbar_created_message,
        taskbar_tx,
    });

    std::thread::spawn(move || taskbar_worker(taskbar_rx));

    let hwnd = unsafe { create_hidden_window() };
    if hwnd.is_null() {
        return;
    }

    app().main_hwnd.store(hwnd as isize, Ordering::SeqCst);
    install_foreground_hook();
    refresh_foreground_state();
    update_hooks();
    signal_taskbar_refresh();

    unsafe {
        let mut msg: MSG = zeroed();
        loop {
            let result = GetMessageW(&mut msg, null_mut(), 0, 0);
            if result <= 0 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn app() -> &'static AppState {
    APP.get().expect("application state should be initialised")
}

unsafe fn create_hidden_window() -> HWND {
    let class_name = to_wide(WINDOW_CLASS_NAME);
    let window_name = to_wide(APP_NAME);
    let app_icon = load_app_icon();

    let mut wc: WNDCLASSEXW = zeroed();
    wc.cbSize = size_of::<WNDCLASSEXW>() as u32;
    wc.lpfnWndProc = Some(window_proc);
    wc.hInstance = GetModuleHandleW(null());
    wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
    wc.hIcon = app_icon;
    wc.hIconSm = app_icon;
    wc.lpszClassName = class_name.as_ptr();

    if RegisterClassExW(&wc) == 0 {
        return null_mut();
    }

    let hwnd = CreateWindowExW(
        0,
        class_name.as_ptr(),
        window_name.as_ptr(),
        0,
        0,
        0,
        0,
        0,
        null_mut(),
        null_mut(),
        GetModuleHandleW(null()),
        null_mut(),
    );

    if !hwnd.is_null() {
        apply_window_identity(hwnd, app_icon);
    }

    hwnd
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            add_notification_icon(hwnd);
            register_hotkeys(hwnd);
            0
        }
        WM_DESTROY => {
            unsafe { UnregisterHotKey(hwnd, HOTKEY_ID_TOGGLE); }
            cleanup_on_destroy(hwnd);
            PostQuitMessage(0);
            0
        }
        _ if msg == TRAY_CALLBACK_MESSAGE => {
            handle_tray_callback(hwnd, wparam, lparam);
            0
        }
        _ if msg == WM_APP + 2 => {
            HOTKEY_EDIT_CLASS_REGISTERED.get_or_init(|| register_hotkey_edit_class());

            let mut state = HotkeyEditState {
                hotkey: current_config().toggle_shortcut,
                original: HotkeyConfig::default(),
                capturing: false,
            };

            let template = create_settings_dialog_template();

            DialogBoxIndirectParamW(
                GetModuleHandleW(null()),
                template.as_ptr() as *const DLGTEMPLATE,
                hwnd,
                Some(settings_dialog_proc),
                &mut state as *mut _ as LPARAM,
            );
            0
        }
        _ if msg == app().taskbar_created_message => {
            add_notification_icon(hwnd);
            0
        }
        WM_HOTKEY => {
            if wparam == HOTKEY_ID_TOGGLE as usize {
                let new_value = {
                    let mut config = lock_config();
                    config.enabled = !config.enabled;
                    save_config(&config);
                    config.enabled
                };
                if new_value {
                    refresh_foreground_state();
                } else {
                    app().should_show_due_to_focus.store(true, Ordering::SeqCst);
                }
                update_hooks();
                signal_taskbar_refresh();
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn register_hotkeys(hwnd: HWND) {
    let config = current_config();
    unsafe {
        UnregisterHotKey(hwnd, HOTKEY_ID_TOGGLE);
        if !config.toggle_shortcut.is_empty() {
            RegisterHotKey(
                hwnd,
                HOTKEY_ID_TOGGLE,
                config.toggle_shortcut.modifiers(),
                config.toggle_shortcut.key,
            );
        }
    }
}

unsafe fn handle_tray_callback(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) {
    let icon_event = if app().tray_uses_version_4.load(Ordering::SeqCst) {
        loword(lparam as u32)
    } else {
        lparam as u32
    };

    match icon_event {
        NIN_SELECT | NIN_KEYSELECT | WM_CONTEXTMENU | WM_RBUTTONUP | WM_RBUTTONDOWN | WM_LBUTTONUP => {
            let mut anchor = cursor_anchor();
            if app().tray_uses_version_4.load(Ordering::SeqCst) {
                anchor = point_from_wparam(wparam);
            }
            show_tray_menu(hwnd, anchor.x, anchor.y);
        }
        _ => {}
    }
}

unsafe fn show_tray_menu(hwnd: HWND, x: i32, y: i32) {
    app().menu_active.store(true, Ordering::SeqCst);
    update_hooks();
    signal_taskbar_refresh();

    let config = current_config();
    let menu = CreatePopupMenu();
    if menu.is_null() {
        app().menu_active.store(false, Ordering::SeqCst);
        update_hooks();
        return;
    }

    let mut labels: Vec<Vec<u16>> = Vec::new();
    append_menu_item(menu, &mut labels, CMD_TOGGLE_ENABLED, "Enabled", config.enabled);
    append_menu_text(menu, &mut labels, CMD_SETTINGS, "Settings...");
    append_menu_item(
        menu,
        &mut labels,
        CMD_TOGGLE_SCROLL,
        "Scroll to open Start",
        config.scroll_activation_enabled,
    );
    append_menu_item(
        menu,
        &mut labels,
        CMD_TOGGLE_AUTOHIDE,
        "Keep auto-hide when disabled",
        config.autohide_when_disabled,
    );
    append_menu_item(
        menu,
        &mut labels,
        CMD_TOGGLE_STARTUP,
        "Start at log-in (non-admin)",
        config.auto_launch_enabled,
    );
    AppendMenuW(menu, MF_SEPARATOR, 0, null());
    append_menu_text(menu, &mut labels, CMD_OPEN_RELEASES, "Open releases page");
    AppendMenuW(menu, MF_SEPARATOR, 0, null());
    let version_label = format!("buttery-taskbar_v{}", APP_VERSION);
    {
        let wide = to_wide(&version_label);
        AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, wide.as_ptr());
        labels.push(wide);
    }
    append_menu_text(menu, &mut labels, CMD_QUIT, "Quit");

    let rect = monitor_work_area_from_point(x, y);
    let clamped_x = x.clamp(rect.left, rect.right.saturating_sub(1));
    let clamped_y = y.clamp(rect.top, rect.bottom.saturating_sub(1));
    let mut flags = TPM_NONOTIFY | TPM_RETURNCMD | TPM_RIGHTBUTTON;
    if clamped_y >= rect.bottom.saturating_sub(1) {
        flags |= TPM_BOTTOMALIGN;
    }
    if clamped_x >= rect.right.saturating_sub(1) {
        flags |= TPM_RIGHTALIGN;
    }

    SetForegroundWindow(hwnd);
    let command = TrackPopupMenu(
        menu,
        flags,
        clamped_x,
        clamped_y,
        0,
        hwnd,
        null(),
    );

    DestroyMenu(menu);
    tray_notify_focus(hwnd);
    PostMessageW(hwnd, WM_NULL, 0, 0);

    app().menu_active.store(false, Ordering::SeqCst);
    update_hooks();
    refresh_foreground_state();
    signal_taskbar_refresh();

    if command != 0 {
        handle_menu_command(command as usize);
    }
}

unsafe fn append_menu_item(menu: *mut c_void, labels: &mut Vec<Vec<u16>>, command: usize, label: &str, checked: bool) {
    let wide = to_wide(label);
    let flags = MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED };
    AppendMenuW(menu, flags, command, wide.as_ptr());
    labels.push(wide);
}

unsafe fn append_menu_text(menu: *mut c_void, labels: &mut Vec<Vec<u16>>, command: usize, label: &str) {
    let wide = to_wide(label);
    AppendMenuW(menu, MF_STRING, command, wide.as_ptr());
    labels.push(wide);
}

fn handle_menu_command(command: usize) {
    match command {
        CMD_TOGGLE_ENABLED => {
            let new_value = {
                let mut config = lock_config();
                config.enabled = !config.enabled;
                save_config(&config);
                config.enabled
            };
            if new_value {
                refresh_foreground_state();
            } else {
                app().should_show_due_to_focus.store(true, Ordering::SeqCst);
            }
            update_hooks();
            signal_taskbar_refresh();
        }
        CMD_SETTINGS => unsafe {
            show_settings_dialog();
        },
        CMD_TOGGLE_SCROLL => {
            {
                let mut config = lock_config();
                config.scroll_activation_enabled = !config.scroll_activation_enabled;
                save_config(&config);
            }
            update_hooks();
        }
        CMD_TOGGLE_AUTOHIDE => {
            {
                let mut config = lock_config();
                config.autohide_when_disabled = !config.autohide_when_disabled;
                save_config(&config);
            }
            signal_taskbar_refresh();
        }
        CMD_TOGGLE_STARTUP => {
            let desired_enabled = !current_config().auto_launch_enabled;
            if let Some(actual_enabled) = set_auto_launch_enabled(desired_enabled) {
                let mut config = lock_config();
                config.auto_launch_enabled = actual_enabled;
                save_config(&config);
            }
        }
        CMD_OPEN_RELEASES => unsafe {
            open_releases_page();
        },
        CMD_QUIT => request_quit(),
        _ => {}
    }
}

fn request_quit() {
    if app().is_finalizing.swap(true, Ordering::SeqCst) {
        return;
    }

    app().menu_active.store(false, Ordering::SeqCst);
    update_hooks();
    restore_taskbar_now(true);
    let _ = app().taskbar_tx.try_send(TaskbarSignal::Exit);

    let hwnd = app().main_hwnd.load(Ordering::SeqCst) as HWND;
    unsafe {
        if !hwnd.is_null() {
            DestroyWindow(hwnd);
        } else {
            PostQuitMessage(0);
        }
    }
}

unsafe fn cleanup_on_destroy(hwnd: HWND) {
    remove_notification_icon(hwnd);
    uninstall_foreground_hook();
    uninstall_hook(&app().keyboard_hook);
    uninstall_hook(&app().mouse_hook);
    restore_taskbar_now(true);
}

unsafe fn add_notification_icon(hwnd: HWND) {
    let mut nid = base_notify_icon_data(hwnd);
    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP;
    nid.uCallbackMessage = TRAY_CALLBACK_MESSAGE;
    nid.hIcon = load_app_icon();
    fill_wide_buffer(&mut nid.szTip, &format!("{} {}", APP_NAME, APP_VERSION));
    nid.Anonymous.uVersion = NOTIFYICON_VERSION_4_VALUE;

    if Shell_NotifyIconW(NIM_ADD, &nid) != 0 {
        let uses_v4 = Shell_NotifyIconW(NIM_SETVERSION, &nid) != 0;
        app().tray_uses_version_4.store(uses_v4, Ordering::SeqCst);
    } else {
        app().tray_uses_version_4.store(false, Ordering::SeqCst);
    }
}

unsafe fn apply_window_identity(hwnd: HWND, app_icon: *mut c_void) {
    if !app_icon.is_null() {
        SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, app_icon as isize);
        SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, app_icon as isize);
    }

    // Feed shell relaunch metadata so Win11 surfaces the process icon more reliably.
    let mut property_store: *mut c_void = null_mut();
    if SHGetPropertyStoreForWindow(hwnd, &IID_PROPERTY_STORE, &mut property_store) < 0 || property_store.is_null() {
        return;
    }

    let store = property_store as *mut IPropertyStoreRaw;
    let mut did_set_property = false;

    did_set_property |= set_window_property_string(store, "System.AppUserModel.ID", APP_USER_MODEL_ID);
    did_set_property |= set_window_property_string(store, "System.AppUserModel.RelaunchDisplayNameResource", APP_NAME);

    if let Ok(exe_path) = env::current_exe() {
        let exe_path = exe_path.to_string_lossy().into_owned();
        let relaunch_command = format!("\"{}\"", exe_path);
        let relaunch_icon = format!("{},-{}", exe_path, APP_ICON_RESOURCE_ID);
        did_set_property |= set_window_property_string(store, "System.AppUserModel.RelaunchCommand", &relaunch_command);
        did_set_property |= set_window_property_string(
            store,
            "System.AppUserModel.RelaunchIconResource",
            &relaunch_icon,
        );
    }

    if did_set_property {
        ((*(*store).lp_vtbl).commit)(property_store);
    }

    ((*(*store).lp_vtbl).base__.release)(property_store);
}

unsafe fn set_window_property_string(store: *mut IPropertyStoreRaw, property_name: &str, value: &str) -> bool {
    let mut property_key: PROPERTYKEY = zeroed();
    let property_name = to_wide(property_name);
    if PSGetPropertyKeyFromName(property_name.as_ptr(), &mut property_key) < 0 {
        return false;
    }

    let Some(property_value) = WideStringPropVariant::new(value) else {
        return false;
    };

    ((*(*store).lp_vtbl).set_value)(store as *mut c_void, &property_key, &property_value.0) >= 0
}

unsafe fn remove_notification_icon(hwnd: HWND) {
    let nid = base_notify_icon_data(hwnd);
    Shell_NotifyIconW(NIM_DELETE, &nid);
}

unsafe fn tray_notify_focus(hwnd: HWND) {
    let nid = base_notify_icon_data(hwnd);
    Shell_NotifyIconW(NIM_SETFOCUS, &nid);
}

unsafe fn base_notify_icon_data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut nid: NOTIFYICONDATAW = zeroed();
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;
    nid
}

fn install_foreground_hook() {
    let mut hook = lock_handle(&app().foreground_hook);
    if *hook != 0 {
        return;
    }

    unsafe {
        *hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            null_mut(),
            Some(foreground_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        ) as isize;
    }
}

fn uninstall_foreground_hook() {
    let mut hook = lock_handle(&app().foreground_hook);
    if *hook == 0 {
        return;
    }

    unsafe {
        UnhookWinEvent(*hook as HWINEVENTHOOK);
    }
    *hook = 0;
}

unsafe extern "system" fn foreground_event_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    if hwnd.is_null() {
        return;
    }

    refresh_foreground_state_for_window(hwnd);
}

fn refresh_foreground_state() {
    let hwnd = unsafe { GetForegroundWindow() };
    refresh_foreground_state_for_window(hwnd);
}

fn refresh_foreground_state_for_window(hwnd: HWND) {
    let enabled = current_config().enabled;
    let should_show = if app().menu_active.load(Ordering::SeqCst) {
        true
    } else if !enabled {
        true
    } else {
        should_show_taskbar_for_foreground(hwnd)
    };

    app().should_show_due_to_focus.store(should_show, Ordering::SeqCst);
    signal_taskbar_refresh();
}

fn should_show_taskbar_for_foreground(hwnd: HWND) -> bool {
    if hwnd.is_null() {
        return false;
    }

    let class_name = window_class_name(hwnd);
    matches!(
        class_name.as_str(),
        "Windows.UI.Core.CoreWindow"
            | "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "TopLevelWindowForOverflowXamlIsland"
            | "XamlExplorerHostIslandWindow"
            | "NotifyIconOverflowWindow"
    )
}

fn window_class_name(hwnd: HWND) -> String {
    let mut buffer = [0u16; 128];
    let len = unsafe { GetClassNameW(hwnd, buffer.as_mut_ptr(), (buffer.len() - 1) as i32) };
    if len <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..len as usize])
}

fn update_hooks() {
    let config = current_config();
    let menu_active = app().menu_active.load(Ordering::SeqCst);
    let finalizing = app().is_finalizing.load(Ordering::SeqCst);

    let should_hook_keyboard = !finalizing && !menu_active && config.enabled;
    let should_hook_mouse = !finalizing && !menu_active && config.scroll_activation_enabled;

    install_or_remove_hook(&app().keyboard_hook, should_hook_keyboard, WH_KEYBOARD_LL, keyboard_hook_proc);
    install_or_remove_hook(&app().mouse_hook, should_hook_mouse, WH_MOUSE_LL, mouse_hook_proc);
}

fn install_or_remove_hook(
    handle: &Mutex<isize>,
    should_install: bool,
    hook_type: i32,
    callback: unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT,
) {
    let mut guard = lock_handle(handle);
    unsafe {
        if should_install {
            if *guard == 0 {
                *guard = SetWindowsHookExW(hook_type, Some(callback), null_mut(), 0) as isize;
            }
        } else if *guard != 0 {
            UnhookWindowsHookEx(*guard as HHOOK);
            *guard = 0;
        }
    }
}

fn uninstall_hook(handle: &Mutex<isize>) {
    let mut guard = lock_handle(handle);
    unsafe {
        if *guard != 0 {
            UnhookWindowsHookEx(*guard as HHOOK);
            *guard = 0;
        }
    }
}

unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(lparam as *const KBDLLHOOKSTRUCT);
        match wparam as u32 {
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                handle_key_down(info.vkCode);
            }
            WM_KEYUP | WM_SYSKEYUP => {
                if info.vkCode == VK_LWIN as u32 || info.vkCode == VK_RWIN as u32 {
                    if current_config().enabled {
                        let other = if info.vkCode == VK_LWIN as u32 { VK_RWIN } else { VK_LWIN };
                        let other_still_down = (GetKeyState(other as i32) as u16 & 0xF0) > 0;
                        app().is_win_key_down.store(other_still_down, Ordering::SeqCst);
                        app().should_stay_visible_before.store(current_millis() + 400, Ordering::SeqCst);
                        signal_taskbar_refresh();
                    }
                }
            }
            _ => {}
        }
    }

    CallNextHookEx(null_mut(), code, wparam, lparam)
}

fn handle_key_down(vk: u32) {
    let vk = vk as i32;
    if vk == VK_LWIN as i32 || vk == VK_RWIN as i32 {
        if current_config().enabled && !app().is_win_key_down.swap(true, Ordering::SeqCst) {
            signal_taskbar_refresh();
        }
    }
}

unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let msg = wparam as u32;
        let info = &*(lparam as *const MSLLHOOKSTRUCT);
        let (mouse_x, mouse_y) = (info.pt.x, info.pt.y);

        if msg == WM_MOUSEMOVE {
            handle_mouse_move(mouse_x, mouse_y);
        } else if msg == WM_MOUSEWHEEL {
            let delta = hiword(info.mouseData) as i16;
            if handle_mouse_scroll(delta, mouse_x, mouse_y) {
                return 1;
            }
        }
    }

    CallNextHookEx(null_mut(), code, wparam, lparam)
}

fn handle_mouse_move(mouse_x: i32, mouse_y: i32) {
    if !current_config().scroll_activation_enabled {
        return;
    }

    let monitor = primary_monitor_rect();
    if mouse_y >= monitor.bottom - 2
        && mouse_y < monitor.bottom
        && mouse_x >= monitor.left
        && mouse_x < monitor.right
        && !should_show_taskbar()
    {
        app().should_show_due_to_focus.store(true, Ordering::SeqCst);
        signal_taskbar_refresh();
    }
}

fn handle_mouse_scroll(_delta: i16, mouse_x: i32, mouse_y: i32) -> bool {
    let config = current_config();
    if !config.scroll_activation_enabled {
        return false;
    }

    let monitor = primary_monitor_rect();
    if mouse_y >= monitor.bottom - 2
        && mouse_y < monitor.bottom
        && mouse_x >= monitor.left
        && mouse_x < monitor.right
    {
        app().win_key_press_requested.store(true, Ordering::SeqCst);
        signal_taskbar_refresh();
        return true;
    }

    false
}

fn taskbar_worker(rx: Receiver<TaskbarSignal>) {
    loop {
        match rx.recv() {
            Ok(TaskbarSignal::Refresh) => {}
            Ok(TaskbarSignal::Exit) | Err(_) => break,
        }

        let mut restart = true;
        while restart {
            restart = false;
            if apply_taskbar_state(&rx) {
                return;
            }

            loop {
                match rx.try_recv() {
                    Ok(TaskbarSignal::Refresh) => {
                        restart = true;
                    }
                    Ok(TaskbarSignal::Exit) => return,
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return,
                }
            }
        }
    }
}

fn apply_taskbar_state(rx: &Receiver<TaskbarSignal>) -> bool {
    if app().win_key_press_requested.swap(false, Ordering::SeqCst) {
        unsafe {
            send_windows_key_press();
        }
    }

    let config = current_config();
    let taskbars = taskbar_windows();
    set_taskbar_appbar_state(&taskbars, &config, app().is_finalizing.load(Ordering::SeqCst));

    let mut attempts = 0;
    while attempts < 60 {
        attempts += 1;

        match rx.try_recv() {
            Ok(TaskbarSignal::Refresh) => return false,
            Ok(TaskbarSignal::Exit) => return true,
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => return true,
        }

        let should_show = should_show_taskbar();

        let mut failed = false;
        for hwnd in &taskbars {
            unsafe {
                ShowWindow(*hwnd, if should_show { SW_SHOWNOACTIVATE } else { SW_HIDE });
                let actually_visible = IsWindowVisible(*hwnd) != 0;
                if actually_visible != should_show {
                    failed = true;
                }
            }
        }

        let visible_until = app().should_stay_visible_before.load(Ordering::SeqCst);
        if current_millis() < visible_until {
            std::thread::sleep(std::time::Duration::from_millis(100));
            attempts = 0;
        } else if failed {
            std::thread::sleep(std::time::Duration::from_millis(10));
        } else if should_show {
            break;
        } else {
            std::thread::sleep(std::time::Duration::from_millis(50));
            attempts += 8;
        }
    }

    false
}

fn should_show_taskbar() -> bool {
    app().menu_active.load(Ordering::SeqCst)
        || app().should_show_due_to_focus.load(Ordering::SeqCst)
        || app().is_win_key_down.load(Ordering::SeqCst)
        || current_millis() < app().should_stay_visible_before.load(Ordering::SeqCst)
}

fn restore_taskbar_now(finalizing: bool) {
    app().should_show_due_to_focus.store(true, Ordering::SeqCst);
    let taskbars = taskbar_windows();
    let config = current_config();
    set_taskbar_appbar_state(&taskbars, &config, finalizing);
    for hwnd in &taskbars {
        unsafe {
            ShowWindow(*hwnd, SW_SHOWNOACTIVATE);
        }
    }
}

fn taskbar_windows() -> Vec<HWND> {
    let mut windows = Vec::new();
    unsafe {
        let primary = FindWindowW(to_wide("Shell_TrayWnd").as_ptr(), null());
        if !primary.is_null() {
            windows.push(primary);
        }

        let mut current = null_mut();
        let secondary_class = to_wide("Shell_SecondaryTrayWnd");
        loop {
            current = FindWindowExW(null_mut(), current, secondary_class.as_ptr(), null());
            if current.is_null() {
                break;
            }
            windows.push(current);
        }
    }
    windows
}

fn set_taskbar_appbar_state(taskbars: &[HWND], config: &Config, finalizing: bool) {
    if let Some(primary) = taskbars.first().copied() {
        let mut data: APPBARDATA = unsafe { zeroed() };
        data.cbSize = size_of::<APPBARDATA>() as u32;
        data.hWnd = primary;
        data.lParam = if (config.enabled && !finalizing) || config.autohide_when_disabled {
            ABS_AUTOHIDE as isize
        } else {
            0
        };
        unsafe {
            windows_sys::Win32::UI::Shell::SHAppBarMessage(ABM_SETSTATE, &mut data);
        }
    }
}

unsafe fn send_windows_key_press() {
    let mut input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_LWIN as u16,
                wScan: 0,
                dwFlags: 0,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    SendInput(1, &input, size_of::<INPUT>() as i32);
    input.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
    SendInput(1, &input, size_of::<INPUT>() as i32);
}

unsafe fn open_releases_page() {
    let verb = to_wide("open");
    let url = to_wide(RELEASES_URL);
    ShellExecuteW(null_mut(), verb.as_ptr(), url.as_ptr(), null(), null(), SW_SHOWNORMAL);
}

fn signal_taskbar_refresh() {
    match app().taskbar_tx.try_send(TaskbarSignal::Refresh) {
        Ok(_) | Err(TrySendError::Full(_)) => {}
        Err(TrySendError::Disconnected(_)) => {}
    }
}

fn load_config(path: &PathBuf) -> Config {
    let mut config = Config::default_with_system_state();
    if let Ok(data) = fs::read_to_string(path) {
        if let Ok(loaded) = serde_json::from_str::<Config>(&data) {
            config = loaded;
        }
    }
    config
}

fn save_config(config: &Config) {
    if let Some(parent) = app().config_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(serialized) = serde_json::to_string_pretty(config) {
        let _ = fs::write(&app().config_path, serialized);
    }
}

fn config_path() -> PathBuf {
    let mut path = env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    path.push(APP_NAME);
    path.push("config.json");
    path
}

fn quoted_exe_path() -> String {
    match env::current_exe() {
        Ok(path) => format!("\"{}\"", path.display()),
        Err(_) => format!("\"{}\"", APP_NAME),
    }
}

fn query_auto_launch_enabled(expected_value: &str) -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = match hkcu.open_subkey_with_flags("Software\\Microsoft\\Windows\\CurrentVersion\\Run", KEY_READ) {
        Ok(key) => key,
        Err(_) => return false,
    };

    match run_key.get_value::<String, _>(APP_NAME) {
        Ok(value) => value == expected_value,
        Err(_) => false,
    }
}

fn set_auto_launch_enabled(enabled: bool) -> Option<bool> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (run_key, _) = hkcu
        .create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        .ok()?;

    if enabled {
        run_key.set_value(APP_NAME, &app().quoted_exe_path).ok()?;
    } else {
        let _ = run_key.delete_value(APP_NAME);
    }

    Some(query_auto_launch_enabled(&app().quoted_exe_path))
}

fn current_config() -> Config {
    lock_config().clone()
}

fn lock_config() -> std::sync::MutexGuard<'static, Config> {
    app().config.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn lock_handle(handle: &Mutex<isize>) -> std::sync::MutexGuard<'_, isize> {
    handle.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn current_millis() -> u64 {
    unsafe { GetTickCount64() }
}

fn system_taskbar_autohide_enabled() -> bool {
    let mut data: APPBARDATA = unsafe { zeroed() };
    data.cbSize = size_of::<APPBARDATA>() as u32;
    let state = unsafe { windows_sys::Win32::UI::Shell::SHAppBarMessage(ABM_GETSTATE, &mut data) };
    (state & ABS_AUTOHIDE as usize) != 0
}

fn primary_monitor_rect() -> RECT {
    let mut info: MONITORINFO = unsafe { zeroed() };
    info.cbSize = size_of::<MONITORINFO>() as u32;
    unsafe {
        let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        GetMonitorInfoW(monitor, &mut info);
    }
    info.rcMonitor
}

fn monitor_work_area_from_point(x: i32, y: i32) -> RECT {
    let mut info: MONITORINFO = unsafe { zeroed() };
    info.cbSize = size_of::<MONITORINFO>() as u32;
    unsafe {
        let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTOPRIMARY);
        GetMonitorInfoW(monitor, &mut info);
    }
    info.rcWork
}

unsafe fn load_app_icon() -> *mut c_void {
    let resource_icon = LoadImageW(
        GetModuleHandleW(null()),
        make_int_resource(APP_ICON_RESOURCE_ID),
        IMAGE_ICON,
        0,
        0,
        LR_DEFAULTSIZE | LR_SHARED,
    );
    if !resource_icon.is_null() {
        return resource_icon;
    }

    LoadIconW(null_mut(), IDI_APPLICATION)
}

unsafe fn cursor_anchor() -> POINT {
    let mut point = POINT { x: 0, y: 0 };
    GetCursorPos(&mut point);
    point
}

fn point_from_wparam(wparam: WPARAM) -> POINT {
    POINT {
        x: loword(wparam as u32) as i16 as i32,
        y: hiword(wparam as u32) as i16 as i32,
    }
}

fn fill_wide_buffer(buffer: &mut [u16], value: &str) {
    let wide = to_wide(value);
    let max_len = buffer.len().saturating_sub(1).min(wide.len().saturating_sub(1));
    buffer[..max_len].copy_from_slice(&wide[..max_len]);
    buffer[max_len] = 0;
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn make_int_resource(id: u16) -> *const u16 {
    id as usize as *const u16
}

fn loword(value: u32) -> u32 {
    value & 0xffff
}

fn hiword(value: u32) -> u32 {
    (value >> 16) & 0xffff
}

const IDC_TOGGLE_HOTKEY_EDIT: i32 = 101;
const IDC_TOGGLE_HOTKEY_CLEAR: i32 = 102;
const IDC_OK: i32 = IDOK as i32;
const IDC_CANCEL: i32 = IDCANCEL as i32;

const IDOK: usize = 1;
const IDCANCEL: usize = 2;
const DS_SHELLFONT: u32 = (DS_SETFONT | 0x0008 | 0x0010) as u32;

#[inline]
fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

struct HotkeyEditState {
    hotkey: HotkeyConfig,
    original: HotkeyConfig,
    capturing: bool,
}

const HOTKEY_EDIT_CLASS_NAME: &str = "Buttery.HotkeyEdit";

static HOTKEY_EDIT_CLASS_REGISTERED: OnceLock<()> = OnceLock::new();

fn register_hotkey_edit_class() {
    unsafe {
        let class_name = to_wide(HOTKEY_EDIT_CLASS_NAME);
        let mut wc: WNDCLASSEXW = zeroed();
        wc.cbSize = size_of::<WNDCLASSEXW>() as u32;
        wc.style = 0; // CS_HREDRAW | CS_VREDRAW not needed since we invalidate on change
        wc.lpfnWndProc = Some(hotkey_edit_proc);
        wc.hInstance = GetModuleHandleW(null());
        wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        wc.hbrBackground = (COLOR_WINDOW + 1) as *mut c_void;
        wc.lpszClassName = class_name.as_ptr();

        RegisterClassExW(&wc);
    }
}

unsafe extern "system" fn hotkey_edit_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let create_struct = lparam as *const windows_sys::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
            if let Some(create_struct) = create_struct.as_ref() {
                let state_ptr = create_struct.lpCreateParams as *mut HotkeyEditState;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_NCDESTROY => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_SETFOCUS => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HotkeyEditState;
            if let Some(state) = state_ptr.as_mut() {
                state.capturing = true;
                state.original = state.hotkey.clone();
                invalidate_hotkey_edit(hwnd);
            }
            0
        }
        WM_KILLFOCUS => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HotkeyEditState;
            if let Some(state) = state_ptr.as_mut() {
                state.capturing = false;
                invalidate_hotkey_edit(hwnd);
            }
            0
        }
        WM_LBUTTONDOWN => {
            unsafe { SetFocus(hwnd); }
            0
        }
        WM_ERASEBKGND => {
            1
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HotkeyEditState;
            if state_ptr.is_null() {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            let state = &mut *state_ptr;

            let vk = wparam as u32;

            if vk == VK_ESCAPE as u32 {
                state.hotkey = state.original.clone();
                state.capturing = false;
                invalidate_hotkey_edit(hwnd);
                SetFocus(GetParent(hwnd));
                return 0;
            }

            if vk == VK_BACK as u32 || vk == VK_DELETE as u32 {
                state.hotkey = HotkeyConfig::default();
                invalidate_hotkey_edit(hwnd);
                return 0;
            }

            if is_modifier_vk(vk) {
                return 0;
            }

            let mut hk = HotkeyConfig::default();
            hk.ctrl = (GetAsyncKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0;
            hk.alt = (GetAsyncKeyState(VK_MENU as i32) as u16 & 0x8000) != 0;
            hk.shift = (GetAsyncKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;
            hk.win = (GetAsyncKeyState(VK_LWIN as i32) as u16 & 0x8000) != 0
                || (GetAsyncKeyState(VK_RWIN as i32) as u16 & 0x8000) != 0;

            let char_vk = MapVirtualKeyW(vk, MAPVK_VK_TO_CHAR);
            if char_vk != 0 {
                hk.key = char_vk & 0xFF;
                if hk.key >= 'a' as u32 && hk.key <= 'z' as u32 {
                    hk.key -= 32;
                }
            } else {
                hk.key = vk;
            }

            if !hk.ctrl && !hk.alt && !hk.shift && !hk.win {
                hk.alt = true;
            }

            state.hotkey = hk;
            invalidate_hotkey_edit(hwnd);
            0
        }
        WM_PAINT => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HotkeyEditState;
            let mut ps: PAINTSTRUCT = zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            
            let mut rc: RECT = zeroed();
            GetClientRect(hwnd, &mut rc);

            let mut is_capturing = false;
            let mut hotkey_clone = HotkeyConfig::default();
            
            if let Some(state) = state_ptr.as_ref() {
                is_capturing = state.capturing;
                hotkey_clone = state.hotkey.clone();
            }

            let bg_color = if is_capturing {
                rgb(255, 255, 220)
            } else {
                GetSysColor(COLOR_WINDOW)
            };
            
            let bg_brush = CreateSolidBrush(bg_color);
            FillRect(hdc, &rc, bg_brush);
            DeleteObject(bg_brush as *mut c_void);

            let border_brush = CreateSolidBrush(GetSysColor(COLOR_WINDOWFRAME));
            FrameRect(hdc, &rc, border_brush);
            DeleteObject(border_brush as *mut c_void);

            let text = if is_capturing && hotkey_clone.is_empty() {
                "Press shortcut...".to_string()
            } else {
                hotkey_clone.to_display_string()
            };

            let mut text_wide = to_wide(&text);
            SetBkColor(hdc, bg_color);
            SetTextColor(hdc, GetSysColor(COLOR_WINDOWTEXT));

            // padding
            rc.left += 4;
            rc.right -= 4;

            DrawTextW(
                hdc,
                text_wide.as_mut_ptr(),
                -1,
                &mut rc,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            EndPaint(hwnd, &ps);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn invalidate_hotkey_edit(hwnd: HWND) {
    unsafe {
        windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, null(), 1);
    }
}

#[allow(non_snake_case)]
fn GetParent(hwnd: HWND) -> HWND {
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetAncestor(hwnd, windows_sys::Win32::UI::WindowsAndMessaging::GA_PARENT) }
}

fn is_modifier_vk(vk: u32) -> bool {
    let v = vk as u16;
    matches!(v,
        VK_CONTROL | VK_LCONTROL | VK_RCONTROL |
        VK_MENU | VK_LMENU | VK_RMENU |
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT |
        VK_LWIN | VK_RWIN)
}

impl HotkeyConfig {
    fn to_display_string(&self) -> String {
        if self.is_empty() {
            return "(None)".to_string();
        }

        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.win {
            parts.push("Win");
        }

        let key_str = vk_to_string(self.key);
        parts.push(&key_str);

        parts.join(" + ")
    }
}

fn vk_to_string(vk: u32) -> String {
    if vk >= 'A' as u32 && vk <= 'Z' as u32 {
        return (vk as u8 as char).to_string();
    }
    if vk >= '0' as u32 && vk <= '9' as u32 {
        return (vk as u8 as char).to_string();
    }
    if vk >= VK_F1 as u32 && vk <= VK_F24 as u32 {
        return format!("F{}", vk - VK_F1 as u32 + 1);
    }
    match vk as u16 {
        VK_SPACE => "Space".to_string(),
        VK_TAB => "Tab".to_string(),
        VK_RETURN => "Enter".to_string(),
        VK_ESCAPE => "Esc".to_string(),
        VK_BACK => "Backspace".to_string(),
        VK_DELETE => "Delete".to_string(),
        VK_INSERT => "Insert".to_string(),
        VK_HOME => "Home".to_string(),
        VK_END => "End".to_string(),
        VK_PRIOR => "Page Up".to_string(),
        VK_NEXT => "Page Down".to_string(),
        VK_LEFT => "Left".to_string(),
        VK_UP => "Up".to_string(),
        VK_RIGHT => "Right".to_string(),
        VK_DOWN => "Down".to_string(),
        _ => format!("0x{:02X}", vk),
    }
}

unsafe fn show_settings_dialog() {
    let hwnd = app().main_hwnd.load(Ordering::SeqCst) as HWND;
    if !hwnd.is_null() {
        // Must use PostMessage to return from the tray menu TrackPopupMenu modal loop quickly,
        // otherwise the dialog may block or fail to open.
        PostMessageW(hwnd, WM_APP + 2, 0, 0);
    }
}

unsafe extern "system" fn settings_dialog_proc(hwnd: HWND, msg: u32, wparam: WPARAM, _lparam: LPARAM) -> isize {
    match msg {
        WM_INITDIALOG => {
            let state_ptr = _lparam as *mut HotkeyEditState;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);

            let edit_hwnd = CreateWindowExW(
                0,
                to_wide(HOTKEY_EDIT_CLASS_NAME).as_ptr(),
                to_wide("").as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                0, 0, 0, 0,
                hwnd,
                IDC_TOGGLE_HOTKEY_EDIT as isize as *mut _,
                GetModuleHandleW(null()),
                state_ptr as *mut _,
            );

            let font = SendMessageW(hwnd, WM_GETFONT, 0, 0);
            SendMessageW(edit_hwnd, WM_SETFONT, font as usize, 1);

            let clear_hwnd = GetDlgItem(hwnd, IDC_TOGGLE_HOTKEY_CLEAR);
            let mut clear_rc: RECT = zeroed();
            GetWindowRect(clear_hwnd, &mut clear_rc);
            
            let mut pts = [
                POINT { x: clear_rc.left, y: clear_rc.top },
                POINT { x: clear_rc.right, y: clear_rc.bottom },
            ];
            windows_sys::Win32::Graphics::Gdi::ScreenToClient(hwnd, &mut pts[0]);
            windows_sys::Win32::Graphics::Gdi::ScreenToClient(hwnd, &mut pts[1]);
            
            let mut dlg_rect = RECT { left: 100, top: 0, right: 0, bottom: 0 };
            MapDialogRect(hwnd, &mut dlg_rect);

            let edit_x = dlg_rect.left;
            let edit_w = pts[0].x - edit_x - 4;
            let edit_y = pts[0].y;
            let edit_h = pts[1].y - pts[0].y;

            MoveWindow(edit_hwnd, edit_x, edit_y, edit_w, edit_h, 1);

            1
        }
        WM_COMMAND => {
            let id = loword(wparam as u32) as i32;
            if id == IDC_TOGGLE_HOTKEY_CLEAR {
                let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HotkeyEditState;
                if let Some(state) = state_ptr.as_mut() {
                    state.hotkey = HotkeyConfig::default();
                    invalidate_hotkey_edit(GetDlgItem(hwnd, IDC_TOGGLE_HOTKEY_EDIT));
                }
            } else if id == IDC_OK {
                let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HotkeyEditState;
                if let Some(state) = state_ptr.as_ref() {
                    let new_hotkey = state.hotkey.clone();
                    {
                        let mut config = lock_config();
                        config.toggle_shortcut = new_hotkey.clone();
                        save_config(&config);
                    }

                    let main_hwnd = app().main_hwnd.load(Ordering::SeqCst) as HWND;

                    if !new_hotkey.is_empty() {
                        unsafe {
                            UnregisterHotKey(main_hwnd, HOTKEY_ID_TOGGLE);
                            if RegisterHotKey(
                                main_hwnd,
                                HOTKEY_ID_TOGGLE,
                                new_hotkey.modifiers(),
                                new_hotkey.key,
                            ) == 0 {
                                if GetLastError() == ERROR_HOTKEY_ALREADY_REGISTERED {
                                    MessageBoxW(
                                        hwnd,
                                        to_wide("This shortcut is already in use by another program.").as_ptr(),
                                        to_wide("Shortcut Conflict").as_ptr(),
                                        MB_ICONWARNING | MB_OK,
                                    );
                                }
                            }
                        }
                    } else {
                        unsafe { UnregisterHotKey(main_hwnd, HOTKEY_ID_TOGGLE); }
                    }

                    update_hooks();
                }
                EndDialog(hwnd, 1);
            } else if id == IDC_CANCEL {
                EndDialog(hwnd, 0);
            }
            1
        }
        WM_CLOSE => {
            EndDialog(hwnd, 0);
            1
        }
        _ => 0,
    }
}

fn create_settings_dialog_template() -> Vec<u8> {
    let mut buffer = Vec::<u8>::new();
    
    fn write_u16(buffer: &mut Vec<u8>, val: u16) {
        buffer.extend_from_slice(&val.to_le_bytes());
    }
    
    fn write_u32(buffer: &mut Vec<u8>, val: u32) {
        buffer.extend_from_slice(&val.to_le_bytes());
    }
    
    fn align_dword(buffer: &mut Vec<u8>) {
        while buffer.len() % 4 != 0 {
            buffer.push(0);
        }
    }

    fn write_item(
        buffer: &mut Vec<u8>,
        style: u32,
        ex_style: u32,
        x: u16, y: u16, cx: u16, cy: u16,
        id: u16,
        class_atom: u16,
        title: &[u16],
    ) {
        align_dword(buffer);
        write_u32(buffer, style);
        write_u32(buffer, ex_style);
        write_u16(buffer, x);
        write_u16(buffer, y);
        write_u16(buffer, cx);
        write_u16(buffer, cy);
        write_u16(buffer, id);
        write_u16(buffer, 0xFFFF);
        write_u16(buffer, class_atom);
        for &ch in title { write_u16(buffer, ch); }
        write_u16(buffer, 0); // cbCreateData
    }

    let style = DS_CENTER as u32 | DS_MODALFRAME as u32 | DS_SHELLFONT as u32 | WS_CAPTION as u32 | WS_VISIBLE as u32 | WS_SYSMENU as u32;
    write_u32(&mut buffer, style);
    write_u32(&mut buffer, 0); // dwExtendedStyle
    write_u16(&mut buffer, 4); // cdit
    write_u16(&mut buffer, 0); // x
    write_u16(&mut buffer, 0); // y
    write_u16(&mut buffer, 240); // cx
    write_u16(&mut buffer, 80); // cy
    
    write_u16(&mut buffer, 0); // menu
    write_u16(&mut buffer, 0); // class
    
    let title = to_wide("Settings - Shortcuts");
    for &ch in &title { write_u16(&mut buffer, ch); }
    
    write_u16(&mut buffer, 10); // font size
    let font_name = to_wide("Segoe UI");
    for &ch in &font_name { write_u16(&mut buffer, ch); }

    align_dword(&mut buffer);

    let label_style = WS_CHILD as u32 | WS_VISIBLE as u32;
    let label1 = to_wide("Toggle shortcut:");
    write_item(&mut buffer, label_style, 0, 10, 12, 80, 12, 0, 0x0082, &label1);

    let btn_style = WS_CHILD as u32 | WS_VISIBLE as u32 | WS_TABSTOP as u32;
    let btn_label = to_wide("X");
    write_item(&mut buffer, btn_style, 0, 210, 10, 20, 16, IDC_TOGGLE_HOTKEY_CLEAR as u16, 0x0080, &btn_label);

    let ok_label = to_wide("OK");
    write_item(&mut buffer, btn_style, 0, 120, 55, 50, 14, IDC_OK as u16, 0x0080, &ok_label);

    let cancel_label = to_wide("Cancel");
    write_item(&mut buffer, btn_style, 0, 180, 55, 50, 14, IDC_CANCEL as u16, 0x0080, &cancel_label);

    buffer
}
