# Buttery Taskbar (Rust Rewrite)

English | [简体中文](README.zh-CN.md)

This repository now hosts a Rust rewrite of Buttery Taskbar, based on the behavior and feature set of the original Jai implementation.

The legacy Jai source tree is still included locally under `ButteryTaskbar2_jai/`, while the repository root is now the actively developed Rust project.

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
- `Ctrl` + `Win` + `F11` toggle support
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
- keyboard hook for the Windows key and `Ctrl` + `Win` + `F11`
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

## Legacy Reference

If you need the old implementation for comparison, debugging, or migration work:

- local legacy README: `ButteryTaskbar2_jai/README.md`
- local legacy sources: `ButteryTaskbar2_jai/`
- original upstream repository: [LuisThiamNye/ButteryTaskbar2](https://github.com/LuisThiamNye/ButteryTaskbar2)