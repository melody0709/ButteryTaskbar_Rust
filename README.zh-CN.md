# Buttery Taskbar v2.5.1

[English](README.md) | 简体中文

本仓库是 Buttery Taskbar 的 Rust 重写版本，基于原版 Jai 实现的行为和功能集进行重构。

遗留的 Jai 源码仍保留在 `ButteryTaskbar2_jai/` 目录下，仓库根目录为活跃开发的 Rust 项目。

## 项目截图

<img src="assets/icon.webp" width="50%" />

<img src="assets/right.webp" width="50%" />

## 项目简介

Buttery Taskbar 比 Windows 自带的自动隐藏更"激进"地隐藏任务栏。任务栏只在真正需要时才出现——例如开始菜单、系统托盘溢出区等 Shell 界面激活时。

这个 Rust 版本是对原项目的重构，目标是保留用户体验行为，同时去除对 Jai 编译器的依赖，使 Windows 10/11 支持更易维护。

## 与原版的关系

- 原版上游项目：[LuisThiamNye/ButteryTaskbar2](https://github.com/LuisThiamNye/ButteryTaskbar2)
- 原版发布页：[ButteryTaskbar2 releases](https://github.com/LuisThiamNye/ButteryTaskbar2/releases)
- 本仓库中的遗留源码：`ButteryTaskbar2_jai/`

Rust 移植版基于原版的行为，包括：

- 基于托盘图标的控制流程
- 通过 Win32 API 控制任务栏显隐
- 可自定义切换快捷键（默认：`Ctrl` + `Win` + `F11`）
- 屏幕底部边缘滚轮激活
- 与 Windows 自动隐藏状态协调
- 在 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 注册开机自启

## 当前 Rust 实现功能

- 隐藏的 Win32 消息窗口
- 原生托盘图标与回调处理
- Win11 安全的托盘回调与右键菜单调用
- 原生弹出式托盘菜单（含版本号显示）
- 主显示器和副显示器任务栏显隐控制
- 开始菜单/任务栏等 Shell UI 可见性启发式检测
- 键盘钩子追踪 Win 键状态
- 通过 `RegisterHotKey` API 实现可自定义全局热键
- 鼠标钩子实现屏幕边缘唤出（2 像素激活区域）及底部边缘鼠标移动检测
- 配置持久化至 `%APPDATA%\Buttery Taskbar\config.json`
- 通过注册表 Run 键切换开机自启
- 通过 `tauri-winres` 嵌入应用图标

与原版 Jai 实现的差异：

- 原版自绘菜单 UI 已替换为原生 Windows 弹出菜单
- 原版 GitHub 更新检查 UI 尚未重新实现
- 配置格式从固定大小二进制块改为 JSON
- 切换快捷键从硬编码改为可自定义

## 项目结构

- `src/`：活跃的 Rust 实现
- `assets/`：应用资源，包括嵌入的应用图标
- `ButteryTaskbar2_jai/`：归档的遗留 Jai 实现，仅供参考

## 构建

环境要求：

- Windows
- Rust 工具链 + Cargo（edition 2024）
- MSVC 工具链 / Windows SDK

构建命令：

```pwsh
cargo build          # 开发构建
cargo build --release # 发布构建
```

发布二进制：

```text
target/release/buttery-taskbar.exe
```

一键构建带版本号的发布版：

```pwsh
.\build_release.bat
# 生成：target\release\buttery-taskbar_v2.5.1.exe
```

## 配置

配置文件：`%APPDATA%\Buttery Taskbar\config.json`

| 选项 | 类型 | 默认值 | 菜单文字 | 说明 |
|------|------|--------|---------|------|
| `enabled` | bool | `true` | Enabled | 任务栏隐藏主开关 |
| `scroll_activation_enabled` | bool | `true` | Scroll to open Start | 底部边缘滚轮打开开始菜单 |
| `toggle_shortcut` | object | `Win+Ctrl+F11` | Settings... | 可自定义切换快捷键 |
| `auto_launch_enabled` | bool | `false` | Start at log-in (non-admin) | 开机自启 |
| `autohide_when_disabled` | bool | 系统当前状态 | Keep auto-hide when disabled | 禁用 Buttery 时保持 Windows 自动隐藏 |

### 选项详解

#### Enabled（主开关）

启用时，Buttery 激进地隐藏任务栏，任务栏只在需要时出现（Win 键、开始菜单、托盘溢出等）。禁用时，任务栏恢复 Windows 默认行为。

#### Scroll to open Start（底部滚轮打开开始菜单）

控制是否允许在屏幕底部边缘通过鼠标滚轮打开开始菜单。

| Scroll to open Start | 鼠标触碰底部边缘 | 底部边缘滚轮 |
|---------------------|----------------|------------|
| ☑ 勾选 | 任务栏出现（钩子检测 + Windows 自动隐藏，更可靠） | 打开开始菜单 |
| ☐ 不勾选 | 任务栏出现（仅 Windows 自动隐藏，可能不太可靠） | 无特殊效果 |

**注意**：鼠标触碰底部边缘显示任务栏在 Enabled 勾选时始终生效，这是 `ABS_AUTOHIDE` 的副作用（`ABS_AUTOHIDE` 是任务栏可靠隐藏的必要条件）。勾选此选项时，鼠标钩子也会改善触底显示任务栏的可靠性。

#### Keep auto-hide when disabled（禁用时保持自动隐藏）

仅在 Buttery **被禁用**（Enabled 未勾选）时生效：

| Keep auto-hide | Buttery 禁用后的行为 |
|----------------|-------------------|
| ☑ 勾选 | 任务栏仍以 Windows 原生方式自动隐藏 |
| ☐ 不勾选 | 任务栏始终可见（恢复 Windows 默认） |

### 行为矩阵

| Enabled | Scroll to open Start | Keep auto-hide | 鼠标触碰底部 | 底部滚轮 | Win键/Shell UI |
|---------|---------------------|---------------|------------|---------|---------------|
| ✅ | ✅ | 任意 | ✅ 显示 | ✅ 打开开始菜单 | ✅ |
| ✅ | ❌ | 任意 | ✅ 显示 | ❌ 无特殊效果 | ✅ |
| ❌ | 任意 | ✅ | ✅ (Windows原生) | ❌ | ✅ |
| ❌ | 任意 | ❌ | ❌ (任务栏始终可见) | ❌ | ✅ |

## 运行时行为

- 按住 Win 键时任务栏可见
- 开始菜单、托盘溢出等 Shell UI 处于前台时任务栏可见
- 通过托盘图标打开右键菜单，菜单位置在任务栏边缘上方
- 启用时，鼠标触碰屏幕底部始终会显示任务栏（Windows 自动隐藏行为）
- Scroll to open Start 开启时，底部滚轮打开开始菜单
- Scroll to open Start 关闭时，底部滚轮无特殊效果
- 禁用时，可选择保持 Windows 原生自动隐藏模式

## 遗留参考

如需原版实现用于对比、调试或迁移：

- 本地遗留 README：`ButteryTaskbar2_jai/README.md`
- 本地遗留源码：`ButteryTaskbar2_jai/`
- 原版上游仓库：[LuisThiamNye/ButteryTaskbar2](https://github.com/LuisThiamNye/ButteryTaskbar2)
