# mobile_use（Android 移动端自动化 sidecar，内置工具）一键构建脚本
# 用法：powershell -ExecutionPolicy Bypass -File ai_mobile\build.ps1
# 产物：target\release\mobile_use.exe，并拷贝到 src-tauri\static\mobile\
param(
    [string]$Target = "release"
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$workspace = Split-Path -Parent $root
Write-Host "==> cargo build --$Target" -ForegroundColor Cyan
Push-Location $root
try {
    & cargo build "--$Target"
    if ($LASTEXITCODE -ne 0) { throw "cargo build 失败" }
} finally {
    Pop-Location
}
$exe = Join-Path $root "target\$Target\mobile_use.exe"
if (-not (Test-Path $exe)) { throw "未找到产物：$exe" }
$dest = Join-Path $workspace "src-tauri\static\mobile"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item -Path $exe -Destination (Join-Path $dest "mobile_use.exe") -Force
$adb = Join-Path $workspace "src-tauri\static\mobile\platform-tools"
if (-not (Test-Path (Join-Path $adb "adb.exe"))) { Write-Host "==> 提示：platform-tools 未随包，请把 adb.exe 放到 src-tauri\static\mobile\platform-tools\" -ForegroundColor Yellow }
Write-Host "==> 产物：$exe" -ForegroundColor Green
Write-Host "==> 随包副本：$(Join-Path $dest 'mobile_use.exe')" -ForegroundColor Green
Write-Host "==> 调试：& '$exe' pipe --pipe=<名称>" -ForegroundColor DarkGray