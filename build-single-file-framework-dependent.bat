@echo off
setlocal
cd /d "%~dp0"

rem ============================================================
rem  Single-file release, framework-dependent (does NOT bundle .NET)
rem  Output: one YtDlpDownloader.exe; the target PC needs the .NET 10
rem  Windows Desktop Runtime installed.
rem  Usage: build-single-file-framework-dependent.bat [--no-pause]
rem ============================================================

set "PROJECT=src\YtDlpDownloader.App\YtDlpDownloader.App.csproj"
set "RID=win-x64"
set "OUTDIR=publish\single-file-framework-dependent"

echo ============================================================
echo  Building single-file release (framework-dependent)
echo  Project: %PROJECT%
echo  Output : %OUTDIR%
echo ============================================================
echo.

where dotnet >nul 2>nul
if errorlevel 1 (
    echo [ERROR] dotnet CLI not found. Please install the .NET 10 SDK.
    goto :fail
)

dotnet publish "%PROJECT%" -c Release -r %RID% --self-contained false ^
    -p:PublishSingleFile=true ^
    -p:DebugType=None ^
    -o "%OUTDIR%"
if errorlevel 1 goto :fail

echo.
echo [OK] Output directory: %CD%\%OUTDIR%
echo      The target machine needs the .NET 10 Windows Desktop Runtime.
goto :done

:fail
echo.
echo [FAILED] The build did not succeed. See the messages above.

:done
if /i not "%~1"=="--no-pause" pause
endlocal
