# Buttery Taskbar (Rust Rewrite) — v2.5.0

Release: v2.5.0 — configurable global shortcut, Win11 fixes, and Rust port.

English | [简体中文](README.zh-CN.md)

This repository now hosts a Rust rewrite of Buttery Taskbar, based on the behavior and feature set of the original Jai implementation.

The legacy Jai source tree is still included locally under `ButteryTaskbar2_jai/`, while the repository root is now the actively developed Rust project.
## Project Screenshots

<img src="assets/icon.webp" width="50%" />

<img src="assets/right.webp" width="50%" />

## What This Project Is

Buttery Taskbar hides the Windows taskbar more aggressively than the built-in auto-hide mode. The taskbar stays out of the way until it is actually needed, such as when the Start menu, tray overflow, or other shell UI becomes active.

This Rust version is a reconstruction of the original project, with the goal of preserving the user-facing behavior while removing the dependency on the Jai compiler and making Windows 10/11 support easier to maintain.

## Relationship To The Original Version

- Original upstream project: [LuisThiamNye/ButteryTaskbar2](https://github.com/LuisThiamNye/ButteryTaskbar2)
- Legacy upstream releases: [ButteryTaskbar2 releases](https://github.com/LuisThiamNye/ButteryTaskbar2/releases)
- Legacy local source tree in this repository: `ButteryTaskbar2_jai/`

The Rust port is based on the old version's behavior, including:

- tray-based control flow
- taskbar show/hide control through Win32 APIs
- configurable global toggle shortcut (default: `Ctrl` + `Win` + `F11`)
- scroll-at-screen-edge activation
- auto-hide state coordination with Windows
- startup registration in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`

## Current Rust Implementation

Implemented in the current Rust version:

- hidden Win32 message window
- native tray icon and callback handling
- Win11-safe tray callback handling for context-menu invocation
- native popup tray menu
- taskbar visibility control for primary and secondary taskbars
- Start/menu/taskbar shell-UI visibility heuristics
- keyboard hook for the Windows key and the configurable global toggle shortcut
- mouse wheel hook for screen-edge reveal
- config persistence in `%APPDATA%\Buttery Taskbar\config.json`
- startup toggle through the per-user Run registry key
- original icon carried over from the old release and embedded into the Rust executable

Current differences from the old Jai version:

- the old custom-drawn menu UI has been replaced by a native Windows popup menu
- the old GitHub update-check status UI has not yet been reimplemented
- the config format is now JSON instead of the fixed-size Jai binary config block

## Project Layout

- `src/`: active Rust implementation
- `assets/`: Rust-side application assets, including the embedded app icon
- `ButteryTaskbar2_jai/`: archived legacy Jai implementation kept for reference and parity work



## Build

Requirements:

- Windows
- Rust toolchain with Cargo
- MSVC toolchain / Windows SDK resource tools available to Cargo for icon embedding

Commands:

```pwsh
cargo build
cargo build --release
```

Release binary:

```text
target/release/buttery-taskbar.exe
```

## Runtime Behavior

The Rust rewrite currently keeps the following behavior model:

- the taskbar is visible while the Windows key is held
- the taskbar is visible while shell UI such as Start or tray overflow is foregrounded
- the tray menu is opened from the notification icon and is positioned above the taskbar edge
- when enabled, scrolling at the bottom edge of the primary monitor triggers a synthetic Windows key press so Start can open
- when disabled, the app can optionally keep Windows auto-hide enabled

## Custom Toggle Shortcut

The global toggle shortcut is configurable through `%APPDATA%\Buttery Taskbar\config.json`.

You can open that file directly from the tray icon's right-click menu through `Edit shortcut settings...`.

Relevant fields:

- `toggle_shortcut_enabled`: enables or disables the global toggle shortcut
- `toggle_shortcut`: shortcut string, default `Ctrl+Win+F11`

Supported format:

- zero or more modifiers: `Ctrl`, `Alt`, `Shift`, `Win`
- exactly one non-modifier key such as `A`, `5`, `F10`, `Pause`, `Insert`, `Delete`, `Home`, `End`, `PageUp`, `PageDown`, `Up`, `Down`, `Left`, or `Right`

Examples:

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

Invalid shortcut strings fall back to the default shortcut. Restart the app after editing the config file manually.

## Legacy Reference

If you need the old implementation for comparison, debugging, or migration work:

- local legacy README: `ButteryTaskbar2_jai/README.md`
- local legacy sources: `ButteryTaskbar2_jai/`
- original upstream repository: [LuisThiamNye/ButteryTaskbar2](https://github.com/LuisThiamNye/ButteryTaskbar2)