@echo off
REM Windows fallback when Cursor cannot resolve `node` on PATH.
REM Prefer mcp.json -> node dist\launch.js; this .cmd prepends common locations.
setlocal
set "ROOT=%~dp0"
set "PATH=%USERPROFILE%\AppData\Local\mise\installs\node\24.16.0;%USERPROFILE%\AppData\Local\mise\shims;%ProgramFiles%\nodejs;%ProgramFiles%\Git\bin;%PATH%"

if defined MARENGO_MCP_NODE (
  "%MARENGO_MCP_NODE%" "%ROOT%dist\launch.js"
  exit /b %ERRORLEVEL%
)

where node >nul 2>&1
if %ERRORLEVEL%==0 (
  node "%ROOT%dist\launch.js"
  exit /b %ERRORLEVEL%
)

if exist "%USERPROFILE%\AppData\Local\mise\installs\node\24.16.0\node.exe" (
  "%USERPROFILE%\AppData\Local\mise\installs\node\24.16.0\node.exe" "%ROOT%dist\launch.js"
  exit /b %ERRORLEVEL%
)

echo run-mcp.cmd: node not found. Install Node 24 or set MARENGO_MCP_NODE. 1>&2
exit /b 127
