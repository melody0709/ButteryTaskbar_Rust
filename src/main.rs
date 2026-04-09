#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};
use std::ffi::c_void;
use std::env;
use std::fs;
use std::mem::{size_of, zeroed};
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicIsize, Ordering};
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
    VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN,
    VK_SHIFT,
};
use windows_sys::Win32::UI::Shell::{
    SetCurrentProcessExplicitAppUserModelID, Shell_NotifyIconW, ShellExecuteW, APPBARDATA, NIF_ICON,
    NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETFOCUS, NIM_SETVERSION, NIN_SELECT,
    NOTIFYICONDATAW, ABM_GETSTATE, ABM_SETSTATE, ABS_AUTOHIDE,
};
use windows_sys::Win32::UI::Shell::PropertiesSystem::{PSGetPropertyKeyFromName, SHGetPropertyStoreForWindow};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CallNextHookEx, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, FindWindowExW, FindWindowW, GetClassNameW, GetCursorPos, GetForegroundWindow, GetMessageW,
    EVENT_SYSTEM_FOREGROUND, HHOOK, ICON_BIG, ICON_SMALL, IDC_ARROW, IDI_APPLICATION, IMAGE_ICON,
    IsWindowVisible,
    KBDLLHOOKSTRUCT, LR_DEFAULTSIZE, LR_SHARED, LoadCursorW, LoadIconW, LoadImageW, MF_CHECKED,
    MF_SEPARATOR, MF_STRING, MF_UNCHECKED, MSLLHOOKSTRUCT, MSG, PostMessageW, PostQuitMessage,
    RegisterClassExW, RegisterWindowMessageW, SendMessageW, SetForegroundWindow, SetWindowsHookExW, ShowWindow,
    TPM_BOTTOMALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTALIGN, TPM_RIGHTBUTTON, TrackPopupMenu,
    TranslateMessage, UnhookWindowsHookEx, WNDCLASSEXW, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_APP, WM_CONTEXTMENU,
    WM_CREATE, WM_DESTROY, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONUP, WM_MOUSEWHEEL, WM_NULL, WM_RBUTTONDOWN,
    WM_RBUTTONUP, WM_SETICON, WM_SYSKEYDOWN, WM_SYSKEYUP, SW_HIDE, SW_SHOWNOACTIVATE, SW_SHOWNORMAL,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};

const APP_NAME: &str = "Buttery Taskbar";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const APP_USER_MODEL_ID: &str = "melody0709.ButteryTaskbar_Rust";
const RELEASES_URL: &str = "https://github.com/melody0709/ButteryTaskbar_Rust/releases";
const WINDOW_CLASS_NAME: &str = "BUTTERY_TASKBAR_RS";
const CONFIG_VERSION: u32 = 2;
const DEFAULT_TOGGLE_SHORTCUT: &str = "Ctrl+Win+F11";
const TRAY_CALLBACK_MESSAGE: u32 = WM_APP + 1;
const TRAY_ICON_ID: u32 = 1;
const APP_ICON_RESOURCE_ID: u16 = 1;
const NOTIFYICON_VERSION_4_VALUE: u32 = 4;
const NINF_KEY: u32 = 1;
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;
const IID_PROPERTY_STORE: GUID = GUID::from_u128(0x886d8eeb_8cf2_4446_8d02_cdba1dbdcf99);

const CMD_TOGGLE_ENABLED: usize = 1001;
const CMD_TOGGLE_SHORTCUT: usize = 1002;
const CMD_OPEN_SHORTCUT_SETTINGS: usize = 1003;
const CMD_TOGGLE_SCROLL: usize = 1004;
const CMD_TOGGLE_AUTOHIDE: usize = 1005;
const CMD_TOGGLE_STARTUP: usize = 1006;
const CMD_OPEN_RELEASES: usize = 1007;
const CMD_QUIT: usize = 1999;

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToggleHotkey {
    ctrl: bool,
    alt: bool,
    shift: bool,
    win: bool,
    key_vk: u32,
    key_name: String,
}

impl ToggleHotkey {
    fn parse(value: &str) -> Option<Self> {
        let mut hotkey = Self {
            ctrl: false,
            alt: false,
            shift: false,
            win: false,
            key_vk: 0,
            key_name: String::new(),
        };
        let mut has_key = false;

        for token in value.split('+') {
            let token = token.trim();
            if token.is_empty() {
                return None;
            }

            match normalize_hotkey_token(token).as_str() {
                "CTRL" | "CONTROL" | "CTL" => {
                    if hotkey.ctrl {
                        return None;
                    }
                    hotkey.ctrl = true;
                }
                "ALT" => {
                    if hotkey.alt {
                        return None;
                    }
                    hotkey.alt = true;
                }
                "SHIFT" => {
                    if hotkey.shift {
                        return None;
                    }
                    hotkey.shift = true;
                }
                "WIN" | "WINDOWS" | "META" | "SUPER" => {
                    if hotkey.win {
                        return None;
                    }
                    hotkey.win = true;
                }
                _ => {
                    if has_key {
                        return None;
                    }

                    let (key_vk, key_name) = parse_hotkey_key(token)?;
                    hotkey.key_vk = key_vk;
                    hotkey.key_name = key_name;
                    has_key = true;
                }
            }
        }

        if !has_key {
            return None;
        }

        Some(hotkey)
    }

    fn display(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl".to_string());
        }
        if self.alt {
            parts.push("Alt".to_string());
        }
        if self.shift {
            parts.push("Shift".to_string());
        }
        if self.win {
            parts.push("Win".to_string());
        }
        parts.push(self.key_name.clone());
        parts.join("+")
    }

    fn matches_key_down(&self, vk: u32) -> bool {
        vk == self.key_vk && self.modifiers_match()
    }

    fn uses_vk(&self, vk: u32) -> bool {
        vk == self.key_vk
            || (self.ctrl && is_control_key(vk))
            || (self.alt && is_alt_key(vk))
            || (self.shift && is_shift_key(vk))
            || (self.win && is_win_key(vk))
    }

    fn modifiers_match(&self) -> bool {
        (!self.ctrl || key_is_down(VK_CONTROL as i32))
            && (!self.alt || key_is_down(VK_MENU as i32))
            && (!self.shift || key_is_down(VK_SHIFT as i32))
            && (!self.win || key_is_down(VK_LWIN as i32) || key_is_down(VK_RWIN as i32))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Config {
    version: u32,
    enabled: bool,
    scroll_activation_enabled: bool,
    toggle_shortcut_enabled: bool,
    #[serde(default = "default_toggle_hotkey_string")]
    toggle_shortcut: String,
    auto_launch_enabled: bool,
    autohide_when_disabled: bool,
}

impl Config {
    fn default_with_system_state() -> Self {
        Self {
            version: CONFIG_VERSION,
            enabled: true,
            scroll_activation_enabled: true,
            toggle_shortcut_enabled: false,
            toggle_shortcut: default_toggle_hotkey_string(),
            auto_launch_enabled: false,
            autohide_when_disabled: system_taskbar_autohide_enabled(),
        }
    }
}

fn default_toggle_hotkey_string() -> String {
    DEFAULT_TOGGLE_SHORTCUT.to_string()
}

fn default_toggle_hotkey() -> ToggleHotkey {
    ToggleHotkey::parse(DEFAULT_TOGGLE_SHORTCUT).expect("default toggle shortcut should be valid")
}

enum TaskbarSignal {
    Refresh,
    Exit,
}

struct AppState {
    config: Mutex<Config>,
    toggle_hotkey: Mutex<ToggleHotkey>,
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
    should_stay_visible_before: AtomicI64,
    is_win_key_down: AtomicBool,
    toggle_shortcut_latched: AtomicBool,
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
    let (mut config, should_persist_config) = load_config(&config_path);
    config.auto_launch_enabled = query_auto_launch_enabled(&quoted_exe_path);
    let toggle_hotkey = ToggleHotkey::parse(&config.toggle_shortcut).unwrap_or_else(default_toggle_hotkey);

    let taskbar_created_message = unsafe { RegisterWindowMessageW(to_wide("TaskbarCreated").as_ptr()) };
    let (taskbar_tx, taskbar_rx) = sync_channel(1);

    let _ = APP.set(AppState {
        config: Mutex::new(config),
        toggle_hotkey: Mutex::new(toggle_hotkey),
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
        should_stay_visible_before: AtomicI64::new(0),
        is_win_key_down: AtomicBool::new(false),
        toggle_shortcut_latched: AtomicBool::new(false),
        win_key_press_requested: AtomicBool::new(false),
        taskbar_created_message,
        taskbar_tx,
    });

    if should_persist_config {
        save_config(&current_config());
    }

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
            0
        }
        WM_DESTROY => {
            cleanup_on_destroy(hwnd);
            PostQuitMessage(0);
            0
        }
        _ if msg == TRAY_CALLBACK_MESSAGE => {
            handle_tray_callback(hwnd, wparam, lparam);
            0
        }
        _ if msg == app().taskbar_created_message => {
            add_notification_icon(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
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
    let toggle_shortcut_label = format!("{} to toggle", current_toggle_hotkey().display());
    append_menu_item(menu, &mut labels, CMD_TOGGLE_ENABLED, "Enabled", config.enabled);
    append_menu_item(
        menu,
        &mut labels,
        CMD_TOGGLE_SHORTCUT,
        &toggle_shortcut_label,
        config.toggle_shortcut_enabled,
    );
    append_menu_text(menu, &mut labels, CMD_OPEN_SHORTCUT_SETTINGS, "Edit shortcut settings...");
    append_menu_item(
        menu,
        &mut labels,
        CMD_TOGGLE_SCROLL,
        "Scroll to reveal taskbar",
        config.scroll_activation_enabled,
    );
    append_menu_item(
        menu,
        &mut labels,
        CMD_TOGGLE_AUTOHIDE,
        "Auto-hide when disabled",
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
        CMD_TOGGLE_SHORTCUT => {
            {
                let mut config = lock_config();
                config.toggle_shortcut_enabled = !config.toggle_shortcut_enabled;
                save_config(&config);
            }
            update_hooks();
        }
        CMD_OPEN_SHORTCUT_SETTINGS => unsafe {
            open_shortcut_settings();
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

    let should_hook_keyboard = !finalizing && !menu_active && (config.enabled || config.toggle_shortcut_enabled);
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
                if handle_key_down(info.vkCode) {
                    return 1;
                }
            }
            WM_KEYUP | WM_SYSKEYUP => {
                handle_key_up(info.vkCode);
                if info.vkCode == VK_LWIN as u32 || info.vkCode == VK_RWIN as u32 {
                    if current_config().enabled {
                        let other = if info.vkCode == VK_LWIN as u32 { VK_RWIN } else { VK_LWIN };
                        let other_still_down = key_is_down(other as i32);
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

fn handle_key_down(vk: u32) -> bool {
    let vk = vk as i32;
    if vk == VK_LWIN as i32 || vk == VK_RWIN as i32 {
        if current_config().enabled && !app().is_win_key_down.swap(true, Ordering::SeqCst) {
            signal_taskbar_refresh();
        }
        return false;
    }

    let config = current_config();
    if !config.toggle_shortcut_enabled {
        return false;
    }

    let hotkey = current_toggle_hotkey();
    if !hotkey.matches_key_down(vk as u32) {
        return false;
    }

    let already_latched = app().toggle_shortcut_latched.swap(true, Ordering::SeqCst);
    if !already_latched {
        let new_enabled = !config.enabled;
        {
            let mut config = lock_config();
            config.enabled = new_enabled;
            save_config(&config);
        }
        if new_enabled {
            refresh_foreground_state();
        } else {
            app().should_show_due_to_focus.store(true, Ordering::SeqCst);
        }
        update_hooks();
        signal_taskbar_refresh();
    }

    true
}

fn handle_key_up(vk: u32) {
    if app().toggle_shortcut_latched.load(Ordering::SeqCst) && current_toggle_hotkey().uses_vk(vk) {
        app().toggle_shortcut_latched.store(false, Ordering::SeqCst);
    }
}

unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && wparam as u32 == WM_MOUSEWHEEL {
        let info = &*(lparam as *const MSLLHOOKSTRUCT);
        let delta = hiword(info.mouseData) as i16;
        if handle_mouse_scroll(delta, info.pt.x, info.pt.y) {
            return 1;
        }
    }

    CallNextHookEx(null_mut(), code, wparam, lparam)
}

fn handle_mouse_scroll(_delta: i16, mouse_x: i32, mouse_y: i32) -> bool {
    let config = current_config();
    if !config.scroll_activation_enabled {
        return false;
    }

    let rect = primary_monitor_work_area();
    if mouse_y == rect.bottom - 1
        && mouse_x >= rect.left
        && mouse_x < rect.right
        && !should_show_taskbar()
    {
        app().should_show_due_to_focus.store(true, Ordering::SeqCst);
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

unsafe fn open_shortcut_settings() {
    let config = current_config();
    save_config(&config);

    let verb = to_wide("open");
    let executable = to_wide("notepad.exe");
    let parameter = to_wide(&format!("\"{}\"", app().config_path.display()));
    ShellExecuteW(
        null_mut(),
        verb.as_ptr(),
        executable.as_ptr(),
        parameter.as_ptr(),
        null(),
        SW_SHOWNORMAL,
    );
}

fn signal_taskbar_refresh() {
    match app().taskbar_tx.try_send(TaskbarSignal::Refresh) {
        Ok(_) | Err(TrySendError::Full(_)) => {}
        Err(TrySendError::Disconnected(_)) => {}
    }
}

fn load_config(path: &PathBuf) -> (Config, bool) {
    let mut config = Config::default_with_system_state();
    let mut should_persist = !path.exists();
    if let Ok(data) = fs::read_to_string(path) {
        if let Ok(loaded) = serde_json::from_str::<Config>(&data) {
            config = loaded;
        } else {
            should_persist = true;
        }
    }

    should_persist |= normalize_config(&mut config);
    (config, should_persist)
}

fn save_config(config: &Config) {
    let mut config_to_write = config.clone();
    normalize_config(&mut config_to_write);
    write_config(&app().config_path, &config_to_write);
}

fn write_config(path: &PathBuf, config: &Config) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(serialized) = serde_json::to_string_pretty(config) {
        let _ = fs::write(path, serialized);
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

fn current_toggle_hotkey() -> ToggleHotkey {
    lock_toggle_hotkey().clone()
}

fn lock_config() -> std::sync::MutexGuard<'static, Config> {
    app().config.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn lock_toggle_hotkey() -> std::sync::MutexGuard<'static, ToggleHotkey> {
    app().toggle_hotkey.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn lock_handle(handle: &Mutex<isize>) -> std::sync::MutexGuard<'_, isize> {
    handle.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn current_millis() -> i64 {
    unsafe { GetTickCount64() as i64 }
}

fn system_taskbar_autohide_enabled() -> bool {
    let mut data: APPBARDATA = unsafe { zeroed() };
    data.cbSize = size_of::<APPBARDATA>() as u32;
    let state = unsafe { windows_sys::Win32::UI::Shell::SHAppBarMessage(ABM_GETSTATE, &mut data) };
    (state & ABS_AUTOHIDE as usize) != 0
}

fn primary_monitor_work_area() -> RECT {
    monitor_work_area_from_point(0, 0)
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

fn normalize_config(config: &mut Config) -> bool {
    let mut changed = false;

    if config.version != CONFIG_VERSION {
        config.version = CONFIG_VERSION;
        changed = true;
    }

    let normalized_shortcut = ToggleHotkey::parse(&config.toggle_shortcut)
        .unwrap_or_else(default_toggle_hotkey)
        .display();
    if config.toggle_shortcut != normalized_shortcut {
        config.toggle_shortcut = normalized_shortcut;
        changed = true;
    }

    changed
}

fn normalize_hotkey_token(token: &str) -> String {
    token
        .chars()
        .filter(|character| !character.is_ascii_whitespace() && *character != '_' && *character != '-')
        .collect::<String>()
        .to_ascii_uppercase()
}

fn parse_hotkey_key(token: &str) -> Option<(u32, String)> {
    let normalized = normalize_hotkey_token(token);

    if normalized.len() == 1 {
        let character = normalized.chars().next()?;
        if character.is_ascii_alphabetic() || character.is_ascii_digit() {
            return Some((character as u32, character.to_string()));
        }
    }

    if let Some(function_number) = normalized.strip_prefix('F') {
        if let Ok(function_number) = function_number.parse::<u32>() {
            if (1..=24).contains(&function_number) {
                return Some((0x6F + function_number, format!("F{}", function_number)));
            }
        }
    }

    match normalized.as_str() {
        "ESC" | "ESCAPE" => Some((0x1B, "Esc".to_string())),
        "TAB" => Some((0x09, "Tab".to_string())),
        "SPACE" | "SPACEBAR" => Some((0x20, "Space".to_string())),
        "ENTER" | "RETURN" => Some((0x0D, "Enter".to_string())),
        "BACKSPACE" | "BKSP" => Some((0x08, "Backspace".to_string())),
        "DELETE" | "DEL" => Some((0x2E, "Delete".to_string())),
        "INSERT" | "INS" => Some((0x2D, "Insert".to_string())),
        "HOME" => Some((0x24, "Home".to_string())),
        "END" => Some((0x23, "End".to_string())),
        "PAGEUP" | "PGUP" | "PRIOR" => Some((0x21, "PageUp".to_string())),
        "PAGEDOWN" | "PGDN" | "NEXT" => Some((0x22, "PageDown".to_string())),
        "UP" => Some((0x26, "Up".to_string())),
        "DOWN" => Some((0x28, "Down".to_string())),
        "LEFT" => Some((0x25, "Left".to_string())),
        "RIGHT" => Some((0x27, "Right".to_string())),
        "PAUSE" | "BREAK" => Some((0x13, "Pause".to_string())),
        "PRINTSCREEN" | "PRTSC" | "PRTSCN" => Some((0x2C, "PrintScreen".to_string())),
        "CAPSLOCK" => Some((0x14, "CapsLock".to_string())),
        "NUMLOCK" => Some((0x90, "NumLock".to_string())),
        "SCROLLLOCK" => Some((0x91, "ScrollLock".to_string())),
        _ => None,
    }
}

fn is_control_key(vk: u32) -> bool {
    vk == VK_CONTROL as u32 || vk == VK_LCONTROL as u32 || vk == VK_RCONTROL as u32
}

fn is_alt_key(vk: u32) -> bool {
    vk == VK_MENU as u32 || vk == VK_LMENU as u32 || vk == VK_RMENU as u32
}

fn is_shift_key(vk: u32) -> bool {
    vk == VK_SHIFT as u32 || vk == VK_LSHIFT as u32 || vk == VK_RSHIFT as u32
}

fn is_win_key(vk: u32) -> bool {
    vk == VK_LWIN as u32 || vk == VK_RWIN as u32
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

fn key_is_down(vk: i32) -> bool {
    unsafe { (GetKeyState(vk) as u16 & 0x8000) != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_toggle_hotkey() {
        let hotkey = ToggleHotkey::parse("ctrl + win + f11").expect("expected the default hotkey to parse");
        assert_eq!(hotkey.display(), "Ctrl+Win+F11");
        assert!(hotkey.ctrl);
        assert!(hotkey.win);
        assert_eq!(hotkey.key_vk, 0x7A);
    }

    #[test]
    fn canonicalizes_navigation_key_aliases() {
        let hotkey = ToggleHotkey::parse("win + pgdn").expect("expected the alias to parse");
        assert_eq!(hotkey.display(), "Win+PageDown");
        assert_eq!(hotkey.key_vk, 0x22);
    }

    #[test]
    fn rejects_modifier_only_hotkeys() {
        assert!(ToggleHotkey::parse("Ctrl+Win").is_none());
    }

    #[test]
    fn rejects_duplicate_modifiers() {
        assert!(ToggleHotkey::parse("Ctrl+Ctrl+B").is_none());
    }
}
