# 构建 Rust 发行版，并把编译好的 exe 复制到 release\ 目录。
#
#   powershell -ExecutionPolicy Bypass -File .\build-release.ps1
#
# 为什么必须带 --features custom-protocol：
#   Tauri 是加载 devUrl(localhost:1420) 还是加载内嵌的前端资源，取决于
#   `tauri` crate 的 custom-protocol feature，而不是 debug/release。
#   `tauri` 的 build script 会算出 dev = !custom-protocol 并发给 tauri-codegen，
#   只有 dev 为 false 时才会把 frontendDist 真正嵌进 exe。
#   裸 `cargo build --release` 会漏掉它，于是运行时报 "localhost 拒绝连接"。
#   `npm run tauri build` 会自动启用，这里用 cargo 所以必须显式传。

$ErrorActionPreference = "Stop"
Set-Location -LiteralPath $PSScriptRoot

$frontend = Join-Path $PSScriptRoot "apps\desktop"

# 1) 前端：tauri-build 在编译期嵌入 apps\desktop\dist
if (Test-Path (Join-Path $frontend "node_modules")) {
    Write-Host "==> 构建前端 (npm run build)" -ForegroundColor Cyan
    Push-Location $frontend
    try { npm run build } finally { Pop-Location }
    if ($LASTEXITCODE -ne 0) { throw "前端构建失败 (npm run build)" }
} elseif (Test-Path (Join-Path $frontend "dist\index.html")) {
    Write-Warning "未找到 apps\desktop\node_modules，跳过前端构建，直接使用现有的 dist\"
} else {
    throw "缺少 apps\desktop\node_modules 与 dist\index.html；请先在 apps\desktop 执行 npm install && npm run build"
}

# 2) Rust 发行版
Write-Host "==> 构建 Rust 发行版 (cargo build --release)" -ForegroundColor Cyan
cargo build --release -p ytdlp-downloader-desktop --features custom-protocol
if ($LASTEXITCODE -ne 0) { throw "cargo build 失败" }

# 3) 复制产物到 release\
$source = Join-Path $PSScriptRoot "target\release\ytdlp-downloader-desktop.exe"
if (-not (Test-Path $source)) { throw "未找到构建产物: $source" }

$releaseDir = Join-Path $PSScriptRoot "release"
New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null

$target = Join-Path $releaseDir "YtDlpDownloader.exe"
Copy-Item -LiteralPath $source -Destination $target -Force

$size = [math]::Round((Get-Item $target).Length / 1MB, 2)
Write-Host "==> 完成：$target ($size MB)" -ForegroundColor Green