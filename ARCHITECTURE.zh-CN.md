# ButteryTaskbar v2.5.1 架构文档

[English](ARCHITECTURE.md) | 简体中文

## 1. 项目概述

**Buttery Taskbar** 是一个用 Rust 编写的 Windows 任务栏增强工具，目标是比 Windows 自带的自动隐藏更"激进"地隐藏任务栏。任务栏只在真正需要时才出现——例如开始菜单、系统托盘溢出区等 Shell 界面激活时。

| 项目 | 说明 |
|------|------|
| 语言 | Rust (edition 2024) |
| 架构 | 单文件 `src/main.rs`（约 1710 行） |
| UI 框架 | 原生 Win32 API（无第三方 GUI 框架） |
| 运行时依赖 | `windows-sys`、`winreg`、`serde`/`serde_json` |
| 构建依赖 | `tauri-winres`（图标与资源嵌入） |
| 许可证 | EPL-2.0 |

---

## 2. 架构总览

### 2.1 双线程模型

```
┌─────────────────────────────────────────────────────────┐
│                      主线程 (Main Thread)                │
│                                                         │
│  Win32 消息循环 (GetMessageW / DispatchMessageW)        │
│  ├── 窗口过程 (window_proc)                             │
│  │   ├── 托盘图标回调                                   │
│  │   ├── 全局热键 (WM_HOTKEY)                           │
│  │   ├── 设置对话框                                     │
│  │   └── TaskbarCreated (Explorer 重启恢复)             │
│  ├── 前台窗口钩子回调 (foreground_event_proc)           │
│  ├── 键盘低级钩子回调 (keyboard_hook_proc)              │
│  └── 鼠标低级钩子回调 (mouse_hook_proc)                 │
│                                                         │
│  信号发送 ──sync_channel(1)──→                           │
└─────────────────────────────────────────────────────────┘
                            │
                            │ TaskbarSignal::Refresh / Exit
                            ▼
┌─────────────────────────────────────────────────────────┐
│                   任务栏工作线程 (Worker Thread)          │
│                                                         │
│  taskbar_worker()                                       │
│  └── apply_taskbar_state()                              │
│      ├── 模拟 Win 键按下 (SendInput)                    │
│      ├── 设置 AppBar 状态 (SHAppBarMessage)             │
│      └── 循环 ShowWindow(SW_HIDE / SW_SHOWNOACTIVATE)  │
│          └── 重试机制 (最多 60 次)                       │
└─────────────────────────────────────────────────────────┘
```

**设计原因**：任务栏的 `ShowWindow` 操作可能被 Windows 系统强制覆盖（例如 Explorer 重绘任务栏），需要重试循环保证可靠性。将此逻辑放在独立线程中，避免阻塞主线程的消息循环和钩子回调。

### 2.2 全局状态管理

全局状态通过 `OnceLock<AppState>` 实现单例模式，确保整个程序生命周期中只有一个实例：

```
AppState
├── config: Mutex<Config>              ← 配置（互斥锁保护）
├── config_path: PathBuf               ← 配置文件路径
├── quoted_exe_path: String            ← exe 路径（注册表自启用）
├── main_hwnd: AtomicIsize             ← 主窗口句柄
├── keyboard_hook: Mutex<isize>        ← 键盘钩子句柄
├── mouse_hook: Mutex<isize>           ← 鼠标钩子句柄
├── foreground_hook: Mutex<isize>      ← 前台钩子句柄
├── tray_uses_version_4: AtomicBool    ← 托盘图标协议版本
├── menu_active: AtomicBool            ← 右键菜单是否打开
├── is_finalizing: AtomicBool          ← 是否正在退出
├── should_show_due_to_focus: AtomicBool  ← 前台窗口需要显示
├── should_stay_visible_before: AtomicU64 ← 保持可见截止时间戳
├── is_win_key_down: AtomicBool        ← Win 键是否按下
├── win_key_press_requested: AtomicBool   ← 需要模拟 Win 键
├── taskbar_created_message: u32       ← TaskbarCreated 消息 ID
└── taskbar_tx: SyncSender<TaskbarSignal> ← 工作线程信号发送端
```

**并发策略**：
- `Mutex` 保护配置和钩子句柄（需要跨线程读写）
- `Atomic*` 用于高频状态标志（钩子回调和 UI 线程都需要访问）
- 所有原子操作统一使用 `Ordering::SeqCst`（强一致性，性能影响可忽略）

### 2.3 线程间通信

使用 `sync_channel(1)` 传递 `TaskbarSignal` 枚举：

```rust
enum TaskbarSignal {
    Refresh,  // 刷新任务栏状态
    Exit,     // 退出工作线程
}
```

- 容量为 1：多个 `Refresh` 信号可以合并，工作线程只需处理最新状态
- `try_send`：发送端不阻塞，通道满时丢弃信号（反正工作线程会处理）

---

## 3. 启动与退出流程

### 3.1 启动序列

```
main()
 │
 ├── 1. ComGuard::initialize()          ← COM 初始化（RAII 守卫）
 ├── 2. SetProcessDpiAwarenessContext() ← Per-Monitor DPI V2
 ├── 3. SetCurrentProcessExplicitAppUserModelID() ← 任务栏图标分组
 ├── 4. load_config()                   ← 加载 JSON 配置
 ├── 5. query_auto_launch_enabled()     ← 从注册表同步自启状态
 ├── 6. RegisterWindowMessageW("TaskbarCreated") ← Explorer 重启消息
 ├── 7. sync_channel(1)                 ← 创建信号通道
 ├── 8. APP.set(AppState{...})          ← 初始化全局单例
 ├── 9. thread::spawn(taskbar_worker)   ← 启动工作线程
 ├── 10. create_hidden_window()         ← 创建隐藏窗口
 │       └── WM_CREATE → add_notification_icon() + register_hotkeys()
 ├── 11. install_foreground_hook()      ← 安装前台窗口切换钩子
 ├── 12. refresh_foreground_state()     ← 初始前台窗口检测
 ├── 13. update_hooks()                 ← 根据配置安装键盘/鼠标钩子
 ├── 14. signal_taskbar_refresh()       ← 触发首次任务栏状态刷新
 └── 15. 消息循环 (GetMessageW loop)
```

### 3.2 退出流程

```
request_quit()
 │
 ├── is_finalizing.swap(true)           ← 一次性保护，防止重复退出
 ├── menu_active = false
 ├── update_hooks()                     ← 卸载所有钩子
 ├── restore_taskbar_now(true)          ← 恢复任务栏可见
 ├── taskbar_tx.try_send(Exit)          ← 通知工作线程退出
 ├── DestroyWindow(hwnd)                ← 销毁主窗口
 │   └── WM_DESTROY
 │       ├── UnregisterHotKey()
 │       ├── cleanup_on_destroy()
 │       │   ├── remove_notification_icon()
 │       │   ├── uninstall_foreground_hook()
 │       │   ├── uninstall_hook(keyboard)
 │       │   ├── uninstall_hook(mouse)
 │       │   └── restore_taskbar_now(true)  ← 再次确保恢复
 │       └── PostQuitMessage(0)         ← 终止消息循环
 └── (工作线程收到 Exit 信号后自行退出)
```

---

## 4. 核心机制详解

### 4.1 任务栏显隐控制

采用**双重策略**确保任务栏可靠隐藏：

**策略一：ShowWindow 直接控制**

```rust
ShowWindow(hwnd, SW_HIDE);           // 隐藏
ShowWindow(hwnd, SW_SHOWNOACTIVATE); // 显示（不抢焦点）
```

对**所有**任务栏窗口（主 + 副显示器）统一执行。

**策略二：AppBar 自动隐藏状态**

```rust
SHAppBarMessage(ABM_SETSTATE, ABS_AUTOHIDE); // 设置自动隐藏
SHAppBarMessage(ABM_SETSTATE, 0);            // 取消自动隐藏
```

仅对主任务栏（`Shell_TrayWnd`）设置，因为 AppBar 状态是全局的。设置条件：

| 条件 | ABS_AUTOHIDE |
|------|-------------|
| `enabled` | ✅ 设置（任务栏隐藏的必要条件） |
| `!enabled && autohide_when_disabled` | ✅ 设置（禁用时保持自动隐藏） |
| `!enabled && !autohide_when_disabled` | ❌ 不设置 |

**注意**：`ABS_AUTOHIDE` 是任务栏可靠隐藏的必要条件。没有它，Windows 会不断对抗 `ShowWindow(SW_HIDE)`，导致任务栏无法隐藏。副作用是鼠标触碰屏幕底部时 Windows 会自动显示任务栏，这是 Windows 原生行为，不受 "Scroll to open Start" 选项控制。

### 4.2 显示决策逻辑

`should_show_taskbar()` 综合四个条件判断是否显示任务栏：

```
should_show_taskbar() = 
    menu_active                    ← 右键菜单打开时
    || should_show_due_to_focus    ← 前台窗口需要任务栏时
    || is_win_key_down             ← Win 键按住时
    || current_millis() < should_stay_visible_before  ← Win 键释放后 400ms 内
```

### 4.3 重试循环

`apply_taskbar_state()` 中的重试循环（最多 60 次尝试）应对 Windows 强制恢复任务栏的情况：

| 场景 | 行为 | 等待时间 | 尝试消耗 |
|------|------|---------|---------|
| Win 键保持期 | 持续轮询，重置计数 | 100ms | 0（重置为 0） |
| ShowWindow 失败 | 重试 | 10ms | 1 |
| 成功隐藏 | 额外验证 | 50ms | 8 |
| 成功显示 | 直接退出 | — | — |

### 4.4 前台窗口检测

通过 `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` 监听前台窗口切换，判断新窗口是否需要显示任务栏：

**需要显示任务栏的窗口类名**：

| 类名 | 对应界面 |
|------|---------|
| `Windows.UI.Core.CoreWindow` | 开始菜单、搜索、通知中心、日历弹窗等 UWP Shell 界面 |
| `Shell_TrayWnd` | 主显示器任务栏本身 |
| `Shell_SecondaryTrayWnd` | 副显示器任务栏 |
| `TopLevelWindowForOverflowXamlIsland` | 系统托盘溢出区 |
| `XamlExplorerHostIslandWindow` | 文件资源管理器相关的 Xaml 宿主窗口 |
| `NotifyIconOverflowWindow` | 通知图标溢出窗口 |

### 4.5 键盘钩子（Win 键追踪）

`WH_KEYBOARD_LL` 低级键盘钩子专门追踪 Win 键状态：

- **Win 键按下**：设置 `is_win_key_down = true`，触发任务栏显示
- **Win 键释放**：检查另一个 Win 键是否仍按住，设置 400ms 保持期
- **不拦截任何按键**：始终调用 `CallNextHookEx`，仅观察状态变化

**400ms 缓冲期**：Win 键释放后任务栏保持可见 400ms，避免开始菜单弹出时任务栏闪烁。

### 4.6 鼠标钩子（滚轮激活与边缘检测）

`WH_MOUSE_LL` 低级鼠标钩子同时处理 `WM_MOUSEMOVE` 和 `WM_MOUSEWHEEL`：

**鼠标移动 (WM_MOUSEMOVE)**：
```
触发条件：
  1. scroll_activation_enabled = true（即 "Scroll to open Start" 勾选）
  2. 鼠标位于主显示器底部 2 像素区域 (monitor.bottom - 2 <= y < monitor.bottom)
  3. 鼠标 x 坐标在显示器水平范围内
  4. 任务栏当前应隐藏 (!should_show_taskbar())

触发效果：
  1. should_show_due_to_focus = true → 显示任务栏
```

**鼠标滚轮 (WM_MOUSEWHEEL)**：
```
触发条件：
  1. scroll_activation_enabled = true（即 "Scroll to open Start" 勾选）
  2. 鼠标位于主显示器底部 2 像素区域
  3. 鼠标 x 坐标在显示器水平范围内

触发效果：
  1. win_key_press_requested = true → 模拟 Win 键按下（打开开始菜单）
  2. 拦截滚轮事件（return 1，不传递给其他应用）
```

**"Scroll to open Start" 控制鼠标钩子**：勾选时安装鼠标钩子——底部边缘鼠标移动显示任务栏（改善 Windows 原生自动隐藏的可靠性），底部边缘滚轮打开开始菜单；不勾选时卸载鼠标钩子。鼠标触碰底部显示任务栏是 `ABS_AUTOHIDE` 的副作用，不受此选项控制。

**⚠️ 已知限制**：滚轮激活仅在主显示器底部边缘生效，副显示器不支持。

---

## 5. 配置选项详解

### 5.1 配置文件

路径：`%APPDATA%\Buttery Taskbar\config.json`

```rust
struct Config {
    version: u32,                     // 配置版本号，当前固定为 1
    enabled: bool,                    // 是否启用任务栏隐藏
    scroll_activation_enabled: bool,  // 是否启用滚轮激活
    toggle_shortcut: HotkeyConfig,    // 切换快捷键
    auto_launch_enabled: bool,        // 是否开机自启
    autohide_when_disabled: bool,     // 禁用时是否保持自动隐藏
}
```

### 5.2 各选项详细说明

#### `enabled` (bool, 默认: true)

**核心开关**。控制 Buttery Taskbar 的任务栏隐藏功能是否启用。

| 状态 | 行为 |
|------|------|
| `true` | 程序主动隐藏任务栏，仅在 Shell UI 激活时显示。键盘钩子生效（追踪 Win 键）。 |
| `false` | 任务栏始终可见，不安装键盘钩子。如果 `autohide_when_disabled` 为 true，则设置 Windows 原生自动隐藏。 |

**交互方式**：
- 托盘菜单 "Enabled" 复选框
- 全局热键切换

**状态切换副作用**：
- `true → false`：`should_show_due_to_focus = true`（立即显示任务栏）
- `false → true`：`refresh_foreground_state()`（根据当前前台窗口决定是否显示）

#### `scroll_activation_enabled` (bool, 默认: true)

**Scroll to open Start**。控制是否允许在屏幕底部边缘通过鼠标滚轮打开开始菜单。

| 状态 | 行为 |
|------|------|
| `true` | 安装鼠标低级钩子。底部边缘滚轮 → 打开开始菜单。底部边缘鼠标移动 → 显示任务栏（改善可靠性）。 |
| `false` | 不安装鼠标钩子，底部边缘滚轮无特殊效果。 |

**注意**：此选项控制鼠标钩子。鼠标触碰屏幕底部时 Windows 自动显示任务栏是 `ABS_AUTOHIDE` 的副作用（只要 Enabled 勾选就会发生），但钩子能改善此行为的可靠性。

**交互方式**：托盘菜单 "Scroll to open Start" 复选框

#### `toggle_shortcut` (HotkeyConfig, 默认: Win+Ctrl+F11)

**切换快捷键**。用于快速切换 `enabled` 状态的全局热键。

```rust
struct HotkeyConfig {
    win: bool,    // Win 修饰键
    ctrl: bool,   // Ctrl 修饰键
    shift: bool,  // Shift 修饰键
    alt: bool,    // Alt 修饰键
    key: u32,     // 虚拟键码 (VK code)，0 表示无快捷键
}
```

**特殊规则**：
- 注册时自动附加 `MOD_NOREPEAT`，防止按住不放重复触发
- 用户设置时如果没有修饰键，自动添加 `Alt`
- 快捷键冲突时弹出 "Shortcut Conflict" 警告
- `key == 0` 表示无快捷键（不注册热键）

**交互方式**：设置对话框中的 HotkeyEdit 控件

#### `auto_launch_enabled` (bool, 默认: false)

**开机自启**。是否在用户登录时自动启动。

| 状态 | 行为 |
|------|------|
| `true` | 在注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 下写入 `"Buttery Taskbar" = "C:\path\buttery-taskbar.exe"` |
| `false` | 删除该注册表键值 |

**注意**：每次启动时从注册表读取实际状态覆盖配置文件中的值，确保与系统实际状态一致。

**交互方式**：托盘菜单 "Start at log-in (non-admin)" 复选框

#### `autohide_when_disabled` (bool, 默认: 系统当前状态)

**禁用时保持自动隐藏**。当 Buttery Taskbar 被禁用（`enabled = false`）时，是否仍将任务栏设为 Windows 原生自动隐藏模式。

| 状态 | 行为 |
|------|------|
| `true` | 禁用时设置 `SHAppBarMessage(ABM_SETSTATE, ABS_AUTOHIDE)`，任务栏以 Windows 原生自动隐藏模式运行 |
| `false` | 禁用时取消自动隐藏 `SHAppBarMessage(ABM_SETSTATE, 0)`，任务栏始终可见 |

**默认值**：通过 `system_taskbar_autohide_enabled()` 读取当前系统任务栏自动隐藏状态，保持用户原有设置。

**交互方式**：托盘菜单 "Keep auto-hide when disabled" 复选框

### 5.3 托盘菜单选项映射

| 菜单项 | 命令 ID | 配置字段 | 类型 |
|--------|---------|---------|------|
| Enabled | CMD_TOGGLE_ENABLED (1001) | `enabled` | 复选框 |
| Settings... | CMD_SETTINGS (1002) | `toggle_shortcut` | 按钮（打开对话框） |
| Scroll to open Start | CMD_TOGGLE_SCROLL (1003) | `scroll_activation_enabled` | 复选框 |
| Keep auto-hide when disabled | CMD_TOGGLE_AUTOHIDE (1004) | `autohide_when_disabled` | 复选框 |
| Start at log-in (non-admin) | CMD_TOGGLE_STARTUP (1005) | `auto_launch_enabled` | 复选框 |
| Open releases page | CMD_OPEN_RELEASES (1006) | — | 链接 |
| Quit | CMD_QUIT (1999) | — | 退出 |

### 5.4 设置对话框

通过托盘菜单 "Settings..." 打开，标题 "Settings - Shortcuts"。

**HotkeyEdit 自定义控件**（窗口类 `Buttery.HotkeyEdit`）：

| 操作 | 行为 |
|------|------|
| 获得焦点 | 进入捕获模式，背景变为淡黄色 |
| 按下非修饰键 | 结合当前修饰键组合成新快捷键 |
| 单独按修饰键 | 忽略 |
| Escape | 恢复为原始快捷键 |
| Backspace / Delete | 清除快捷键 |
| 无修饰键时录入 | 自动添加 Alt 修饰键 |

---

## 6. 钩子系统

### 6.1 三种钩子

| 钩子 | 类型 | 用途 | 安装条件 |
|------|------|------|---------|
| 前台窗口钩子 | `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` | 监听前台窗口切换 | 始终安装（不受 enabled 控制） |
| 键盘低级钩子 | `SetWindowsHookExW(WH_KEYBOARD_LL)` | 追踪 Win 键状态 | `enabled && !menu_active && !finalizing` |
| 鼠标低级钩子 | `SetWindowsHookExW(WH_MOUSE_LL)` | 滚轮激活 + 改善边缘检测 | `scroll_activation_enabled && !menu_active && !finalizing` |

### 6.2 钩子动态管理

`update_hooks()` 根据当前状态动态安装/卸载钩子：

```
update_hooks()
 ├── should_hook_keyboard = !finalizing && !menu_active && config.enabled
 ├── should_hook_mouse    = !finalizing && !menu_active && config.scroll_activation_enabled
 ├── install_or_remove_hook(keyboard_hook, should_hook_keyboard, ...)
 └── install_or_remove_hook(mouse_hook, should_hook_mouse, ...)
```

**调用时机**：
- 配置变更（enabled / scroll_activation_enabled 切换）
- 菜单打开/关闭
- 程序退出

---

## 7. 多显示器支持

### 7.1 任务栏窗口枚举

`taskbar_windows()` 枚举所有显示器上的任务栏窗口：

```
Shell_TrayWnd           → 主显示器任务栏（仅一个）
Shell_SecondaryTrayWnd  → 副显示器任务栏（可能有多个）
```

使用 `FindWindowExW` 的 `hwndChildAfter` 参数遍历所有副任务栏窗口。

### 7.2 显隐操作覆盖

`apply_taskbar_state()` 中对所有枚举到的任务栏窗口统一执行 `ShowWindow`，确保多显示器场景下所有任务栏同步隐藏/显示。

### 7.3 已知限制

| 限制 | 说明 |
|------|------|
| 滚轮激活仅主显示器 | `handle_mouse_scroll()` 使用 `primary_monitor_rect()` 判断鼠标位置 |
| AppBar 状态仅设主任务栏 | `set_taskbar_appbar_state()` 只对 `taskbars.first()` 设置 |
| 前台检测包含副任务栏 | `Shell_SecondaryTrayWnd` 在匹配列表中，副任务栏获焦时会显示所有任务栏 |

---

## 8. 托盘图标与 UI

### 8.1 托盘图标生命周期

```
add_notification_icon()
 ├── Shell_NotifyIconW(NIM_ADD)
 ├── Shell_NotifyIconW(NIM_SETVERSION)  ← 尝试升级到 NOTIFYICON_VERSION_4
 │   ├── 成功 → tray_uses_version_4 = true
 │   └── 失败 → tray_uses_version_4 = false（回退兼容模式）
 └── apply_window_identity()  ← 设置 AppUserModel 属性

remove_notification_icon()
 └── Shell_NotifyIconW(NIM_DELETE)
```

**Explorer 重启恢复**：监听 `TaskbarCreated` 消息，重新调用 `add_notification_icon()`。

### 8.2 右键菜单交互流程

```
用户右键托盘图标
 → handle_tray_callback()
   → menu_active = true
   → update_hooks()          ← 卸载钩子，避免干扰菜单操作
   → signal_taskbar_refresh() ← 显示任务栏
   → TrackPopupMenu()         ← 模态菜单（阻塞直到选择或取消）
   → menu_active = false
   → update_hooks()           ← 重新安装钩子
   → refresh_foreground_state() ← 刷新前台窗口状态
   → signal_taskbar_refresh()  ← 根据状态隐藏/显示任务栏
   → handle_menu_command()     ← 处理选中的菜单命令
```

**关键设计**：菜单打开期间强制显示任务栏并卸载钩子，确保菜单操作不受任务栏隐藏影响。

### 8.3 窗口属性设置

`apply_window_identity()` 通过 `IPropertyStore` 设置窗口属性，使 Win11 能正确显示进程图标：

| 属性 | 值 |
|------|-----|
| `System.AppUserModel.ID` | `melody0709.ButteryTaskbar_Rust` |
| `System.AppUserModel.RelaunchCommand` | 当前 exe 路径 |
| `System.AppUserModel.RelaunchDisplayNameResource` | `Buttery Taskbar` |
| `System.AppUserModel.RelaunchIconResource` | 当前 exe 路径 + 图标资源 ID |

---

## 9. COM 基础设施

项目手动定义了 COM 接口（`IUnknownVtable`、`IPropertyStoreVtable`），用于调用 `IPropertyStore` 的方法设置窗口属性。

```
ComGuard (RAII)
 ├── new() → CoInitializeEx(COINIT_APARTMENTTHREADED)
 └── Drop  → CoUninitialize()

WideStringPropVariant (RAII)
 ├── new(str) → PROPVARIANT { vt: VT_LPWSTR, ... }
 └── Drop     → PropVariantClear()
```

---

## 10. 发现的 Bug 与优化建议

### Bug（v2.5.1 已修复）

#### Bug 1：64 位系统上指针截断（已修复 ✅）

**位置**：~~main.rs:1302、main.rs:1535~~

**问题**：`SetWindowLongW` / `GetWindowLongW` 在 64 位系统上只操作 32 位值，将 64 位指针转为 `i32` 会截断高 32 位。

**修复**：已替换为 `SetWindowLongPtrW` / `GetWindowLongPtrW`，使用 `isize` 传递指针。

#### Bug 2：`current_millis()` 类型不严谨（已修复 ✅）

**位置**：~~main.rs:1179~~

**问题**：`GetTickCount64()` 返回 `u64`，转为 `i64` 后理论上可能溢出。

**修复**：`current_millis()` 返回类型改为 `u64`，`should_stay_visible_before` 改为 `AtomicU64`。

#### Bug 3：滚轮激活像素匹配过于严格（已修复 ✅）

**位置**：[main.rs:924](src/main.rs#L924)

**原代码**：
```rust
if mouse_y == rect.bottom - 1  // ❌ 精确像素匹配，差 1 像素就失败
```

**问题**：`mouse_y == rect.bottom - 1` 是精确像素匹配，只接受鼠标恰好在屏幕底部倒数第 1 个像素行的情况。实际使用中，鼠标在屏幕底部时 y 坐标可能是 `rect.bottom - 2` 或 `rect.bottom - 1`，取决于光标裁剪、DPI 缩放等因素。这导致滚轮激活非常不稳定——有时能触发，有时只显示窗口调整大小的光标。

**修复**：改为 2 像素范围的匹配：
```rust
if mouse_y >= rect.bottom - 2  // ✅ 2 像素激活区域
    && mouse_y < rect.bottom
```

#### Bug 4：滚轮激活在 ABS_AUTOHIDE 下无法触发（已修复 ✅）

**问题**：滚轮激活要求 `!should_show_taskbar()` 才触发，但 `ABS_AUTOHIDE` 使鼠标触底时任务栏已经显示，条件永远不满足，导致滚轮激活功能形同虚设。

**修复**：移除 `!should_show_taskbar()` 条件，滚轮激活不再负责显示任务栏（由 `ABS_AUTOHIDE` 处理），仅负责模拟 Win 键打开开始菜单。同时移除了不再需要的 `scroll_reveal_until` 超时机制。

### 🟡 优化建议

#### 优化 1：增加可选日志系统

当前所有错误静默忽略（`let _ = ...`），调试非常困难。建议增加可选的日志输出：

- 钩子安装/卸载失败
- ShowWindow 重试次数
- 配置文件读写失败
- 前台窗口类名（用于调试匹配逻辑）

可通过配置项或环境变量控制日志级别。

#### 优化 2：滚轮激活支持多显示器

当前 `handle_mouse_scroll()` 仅使用 `primary_monitor_rect()`，副显示器底部边缘的滚轮操作不会触发激活。建议遍历所有显示器的区域，判断鼠标是否在任一显示器的底部边缘。

#### 优化 3：`taskbar_windows()` 缓存

`taskbar_windows()` 每次调用都执行 `FindWindowW` + `FindWindowExW` 循环。在工作线程的重试循环中可能被频繁调用。可考虑缓存结果，并在 `TaskbarCreated` 消息时失效。

#### 优化 4：常量字符串预分配

`to_wide()` 每次调用都分配新的 `Vec<u16>`。频繁使用的常量字符串（如 `"Shell_TrayWnd"`、`"Shell_SecondaryTrayWnd"`）可预分配为静态变量。

#### 优化 5：自动更新检查

Jai 版本通过 WinHTTP 访问 GitHub API 检查更新。Rust 版本仅有 "Open releases page" 链接，建议增加自动更新检查功能。

#### 优化 6：原子操作内存序优化

当前所有原子操作使用 `Ordering::SeqCst`。部分场景可使用 `Ordering::Acquire` / `Ordering::Release` 降低开销，例如：
- `is_win_key_down`：钩子线程写，工作线程读，`Release/Acquire` 足够
- `menu_active`：主线程写，工作线程读，`Release/Acquire` 足够

但在这种低频操作场景下，性能差异可忽略。

#### 优化 7：代码模块化

`main.rs` 已约 1710 行，可考虑按功能拆分为模块：

```
src/
├── main.rs          ← 入口 + 消息循环
├── config.rs        ← Config、HotkeyConfig、配置读写
├── hooks.rs         ← 钩子安装/卸载/回调
├── taskbar.rs       ← 任务栏显隐控制、工作线程
├── tray.rs          ← 托盘图标、右键菜单
├── hotkey_edit.rs   ← HotkeyEdit 自定义控件
├── settings.rs      ← 设置对话框
└── win32.rs         ← COM、工具函数
```

#### 优化 8：`handle_key_down` 返回值（已修复 ✅）

`handle_key_down()` 已简化为无返回值函数，移除了始终为 `false` 的返回值和 `keyboard_hook_proc` 中不可达的拦截逻辑。

---

## 11. 与 Jai 版本的关键差异

| 方面 | Jai 版 | Rust 版 |
|------|--------|---------|
| 菜单 UI | Simp + GetRect 自绘 OpenGL 窗口 | 原生 Windows 弹出菜单 + DialogBox |
| 更新检查 | WinHTTP 访问 GitHub API | 未实现（仅 "Open releases page" 链接） |
| 配置格式 | 固定 512 字节二进制 | JSON（serde_json） |
| 配置路径 | `%ProgramData%` | `%APPDATA%` |
| 快捷键 | 硬编码 Ctrl+Win+F11 | 可自定义（HotkeyConfig + RegisterHotKey） |
| 前台检测 | RegisterShellHookWindow + HSHELL_* | SetWinEventHook(EVENT_SYSTEM_FOREGROUND) |
| 深色模式 | ShouldAppsUseDarkMode + DwmSetWindowAttribute | 未实现（原生系统主题） |
| 窗口属性 | 未设置 | IPropertyStore 设置 AppUserModel 属性 |
| 图标嵌入 | 编译后注入 | 构建时 tauri-winres 嵌入 |

---

## 12. 关键代码位置索引

| 功能 | 位置 (main.rs 行号) |
|------|---------------------|
| 常量定义 | 70-94 |
| COM 基础设施 | 98-169 |
| HotkeyConfig 结构体 | 171-225 |
| Config 结构体 | 227-249 |
| AppState 全局状态 | 251-273 |
| main() 入口 | 275-334 |
| 隐藏窗口创建 | 340-378 |
| window_proc | 380-441 |
| 热键注册 | 443-456 |
| 托盘回调与菜单 | 458-618 |
| 退出与清理 | 620-646 |
| 托盘图标管理 | 648-731 |
| 前台窗口钩子 | 733-814 |
| 键盘/鼠标钩子 | 825-960 |
| 任务栏工作线程 | 962-1034 |
| 任务栏窗口查找 | 1036-1095 |
| 信号与配置持久化 | 1097-1177 |
| 工具函数 | 1179-1254 |
| HotkeyEdit 控件 | 1256-1463 |
| 设置对话框 | 1522-1710 |
