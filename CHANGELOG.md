# Changelog

All notable changes to this project are documented in this file.

## [v2.5.0] - 2026-04-09
### Added
- Configurable global toggle shortcut (config-driven, default: `Ctrl+Win+F11`).
- `Edit shortcut settings...` entry in the tray right-click menu to open the config file for editing.
- Unit tests for hotkey parsing and canonicalization.

### Changed
- Rust rewrite and ongoing porting from the original Jai implementation (behavior-preserving migration).
- Configuration format changed from the old Jai binary layout to JSON at `%APPDATA%\Buttery Taskbar\config.json`.
- Tray menu uses a native Windows popup menu instead of the old custom-drawn menu UI.
- Embedded the legacy multi-size application icon into the Rust binary for better Task Manager/Win11 compatibility.

### Fixed
- Win11 tray right-click / context-menu compatibility and popup placement above the taskbar.
- Improved process identity handling so Win11 shows the app icon more consistently.
- Hotkey matching: implemented robust parser/normalizer, no-repeat latch behavior, and safe fallback on invalid config.

### Removed
- Old Jai build tooling dependency — project no longer requires the private Jai compiler to build the Rust binary.

### Notes
- This release focuses on parity and compatibility with the original project while providing a maintainable Rust codebase and configuration model. Further UX work (in-app key recording UI, GitHub update check UI) remains planned.
