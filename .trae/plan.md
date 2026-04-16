# 计划：重新设计选项体系

## 背景

原版 Jai 的设计中，`ABS_AUTOHIDE` 在 `enabled = true` 时始终生效。这导致：
- 鼠标触碰屏幕底部 → Windows 自动显示任务栏（这是 `ABS_AUTOHIDE` 的副作用）
- "Scroll to reveal" 只控制滚轮钩子，不控制鼠标触底行为
- 用户取消 "Scroll to reveal" 后，鼠标触底仍能显示任务栏，觉得选项没用

`ABS_AUTOHIDE` 的双重作用：
1. **安全网**：告诉 Windows 任务栏处于自动隐藏状态，工作区计算正确
2. **副作用**：鼠标触底时 Windows 自动显示任务栏

## 方案：统一 Edge activation 控制 ABS_AUTOHIDE

**核心思路**：让 "Edge activation" 同时控制鼠标触底和滚轮两种行为。

### 菜单

```
☑ Enabled
☑ Edge activation
☐ Keep auto-hide when off
  Settings...
  Start at log-in (non-admin)
  ─────────────
  Open releases page
  ─────────────
  buttery-taskbar_v2.5.1
  Quit
```

### 行为说明

**Edge activation**（勾选时）：
- 鼠标触碰屏幕底部 → 任务栏出现
- 底部滚轮 → 任务栏出现 + 打开开始菜单

**Edge activation**（不勾选时）：
- 鼠标触碰底部 → 无反应
- 底部滚轮 → 无反应
- 只有 Win 键 / 开始菜单 / Shell UI 能唤出任务栏
- ⚠️ 可能出现窗口延伸到任务栏位置的情况（因为没设 ABS_AUTOHIDE）

**Keep auto-hide when off**（仅 Buttery 禁用时生效）：
- 勾选 → 禁用后任务栏仍以 Windows 原生方式自动隐藏
- 不勾选 → 禁用后任务栏始终可见

### 代码修改

1. `set_taskbar_appbar_state()` 逻辑：
   - `enabled && scroll_activation_enabled` → `ABS_AUTOHIDE`
   - `!enabled && autohide_when_disabled` → `ABS_AUTOHIDE`
   - 其他 → `0`

2. 菜单文字：
   - "Scroll to reveal taskbar" → "Edge activation"
   - "Auto-hide when disabled" → "Keep auto-hide when off"

3. 配置字段名不变，保持兼容性

### 更新文档

- ARCHITECTURE.md
- README.md / README.zh-CN.md
