# Changelog

All notable changes to Buttery Taskbar (Rust) will be documented in this file.

English | [简体中文](CHANGELOG.zh-CN.md)

## [2.5.1] - 2026-04-16

### Added

- Tray context menu now displays the current version number (grayed out, non-clickable)
- `ProductVersion` and `FileVersion` metadata embedded in the executable
- Release build script (`build_release.bat`) that produces versioned exe output

### Fixed

- **64-bit pointer truncation in settings dialog** — `SetWindowLongW`/`GetWindowLongW` replaced with `SetWindowLongPtrW`/`GetWindowLongPtrW` to prevent pointer truncation on 64-bit systems, which could cause crashes when opening the Settings dialog
- **Scroll-to-reveal activation instability** — Changed from exact 1-pixel match (`y == rect.bottom - 1`) to a 2-pixel activation zone (`rect.bottom - 2 <= y < rect.bottom`), significantly improving reliability of the scroll-at-screen-edge feature
- **Scroll activation unreachable under ABS_AUTOHIDE** — Removed the `!should_show_taskbar()` condition that prevented scroll activation from ever triggering when `ABS_AUTOHIDE` was active. Scroll activation now only simulates a Win key press to open the Start menu (taskbar showing is handled by `ABS_AUTOHIDE` and the `WM_MOUSEMOVE` hook)
- **`current_millis()` type mismatch** — Changed return type from `i64` to `u64` and `should_stay_visible_before` from `AtomicI64` to `AtomicU64` to match `GetTickCount64()` semantics, eliminating a theoretical overflow risk
- **`handle_key_down` dead return value** — Simplified from `fn(...) -> bool` (always returned `false`) to `fn(...)` with no return value, removing misleading code

### Changed

- Version aligned to `2.5.1` across `Cargo.toml`, `build.rs`, and tray menu
- Icon embedding library migrated from `winres` to `tauri-winres` (icon resource ID changed to `32512`)
- Mouse hook now also handles `WM_MOUSEMOVE` to improve reliability of bottom-edge taskbar reveal
- Mouse position detection changed from `primary_monitor_work_area()` to `primary_monitor_rect()`, preventing edge detection failure when taskbar is visible and work area shrinks
- Menu option text changed from "Scroll to reveal taskbar" → "Edge activation" → "Scroll to open Start"
- Menu option text changed from "Auto-hide when disabled" → "Keep auto-hide when disabled"
