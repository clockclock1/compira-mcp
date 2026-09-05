@echo off

setlocal

set "ROOT=%~dp0"

set "COMPIRA_DATA_DIR=%ROOT%data"

set "COMPIRA_HOST=127.0.0.1"

set "COMPIRA_PORT=8080"

set "RUST_LOG=info,compira_mcp=debug"



if not exist "%COMPIRA_DATA_DIR%" mkdir "%COMPIRA_DATA_DIR%"



echo Starting CompiraMCP (dev mode)...

echo   Backend  : http://127.0.0.1:8080  (cargo run, debug)

echo   Frontend : http://127.0.0.1:5173  (vite dev, hot reload)

echo   MCP      : http://127.0.0.1:8080/mcp

echo.

echo Close each window to stop the corresponding service.

echo.



start "CompiraMCP Backend" cmd /k "cd /d "%ROOT%backend" && set "COMPIRA_DATA_DIR=%COMPIRA_DATA_DIR%" && set "COMPIRA_HOST=%COMPIRA_HOST%" && set "COMPIRA_PORT=%COMPIRA_PORT%" && set "RUST_LOG=%RUST_LOG%" && cargo run"



timeout /t 2 /nobreak >nul



start "CompiraMCP Frontend" cmd /k "cd /d "%ROOT%frontend" && npm run dev"



echo Dev servers started in separate windows.

