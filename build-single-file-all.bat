@echo off
setlocal
cd /d "%~dp0"

rem ============================================================
rem  Build both single-file releases (self-contained + framework-dependent)
rem  Usage: build-single-file-all.bat [--no-pause]
rem ============================================================

echo ############################################################
echo  Building all single-file releases
echo ############################################################
echo.

set "FAILED="

call "%~dp0build-single-file-self-contained.bat" --no-pause
if errorlevel 1 (
    echo.
    echo [FAILED] self-contained build failed.
    set "FAILED=1"
)

call "%~dp0build-single-file-framework-dependent.bat" --no-pause
if errorlevel 1 (
    echo.
    echo [FAILED] framework-dependent build failed.
    set "FAILED=1"
)

echo.
if defined FAILED (
    echo [RESULT] At least one build failed. See the logs above.
) else (
    echo [RESULT] Both builds succeeded. See the publish directory.
)

if /i not "%~1"=="--no-pause" pause
if defined FAILED (
    endlocal
    exit /b 1
)
endlocal
