# 更新日志

本文件记录 Buttery Taskbar (Rust) 的所有重要变更。

[English](CHANGELOG.md) | 简体中文

## [2.5.1] - 2026-04-16

### 新增

- 托盘右键菜单现在显示当前版本号（灰色不可点击）
- 可执行文件嵌入 `ProductVersion` 和 `FileVersion` 元数据
- 新增发布构建脚本 (`build_release.bat`)，生成带版本号的 exe 输出

### 修复

- **设置对话框 64 位指针截断** — `SetWindowLongW`/`GetWindowLongW` 替换为 `SetWindowLongPtrW`/`GetWindowLongPtrW`，防止 64 位系统上指针截断，避免打开设置对话框时崩溃
- **滚轮激活不稳定** — 从精确 1 像素匹配 (`y == rect.bottom - 1`) 改为 2 像素激活区域 (`rect.bottom - 2 <= y < rect.bottom`)，显著提高屏幕底部边缘滚轮功能的可靠性
- **滚轮激活在 ABS_AUTOHIDE 下无法触发** — 移除 `!should_show_taskbar()` 条件，滚轮激活不再负责显示任务栏（由 `ABS_AUTOHIDE` 和 `WM_MOUSEMOVE` 钩子处理），仅模拟 Win 键打开开始菜单
- **`current_millis()` 类型不匹配** — 返回类型从 `i64` 改为 `u64`，`should_stay_visible_before` 从 `AtomicI64` 改为 `AtomicU64`，匹配 `GetTickCount64()` 语义，消除理论上的溢出风险
- **`handle_key_down` 无效返回值** — 从 `fn(...) -> bool`（始终返回 `false`）简化为 `fn(...)` 无返回值，移除误导性代码

### 变更

- 版本号在 `Cargo.toml`、`build.rs` 和托盘菜单中统一对齐为 `2.5.1`
- 图标嵌入库从 `winres` 迁移到 `tauri-winres`（图标资源 ID 变为 `32512`）
- 鼠标钩子新增 `WM_MOUSEMOVE` 处理，改善鼠标触底显示任务栏的可靠性
- 鼠标位置判断从 `primary_monitor_work_area()` 改为 `primary_monitor_rect()`，避免任务栏可见时工作区缩小导致边缘检测失败
- 菜单选项文字从 "Scroll to reveal taskbar" → "Edge activation" → "Scroll to open Start"
- 菜单选项文字从 "Auto-hide when disabled" → "Keep auto-hide when disabled"
