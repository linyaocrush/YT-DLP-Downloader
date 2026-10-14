@echo off
setlocal
cd /d "%~dp0"

rem ============================================================
rem  Single-file release, self-contained (bundles the .NET runtime)
rem  Output: one YtDlpDownloader.exe; the target PC needs no .NET install.
rem  Usage: build-single-file-self-contained.bat [--no-pause]
rem ============================================================

set "PROJECT=src\YtDlpDownloader.App\YtDlpDownloader.App.csproj"
set "RID=win-x64"
set "OUTDIR=publish\single-file-self-contained"

echo ============================================================
echo  Building single-file release (self-contained)
echo  Project: %PROJECT%
echo  Output : %OUTDIR%
echo ============================================================
echo.

where dotnet >nul 2>nul
if errorlevel 1 (
    echo [ERROR] dotnet CLI not found. Please install the .NET 10 SDK.
    goto :fail
)

dotnet publish "%PROJECT%" -c Release -r %RID% --self-contained true ^
    -p:PublishSingleFile=true ^
    -p:IncludeNativeLibrariesForSelfExtract=true ^
    -p:DebugType=None ^
    -o "%OUTDIR%"
if errorlevel 1 goto :fail

echo.
echo [OK] Output directory: %CD%\%OUTDIR%
echo      The target machine does not need the .NET runtime installed.
goto :done

:fail
echo.
echo [FAILED] The build did not succeed. See the messages above.

:done
if /i not "%~1"=="--no-pause" pause
endlocal
