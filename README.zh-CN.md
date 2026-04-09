# Buttery Taskbar（Rust 重构版） — v2.5.0

发行：v2.5.0 — 可配置全局快捷键、Win11 兼容修复与 Rust 重构。

[English](README.md) | 简体中文

这个仓库现在承载的是 Buttery Taskbar 的 Rust 重构版本，行为目标以原来的 Jai 版本为基准。

旧版 Jai 工程仍然完整保留在 `ButteryTaskbar2_jai/` 目录中，仓库根目录则只保留新的 Rust 项目。

## 项目截图

<img src="assets/icon.webp" width="50%" />

<img src="assets/right.webp" width="50%" />

## 项目说明

Buttery Taskbar 的目标是比 Windows 自带的自动隐藏更“激进”地隐藏任务栏。只有在真正需要的时候，比如开始菜单、托盘溢出区或其他 Shell 界面激活时，任务栏才会重新出现。

这次 Rust 版本不是简单改壳，而是以旧项目功能为基准进行重构，目的是摆脱 Jai 编译环境依赖，同时更容易维护 Windows 10 和 Windows 11 的兼容性。

## 与旧版的关系

- 原始上游仓库：[LuisThiamNye/ButteryTaskbar2](https://github.com/LuisThiamNye/ButteryTaskbar2)
- 原始上游发布页：[ButteryTaskbar2 releases](https://github.com/LuisThiamNye/ButteryTaskbar2/releases)
- 本仓库内保留的旧版源码：`ButteryTaskbar2_jai/`

Rust 重构版沿用了旧版的核心设计，包括：

- 托盘图标驱动的控制入口
- 基于 Win32 API 的任务栏显隐控制
- 可配置的全局启用/禁用快捷键（默认值为 `Ctrl` + `Win` + `F11`）
- 屏幕底边滚轮唤出逻辑
- 与 Windows 自动隐藏状态协同
- `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 的开机启动开关

## 当前 Rust 版已实现功能

- 隐藏 Win32 消息窗口
- 原生托盘图标与回调处理
- 面向 Win11 的托盘右键菜单兼容处理
- 原生 Windows 弹出菜单
- 主任务栏与副任务栏的显隐控制
- 对开始菜单、托盘溢出区等 Shell 前台窗口的可见性判断
- Windows 键与可配置全局快捷键的低级键盘钩子
- 底边滚轮触发的鼠标钩子
- `%APPDATA%\Buttery Taskbar\config.json` 配置持久化
- 基于注册表的开机启动开关
- 从旧版发布包中提取并嵌入的新程序图标

## 与旧版目前的差异

- 旧版自绘菜单界面暂时改成了原生 Windows 右键菜单
- 旧版菜单里的 GitHub 更新检查状态暂时还没有重做
- 配置文件格式从旧版固定长度二进制改成了 JSON

## 目录结构

- `src/`：当前 Rust 实现
- `assets/`：Rust 版资源文件，包括嵌入式图标
- `ButteryTaskbar2_jai/`：保留的旧版 Jai 工程，用于参考和功能对照

## 构建方法

要求：

- Windows
- 已安装 Rust / Cargo
- 可供 Cargo 调用的 MSVC 工具链和 Windows SDK 资源编译工具

命令：

```pwsh
cargo build
cargo build --release
```

Release 产物：

```text
target/release/buttery-taskbar.exe
```

## 当前运行行为

Rust 版目前保留了以下行为模型：

- 按住 Windows 键时显示任务栏
- 当前台是开始菜单、托盘溢出区等 Shell 界面时显示任务栏
- 从托盘图标打开右键菜单，菜单会尽量浮在任务栏上方
- 启用滚轮功能后，在主显示器底边滚动会模拟一次 Windows 键以唤出开始菜单
- 程序禁用时，可以按设置保留 Windows 自带自动隐藏

## 自定义启用/禁用快捷键

全局启用/禁用快捷键可以通过 `%APPDATA%\Buttery Taskbar\config.json` 配置。

也可以从托盘图标右键菜单里的 `Edit shortcut settings...` 直接打开这个配置文件进行修改。

相关字段：

- `toggle_shortcut_enabled`：是否启用全局快捷键
- `toggle_shortcut`：快捷键字符串，默认值为 `Ctrl+Win+F11`

支持的格式：

- 零个或多个修饰键：`Ctrl`、`Alt`、`Shift`、`Win`
- 再加上一个普通按键，例如 `A`、`5`、`F10`、`Pause`、`Insert`、`Delete`、`Home`、`End`、`PageUp`、`PageDown`、`Up`、`Down`、`Left`、`Right`

示例：

```json
{
	"toggle_shortcut": "Ctrl+Alt+B"
}
```

```json
{
	"toggle_shortcut": "Shift+F10"
}
```

```json
{
	"toggle_shortcut": "Win+Pause"
}
```

如果快捷键字符串无效，程序会自动回退到默认快捷键。手动修改配置文件后需要重启程序才能生效。

## 旧版参考

如果你需要查看旧版实现、做行为对照或继续迁移：

- 旧版本地说明：`ButteryTaskbar2_jai/README.md`
- 旧版本地源码：`ButteryTaskbar2_jai/`
- 旧版上游仓库：[LuisThiamNye/ButteryTaskbar2](https://github.com/LuisThiamNye/ButteryTaskbar2)