# AGENTS.md

## 项目

Rust 编写的 Windows 任务栏增强工具，用于更激进地隐藏任务栏。项目根目录为活跃开发的 Rust 实现，`ButteryTaskbar2_jai/` 包含遗留的 Jai 参考实现。

## 构建

```pwsh
cargo build          # 开发构建
cargo build --release # 发布构建
```

- 发布二进制：`target/release/buttery-taskbar.exe`
- 一键构建：`.\build_release.bat` → `target\release\buttery-taskbar_v{版本号}.exe`

## 环境

- Windows + Rust toolchain + Cargo
- MSVC toolchain / Windows SDK
- edition = "2024"（需要较新 Rust 版本）

## 依赖

- `windows-sys`：Win32 API 调用（任务栏控制、钩子）
- `winreg`：注册表操作（开机自启）
- `serde` / `serde_json`：配置序列化
- `tauri-winres`（build-dependencies）：图标与资源嵌入

## 配置

`%APPDATA%\Buttery Taskbar\config.json`

## 目录

- `src/`：活跃的 Rust 实现（单文件 main.rs）
- `assets/`：应用资源（图标等）
- `ButteryTaskbar2_jai/`：遗留 Jai 实现，仅供参考

## 踩坑规则

> AI 在完成重大修改或解决复杂报错后，可追加规则。

- `tauri-winres` 嵌入的图标资源 ID 为 `32512`（不是 `1`），代码中 `APP_ICON_RESOURCE_ID` 必须与之匹配
- `SetWindowLongPtrW` / `GetWindowLongPtrW` 用于 64 位安全的指针存储，不要回退到 `SetWindowLongW`
- 滚轮激活使用 2 像素区域（`rect.bottom - 2` 到 `rect.bottom`），不要改为精确 1 像素匹配
- `current_millis()` 返回 `u64`，`should_stay_visible_before` 为 `AtomicU64`，不要用有符号类型
- `build_release.bat` 使用 `findstr /b "version"` 匹配行首的 version，避免匹配到依赖版本号
