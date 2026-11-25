# browser_panel（CEF C++ sidecar）一键构建脚本（已合并原 build_all.bat）。
# 用法（PowerShell）：
#   powershell -ExecutionPolicy Bypass -File ai_browser\build.ps1
#   可选参数：-BuildType Release -SdkVersion 10.0.22621.0 -QtDir <Qt6 msvc2019_64 目录> -CefRoot <cef_binary_..._windows64 目录>
# 前置条件：VS2019 生成工具 16.11+（MSVC v142）、Windows SDK 10.0.22621、Qt 6.5.3 msvc2019_64、CEF 111 标准版发行包已解压到 ai_browser\third_party\cef\
param(
    [string]$BuildType = "Release",
    [string]$SdkVersion = "10.0.22621.0",
    [string]$QtDir = "D:\Qt\6.5.3\msvc2019_64",
    [string]$CefRoot = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path

# 1) 定位 VS 安装并导入 x64 开发者环境（指定 SDK，避免误用 10.0.19041）
$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path $vswhere)) {
    throw "未找到 vswhere.exe，请确认已安装 Visual Studio 2019 生成工具"
}
$vsPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vsPath) {
    throw "未找到带 C++ 工具集的 Visual Studio 安装（需勾选「使用 C++ 的桌面开发」）"
}
$sdkDir = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\Include\$SdkVersion"
if (-not (Test-Path $sdkDir)) {
    throw "未安装 Windows SDK $SdkVersion（现有版本见 $((Get-ChildItem (Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\Include') -Directory | ForEach-Object Name) -join ', ')）"
}
Import-Module (Join-Path $vsPath "Common7\Tools\Microsoft.VisualStudio.DevShell.dll")
Enter-VsDevShell -VsInstallPath $vsPath -SkipAutomaticLocation `
    -DevCmdArguments "-arch=x64 -host_arch=x64 -winsdk=$SdkVersion" | Out-Null

# 2) 使用 VS 内置的 CMake + Ninja
$cmakeBin = Join-Path $vsPath "Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin"
$ninjaBin = Join-Path $vsPath "Common7\IDE\CommonExtensions\Microsoft\CMake\Ninja"
$env:PATH = "$cmakeBin;$ninjaBin;$env:PATH"
if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) {
    throw "未找到 VS 内置 CMake（$cmakeBin）"
}
if (-not (Get-Command ninja -ErrorAction SilentlyContinue)) {
    throw "未找到 VS 内置 Ninja（$ninjaBin）；可在 VS Installer 的「C++ CMake tools for Windows」组件中安装"
}

# 3) 配置与构建（CMAKE_PREFIX_PATH 指向 Qt6，否则 find_package(Qt6) 直接失败）
if (-not (Test-Path (Join-Path $QtDir "lib\cmake"))) {
    throw "未找到 Qt6（$QtDir）；CMakeLists 依赖 find_package(Qt6 Widgets)，请用 -QtDir 指定 msvc2019_64 目录"
}
$buildDir = Join-Path $root "build"
$configure = @("-S", $root, "-B", $buildDir, "-G", "Ninja",
               "-DCMAKE_BUILD_TYPE=$BuildType", "-DCMAKE_SYSTEM_VERSION=$SdkVersion",
               "-DCMAKE_PREFIX_PATH=$QtDir")
if ($CefRoot) {
    $configure += "-DCEF_ROOT=$CefRoot"
}
Write-Host "==> cmake $($configure -join ' ')" -ForegroundColor Cyan
& cmake @configure
if ($LASTEXITCODE -ne 0) {
    throw "CMake 配置失败（多为未找到 CEF 发行包：请解压到 ai_browser\third_party\cef\ 或用 -CefRoot 指定）"
}
& cmake --build $buildDir
if ($LASTEXITCODE -ne 0) {
    throw "构建失败"
}

$exe = Join-Path $root "out\$BuildType\browser_panel.exe"
Write-Host "==> 产物：$exe" -ForegroundColor Green
Write-Host "==> 单独调试：& '$exe' --url=https://example.com" -ForegroundColor DarkGray

