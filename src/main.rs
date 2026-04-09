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
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTOPRIMARY};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
use windows_sys::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows_sys::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_CONTROL, VK_F11,
    VK_LWIN, VK_RWIN,
};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, ShellExecuteW, APPBARDATA, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD,
    NIM_DELETE, NIM_SETFOCUS, NIM_SETVERSION, NIN_SELECT, NOTIFYICONDATAW, ABM_GETSTATE, ABM_SETSTATE,
    ABS_AUTOHIDE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CallNextHookEx, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, FindWindowExW, FindWindowW, GetClassNameW, GetCursorPos, GetForegroundWindow, GetMessageW,
    EVENT_SYSTEM_FOREGROUND, HHOOK, IDC_ARROW, IDI_APPLICATION, IMAGE_ICON, IsWindowVisible,
    KBDLLHOOKSTRUCT, LR_DEFAULTSIZE, LR_SHARED, LoadCursorW, LoadIconW, LoadImageW, MF_CHECKED,
    MF_SEPARATOR, MF_STRING, MF_UNCHECKED, MSLLHOOKSTRUCT, MSG, PostMessageW, PostQuitMessage,
    RegisterClassExW, RegisterWindowMessageW, SetForegroundWindow, SetWindowsHookExW, ShowWindow,
    TPM_BOTTOMALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTALIGN, TPM_RIGHTBUTTON, TrackPopupMenu,
    TranslateMessage, UnhookWindowsHookEx, WNDCLASSEXW, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_APP, WM_CONTEXTMENU,
    WM_CREATE, WM_DESTROY, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONUP, WM_MOUSEWHEEL, WM_NULL, WM_RBUTTONDOWN,
    WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP, SW_HIDE, SW_SHOWNOACTIVATE, SW_SHOWNORMAL,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};

const APP_NAME: &str = "Buttery Taskbar";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASES_URL: &str = "https://github.com/LuisThiamNye/ButteryTaskbar2/releases";
const WINDOW_CLASS_NAME: &str = "BUTTERY_TASKBAR_RS";
const TRAY_CALLBACK_MESSAGE: u32 = WM_APP + 1;
const TRAY_ICON_ID: u32 = 1;
const APP_ICON_RESOURCE_ID: u16 = 1;
const NOTIFYICON_VERSION_4_VALUE: u32 = 4;
const NINF_KEY: u32 = 1;
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;

const CMD_TOGGLE_ENABLED: usize = 1001;
const CMD_TOGGLE_SHORTCUT: usize = 1002;
const CMD_TOGGLE_SCROLL: usize = 1003;
const CMD_TOGGLE_AUTOHIDE: usize = 1004;
const CMD_TOGGLE_STARTUP: usize = 1005;
const CMD_OPEN_RELEASES: usize = 1006;
const CMD_QUIT: usize = 1999;

static APP: OnceLock<AppState> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    version: u32,
    enabled: bool,
    scroll_activation_enabled: bool,
    toggle_shortcut_enabled: bool,
    auto_launch_enabled: bool,
    autohide_when_disabled: bool,
}

impl Config {
    fn default_with_system_state() -> Self {
        Self {
            version: 1,
            enabled: true,
            scroll_activation_enabled: true,
            toggle_shortcut_enabled: false,
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
    should_stay_visible_before: AtomicI64,
    is_win_key_down: AtomicBool,
    win_key_press_requested: AtomicBool,
    taskbar_created_message: u32,
    taskbar_tx: SyncSender<TaskbarSignal>,
}

fn main() {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
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
        should_stay_visible_before: AtomicI64::new(0),
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

    CreateWindowExW(
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
    )
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
    append_menu_item(menu, &mut labels, CMD_TOGGLE_ENABLED, "Enabled", config.enabled);
    append_menu_item(
        menu,
        &mut labels,
        CMD_TOGGLE_SHORTCUT,
        "Ctrl+Win+F11 to toggle",
        config.toggle_shortcut_enabled,
    );
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
    let should_hook_mouse = !finalizing && !menu_active && config.enabled && config.scroll_activation_enabled;

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

fn handle_key_down(vk: u32) -> bool {
    let vk = vk as i32;
    if vk == VK_LWIN as i32 || vk == VK_RWIN as i32 {
        if current_config().enabled && !app().is_win_key_down.swap(true, Ordering::SeqCst) {
            signal_taskbar_refresh();
        }
        return false;
    }

    if vk == VK_F11 as i32 {
        let control = key_is_down(VK_CONTROL as i32);
        let win = key_is_down(VK_LWIN as i32) || key_is_down(VK_RWIN as i32);
        if control && win && current_config().toggle_shortcut_enabled {
            let new_enabled = !current_config().enabled;
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
            return true;
        }
    }

    false
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
    if !config.enabled || !config.scroll_activation_enabled {
        return false;
    }

    let rect = primary_monitor_work_area();
    if mouse_y == rect.bottom - 1
        && mouse_x >= rect.left
        && mouse_x < rect.right
        && !app().should_show_due_to_focus.load(Ordering::SeqCst)
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
    unsafe { (GetKeyState(vk) as u16 & 0xF0) > 0 }
}
