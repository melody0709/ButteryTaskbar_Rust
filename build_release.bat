@echo off
setlocal enabledelayedexpansion

for /f "tokens=2 delims== " %%v in ('findstr /b "version" Cargo.toml') do (
    set VERSION=%%v
    goto :version_found
)
:version_found

if "%VERSION%"=="" (
    echo ERROR: Failed to read version from Cargo.toml
    exit /b 1
)

set VERSION=%VERSION:"=%

echo Building buttery-taskbar v%VERSION% ...

cargo build --release
if %ERRORLEVEL% neq 0 (
    echo ERROR: Build failed
    exit /b 1
)

set OUTPUT=target\release\buttery-taskbar_v%VERSION%.exe
copy /Y "target\release\buttery-taskbar.exe" "%OUTPUT%" >nul

echo.
echo Done: %OUTPUT%
endlocal
