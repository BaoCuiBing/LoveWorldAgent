# computer_use（桌面自动化 sidecar，内置工具）一键构建脚本
# 用法：powershell -ExecutionPolicy Bypass -File ai_computer_use\build.ps1
# 产物：target\release\computer_use.exe，并拷贝到 src-tauri\static\computer_use\
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
$exe = Join-Path $root "target\$Target\computer_use.exe"
if (-not (Test-Path $exe)) { throw "未找到产物：$exe" }
$dest = Join-Path $workspace "src-tauri\static\computer_use"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item -Path $exe -Destination (Join-Path $dest "computer_use.exe") -Force
$modelsSrc = Join-Path $root "omni_parser\models"
$modelsDest = Join-Path $dest "models"
if (Test-Path $modelsSrc) {
    New-Item -ItemType Directory -Force -Path $modelsDest | Out-Null
    Copy-Item -Path (Join-Path $modelsSrc "*") -Destination $modelsDest -Recurse -Force
    Write-Host "==> 模型随包：$modelsDest" -ForegroundColor Green
}
$cudaSrc = Join-Path $root "omni_parser\cuda_runtime"
$cudaDest = Join-Path $dest "cuda_runtime"
if (Test-Path $cudaSrc) {
    New-Item -ItemType Directory -Force -Path $cudaDest | Out-Null
    Copy-Item -Path (Join-Path $cudaSrc "*") -Destination $cudaDest -Recurse -Force
    Write-Host "==> CUDA 运行库随包：$cudaDest" -ForegroundColor Green
}
Write-Host "==> 产物：$exe" -ForegroundColor Green
Write-Host "==> 随包副本：$(Join-Path $dest 'computer_use.exe')" -ForegroundColor Green
Write-Host "==> 调试：& '$exe' pipe --pipe=<名称>；& '$exe' omni serve" -ForegroundColor DarkGray
