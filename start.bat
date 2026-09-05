@echo off
setlocal
set "ROOT=%~dp0"
cd /d "%ROOT%backend"
REM Prefer ASCII data path on Windows if project path has non-ASCII (libgit2).
if defined COMPIRA_DATA_DIR goto :have_data
set "COMPIRA_DATA_DIR=%ROOT%data"
:have_data
set "COMPIRA_STATIC_DIR=%ROOT%frontend\dist"
set "COMPIRA_PORT=8080"
if not exist "%COMPIRA_DATA_DIR%" mkdir "%COMPIRA_DATA_DIR%"
if not exist "%COMPIRA_STATIC_DIR%\index.html" (
  echo Building frontend...
  pushd "%ROOT%frontend"
  call npm install
  call npm run build
  popd
)
echo Starting CompiraMCP on http://localhost:%COMPIRA_PORT%
echo   data: %COMPIRA_DATA_DIR%
cargo run --release
