# office_option（Office 内置工具 sidecar：word_/ppt_/excel_）一键构建脚本
# 用法：powershell -ExecutionPolicy Bypass -File ai_office_option\build.ps1
# 产物：target\release\office_option.exe，并拷贝到 src-tauri\static\office_option\
param(
    [string]$Target = "release"
)
$ErrorActionPreference = "Continue"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$workspace = Split-Path -Parent $root
Write-Host "==> cargo build --$Target" -ForegroundColor Cyan
Push-Location $root
try {
    & cargo build "--$Target" 2>&1 | ForEach-Object { Write-Host $_ }
    if ($LASTEXITCODE -ne 0) { throw "cargo build 失败" }
} finally {
    Pop-Location
}
$exe = Join-Path $root "target\$Target\office_option.exe"
if (-not (Test-Path $exe)) { throw "未找到产物：$exe" }
$dest = Join-Path $workspace "src-tauri\static\office_option"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item -Path $exe -Destination (Join-Path $dest "office_option.exe") -Force
Write-Host "==> 产物：$exe" -ForegroundColor Green
Write-Host "==> 随包副本：$(Join-Path $dest 'office_option.exe')" -ForegroundColor Green
Write-Host "==> 调试：& '$exe' pipe --pipe=<名称>" -ForegroundColor DarkGray