@echo off
set "SCRIPT_DIR=%~dp0"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT_DIR%start-local.ps1"
if errorlevel 1 (
  echo.
  echo WaKuOffice 启动失败，请检查上面的错误信息。
  pause
)
