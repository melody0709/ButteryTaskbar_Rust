***

name: "release"
description: "Buttery Taskbar 发布流程：版本号迭代、更新文档、构建 exe、git push/tag、GitHub Release。Invoke when user says '发布' or 'release'."
--------------------------------------------------------------------------------------------------------------------------

# Buttery Taskbar Release Skill

执行 Buttery Taskbar 项目的完整发布流程。

## 触发条件

用户说"发布"、"release"、"发版"等关键词，或明确要求执行发布流程。

## 版本号规则

- 版本号定义在 `Cargo.toml` 的 `version = "X.Y.Z"` 中
- `build.rs` 中的 `ProductVersion` / `FileVersion` 需同步更新
- `src/main.rs` 中 `APP_VERSION` 通过 `env!("CARGO_PKG_VERSION")` 自动获取，无需手动修改
- **默认迭代**：取当前最新 git tag，最小一级 +1（如 v2.5.1 → v2.5.2）
- **用户指定**：用户可明确指定版本号（如"发布 v2.6.0"），则使用指定版本
- 获取最新 tag：`git tag --sort=-v:refname | Select-Object -First 1`

## 执行步骤

### 步骤 1：确认版本号

1. 运行 `git tag --sort=-v:refname | Select-Object -First 1` 获取最新 tag
2. 按规则计算新版本号
3. 用 TodoWrite 创建任务列表

### 步骤 2：更新版本号

1. 修改 `Cargo.toml` 中的 `version`：

```toml
version = "X.Y.Z"
```

1. 修改 `build.rs` 中的 `ProductVersion` 和 `FileVersion`：

```rust
res.set("ProductVersion", "X.Y.Z");
res.set("FileVersion", "X.Y.Z");
```

### 步骤 3：更新 CHANGELOG.md 和 CHANGELOG.zh-CN.md，但不要删除以往记录

1. 运行 `git log <prev_tag>..HEAD --oneline` 获取自上次发布以来的变更
2. 根据变更内容，按以下分类整理（参考已有格式）：
   - **Added**：新功能
   - **Fixed**：bug 修复
   - **Changed**：行为变更
3. 在 `CHANGELOG.md` 顶部（`# Changelog` 标题和语言切换链接之后）插入新版本条目：

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added / Fixed / Changed

- **简要描述**: 详细说明
```

4. 同步更新 `CHANGELOG.zh-CN.md`，在 `# 更新日志` 标题和语言切换链接之后插入对应的中文版本条目，确保两个文件内容一致
5. 日期使用当天日期

### 步骤 4：更新 README 版本号

同时更新两个 README 文件标题中的版本号：

- `README.md`：`# Buttery Taskbar vX.Y.Z`
- `README.zh-CN.md`：`# Buttery Taskbar vX.Y.Z`

### 步骤 5：更新 ARCHITECTURE 版本号与内容一致性

同时更新两个架构文档标题中的版本号：

- `ARCHITECTURE.md`：`# ButteryTaskbar vX.Y.Z Architecture`
- `ARCHITECTURE.zh-CN.md`：`# ButteryTaskbar vX.Y.Z 架构文档`

并检查文档内容是否与当前代码一致，逐项核对以下清单：

1. **AppState 字段**：对比 `src/main.rs` 中 `struct AppState` 的字段列表与文档 2.2 节
2. **Config 字段**：对比 `struct Config` 的字段列表与文档 5.1 节
3. **钩子行为**：对比 `handle_mouse_move`、`handle_mouse_scroll`、`keyboard_hook_proc` 的触发条件与效果与文档 4.5/4.6 节
4. **ABS\_AUTOHIDE 逻辑**：对比 `set_taskbar_appbar_state` 的条件与文档 4.1 节
5. **API 调用**：确认文档中引用的 Win32 API 名称与代码一致（如 `SetWindowLongPtrW` vs `SetWindowLongW`、`primary_monitor_rect` vs `primary_monitor_work_area`）
6. **菜单项映射**：对比命令 ID 常量与文档 5.3 节
7. **行号索引**：核对文档 12 节的行号范围与 `src/main.rs` 实际行号（允许 ±5 行误差）
8. **中英文一致性**：确保 `ARCHITECTURE.md` 和 `ARCHITECTURE.zh-CN.md` 的内容同步（术语、逻辑、代码引用一致）

如有不一致则同步更新。

### 步骤 6：检查 AGENTS.md

回顾本次变更是否引入了新的踩坑规则（如新的 Win32 API 陷阱、tauri-winres 问题等）：

- **有新规则**：追加到 AGENTS.md 的"踩坑规则"章节
- **无新规则**：跳过此步骤

### 步骤 6：构建 exe

使用 `build_release.bat` 一键构建：

```powershell
cmd /c build_release.bat
```

或手动构建：

```powershell
cargo build --release
Copy-Item "target\release\buttery-taskbar.exe" "target\release\buttery-taskbar_vX.Y.Z.exe" -Force
```

构建产物：`target\release\buttery-taskbar_vX.Y.Z.exe`

**注意**：

- 需要 MSVC toolchain / Windows SDK（tauri-winres 需要 rc.exe）
- 如果构建失败，检查 `cargo build --release` 的错误输出

### 步骤 7：Git 提交与推送

```powershell
git add -A
git commit -m "release: vX.Y.Z"
git push
```

### 步骤 8：Git 打标签

```powershell
git tag vX.Y.Z
git push origin vX.Y.Z
```

### 步骤 9：GitHub Release

从 CHANGELOG 中提取当前版本的 notes 内容，写入临时文件后使用 `--notes-file` 发布：

```powershell
# 先将 notes 写入 target/release/release-notes.md，然后：
gh release create vX.Y.Z "target/release/buttery-taskbar_vX.Y.Z.exe" --title "vX.Y.Z" --notes-file "target/release/release-notes.md"
```

**注意**：

- Release 只上传 `buttery-taskbar_vX.Y.Z.exe`，不上传 `buttery-taskbar.exe`
- 必须使用 `--notes-file` 而非 `--notes`，因为 PowerShell 会将 notes 中的特殊字符误解析为路径

## 注意事项

- 每个步骤完成后及时更新 TodoWrite
- 构建失败时停止流程，修复问题后重试
- 如果 git 工作区有未提交的变更，先确认是否需要包含在本次发布中
- Release notes 使用 CHANGELOG 中的内容，保持一致
- 版本号需要在 `Cargo.toml` 和 `build.rs` 两处同步更新

