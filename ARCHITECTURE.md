# ButteryTaskbar v2.5.1 Architecture

English | [简体中文](ARCHITECTURE.zh-CN.md)

## 1. Project Overview

**Buttery Taskbar** is a Windows taskbar enhancement tool written in Rust that hides the taskbar more aggressively than Windows' built-in auto-hide. The taskbar only appears when actually needed — for example, when the Start menu, tray overflow, or other Shell UI becomes active.

| Item | Description |
|------|-------------|
| Language | Rust (edition 2024) |
| Architecture | Single-file `src/main.rs` (~1710 lines) |
| UI Framework | Native Win32 API (no third-party GUI framework) |
| Runtime Dependencies | `windows-sys`, `winreg`, `serde`/`serde_json` |
| Build Dependency | `tauri-winres` (icon & resource embedding) |
| License | EPL-2.0 |

---

## 2. Architecture Overview

### 2.1 Dual-Thread Model

```
┌─────────────────────────────────────────────────────────┐
│                      Main Thread                         │
│                                                         │
│  Win32 Message Loop (GetMessageW / DispatchMessageW)    │
│  ├── Window Procedure (window_proc)                     │
│  │   ├── Tray icon callback                            │
│  │   ├── Global hotkey (WM_HOTKEY)                     │
│  │   ├── Settings dialog                               │
│  │   └── TaskbarCreated (Explorer restart recovery)    │
│  ├── Foreground hook callback (foreground_event_proc)   │
│  ├── Keyboard hook callback (keyboard_hook_proc)        │
│  └── Mouse hook callback (mouse_hook_proc)              │
│                                                         │
│  Signal ──sync_channel(1)──→                            │
└─────────────────────────────────────────────────────────┘
                            │
                            │ TaskbarSignal::Refresh / Exit
                            ▼
┌─────────────────────────────────────────────────────────┐
│                   Taskbar Worker Thread                   │
│                                                         │
│  taskbar_worker()                                       │
│  └── apply_taskbar_state()                              │
│      ├── Simulate Win key press (SendInput)             │
│      ├── Set AppBar state (SHAppBarMessage)             │
│      └── Loop ShowWindow(SW_HIDE / SW_SHOWNOACTIVATE)  │
│          └── Retry mechanism (up to 60 attempts)        │
└─────────────────────────────────────────────────────────┘
```

**Design rationale**: `ShowWindow` operations can be overridden by Windows (e.g., Explorer redrawing the taskbar), requiring a retry loop for reliability. This logic runs on a separate thread to avoid blocking the main thread's message loop and hook callbacks.

### 2.2 Global State Management

Global state is implemented as a singleton via `OnceLock<AppState>`:

```
AppState
├── config: Mutex<Config>              ← Configuration (mutex-protected)
├── config_path: PathBuf               ← Config file path
├── quoted_exe_path: String            ← Quoted exe path (for registry auto-launch)
├── main_hwnd: AtomicIsize             ← Main window handle
├── keyboard_hook: Mutex<isize>        ← Keyboard hook handle
├── mouse_hook: Mutex<isize>           ← Mouse hook handle
├── foreground_hook: Mutex<isize>      ← Foreground hook handle
├── tray_uses_version_4: AtomicBool    ← Tray icon protocol version
├── menu_active: AtomicBool            ← Context menu is open
├── is_finalizing: AtomicBool          ← Currently exiting
├── should_show_due_to_focus: AtomicBool  ← Foreground window needs taskbar
├── should_stay_visible_before: AtomicU64 ← Deadline for keeping taskbar visible
├── is_win_key_down: AtomicBool        ← Win key is pressed
├── win_key_press_requested: AtomicBool   ← Need to simulate Win key press
├── taskbar_created_message: u32       ← TaskbarCreated message ID
└── taskbar_tx: SyncSender<TaskbarSignal> ← Worker thread signal sender
```

**Concurrency strategy**:
- `Mutex` protects config and hook handles (cross-thread read/write)
- `Atomic*` for high-frequency state flags (both hook callbacks and UI thread access)
- All atomic operations use `Ordering::SeqCst` (strong consistency, negligible performance impact)

### 2.3 Inter-Thread Communication

Uses `sync_channel(1)` to pass `TaskbarSignal` enum:

```rust
enum TaskbarSignal {
    Refresh,  // Refresh taskbar state
    Exit,     // Exit worker thread
}
```

- Capacity of 1: multiple `Refresh` signals can be coalesced; the worker only needs to process the latest state
- `try_send`: sender never blocks; drops signal if channel is full (worker will process anyway)

---

## 3. Startup & Exit Flow

### 3.1 Startup Sequence

```
main()
 │
 ├── 1. ComGuard::initialize()          ← COM init (RAII guard)
 ├── 2. SetProcessDpiAwarenessContext() ← Per-Monitor DPI V2
 ├── 3. SetCurrentProcessExplicitAppUserModelID() ← Taskbar icon grouping
 ├── 4. load_config()                   ← Load JSON config
 ├── 5. query_auto_launch_enabled()     ← Sync auto-launch state from registry
 ├── 6. RegisterWindowMessageW("TaskbarCreated") ← Explorer restart message
 ├── 7. sync_channel(1)                 ← Create signal channel
 ├── 8. APP.set(AppState{...})          ← Initialize global singleton
 ├── 9. thread::spawn(taskbar_worker)   ← Start worker thread
 ├── 10. create_hidden_window()         ← Create hidden window
 │       └── WM_CREATE → add_notification_icon() + register_hotkeys()
 ├── 11. install_foreground_hook()      ← Install foreground window hook
 ├── 12. refresh_foreground_state()     ← Initial foreground window detection
 ├── 13. update_hooks()                 ← Install keyboard/mouse hooks based on config
 ├── 14. signal_taskbar_refresh()       ← Trigger initial taskbar state refresh
 └── 15. Message loop (GetMessageW loop)
```

### 3.2 Exit Flow

```
request_quit()
 │
 ├── is_finalizing.swap(true)           ← One-time guard, prevents double exit
 ├── menu_active = false
 ├── update_hooks()                     ← Uninstall all hooks
 ├── restore_taskbar_now(true)          ← Restore taskbar visibility
 ├── taskbar_tx.try_send(Exit)          ← Notify worker thread to exit
 ├── DestroyWindow(hwnd)                ← Destroy main window
 │   └── WM_DESTROY
 │       ├── UnregisterHotKey()
 │       ├── cleanup_on_destroy()
 │       │   ├── remove_notification_icon()
 │       │   ├── uninstall_foreground_hook()
 │       │   ├── uninstall_hook(keyboard)
 │       │   ├── uninstall_hook(mouse)
 │       │   └── restore_taskbar_now(true)  ← Ensure restoration again
 │       └── PostQuitMessage(0)         ← Terminate message loop
 └── (Worker thread exits upon receiving Exit signal)
```

---

## 4. Core Mechanisms

### 4.1 Taskbar Show/Hide Control

Uses a **dual strategy** to ensure reliable taskbar hiding:

**Strategy 1: Direct ShowWindow**

```rust
ShowWindow(hwnd, SW_HIDE);           // Hide
ShowWindow(hwnd, SW_SHOWNOACTIVATE); // Show (without stealing focus)
```

Applied uniformly to **all** taskbar windows (primary + secondary monitors).

**Strategy 2: AppBar Auto-Hide State**

```rust
SHAppBarMessage(ABM_SETSTATE, ABS_AUTOHIDE); // Set auto-hide
SHAppBarMessage(ABM_SETSTATE, 0);            // Cancel auto-hide
```

Only set on the primary taskbar (`Shell_TrayWnd`), since AppBar state is global. Conditions:

| Condition | ABS_AUTOHIDE |
|-----------|-------------|
| `enabled` | ✅ Set (required for reliable taskbar hiding) |
| `!enabled && autohide_when_disabled` | ✅ Set (keep auto-hide when disabled) |
| `!enabled && !autohide_when_disabled` | ❌ Not set |

**Note**: `ABS_AUTOHIDE` is required for reliable taskbar hiding. Without it, Windows continuously fights `ShowWindow(SW_HIDE)`, making the taskbar impossible to hide. A side effect is that Windows automatically shows the taskbar when the mouse touches the bottom screen edge — this is native Windows behavior, not controlled by the "Scroll to open Start" option.

### 4.2 Show Decision Logic

`should_show_taskbar()` combines four conditions:

```
should_show_taskbar() = 
    menu_active                    ← Context menu is open
    || should_show_due_to_focus    ← Foreground window needs taskbar
    || is_win_key_down             ← Win key is held
    || current_millis() < should_stay_visible_before  ← Within 400ms after Win key release
```

### 4.3 Retry Loop

The retry loop in `apply_taskbar_state()` (up to 60 attempts) handles Windows forcefully restoring the taskbar:

| Scenario | Behavior | Wait | Attempt Cost |
|----------|----------|------|-------------|
| Win key held period | Keep polling, reset counter | 100ms | 0 (reset to 0) |
| ShowWindow failed | Retry | 10ms | 1 |
| Successfully hidden | Extra verification | 50ms | 8 |
| Successfully shown | Exit immediately | — | — |

### 4.4 Foreground Window Detection

Uses `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` to monitor foreground window changes and determine whether the new window needs the taskbar:

**Window class names that require the taskbar**:

| Class Name | Corresponding UI |
|-----------|-----------------|
| `Windows.UI.Core.CoreWindow` | Start menu, Search, Notification Center, Calendar popup, etc. |
| `Shell_TrayWnd` | Primary monitor taskbar itself |
| `Shell_SecondaryTrayWnd` | Secondary monitor taskbar |
| `TopLevelWindowForOverflowXamlIsland` | System tray overflow area |
| `XamlExplorerHostIslandWindow` | File Explorer Xaml host window |
| `NotifyIconOverflowWindow` | Notification icon overflow window |

### 4.5 Keyboard Hook (Win Key Tracking)

`WH_KEYBOARD_LL` low-level keyboard hook specifically tracks Win key state:

- **Win key down**: Sets `is_win_key_down = true`, triggers taskbar refresh
- **Win key up**: Checks if the other Win key is still held, sets 400ms hold period
- **Never intercepts any key**: Always calls `CallNextHookEx`, only observes state changes

**400ms buffer**: The taskbar stays visible for 400ms after Win key release, preventing flicker when the Start menu appears.

### 4.6 Mouse Hook (Scroll to Open Start)

`WH_MOUSE_LL` low-level mouse hook handles both `WM_MOUSEMOVE` and `WM_MOUSEWHEEL`:

**Mouse move (WM_MOUSEMOVE)**:
```
Trigger conditions:
  1. scroll_activation_enabled = true (i.e., "Scroll to open Start" checked)
  2. Mouse in primary monitor bottom 2-pixel zone (monitor.bottom - 2 <= y < monitor.bottom)
  3. Mouse x within monitor horizontal range
  4. Taskbar should currently be hidden (!should_show_taskbar())

Effect:
  1. should_show_due_to_focus = true → Show taskbar
```

**Mouse wheel (WM_MOUSEWHEEL)**:
```
Trigger conditions:
  1. scroll_activation_enabled = true (i.e., "Scroll to open Start" checked)
  2. Mouse in primary monitor bottom 2-pixel zone
  3. Mouse x within monitor horizontal range

Effect:
  1. win_key_press_requested = true → Simulate Win key press (open Start menu)
  2. Intercept wheel event (return 1, don't pass to other apps)
```

**"Scroll to open Start" controls the mouse hook**: When checked, the mouse hook is installed — bottom-edge mouse move shows the taskbar (improving reliability beyond Windows' native auto-hide), and bottom-edge wheel opens the Start menu. When unchecked, the mouse hook is uninstalled. Mouse touch at the bottom edge showing the taskbar is a side effect of `ABS_AUTOHIDE`, not controlled by this option.

**⚠️ Known limitation**: Scroll activation only works at the bottom edge of the primary monitor; secondary monitors are not supported.

---

## 5. Configuration Options

### 5.1 Config File

Path: `%APPDATA%\Buttery Taskbar\config.json`

```rust
struct Config {
    version: u32,                     // Config version, currently fixed at 1
    enabled: bool,                    // Enable taskbar hiding
    scroll_activation_enabled: bool,  // Enable scroll-to-open-Start
    toggle_shortcut: HotkeyConfig,    // Toggle shortcut
    auto_launch_enabled: bool,        // Auto-start at log-in
    autohide_when_disabled: bool,     // Keep auto-hide when disabled
}
```

### 5.2 Option Details

#### `enabled` (bool, default: true)

**Master switch**. Controls whether Buttery Taskbar's taskbar hiding is enabled.

| State | Behavior |
|-------|----------|
| `true` | Actively hides the taskbar; only shows for Shell UI. Keyboard hook active (tracks Win key). |
| `false` | Taskbar always visible; keyboard hook not installed. If `autohide_when_disabled` is true, sets Windows native auto-hide. |

**Interaction**: Tray menu "Enabled" checkbox, global hotkey toggle

**State change side effects**:
- `true → false`: `should_show_due_to_focus = true` (immediately show taskbar)
- `false → true`: `refresh_foreground_state()` (decide visibility based on current foreground window)

#### `scroll_activation_enabled` (bool, default: true)

**Scroll to open Start**. Controls whether scrolling the mouse wheel at the bottom edge of the screen opens the Start menu.

| State | Behavior |
|-------|----------|
| `true` | Install mouse hook. Bottom-edge wheel → open Start menu. Bottom-edge mouse move → show taskbar (improved reliability). |
| `false` | No mouse hook; bottom-edge wheel has no special effect. |

**Note**: This option controls the mouse hook. Mouse touch at the bottom edge showing the taskbar is a side effect of `ABS_AUTOHIDE` (always happens when Enabled is checked), but the hook improves reliability of this behavior.

**Interaction**: Tray menu "Scroll to open Start" checkbox

#### `toggle_shortcut` (HotkeyConfig, default: Win+Ctrl+F11)

**Toggle shortcut**. Global hotkey for toggling the `enabled` state.

```rust
struct HotkeyConfig {
    win: bool,    // Win modifier
    ctrl: bool,   // Ctrl modifier
    shift: bool,  // Shift modifier
    alt: bool,    // Alt modifier
    key: u32,     // Virtual key code, 0 = no shortcut
}
```

**Special rules**:
- `MOD_NOREPEAT` is always appended during registration
- If no modifier is set, `Alt` is automatically added
- Shortcut conflicts show a "Shortcut Conflict" warning
- `key == 0` means no shortcut (no hotkey registered)

**Interaction**: HotkeyEdit control in Settings dialog

#### `auto_launch_enabled` (bool, default: false)

**Auto-start at log-in**. Whether to start automatically at user log-in.

| State | Behavior |
|-------|----------|
| `true` | Write to registry `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` |
| `false` | Delete the registry value |

**Note**: On each startup, the actual state is read from the registry and overwrites the config file value.

**Interaction**: Tray menu "Start at log-in (non-admin)" checkbox

#### `autohide_when_disabled` (bool, default: system current state)

**Keep auto-hide when disabled**. When Buttery Taskbar is disabled (`enabled = false`), whether to still set the taskbar to Windows native auto-hide mode.

| State | Behavior |
|-------|----------|
| `true` | Set `SHAppBarMessage(ABM_SETSTATE, ABS_AUTOHIDE)` when disabled |
| `false` | Cancel auto-hide `SHAppBarMessage(ABM_SETSTATE, 0)` when disabled |

**Default**: Read from `system_taskbar_autohide_enabled()`, preserving the user's existing Windows setting.

**Interaction**: Tray menu "Keep auto-hide when disabled" checkbox

### 5.3 Tray Menu Option Mapping

| Menu Item | Command ID | Config Field | Type |
|-----------|-----------|-------------|------|
| Enabled | CMD_TOGGLE_ENABLED (1001) | `enabled` | Checkbox |
| Settings... | CMD_SETTINGS (1002) | `toggle_shortcut` | Button (opens dialog) |
| Scroll to open Start | CMD_TOGGLE_SCROLL (1003) | `scroll_activation_enabled` | Checkbox |
| Keep auto-hide when disabled | CMD_TOGGLE_AUTOHIDE (1004) | `autohide_when_disabled` | Checkbox |
| Start at log-in (non-admin) | CMD_TOGGLE_STARTUP (1005) | `auto_launch_enabled` | Checkbox |
| Open releases page | CMD_OPEN_RELEASES (1006) | — | Link |
| Quit | CMD_QUIT (1999) | — | Exit |

### 5.4 Settings Dialog

Opened via tray menu "Settings...", title "Settings - Shortcuts".

**HotkeyEdit custom control** (window class `Buttery.HotkeyEdit`):

| Action | Behavior |
|--------|----------|
| Gain focus | Enter capture mode, background turns light yellow |
| Press non-modifier key | Combine with currently held modifiers to form new shortcut |
| Press modifier key alone | Ignored |
| Escape | Restore original shortcut |
| Backspace / Delete | Clear shortcut |
| Record without modifier | Automatically add Alt modifier |

---

## 6. Hook System

### 6.1 Three Hooks

| Hook | Type | Purpose | Install Condition |
|------|------|---------|------------------|
| Foreground hook | `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` | Monitor foreground window changes | Always installed (not controlled by `enabled`) |
| Keyboard hook | `SetWindowsHookExW(WH_KEYBOARD_LL)` | Track Win key state | `enabled && !menu_active && !finalizing` |
| Mouse hook | `SetWindowsHookExW(WH_MOUSE_LL)` | Scroll to open Start + improve edge detection | `scroll_activation_enabled && !menu_active && !finalizing` |

### 6.2 Dynamic Hook Management

`update_hooks()` dynamically installs/uninstalls hooks based on current state:

```
update_hooks()
 ├── should_hook_keyboard = !finalizing && !menu_active && config.enabled
 ├── should_hook_mouse    = !finalizing && !menu_active && config.scroll_activation_enabled
 ├── install_or_remove_hook(keyboard_hook, should_hook_keyboard, ...)
 └── install_or_remove_hook(mouse_hook, should_hook_mouse, ...)
```

**Called when**: Config changes, menu open/close, program exit

---

## 7. Multi-Monitor Support

### 7.1 Taskbar Window Enumeration

`taskbar_windows()` enumerates all taskbar windows:

```
Shell_TrayWnd           → Primary monitor taskbar (only one)
Shell_SecondaryTrayWnd  → Secondary monitor taskbars (possibly multiple)
```

Uses `FindWindowExW` with `hwndChildAfter` parameter to iterate all secondary taskbar windows.

### 7.2 Show/Hide Coverage

`apply_taskbar_state()` uniformly executes `ShowWindow` on all enumerated taskbar windows, ensuring all taskbars are synchronized across monitors.

### 7.3 Known Limitations

| Limitation | Description |
|-----------|-------------|
| Scroll activation only on primary monitor | `handle_mouse_scroll()` uses `primary_monitor_rect()` for mouse position |
| AppBar state only on primary taskbar | `set_taskbar_appbar_state()` only sets on `taskbars.first()` |
| Foreground detection includes secondary taskbar | `Shell_SecondaryTrayWnd` is in the match list |

---

## 8. Tray Icon & UI

### 8.1 Tray Icon Lifecycle

```
add_notification_icon()
 ├── Shell_NotifyIconW(NIM_ADD)
 ├── Shell_NotifyIconW(NIM_SETVERSION)  ← Try upgrade to NOTIFYICON_VERSION_4
 │   ├── Success → tray_uses_version_4 = true
 │   └── Failure → tray_uses_version_4 = false (fallback compatibility mode)
 └── apply_window_identity()  ← Set AppUserModel properties

remove_notification_icon()
 └── Shell_NotifyIconW(NIM_DELETE)
```

**Explorer restart recovery**: Listens for `TaskbarCreated` message, re-calls `add_notification_icon()`.

### 8.2 Context Menu Interaction Flow

```
User right-clicks tray icon
 → handle_tray_callback()
   → menu_active = true
   → update_hooks()          ← Uninstall hooks to avoid interfering with menu
   → signal_taskbar_refresh() ← Show taskbar
   → TrackPopupMenu()         ← Modal menu (blocks until selection or cancel)
   → menu_active = false
   → update_hooks()           ← Reinstall hooks
   → refresh_foreground_state() ← Refresh foreground window state
   → signal_taskbar_refresh()  ← Hide/show taskbar based on state
   → handle_menu_command()     ← Process selected menu command
```

**Key design**: Taskbar is forced visible and hooks are uninstalled while the menu is open, ensuring menu operations are not affected by taskbar hiding.

### 8.3 Window Property Setting

`apply_window_identity()` sets window properties via `IPropertyStore`, enabling Win11 to correctly display the process icon:

| Property | Value |
|----------|-------|
| `System.AppUserModel.ID` | `melody0709.ButteryTaskbar_Rust` |
| `System.AppUserModel.RelaunchCommand` | Current exe path |
| `System.AppUserModel.RelaunchDisplayNameResource` | `Buttery Taskbar` |
| `System.AppUserModel.RelaunchIconResource` | Current exe path + icon resource ID |

---

## 9. COM Infrastructure

The project manually defines COM interfaces (`IUnknownVtable`, `IPropertyStoreVtable`) for calling `IPropertyStore` methods to set window properties.

```
ComGuard (RAII)
 ├── new() → CoInitializeEx(COINIT_APARTMENTTHREADED)
 └── Drop  → CoUninitialize()

WideStringPropVariant (RAII)
 ├── new(str) → PROPVARIANT { vt: VT_LPWSTR, ... }
 └── Drop     → PropVariantClear()
```

---

## 10. Bugs & Optimization Suggestions

### Bugs (Fixed in v2.5.1)

#### Bug 1: Pointer Truncation on 64-bit Systems (Fixed ✅)

**Problem**: `SetWindowLongW` / `GetWindowLongW` only operate on 32-bit values on 64-bit systems, truncating the upper 32 bits of pointers.

**Fix**: Replaced with `SetWindowLongPtrW` / `GetWindowLongPtrW`, using `isize` for pointer storage.

#### Bug 2: `current_millis()` Type Mismatch (Fixed ✅)

**Problem**: `GetTickCount64()` returns `u64`, but the value was cast to `i64`, which could theoretically overflow.

**Fix**: `current_millis()` return type changed to `u64`, `should_stay_visible_before` changed to `AtomicU64`.

#### Bug 3: Scroll Activation Pixel Matching Too Strict (Fixed ✅)

**Problem**: `mouse_y == rect.bottom - 1` was an exact pixel match, failing when the cursor was off by even 1 pixel.

**Fix**: Changed to 2-pixel activation zone:
```rust
if mouse_y >= rect.bottom - 2
    && mouse_y < rect.bottom
```

#### Bug 4: Scroll Activation Unreachable Under ABS_AUTOHIDE (Fixed ✅)

**Problem**: Scroll activation required `!should_show_taskbar()` to trigger, but `ABS_AUTOHIDE` made the taskbar already visible when the mouse was at the bottom edge, so the condition was never met.

**Fix**: Removed the `!should_show_taskbar()` condition. Scroll activation no longer handles showing the taskbar (handled by `ABS_AUTOHIDE` and the `WM_MOUSEMOVE` hook), it only simulates a Win key press to open the Start menu.

### 🟡 Optimization Suggestions

#### Optimization 1: Add Optional Logging

All errors are silently ignored (`let _ = ...`), making debugging very difficult. Suggested additions:
- Hook install/uninstall failures
- ShowWindow retry counts
- Config file read/write failures
- Foreground window class names (for debugging match logic)

#### Optimization 2: Scroll Activation for All Monitors

Currently only works on the primary monitor. Could extend to all monitors' bottom edges.

#### Optimization 3: `taskbar_windows()` Caching

Currently re-enumerates on every call. Could cache results and invalidate on `TaskbarCreated`.

#### Optimization 4: Auto-Update Check

The Jai version had this via WinHTTP + GitHub API. The Rust version only has an "Open releases page" link.

#### Optimization 5: Code Modularization

`main.rs` is ~1710 lines. Could be split by function:

```
src/
├── main.rs          ← Entry + message loop
├── config.rs        ← Config, HotkeyConfig, config read/write
├── hooks.rs         ← Hook install/uninstall/callbacks
├── taskbar.rs       ← Taskbar show/hide control, worker thread
├── tray.rs          ← Tray icon, context menu
├── hotkey_edit.rs   ← HotkeyEdit custom control
├── settings.rs      ← Settings dialog
└── win32.rs         ← COM, utility functions
```

---

## 11. Key Differences from Jai Version

| Aspect | Jai Version | Rust Version |
|--------|------------|-------------|
| Menu UI | Simp + GetRect custom-drawn OpenGL window | Native Windows popup menu + DialogBox |
| Update check | WinHTTP + GitHub API | Not implemented ("Open releases page" link only) |
| Config format | Fixed 512-byte binary | JSON (serde_json) |
| Config path | `%ProgramData%` | `%APPDATA%` |
| Toggle shortcut | Hardcoded Ctrl+Win+F11 | Customizable (HotkeyConfig + RegisterHotKey) |
| Foreground detection | RegisterShellHookWindow + HSHELL_* | SetWinEventHook(EVENT_SYSTEM_FOREGROUND) |
| Dark mode | ShouldAppsUseDarkMode + DwmSetWindowAttribute | Not implemented (native system theme) |
| Window properties | Not set | IPropertyStore sets AppUserModel properties |
| Icon embedding | Post-compile injection | Build-time via tauri-winres |

---

## 12. Key Code Location Index

| Functionality | Location (main.rs line numbers) |
|--------------|-------------------------------|
| Constants | 70-94 |
| COM infrastructure | 98-169 |
| HotkeyConfig struct | 171-225 |
| Config struct | 227-249 |
| AppState global state | 251-273 |
| main() entry | 275-334 |
| Hidden window creation | 340-378 |
| window_proc | 380-441 |
| Hotkey registration | 443-456 |
| Tray callback & menu | 458-618 |
| Exit & cleanup | 620-646 |
| Tray icon management | 648-731 |
| Foreground hook | 733-814 |
| Keyboard/mouse hooks | 825-960 |
| Taskbar worker thread | 962-1034 |
| Taskbar window finding | 1036-1095 |
| Signal & config persistence | 1097-1177 |
| Utility functions | 1179-1254 |
| HotkeyEdit control | 1256-1463 |
| Settings dialog | 1522-1710 |
